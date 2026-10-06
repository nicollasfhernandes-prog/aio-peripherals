//! Compx-based mice: DeLUX M900 Pro, ATK, VXE and other brands built on the Compx platform.
//!
//! Protocol recovered from the vendor's WebHID driver (controlhub.top/delux, "Compx HUB WEB"):
//! HID report 8 with a 16-byte payload `[cmd, status, addr_hi, addr_lo, len, data x10, checksum]`.
//! Settings live in a flash map read/written 10 bytes at a time; most single values are stored
//! as a pair `[v, 0x55 - v]`.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use hidapi::{HidApi, HidDevice};

use super::logitech::{LedEffect, LedZone};
use super::{Battery, DeviceInfo, DeviceKind, DpiInfo, ReportRateInfo, SensorSettings};

const VID: u16 = 0x3554;
/// DeLUX PIDs from its driver's cfg.json; any other Compx mouse is named from its USB strings.
const DELUX_PIDS: &[u16] = &[0xfb26, 0xfb23, 0xfb25];
/// Compx keyboards share the VID but use a different flash map; leave them alone.
const KEYBOARD_PIDS: &[u16] = &[0xf809, 0xf50a];
const REPORT_ID: u8 = 8;

const CMD_HANDSHAKE: u8 = 1; // "EncryptionData": returns cid/mid/type
const CMD_ONLINE: u8 = 3;
const CMD_BATTERY: u8 = 4;
const CMD_WRITE: u8 = 7;
const CMD_READ: u8 = 8;

// Flash map (mouse)
const ADDR_REPORT_RATE: u16 = 0;
const ADDR_CURRENT_DPI: u16 = 4;
const ADDR_DPI_VALUES: u16 = 12; // 8 stages x 4 bytes
const ADDR_DPI_COLORS: u16 = 44; // 8 stages x 4 bytes (rgb + checksum)
const ADDR_DPI_LED_MODE: u16 = 76;
const ADDR_DPI_LED_BRIGHTNESS: u16 = 78;
const ADDR_DPI_LED_STATE: u16 = 82;
const ADDR_LOD: u16 = 10;
const ADDR_DEBOUNCE: u16 = 169;
const ADDR_MOTION_SYNC: u16 = 171;
const ADDR_ANGLE_SNAP: u16 = 175;
const ADDR_RIPPLE: u16 = 177;
const MAP_LEN: u16 = 180;
const DEBOUNCE_MAX: u8 = 15;

const DPI_STEP: u16 = 50;

/// `0x55 - sum` over the bytes, the firmware's checksum for packets and stored records.
fn checksum(bytes: &[u8]) -> u8 {
    0x55u8.wrapping_sub(bytes.iter().fold(0u8, |a, b| a.wrapping_add(*b)))
}

struct Compx {
    dev: HidDevice,
}

impl Compx {
    fn transact(&self, cmd: u8, addr: u16, data: &[u8]) -> Result<[u8; 16], String> {
        self.transact_len(cmd, addr, data.len() as u8, data)
    }

    /// `len` goes in byte 4: the payload size for writes, the wanted size for flash reads.
    fn transact_len(&self, cmd: u8, addr: u16, len: u8, data: &[u8]) -> Result<[u8; 16], String> {
        let mut p = [0u8; 16];
        p[0] = cmd;
        p[2] = (addr >> 8) as u8;
        p[3] = addr as u8;
        p[4] = len;
        p[5..5 + data.len()].copy_from_slice(data);
        p[15] = checksum(&p[..15]).wrapping_sub(REPORT_ID);

        let mut buf = [0u8; 64];
        while matches!(self.dev.read_timeout(&mut buf, 0), Ok(n) if n > 0) {}
        let mut out = [0u8; 17];
        out[0] = REPORT_ID;
        out[1..].copy_from_slice(&p);
        self.dev.write(&out).map_err(|e| e.to_string())?;

        let deadline = Instant::now() + Duration::from_millis(400);
        while Instant::now() < deadline {
            let n = self.dev.read_timeout(&mut buf, 50).map_err(|e| e.to_string())?;
            if n < 17 || buf[0] != REPORT_ID {
                continue;
            }
            let r = &buf[1..17];
            // Replies echo the command (and the address for flash reads/writes).
            let echoes = r[0] == cmd && (!matches!(cmd, CMD_READ | CMD_WRITE) || r[2..4] == p[2..4]);
            if !echoes {
                continue;
            }
            if r[1] == 1 {
                return Err("command not supported by this device".into());
            }
            let mut reply = [0u8; 16];
            reply.copy_from_slice(r);
            return Ok(reply);
        }
        Err("mouse not responding (asleep or off?)".into())
    }

