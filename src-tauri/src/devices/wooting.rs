//! Wooting analog keyboards (VID 31e3; legacy 03eb).
//!
//! Based on Wooting's open-source SDKs (MPL-2.0):
//! - wooting-rgb-sdk: config interface (usage page 0x1337, or 0xFF55 on "multi report" firmware),
//!   8-byte feature commands `[report, 0xD0|0xD1, 0xDA, cmd, p3, p2, p1, p0]` followed by a
//!   response read, and the raw colour report (6x21 RGB565 matrix).
//! - wooting-analog-sdk: analog reports on usage page 0xFF54 (v1: [code hi, code lo, value]*)
//!   or 0xFF53 (v2: [matrix pos, key, packed, value]*).
//!
//! Untested on real hardware here. Actuation / Rapid Trigger settings use Wootility's
//! undocumented protocol and are not supported.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use hidapi::{HidApi, HidDevice};

use super::aula::KeyboardInfo;
use super::logitech::{LedEffect, LedZone};
use super::{DeviceInfo, DeviceKind};

const VID: u16 = 0x31e3;
const LEGACY_VID: u16 = 0x03eb;
const PID_MODE_MASK: u16 = 0xfff0;

const CFG_USAGE_PAGE: u16 = 0x1337;
const CFG_V3_USAGE_PAGE: u16 = 0xff55;
const ANALOG_V1_USAGE_PAGE: u16 = 0xff54;
const ANALOG_V2_USAGE_PAGE: u16 = 0xff53;

const RAW_COLORS_REPORT: u8 = 11;
const RESET_ALL_COMMAND: u8 = 32;
const COLOR_INIT_COMMAND: u8 = 33;

const ROWS: usize = 6;
const COLS: usize = 21;
const V2_REPORT_SIZE: usize = 256 + 1;
const V3_REPORT_SIZE: usize = 2046 + 1;
const SMALL_PACKET: usize = 64;

fn model_name(vid: u16, pid: u16) -> Option<&'static str> {
    if vid == LEGACY_VID {
        return match pid {
            0xff01 => Some("Wooting One"),
            0xff02 => Some("Wooting Two"),
            _ => None,
        };
    }
    Some(match pid & PID_MODE_MASK {
        0x1100 => "Wooting One",
        0x1200 => "Wooting Two",
        0x1210 => "Wooting Two Lekker Edition",
        0x1220 => "Wooting Two HE",
        0x1230 => "Wooting Two HE (ARM)",
        0x1300 => "Wooting 60HE",
        0x1310 => "Wooting 60HE (ARM)",
        0x1320 => "Wooting 60HE+",
        0x1340 => "Wooting 60HE v2",
        0x1400 => "Wooting 80HE",
        0x1500 => "Wooting UwU",
        0x1510 => "Wooting UwU RGB",
        _ => return None,
    })
}

