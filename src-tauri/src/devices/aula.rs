//! AULA magnetic (Hall effect) keyboards.
//!
//! Protocol recovered from AULA's WebHID driver (hed.aulacn.com, js/agreement.min.js):
//! every packet is HID report 1 with a 63-byte payload whose first byte is the command.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use hidapi::{HidApi, HidDevice};
use serde::{Deserialize, Serialize};

use super::{DeviceInfo, DeviceKind, ReportRateInfo};

const VID: u16 = 0x2e3c;
const USAGE_PAGE: u16 = 0xff1b;
const USAGE: u16 = 0x91;
const KNOWN: &[u16] = &[0xc365];

const CMD_INFO: u8 = 0x01;
const CMD_LIGHT: u8 = 0x07;
const CMD_LIGHT_MODES: u8 = 0x0a;
/// Per-key colours for the Custom effect: 8 pages of 18 keys x RGB, indexed by matrix index.
const CMD_CUSTOM_LIGHT: u8 = 0x09;
const CUSTOM_READ_SLOT0: u8 = 0x80;
const CUSTOM_PAGES: usize = 8;
const CUSTOM_KEYS_PER_PAGE: usize = 18;
pub const LIGHT_MODE_CUSTOM: u8 = 10;
const CMD_CONFIG: u8 = 0x21;

const SUB_POLL: u8 = 0x09;
const SUB_TRIGGER: u8 = 0x18;
const TRIG_MAX_TRAVEL: u8 = 0x04;
const TRIG_READ_KEY: u8 = 0x05;
const TRIG_TEST_ON: u8 = 0x02;
const TRIG_TEST_OFF: u8 = 0x03;
/// Live key travel report: [0x21, .., 5: 0x01, 6: index / 22, 7: index % 22, 8..9: travel LE]
const TEST_REPORT: u8 = 0x01;

/// Matrix indices of the WIN 60 HE keys (from config/keys/SI2825HEARGB.json).
const WIN60_KEYS: &[u8] = &[
    22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 36, // Esc .. Backspace
    44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 58, // Tab .. \
    66, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 80, // Caps .. Enter
    88, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, // Shifts
    110, 111, 112, 116, 119, 120, 121, 122, // bottom row
];