    fn handshake(&self) -> Result<(u8, u8, u8), String> {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
        let seed = nanos.to_le_bytes();
        let r = self.transact(CMD_HANDSHAKE, 0, &[seed[0], seed[1], seed[2], seed[3], 0, 0, 0, 0])?;
        Ok((r[9], r[10], r[11]))
    }

    fn online(&self) -> bool {
        self.transact(CMD_ONLINE, 0, &[]).map(|r| r[5] == 1).unwrap_or(false)
    }

    fn battery(&self) -> Option<Battery> {
        let r = self.transact(CMD_BATTERY, 0, &[]).ok()?;
        Some(Battery { percent: r[5].min(100), charging: r[6] == 1, estimated: false })
    }

    fn read_flash(&self, addr: u16, len: u16) -> Result<Vec<u8>, String> {
        let mut out = Vec::with_capacity(len as usize);
        let mut a = addr;
        while a < addr + len {
            let n = (addr + len - a).min(10) as u8;
            let r = self.transact_len(CMD_READ, a, n, &[])?;
            out.extend_from_slice(&r[5..5 + n as usize]);
            a += n as u16;
        }
        Ok(out)
    }

    fn write_flash(&self, addr: u16, data: &[u8]) -> Result<(), String> {
        for (i, chunk) in data.chunks(10).enumerate() {
            self.transact(CMD_WRITE, addr + (i * 10) as u16, chunk)?;
        }
        Ok(())
    }

    /// Single settings are stored as `[v, 0x55 - v]`.
    fn write_value(&self, addr: u16, v: u8) -> Result<(), String> {
        self.write_flash(addr, &[v, 0x55u8.wrapping_sub(v)])
    }
}

fn rate_from_flash(v: u8) -> u16 {
    if v >= 16 { (v as u16 / 16) * 2000 } else { 1000 / (v.max(1) as u16) }
}

fn rate_to_flash(hz: u16) -> u8 {
    // Above 1 kHz the unit is 2 kHz = 16 (as the web driver writes it).
    if hz > 1000 { (hz / 2000 * 16) as u8 } else { (1000 / hz.max(1)) as u8 }
}

fn max_rate(kind: u8) -> u16 {
    match kind {
        1 => 4000,
        3 | 5 => 8000,
        4 => 2000,
        _ => 1000,
    }
}

/// Decodes stage `e` from the flash map (x axis): 10-bit value, DPI = (v + 1) * 50, doubled per DPIex bit.
fn decode_dpi(map: &[u8], stage: usize) -> u16 {
    let t = ADDR_DPI_VALUES as usize + 4 * stage;
    let ex = map[t + 2] & 0x03;
    let raw = map[t] as u16 | (((map[t + 2] & 0x0c) as u16 >> 2) << 8);
    let mut dpi = (raw + 1) * DPI_STEP;
    if ex & 1 != 0 {
        dpi *= 2;
    }
    if ex & 2 != 0 {
        dpi *= 2;
    }
    dpi
}

/// Encodes the same DPI on both axes into a 4-byte stage record.
fn encode_dpi(dpi: u16, max_plain: u16) -> [u8; 4] {
    let (raw, ex) = if dpi > max_plain { (dpi / 2 / DPI_STEP - 1, 1u8) } else { (dpi / DPI_STEP - 1, 0) };
    let hi = ((raw >> 8) & 0x03) as u8;
    let mut rec = [raw as u8, raw as u8, (hi << 2) | (hi << 6) | ex | (ex << 4), 0];
    rec[3] = checksum(&rec[..3]);
    rec
}

/// Brightness levels 1-9 as the firmware stores them.
fn brightness_to_flash(level: u8) -> u8 {
    match level.clamp(1, 9) {
        1 => 16,
        5 => 128,
        9 => 230,
        l => 30 * (l - 1),
    }
}

#[cfg(test)]
fn brightness_from_flash(v: u8) -> u8 {
    match v {
        16 => 1,
        128 => 5,
        230 => 9,
        v if v % 30 == 0 => v / 30 + 1,
        _ => 5,
    }
}

/// Sensor per model id (cid 93 = M900 family: odd mid = PAW3395, even = PAW3950).
fn max_plain_dpi(mid: u8) -> u16 {
    if mid % 2 == 0 { 30000 } else { 26000 }
}

fn lod_options(mid: u8) -> Vec<(u8, String)> {
    let mut v = vec![(1, "1 mm".to_string()), (2, "2 mm".to_string())];
    if mid % 2 == 0 {
        v.insert(0, (3, "0.7 mm".into())); // PAW3950
    }
    v
}

/// A stored `[v, 0x55 - v]` pair, or None when the slot was never written.
fn pair(map: &[u8], addr: u16) -> Option<u8> {
    let (v, c) = (map[addr as usize], map[addr as usize + 1]);
    (v.wrapping_add(c) == 0x55).then_some(v)
}