/// RGB565, as the SDK encodes it.
fn encode_color([r, g, b]: [u8; 3]) -> u16 {
    ((r as u16 & 0xf8) << 8) | ((g as u16 & 0xfc) << 3) | ((b as u16 & 0xf8) >> 3)
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Transport {
    /// v2 interface, one 257-byte write
    Big,
    /// v2 interface, four 65-byte writes
    Small,
    /// usage page 0xFF55, report 5 (2047 bytes), magic 0xD1
    Multi,
}

struct Keyboard {
    dev: HidDevice,
    transport: Transport,
    color_init: bool,
}

impl Keyboard {
    fn open(api: &HidApi, vid: u16, pid: u16) -> Option<Keyboard> {
        let info = api
            .device_list()
            .find(|d| d.vendor_id() == vid && d.product_id() == pid && matches!(d.usage_page(), CFG_USAGE_PAGE | CFG_V3_USAGE_PAGE))?;
        let dev = api.open_path(info.path()).ok()?;
        let transport = if info.usage_page() == CFG_V3_USAGE_PAGE {
            Transport::Multi
        } else {
            // The SDK tells small/big packets apart by the descriptor's Report Count item:
            // 0x95 (1-byte count, 64) = small packets, 0x96 (2-byte count, 256) = big.
            let mut desc = [0u8; 4096];
            match dev.get_report_descriptor(&mut desc) {
                Ok(n) if desc[..n].contains(&0x96) && !desc[..n].contains(&0x95) => Transport::Big,
                Ok(n) if desc[..n].contains(&0x96) => {
                    let first95 = desc[..n].iter().position(|b| *b == 0x95).unwrap_or(usize::MAX);
                    let first96 = desc[..n].iter().position(|b| *b == 0x96).unwrap_or(usize::MAX);
                    if first95 < first96 { Transport::Small } else { Transport::Big }
                }
                Ok(_) => Transport::Small,
                Err(_) => Transport::Big,
            }
        };
        Some(Keyboard { dev, transport, color_init: false })
    }

    /// Sends a feature command and waits for (and discards) the device's response.
    fn command(&self, cmd: u8, params: [u8; 4]) -> Result<(), String> {
        let multi = self.transport == Transport::Multi;
        let report = [if multi { 1 } else { 0 }, if multi { 0xd1 } else { 0xd0 }, 0xda, cmd, params[3], params[2], params[1], params[0]];
        self.dev.send_feature_report(&report).map_err(|e| e.to_string())?;
        let mut buf = vec![0u8; if multi { 2048 } else { 257 }];
        let deadline = Instant::now() + Duration::from_millis(1000);
        while Instant::now() < deadline {
            match self.dev.read_timeout(&mut buf, 100) {
                Ok(n) if n > 0 => return Ok(()),
                Ok(_) => {}
                Err(e) => return Err(e.to_string()),
            }
        }
        Err("o teclado não respondeu".into())
    }

    fn write_matrix(&mut self, matrix: &[[u16; COLS]; ROWS]) -> Result<(), String> {
        if !self.color_init {
            self.command(COLOR_INIT_COMMAND, [0; 4])?;
            self.color_init = true;
        }
        let mut payload = Vec::with_capacity(ROWS * COLS * 2);
        for row in matrix {
            for c in row {
                payload.extend_from_slice(&c.to_le_bytes());
            }
        }
        match self.transport {
            Transport::Multi => {
                let mut r = vec![0u8; V3_REPORT_SIZE];
                r[..4].copy_from_slice(&[5, 0xd1, 0xda, RAW_COLORS_REPORT]);
                r[4..4 + payload.len()].copy_from_slice(&payload);
                self.dev.write(&r).map_err(|e| e.to_string())?;
            }
            Transport::Big | Transport::Small => {
                let mut r = vec![0u8; V2_REPORT_SIZE];
                r[..4].copy_from_slice(&[0, 0xd0, 0xda, RAW_COLORS_REPORT]);
                r[4..4 + payload.len()].copy_from_slice(&payload);
                if self.transport == Transport::Big {
                    self.dev.write(&r).map_err(|e| e.to_string())?;
                } else {
                    for chunk in r[1..].chunks(SMALL_PACKET).take(4) {
                        let mut packet = vec![0u8; SMALL_PACKET + 1];
                        packet[1..1 + chunk.len()].copy_from_slice(chunk);
                        self.dev.write(&packet).map_err(|e| e.to_string())?;
                    }
                }
            }
        }
        Ok(())
    }

    fn fill(&mut self, color: [u8; 3]) -> Result<(), String> {
        let c = encode_color(color);
        self.write_matrix(&[[c; COLS]; ROWS])
    }

    /// Hands lighting back to the keyboard's own profile.
    fn reset(&mut self) -> Result<(), String> {
        self.color_init = false;
        self.command(RESET_ALL_COMMAND, [0; 4])
    }
}

#[derive(Default)]
pub struct Session {
    devices: HashMap<(u16, u16), Keyboard>,
    /// Keyboards whose lighting we took over (restored when the app closes).
    overridden: HashSet<(u16, u16)>,
}

fn parse_id(id: &str) -> Result<(u16, u16), String> {
    let rest = id.strip_prefix("wooting:").ok_or_else(|| format!("not a Wooting device: {id}"))?;
    let (v, p) = rest.split_once(':').ok_or("bad id")?;
    Ok((u16::from_str_radix(v, 16).map_err(|e| e.to_string())?, u16::from_str_radix(p, 16).map_err(|e| e.to_string())?))
}

impl Session {
    fn get(&mut self, api: &HidApi, key: (u16, u16)) -> Result<&mut Keyboard, String> {
        if !self.devices.contains_key(&key) {
            let k = Keyboard::open(api, key.0, key.1).ok_or("interface de configuração da Wooting não encontrada")?;
            self.devices.insert(key, k);
        }
        Ok(self.devices.get_mut(&key).unwrap())
    }

    /// Restores profile lighting on every keyboard we changed (called on exit).
    pub fn restore_all(&mut self) {
        for key in self.overridden.drain().collect::<Vec<_>>() {
            if let Some(k) = self.devices.get_mut(&key) {
                let _ = k.reset();
            }
        }
    }
}

pub fn list(api: &HidApi, _session: &mut Session) -> Vec<DeviceInfo> {
    let mut seen: Vec<(u16, u16)> = Vec::new();
    let mut out = Vec::new();
    for d in api.device_list().filter(|d| d.vendor_id() == VID || d.vendor_id() == LEGACY_VID) {
        let key = (d.vendor_id(), d.product_id());
        let Some(name) = model_name(key.0, key.1) else { continue };
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        let pages: Vec<u16> = api
            .device_list()
            .filter(|x| x.vendor_id() == key.0 && x.product_id() == key.1)
            .map(|x| x.usage_page())
            .collect();
        let rgb = key.0 == VID && pages.iter().any(|p| matches!(*p, CFG_USAGE_PAGE | CFG_V3_USAGE_PAGE));
        let analog = pages.iter().any(|p| matches!(*p, ANALOG_V1_USAGE_PAGE | ANALOG_V2_USAGE_PAGE));
        out.push(DeviceInfo {
            id: format!("wooting:{:04x}:{:04x}", key.0, key.1),
            name: name.trim_start_matches("Wooting ").to_string(),
            vendor: "Wooting".into(),
            kind: DeviceKind::Keyboard,
            connection: "USB".into(),
            online: true,
            supported: rgb || analog,
            battery: None,
            dpi: None,
            report_rate: None,
            onboard_mode: None,
            note: Some("Suporte experimental (SDKs oficiais da Wooting), ainda não testado. Atuação e Rapid Trigger continuam no Wootility.".into()),
            keyboard: Some(KeyboardInfo { lighting: None, actuation: None, analog }),
            led_zones: rgb.then(|| {
                vec![LedZone { index: 0, name: "Iluminação".into(), effects: vec!["profile".into(), "static".into(), "off".into()] }]
            }),
            sensor: None,
        });
    }
    out
}

pub fn set_led(api: &HidApi, s: &mut Session, id: &str, fx: &LedEffect) -> Result<(), String> {
    let key = parse_id(id)?;
    let result = {
        let k = s.get(api, key)?;
        match fx.kind.as_str() {
            "profile" => k.reset(),
            "off" => k.fill([0, 0, 0]),
            "static" => {
                // Scale by brightness (the SDK has no separate brightness command).
                let level = fx.brightness.clamp(1, 100) as u16;
                let c = fx.color.map(|v| ((v as u16 * level) / 100) as u8);
                k.fill(c)
            }
            other => Err(format!("efeito não suportado: {other}")),
        }
    };
    if result.is_err() {
        s.devices.remove(&key);
    } else if fx.kind == "profile" {
        s.overridden.remove(&key);
    } else {
        s.overridden.insert(key);
    }
    result
}

/// Streams the analog value of every pressed key until dropped.
pub struct Monitor {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Monitor {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Calls `on_frame` (~60 Hz) with a snapshot of pressed keys: (HID key code, depth 0-1000).
pub fn start_monitor(api: &HidApi, id: &str, on_frame: impl Fn(Vec<(u16, u16)>) + Send + 'static) -> Result<Monitor, String> {
    let (vid, pid) = parse_id(id)?;
    let info = api
        .device_list()
        .find(|d| d.vendor_id() == vid && d.product_id() == pid && matches!(d.usage_page(), ANALOG_V1_USAGE_PAGE | ANALOG_V2_USAGE_PAGE))
        .ok_or("interface analógica não encontrada")?;
    let v2 = info.usage_page() == ANALOG_V2_USAGE_PAGE;
    // Legacy-firmware Wooting One/Two report values that need a 1.2x scale (from the analog SDK).
    let legacy_scale = vid == LEGACY_VID;
    let dev = api.open_path(info.path()).map_err(|e| e.to_string())?;
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let thread = std::thread::spawn(move || {
        let mut buf = [0u8; 65];
        let mut last: Vec<(u16, u16)> = Vec::new();
        let mut last_emit = Instant::now();
        while !flag.load(Ordering::Relaxed) {
            let n = match dev.read_timeout(&mut buf, 20) {
                Ok(n) => n,
                Err(_) => break,
            };
            if n > 0 {
                last = parse_analog(&buf[..n], v2, legacy_scale);
            }
            if last_emit.elapsed() >= Duration::from_millis(16) {
                on_frame(last.clone());
                last_emit = Instant::now();
            }
        }
    });
    Ok(Monitor { stop, thread: Some(thread) })
}

/// Parses one analog report into (code, depth 0-1000) for keys that are pressed.
fn parse_analog(report: &[u8], v2: bool, legacy_scale: bool) -> Vec<(u16, u16)> {
    if v2 {
        report
            .chunks_exact(4)
            .filter_map(|b| {
                let namespace = (b[2] >> 2) & 0x0f;
                let value = ((b[3] as u16) << 2) | ((b[2] >> 6) as u16 & 0x03); // 10-bit
                (value > 0).then(|| (((namespace as u16) << 8) | b[1] as u16, (value as u32 * 1000 / 1023) as u16))
            })
            .collect()
    } else {
        report
            .chunks_exact(3)
            .filter(|s| s[2] != 0)
            .map(|s| {
                let raw = s[2] as u32 * if legacy_scale { 1200 } else { 1000 } / 255;
                (((s[0] as u16) << 8) | s[1] as u16, raw.min(1000) as u16)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb565() {
        assert_eq!(encode_color([255, 255, 255]), 0xffff);
        assert_eq!(encode_color([255, 0, 0]), 0xf800);
        assert_eq!(encode_color([0, 255, 0]), 0x07e0);
        assert_eq!(encode_color([0, 0, 255]), 0x001f);
    }

    #[test]
    fn analog_v1() {
        // 'A' (0x04) half pressed, 'W' (0x1a) fully pressed, trailing zeros ignored
        let r = [0x00, 0x04, 0x80, 0x00, 0x1a, 0xff, 0, 0, 0];
        assert_eq!(parse_analog(&r, false, false), vec![(0x04, 501), (0x1a, 1000)]);
    }

    #[test]
    fn analog_v2() {
        // key 0x04, namespace 0, value = (0xff << 2) | 3 = 1023
        let r = [0x00, 0x04, 0b1100_0001, 0xff];
        assert_eq!(parse_analog(&r, true, false), vec![(0x04, 1000)]);
    }

    #[test]
    fn models() {
        assert_eq!(model_name(VID, 0x1302), Some("Wooting 60HE"));
        assert_eq!(model_name(VID, 0x1402), Some("Wooting 80HE"));
        assert_eq!(model_name(LEGACY_VID, 0xff02), Some("Wooting Two"));
        assert_eq!(model_name(VID, 0x9999), None);
    }
}