const LIGHT_MODE_NAMES: &[(u8, &str)] = &[
    (0, "Fixo"), (1, "Respirar"), (2, "Onda"), (3, "Neon"), (4, "Radar"), (6, "Reativo"),
    (7, "Aurora"), (8, "Marola"), (9, "Cintilante"), (11, "Cruz"), (12, "Resposta rápida"),
    (14, "Onda automática"), (15, "Listras"), (16, "Fogos"), (LIGHT_MODE_CUSTOM, "Personalizado"),
];

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct KeyColor {
    pub index: u8,
    pub color: [u8; 3],
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LightMode {
    pub id: u8,
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Lighting {
    pub on: bool,
    pub mode: u8,
    pub brightness: u8,
    pub speed: u8,
    pub color: [u8; 3],
    pub background: [u8; 3],
    pub direction: u8,
    pub full_color: bool,
    #[serde(default)]
    pub modes: Vec<LightMode>,
    #[serde(default)]
    pub max_brightness: u8,
    #[serde(default)]
    pub max_speed: u8,
    /// Colours used by the Custom effect
    #[serde(default)]
    pub custom: Vec<KeyColor>,
}

/// Trigger values are in device units; `unit_mm` converts them to millimetres.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Trigger {
    /// 0 = fixed actuation, 12 = rapid trigger, 13 = rapid trigger with separate release
    pub mode: u8,
    pub travel: u16,
    pub press: u16,
    pub release: u16,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct KeyTrigger {
    pub index: u8,
    #[serde(flatten)]
    pub trigger: Trigger,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Actuation {
    pub min: u16,
    pub max: u16,
    pub unit_mm: f32,
    pub keys: Vec<KeyTrigger>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct KeyboardInfo {
    pub lighting: Option<Lighting>,
    pub actuation: Option<Actuation>,
}

struct Aula {
    dev: HidDevice,
}

/// Open keyboard handles, kept so the OS keeps draining the keyboard's replies between commands.
#[derive(Default)]
pub struct Session {
    devices: HashMap<u16, Aula>,
    /// Keyboards already taken out of a stale test mode this run
    reset: Vec<u16>,
}

impl Session {
    fn get(&mut self, api: &HidApi, pid: u16) -> Result<&Aula, String> {
        if !self.devices.contains_key(&pid) {
            let aula = Aula::open(api, pid)?;
            // If a previous run was killed during Live test, the keyboard stays in test
            // mode with its lighting off. Clear it once per run (not on every rescan,
            // which would interrupt a Live test in progress).
            if !self.reset.contains(&pid) {
                let _ = aula.set_test_mode(false);
                self.reset.push(pid);
            }
            self.devices.insert(pid, aula);
        }
        Ok(&self.devices[&pid])
    }

    /// Runs `f`; on a transport failure (e.g. keyboard replugged) reopens the handle and retries once.
    fn with<T>(&mut self, api: &HidApi, id: &str, f: impl Fn(&Aula) -> Result<T, String>) -> Result<T, String> {
        let pid = parse_id(id)?;
        match f(self.get(api, pid)?) {
            Ok(v) => Ok(v),
            Err(_) => {
                self.devices.remove(&pid);
                f(self.get(api, pid)?)
            }
        }
    }
}

impl Aula {
    fn open(api: &HidApi, pid: u16) -> Result<Aula, String> {
        let info = api
            .device_list()
            .find(|d| d.vendor_id() == VID && d.product_id() == pid && d.usage_page() == USAGE_PAGE && d.usage() == USAGE)
            .ok_or("keyboard not found")?;
        let dev = api.open_path(info.path()).map_err(|e| e.to_string())?;
        Ok(Aula { dev })
    }

    /// Discards replies nobody waited for (e.g. acks to earlier writes).
    fn drain(&self) {
        let mut buf = [0u8; 64];
        while matches!(self.dev.read_timeout(&mut buf, 0), Ok(n) if n > 0) {}
    }

    fn send(&self, payload: &[(usize, u8)]) -> Result<(), String> {
        let mut msg = [0u8; 64];
        msg[0] = 1; // report id
        for &(i, v) in payload {
            msg[1 + i] = v;
        }
        self.dev.write(&msg).map(|_| ()).map_err(|e| e.to_string())
    }

    /// Sends a command and waits for the reply that satisfies `want` (payload without report id).
    fn query(&self, payload: &[(usize, u8)], want: impl Fn(&[u8]) -> bool) -> Result<Vec<u8>, String> {
        self.drain();
        self.send(payload)?;
        let deadline = Instant::now() + Duration::from_millis(600);
        let mut buf = [0u8; 64];
        while Instant::now() < deadline {
            let n = self.dev.read_timeout(&mut buf, 100).map_err(|e| e.to_string())?;
            if n < 8 || buf[0] != 1 {
                continue;
            }
            let data = &buf[1..n];
            if want(data) {
                return Ok(data.to_vec());
            }
        }
        Err("keyboard not responding".into())
    }

    fn config(&self, sub: u8, extra: &[(usize, u8)], reply_sub: u8) -> Result<Vec<u8>, String> {
        let mut p = vec![(0, CMD_CONFIG), (4, sub)];
        p.extend_from_slice(extra);
        self.query(&p, |d| d[0] == CMD_CONFIG && d[5] == reply_sub)
    }

    fn high_precision(&self) -> bool {
        self.query(&[(0, CMD_INFO)], |d| d[0] == CMD_INFO)
            .map(|d| d[18] & 1 != 0)
            .unwrap_or(false)
    }

    fn report_rate(&self) -> Option<ReportRateInfo> {
        let d = self.config(1, &[(5, SUB_POLL)], SUB_POLL).ok()?;
        let khz = d[6] as u16;
        [1, 2, 4, 8].contains(&khz).then(|| ReportRateInfo {
            current_hz: khz * 1000,
            supported_hz: vec![1000, 2000, 4000, 8000],
        })
    }

    /// Note: the keyboard re-enumerates on USB about a second after a rate change.
    fn set_report_rate(&self, hz: u16) -> Result<(), String> {
        let khz = (hz / 1000) as u8;
        if ![1, 2, 4, 8].contains(&khz) {
            return Err(format!("unsupported polling rate {hz} Hz"));
        }
        self.send(&[(0, CMD_CONFIG), (5, SUB_POLL), (6, 1), (7, khz)])
    }

    fn lighting(&self) -> Option<Lighting> {
        let d = self.query(&[(0, CMD_LIGHT), (1, 1)], |d| d[0] == CMD_LIGHT).ok()?;
        let v = &d[5..5 + (d[4] as usize).min(d.len() - 5)];
        if v.len() < 12 {
            return None;
        }
        // Mode list: [modes.., 0xa5, 0x5a, max speed, max brightness]
        let (mut modes, mut max_speed, mut max_brightness) = (Vec::new(), 4, 4);
        if let Ok(m) = self.query(&[(0, CMD_LIGHT_MODES)], |d| d[0] == CMD_LIGHT_MODES) {
            let len = (m[4] as usize).min(m.len() - 5);
            let list = &m[5..5 + len];
            if len >= 4 {
                max_brightness = list[len - 1];
                max_speed = list[len - 2];
                modes = list[..len - 4]
                    .iter()
                    .map(|&id| LightMode {
                        id,
                        name: LIGHT_MODE_NAMES
                            .iter()
                            .find(|(i, _)| *i == id)
                            .map(|(_, n)| n.to_string())
                            .unwrap_or_else(|| format!("Mode {id}")),
                    })
                    .collect();
                modes.push(LightMode { id: LIGHT_MODE_CUSTOM, name: "Personalizado".into() });
            }
        }
        let custom = self.custom_colors().unwrap_or_default();
        Some(Lighting {
            mode: v[0],
            brightness: v[1],
            speed: v[2],
            color: [v[3], v[4], v[5]],
            background: [v[6], v[7], v[8]],
            // Reads back as direction + 6 after some writes.
            direction: v[9] % 6,
            full_color: v[10] != 0,
            // The firmware's "power" byte is inverted: 0 = lighting on, 1 = off.
            on: v[11] == 0,
            modes,
            max_brightness,
            max_speed,
            custom,
        })
    }

    fn custom_colors(&self) -> Option<Vec<KeyColor>> {
        self.drain();
        self.send(&[(0, CMD_CUSTOM_LIGHT), (1, CUSTOM_READ_SLOT0)]).ok()?;
        let mut rgb = vec![0u8; CUSTOM_PAGES * CUSTOM_KEYS_PER_PAGE * 3];
        let mut seen = 0;
        let deadline = Instant::now() + Duration::from_millis(600);
        let mut buf = [0u8; 64];
        while seen < CUSTOM_PAGES && Instant::now() < deadline {
            let n = self.dev.read_timeout(&mut buf, 50).ok()?;
            let d = &buf[1..n.max(1)];
            if n < 6 || buf[0] != 1 || d[0] != CMD_CUSTOM_LIGHT || d[1] != CUSTOM_READ_SLOT0 {
                continue;
            }
            let page = u16::from_be_bytes([d[2], d[3]]) as usize;
            let len = (d[4] as usize).min(d.len() - 5);
            let start = page * CUSTOM_KEYS_PER_PAGE * 3;
            if start + len <= rgb.len() {
                rgb[start..start + len].copy_from_slice(&d[5..5 + len]);
                seen += 1;
            }
        }
        (seen == CUSTOM_PAGES).then(|| {
            WIN60_KEYS
                .iter()
                .map(|&i| {
                    let o = i as usize * 3;
                    KeyColor { index: i, color: [rgb[o], rgb[o + 1], rgb[o + 2]] }
                })
                .collect()
        })
    }

    /// Writes the Custom effect colours to slot 0 (which also makes it the active slot).
    fn set_custom_colors(&self, colors: &[KeyColor]) -> Result<(), String> {
        let mut rgb = vec![0u8; CUSTOM_PAGES * CUSTOM_KEYS_PER_PAGE * 3];
        for c in colors.iter().filter(|c| WIN60_KEYS.contains(&c.index)) {
            let o = c.index as usize * 3;
            rgb[o..o + 3].copy_from_slice(&c.color);
        }
        // The firmware buffer is 396 bytes: 7 full pages of 54 bytes and a last page of 18.
        const TOTAL: usize = 396;
        for (page, chunk) in rgb[..TOTAL].chunks(54).enumerate() {
            let mut p = vec![(0, CMD_CUSTOM_LIGHT), (1, 0), (2, 0), (3, page as u8), (4, chunk.len() as u8)];
            p.extend(chunk.iter().enumerate().map(|(i, &v)| (5 + i, v)));
            self.send(&p)?;
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    }

    fn set_lighting(&self, l: &Lighting) -> Result<(), String> {
        let [r, g, b] = l.color;
        let [br, bg, bb] = l.background;
        self.send(&[
            (0, CMD_LIGHT), (4, 14), (5, l.mode), (6, l.brightness), (7, l.speed),
            (8, r), (9, g), (10, b), (11, br), (12, bg), (13, bb),
            (14, l.direction), (15, l.full_color as u8), (16, (!l.on) as u8),
        ])
    }

    fn actuation(&self) -> Option<Actuation> {
        let m = self.config(SUB_TRIGGER, &[(5, TRIG_MAX_TRAVEL)], TRIG_MAX_TRAVEL).ok()?;
        let divisor = if self.high_precision() { 1000.0 } else { 100.0 };
        let unit_mm = if m[7] == 0 { 10.0 } else { m[7] as f32 } / divisor;
        let max = u16::from_be_bytes([m[9], m[6]]);
        let min = match m[10] {
            0 if (unit_mm - 0.04).abs() < 1e-6 => 2,
            0 => 1,
            v => v as u16,
        };
        let keys = WIN60_KEYS
            .iter()
            .filter_map(|&index| Some(KeyTrigger { index, trigger: self.key_trigger(index)? }))
            .collect();
        Some(Actuation { min, max, unit_mm, keys })
    }

    fn key_trigger(&self, index: u8) -> Option<Trigger> {
        let k = self
            .config(SUB_TRIGGER, &[(5, TRIG_READ_KEY), (6, index / 22), (7, index % 22)], TRIG_READ_KEY)
            .ok()?;
        Some(Trigger {
            mode: k[6],
            travel: u16::from_be_bytes([k[11], k[7]]),
            press: u16::from_be_bytes([k[13], k[9]]),
            release: u16::from_be_bytes([k[14], k[10]]),
        })
    }

    /// Test mode makes the keyboard stream the travel of every moving key.
    fn set_test_mode(&self, on: bool) -> Result<(), String> {
        if !on {
            return self.send(&[(0, CMD_CONFIG), (4, SUB_TRIGGER), (5, TRIG_TEST_OFF)]);
        }
        let mut p = vec![(0, CMD_CONFIG), (4, SUB_TRIGGER), (5, TRIG_TEST_ON)];
        p.extend(key_mask(WIN60_KEYS).iter().enumerate().map(|(i, &m)| (6 + i, m)));
        self.send(&p)
    }

    /// Applies one trigger setting to the given keys (matrix indices) in a single packet.
    fn set_trigger(&self, t: &Trigger, keys: &[u8]) -> Result<(), String> {
        let mut p = vec![(0, CMD_CONFIG), (4, SUB_TRIGGER), (5, t.mode)];
        p.extend(key_mask(keys).iter().enumerate().map(|(i, &m)| (6 + i, m)));
        let [th, tl] = t.travel.to_be_bytes();
        let [ph, pl] = t.press.to_be_bytes();
        let [rh, rl] = t.release.to_be_bytes();
        p.extend([(28, tl), (29, tl), (30, pl), (31, rl), (32, th), (33, th), (34, ph), (35, rh)]);
        self.send(&p)
    }
}

/// Bitmask the firmware uses to address keys: bit (index / 22) of byte (index % 22).
fn key_mask(keys: &[u8]) -> [u8; 22] {
    let mut mask = [0u8; 22];
    for &k in keys.iter().filter(|k| WIN60_KEYS.contains(k)) {
        mask[(k % 22) as usize] |= 1 << (k / 22);
    }
    mask
}

fn parse_id(id: &str) -> Result<u16, String> {
    id.strip_prefix("aula:")
        .and_then(|p| u16::from_str_radix(p, 16).ok())
        .ok_or_else(|| format!("not an AULA device: {id}"))
}

pub fn list(api: &HidApi, session: &mut Session) -> Vec<DeviceInfo> {
    let mut out = Vec::new();
    for info in api
        .device_list()
        .filter(|d| d.vendor_id() == VID && d.usage_page() == USAGE_PAGE && d.usage() == USAGE)
    {
        let pid = info.product_id();
        let name = info.product_string().unwrap_or("AULA Keyboard").to_string();
        let supported = KNOWN.contains(&pid) && name.contains("60");
        session.devices.remove(&pid); // fresh handle on every rescan
        let aula = session.get(api, pid).ok();
        let online = aula.is_some();
        let (report_rate, lighting, actuation) = match (aula, supported) {
            (Some(a), true) => (a.report_rate(), a.lighting(), a.actuation()),
            _ => (None, None, None),
        };
        out.push(DeviceInfo {
            id: format!("aula:{pid:04x}"),
            name,
            vendor: "AULA".into(),
            kind: DeviceKind::Keyboard,
            connection: "USB".into(),
            online,
            supported,
            battery: None,
            dpi: None,
            report_rate,
            onboard_mode: None,
            note: (!supported).then(|| "Detectado, mas o layout de teclas deste modelo ainda não foi mapeado.".into()),
            keyboard: supported.then_some(KeyboardInfo { lighting, actuation }),
            led_zones: None,
            sensor: None,
        });
    }
    out
}

/// Returns true when the keyboard will reconnect (rate actually changed).
pub fn set_report_rate(api: &HidApi, session: &mut Session, id: &str, hz: u16) -> Result<bool, String> {
    let changed = session.with(api, id, |a| {
        if a.report_rate().is_some_and(|r| r.current_hz == hz) {
            return Ok(false);
        }
        a.set_report_rate(hz).map(|_| true)
    })?;
    if changed {
        // The handle dies when the keyboard reboots; reopen on next use.
        session.devices.remove(&parse_id(id)?);
    }
    Ok(changed)
}

pub fn set_lighting(api: &HidApi, session: &mut Session, id: &str, lighting: &Lighting) -> Result<(), String> {
    session.with(api, id, |a| a.set_lighting(lighting))
}

pub fn set_custom_colors(api: &HidApi, session: &mut Session, id: &str, colors: &[KeyColor]) -> Result<(), String> {
    session.with(api, id, |a| a.set_custom_colors(colors))
}

/// Sets `trigger` on `keys` (matrix indices); an empty list means every key.
pub fn set_trigger(api: &HidApi, session: &mut Session, id: &str, trigger: &Trigger, keys: &[u8]) -> Result<(), String> {
    let keys = if keys.is_empty() { WIN60_KEYS } else { keys };
    session.with(api, id, |a| a.set_trigger(trigger, keys))
}

/// Streams live key travel from a keyboard until dropped.
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

/// Puts the keyboard in test mode and calls `on_batch` (~60 times a second) with the
/// keys whose travel changed, as (matrix index, travel in device units).
pub fn start_monitor(
    api: &HidApi,
    id: &str,
    on_batch: impl Fn(Vec<(u8, u16)>) + Send + 'static,
) -> Result<Monitor, String> {
    // A dedicated handle: the OS hands every open handle its own copy of input reports.
    let reader = Aula::open(api, parse_id(id)?)?;
    reader.set_test_mode(true)?;
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let thread = std::thread::spawn(move || {
        let mut pending: HashMap<u8, u16> = HashMap::new();
        let mut last_flush = Instant::now();
        let mut buf = [0u8; 64];
        while !flag.load(Ordering::Relaxed) {
            match reader.dev.read_timeout(&mut buf, 8) {
                Ok(n) if n >= 11 && buf[0] == 1 => {
                    let d = &buf[1..n];
                    if d[0] == CMD_CONFIG && d[5] == TEST_REPORT {
                        let index = d[6].wrapping_mul(22).wrapping_add(d[7]);
                        pending.insert(index, u16::from_le_bytes([d[8], d[9]]));
                    }
                }
                Ok(_) => {}
                Err(_) => break, // keyboard unplugged
            }
            if !pending.is_empty() && last_flush.elapsed() >= Duration::from_millis(16) {
                on_batch(pending.drain().collect());
                last_flush = Instant::now();
            }
        }
        let _ = reader.set_test_mode(false);
    });
    Ok(Monitor { stop, thread: Some(thread) })
}