fn sensor_settings(map: &[u8], mid: u8) -> SensorSettings {
    SensorSettings {
        angle_snap: pair(map, ADDR_ANGLE_SNAP).map(|v| v == 1),
        ripple_control: pair(map, ADDR_RIPPLE).map(|v| v == 1),
        motion_sync: pair(map, ADDR_MOTION_SYNC).map(|v| v == 1),
        lod: pair(map, ADDR_LOD),
        lod_options: lod_options(mid),
        debounce_ms: pair(map, ADDR_DEBOUNCE),
        debounce_max: DEBOUNCE_MAX,
    }
}

#[derive(Default)]
pub struct Session {
    devices: HashMap<u16, (Compx, u8, u8)>, // pid -> (handle, mid, type)
}

impl Session {
    /// Opens the vendor interface that answers the handshake.
    fn get(&mut self, api: &HidApi, pid: u16) -> Result<&(Compx, u8, u8), String> {
        if !self.devices.contains_key(&pid) {
            let mut found = None;
            for info in api.device_list().filter(|d| d.vendor_id() == VID && d.product_id() == pid && d.usage_page() >= 0xff00) {
                let Ok(dev) = api.open_path(info.path()) else { continue };
                let c = Compx { dev };
                if let Ok((_cid, mid, kind)) = c.handshake() {
                    found = Some((c, mid, kind));
                    break;
                }
            }
            self.devices.insert(pid, found.ok_or("could not talk to the mouse")?);
        }
        Ok(&self.devices[&pid])
    }

    fn with<T>(&mut self, api: &HidApi, id: &str, f: impl Fn(&Compx, u8, u8) -> Result<T, String>) -> Result<T, String> {
        let pid = parse_id(id)?;
        let run = |s: &mut Self| -> Result<T, String> {
            let (c, mid, kind) = s.get(api, pid)?;
            f(c, *mid, *kind)
        };
        run(self).or_else(|_| {
            self.devices.remove(&pid);
            run(self)
        })
    }
}

fn parse_id(id: &str) -> Result<u16, String> {
    id.strip_prefix("compx:")
        .and_then(|p| u16::from_str_radix(p, 16).ok())
        .ok_or_else(|| format!("not a Compx device: {id}"))
}

/// Brand shown in the UI: DeLUX by PID, otherwise the manufacturer string (ATK, VXE…).
fn brand(pid: u16, manufacturer: &str, product: &str) -> String {
    if DELUX_PIDS.contains(&pid) {
        return "DeLUX".into();
    }
    let both = format!("{manufacturer} {product}").to_lowercase();
    for (needle, name) in [("atk", "ATK"), ("vxe", "VXE"), ("mchose", "MCHOSE"), ("delux", "DeLUX")] {
        if both.contains(needle) {
            return name.into();
        }
    }
    if manufacturer.trim().is_empty() { "Compx".into() } else { manufacturer.trim().to_string() }
}

pub fn list(api: &HidApi, session: &mut Session) -> Vec<DeviceInfo> {
    let mut pids: Vec<(u16, String, String)> = Vec::new();
    for d in api.device_list().filter(|d| d.vendor_id() == VID && d.usage_page() >= 0xff00) {
        let product = d.product_string().unwrap_or("").to_string();
        if KEYBOARD_PIDS.contains(&d.product_id()) || product.to_lowercase().contains("keyboard") {
            continue;
        }
        if !pids.iter().any(|(p, _, _)| *p == d.product_id()) {
            pids.push((d.product_id(), d.manufacturer_string().unwrap_or("").to_string(), product));
        }
    }
    let mut out = Vec::new();
    for (pid, manufacturer, product) in pids {
        session.devices.remove(&pid);
        let state = session.get(api, pid).ok().map(|(c, mid, kind)| {
            let wired = matches!(*kind, 2 | 3);
            let online = c.online();
            let map = if online { c.read_flash(0, MAP_LEN).ok() } else { None };
            (online, map, *mid, *kind, wired, if online && !wired { c.battery() } else { None })
        });
        // Not a Compx mouse after all (no handshake): skip silently.
        let Some((online, map, mid, kind, wired, battery)) = state else { continue };
        let vendor = brand(pid, &manufacturer, &product);
        let lower = product.to_lowercase();
        let name = if product.is_empty() || lower.contains("receiver") || lower.contains("dongle") {
            if vendor == "DeLUX" { "M900".to_string() } else { format!("{vendor} mouse") }
        } else {
            product.trim().to_string()
        };
        let (dpi, report_rate) = match &map {
            Some(m) => {
                let stage = (m[ADDR_CURRENT_DPI as usize] as usize).min(7);
                let max = max_rate(kind);
                (
                    Some(DpiInfo { current: decode_dpi(m, stage), min: DPI_STEP, max: max_plain_dpi(mid), step: DPI_STEP }),
                    Some(ReportRateInfo {
                        current_hz: rate_from_flash(m[ADDR_REPORT_RATE as usize]).min(max),
                        supported_hz: [125, 250, 500, 1000, 2000, 4000, 8000].into_iter().filter(|r| *r <= max).collect(),
                    }),
                )
            }
            None => (None, None),
        };
        out.push(DeviceInfo {
            id: format!("compx:{pid:04x}"),
            name,
            vendor,
            kind: DeviceKind::Mouse,
            connection: if wired { "USB".into() } else { "2.4 GHz".into() },
            online,
            supported: map.is_some(),
            battery,
            dpi,
            report_rate,
            onboard_mode: None,
            note: (!online).then(|| "Receptor encontrado, mas o mouse está desligado ou dormindo. Mexa nele e atualize.".into()),
            keyboard: None,
            led_zones: map.is_some().then(|| {
                vec![LedZone { index: 0, name: "LED de DPI".into(), effects: vec!["off".into(), "static".into(), "breathe".into()] }]
            }),
            sensor: map.as_ref().map(|m| sensor_settings(m, mid)),
        });
    }
    out
}

/// Sets the DPI of the active stage.
pub fn set_dpi(api: &HidApi, session: &mut Session, id: &str, dpi: u16) -> Result<(), String> {
    session.with(api, id, |c, mid, _| {
        let stage = c.read_flash(ADDR_CURRENT_DPI, 1)?[0].min(7) as u16;
        c.write_flash(ADDR_DPI_VALUES + 4 * stage, &encode_dpi(dpi, max_plain_dpi(mid)))
    })
}

pub fn set_report_rate(api: &HidApi, session: &mut Session, id: &str, hz: u16) -> Result<(), String> {
    session.with(api, id, |c, _, kind| {
        if hz > max_rate(kind) {
            return Err(format!("{hz} Hz needs a faster receiver"));
        }
        c.write_value(ADDR_REPORT_RATE, rate_to_flash(hz))
    })
}

/// `key`: angleSnap | rippleControl | motionSync (0/1), lod (option value), debounceMs.
pub fn set_sensor(api: &HidApi, session: &mut Session, id: &str, key: &str, value: u8) -> Result<(), String> {
    session.with(api, id, |c, mid, _| {
        let addr = match key {
            "angleSnap" => ADDR_ANGLE_SNAP,
            "rippleControl" => ADDR_RIPPLE,
            "motionSync" => ADDR_MOTION_SYNC,
            "lod" if lod_options(mid).iter().any(|(v, _)| *v == value) => ADDR_LOD,
            "debounceMs" if value <= DEBOUNCE_MAX => ADDR_DEBOUNCE,
            _ => return Err(format!("invalid sensor setting {key}={value}")),
        };
        let v = if matches!(key, "angleSnap" | "rippleControl" | "motionSync") { (value != 0) as u8 } else { value };
        c.write_value(addr, v)
    })
}

/// The DPI indicator LED: off, steady or breathing, in the colour of the active stage.
pub fn set_led(api: &HidApi, session: &mut Session, id: &str, fx: &LedEffect) -> Result<(), String> {
    session.with(api, id, |c, _, _| {
        if fx.kind == "off" {
            return c.write_value(ADDR_DPI_LED_STATE, 0);
        }
        let mode = if fx.kind == "breathe" { 2 } else { 1 };
        let stage = c.read_flash(ADDR_CURRENT_DPI, 1)?[0].min(7) as u16;
        let mut rec = [fx.color[0], fx.color[1], fx.color[2], 0];
        rec[3] = checksum(&rec[..3]);
        c.write_flash(ADDR_DPI_COLORS + 4 * stage, &rec)?;
        c.write_value(ADDR_DPI_LED_MODE, mode)?;
        c.write_value(ADDR_DPI_LED_BRIGHTNESS, brightness_to_flash(((fx.brightness as u16 * 9 + 99) / 100) as u8))?;
        c.write_value(ADDR_DPI_LED_STATE, 1)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_rate_roundtrip() {
        for hz in [125, 250, 500, 1000, 2000, 4000, 8000] {
            assert_eq!(rate_from_flash(rate_to_flash(hz)), hz);
        }
    }

    #[test]
    fn dpi_roundtrip() {
        for dpi in [50, 400, 800, 1600, 3200, 26000] {
            let rec = encode_dpi(dpi, 26000);
            let mut map = vec![0u8; MAP_LEN as usize];
            map[ADDR_DPI_VALUES as usize..ADDR_DPI_VALUES as usize + 4].copy_from_slice(&rec);
            assert_eq!(decode_dpi(&map, 0), dpi);
        }
    }

    #[test]
    fn brightness_roundtrip() {
        for l in 1..=9 {
            assert_eq!(brightness_from_flash(brightness_to_flash(l)), l);
        }
    }
}
