//! Logitech devices over HID++ 2.0 (Lightspeed/Unifying receivers and wired mode).

use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use hidapi::{HidApi, HidDevice};

use serde::{Deserialize, Serialize};

use super::{Battery, DeviceInfo, DeviceKind, DpiInfo, ReportRateInfo};

const VID: u16 = 0x046d;
const RECEIVERS: &[u16] = &[0xc539, 0xc53a, 0xc53f, 0xc541, 0xc545, 0xc547, 0xc52b, 0xc532];
const SW_ID: u8 = 0x0a;
const LONG_REPORT: u8 = 0x11;

const F_DEVICE_NAME: u16 = 0x0005;
const F_BATTERY_STATUS: u16 = 0x1000;
const F_BATTERY_VOLTAGE: u16 = 0x1001;
const F_UNIFIED_BATTERY: u16 = 0x1004;
const F_ADJUSTABLE_DPI: u16 = 0x2201;
const F_REPORT_RATE: u16 = 0x8060;
const F_ONBOARD_PROFILES: u16 = 0x8100;
const F_COLOR_LED_EFFECTS: u16 = 0x8070;

/// Effect ids from the HID++ 0x8070 spec (as used by Solaar/libratbag).
const FX_OFF: u16 = 0x00;
const FX_STATIC: u16 = 0x01;
const FX_CYCLE: u16 = 0x03;
const FX_BREATHE: u16 = 0x0a;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LedZone {
    pub index: u8,
    pub name: String,
    /// Effects this zone supports: "off", "static", "cycle", "breathe"
    pub effects: Vec<String>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LedEffect {
    pub kind: String,
    #[serde(default)]
    pub color: [u8; 3],
    /// Animation period in ms (cycle / breathe)
    #[serde(default = "default_period")]
    pub period_ms: u16,
    /// 1-100
    #[serde(default = "default_brightness")]
    pub brightness: u8,
}

fn default_period() -> u16 {
    5000
}

fn default_brightness() -> u8 {
    100
}

fn effect_name(id: u16) -> Option<&'static str> {
    match id {
        FX_OFF => Some("off"),
        FX_STATIC => Some("static"),
        FX_CYCLE => Some("cycle"),
        FX_BREATHE => Some("breathe"),
        _ => None,
    }
}

struct Hidpp {
    dev: HidDevice,
    index: u8,
    /// feature id -> index (None = unsupported), so lookups cost one round trip per session
    features: RefCell<HashMap<u16, Option<u8>>>,
}

/// Open handles kept between commands so a DPI change is a single HID++ request.
#[derive(Default)]
pub struct Session {
    devices: HashMap<String, Hidpp>,
}

fn error_name(code: u8) -> &'static str {
    match code {
        1 => "unknown",
        2 => "invalid argument",
        3 => "out of range",
        4 => "hardware error",
        5 => "internal error",
        6 => "invalid feature index",
        7 => "invalid function",
        8 => "busy",
        9 => "unsupported",
        _ => "error",
    }
}

impl Hidpp {
    fn request(&self, feat: u8, func: u8, params: &[u8]) -> Result<[u8; 20], String> {
        self.request_timeout(feat, func, params, 800)
    }

    fn request_timeout(&self, feat: u8, func: u8, params: &[u8], timeout_ms: u64) -> Result<[u8; 20], String> {
        let fn_sw = (func << 4) | SW_ID;
        let mut msg = [0u8; 20];
        msg[0] = LONG_REPORT;
        msg[1] = self.index;
        msg[2] = feat;
        msg[3] = fn_sw;
        msg[4..4 + params.len()].copy_from_slice(params);
        self.dev.write(&msg).map_err(|e| e.to_string())?;

        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        let mut buf = [0u8; 20];
        while Instant::now() < deadline {
            let n = self.dev.read_timeout(&mut buf, 100).map_err(|e| e.to_string())?;
            if n < 4 || buf[1] != self.index {
                continue;
            }
            if buf[2] == 0xff && buf[3] == feat && buf[4] == fn_sw {
                return Err(format!("HID++ {}", error_name(buf[5])));
            }
            if buf[2] == feat && buf[3] == fn_sw {
                return Ok(buf);
            }
        }
        Err("device not responding (asleep or off?)".into())
    }

    /// Returns the feature index, or None when the device lacks the feature.
    fn feature(&self, id: u16) -> Option<u8> {
        if let Some(cached) = self.features.borrow().get(&id) {
            return *cached;
        }
        let r = self.request(0, 0, &[(id >> 8) as u8, id as u8]).ok()?;
        let idx = (r[4] != 0).then_some(r[4]);
        self.features.borrow_mut().insert(id, idx);
        idx
    }

    /// Empty receiver slots answer on another HID collection, so keep this short.
    fn ping(&self) -> bool {
        self.request_timeout(0, 1, &[0, 0, 0xaa], 200).is_ok()
    }

    fn name(&self) -> Option<String> {
        let f = self.feature(F_DEVICE_NAME)?;
        let len = self.request(f, 0, &[]).ok()?[4] as usize;
        let mut bytes = Vec::with_capacity(len);
        while bytes.len() < len {
            let r = self.request(f, 1, &[bytes.len() as u8]).ok()?;
            let take = (len - bytes.len()).min(16);
            bytes.extend_from_slice(&r[4..4 + take]);
        }
        Some(String::from_utf8_lossy(&bytes).trim_end_matches('\0').to_string())
    }

    fn battery(&self) -> Option<Battery> {
        if let Some(f) = self.feature(F_UNIFIED_BATTERY) {
            let r = self.request(f, 1, &[]).ok()?;
            return Some(Battery { percent: r[4], charging: matches!(r[6], 1 | 2), estimated: false });
        }
        if let Some(f) = self.feature(F_BATTERY_VOLTAGE) {
            let r = self.request(f, 0, &[]).ok()?;
            let mv = u16::from_be_bytes([r[4], r[5]]);
            return Some(Battery {
                percent: voltage_to_percent(mv),
                charging: r[6] & 0x80 != 0,
                estimated: true,
            });
        }
        if let Some(f) = self.feature(F_BATTERY_STATUS) {
            let r = self.request(f, 0, &[]).ok()?;
            return Some(Battery { percent: r[4], charging: matches!(r[6], 1..=3), estimated: false });
        }
        None
    }

    fn dpi(&self) -> Option<DpiInfo> {
        let f = self.feature(F_ADJUSTABLE_DPI)?;
        let list = self.request(f, 1, &[0]).ok()?;
        let cur = self.request(f, 2, &[0]).ok()?;
        let values: Vec<u16> = list[5..]
            .chunks(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .take_while(|v| *v != 0)
            .collect();
        // A value with the top 3 bits set is a step between its neighbours.
        let step = values.iter().find(|v| *v & 0xe000 == 0xe000).map(|v| v & 0x1fff).unwrap_or(50);
        let plain: Vec<u16> = values.iter().copied().filter(|v| v & 0xe000 != 0xe000).collect();
        Some(DpiInfo {
            current: u16::from_be_bytes([cur[5], cur[6]]),
            min: *plain.iter().min()?,
            max: *plain.iter().max()?,
            step,
        })
    }

    fn report_rate(&self) -> Option<ReportRateInfo> {
        let f = self.feature(F_REPORT_RATE)?;
        let bitmap = self.request(f, 0, &[]).ok()?[4];
        let ms = self.request(f, 1, &[]).ok()?[4].max(1);
        let mut supported_hz: Vec<u16> =
            (0..8).filter(|b| bitmap & (1 << b) != 0).map(|b| 1000 / (b + 1) as u16).collect();
        supported_hz.sort();
        Some(ReportRateInfo { current_hz: 1000 / ms as u16, supported_hz })
    }

    /// (zone index, location, [(effect index, effect id)])
    fn led_zones_raw(&self) -> Option<Vec<(u8, u16, Vec<(u8, u16)>)>> {
        let f = self.feature(F_COLOR_LED_EFFECTS)?;
        let zones = self.request(f, 0, &[]).ok()?[4];
        let mut out = Vec::new();
        for z in 0..zones {
            let zi = self.request(f, 1, &[z]).ok()?;
            let location = u16::from_be_bytes([zi[5], zi[6]]);
            let effects = (0..zi[7])
                .filter_map(|e| {
                    let ei = self.request(f, 2, &[z, e]).ok()?;
                    Some((e, u16::from_be_bytes([ei[6], ei[7]])))
                })
                .collect();
            out.push((z, location, effects));
        }
        Some(out)
    }

    fn led_zones(&self) -> Option<Vec<LedZone>> {
        let zones = self.led_zones_raw()?;
        Some(
            zones
                .into_iter()
                .map(|(index, location, effects)| LedZone {
                    index,
                    name: match location {
                        1 => "Principal".into(),
                        2 => "Logo".into(),
                        l => format!("Zona {l}"),
                    },
                    effects: effects.iter().filter_map(|(_, id)| effect_name(*id).map(String::from)).collect(),
                })
                .collect(),
        )
    }

    fn set_led(&self, zone: u8, fx: &LedEffect) -> Result<(), String> {
        let f = self.feature(F_COLOR_LED_EFFECTS).ok_or("RGB not supported")?;
        let zones = self.led_zones_raw().ok_or("could not read LED zones")?;
        let (_, _, effects) = zones.iter().find(|(z, _, _)| *z == zone).ok_or("unknown LED zone")?;
        let id = match fx.kind.as_str() {
            "off" => FX_OFF,
            "static" => FX_STATIC,
            "cycle" => FX_CYCLE,
            "breathe" => FX_BREATHE,
            k => return Err(format!("unknown effect {k}")),
        };
        let effect_index = effects.iter().find(|(_, e)| *e == id).ok_or("effect not supported by this zone")?.0;
        let [r, g, b] = fx.color;
        let [ph, pl] = fx.period_ms.to_be_bytes();
        let level = fx.brightness.clamp(1, 100);
        let params: [u8; 10] = match id {
            FX_STATIC => [r, g, b, 0, 0, 0, 0, 0, 0, 0],
            FX_CYCLE => [0, 0, 0, 0, 0, ph, pl, level, 0, 0],
            FX_BREATHE => [r, g, b, ph, pl, 0, level, 0, 0, 0],
            _ => [0; 10],
        };
        let mut msg = vec![zone, effect_index];
        msg.extend_from_slice(&params);
        msg.push(1); // persist
        self.request(f, 3, &msg).map(|_| ())
    }

    fn onboard_mode(&self) -> Option<bool> {
        let f = self.feature(F_ONBOARD_PROFILES)?;
        Some(self.request(f, 2, &[]).ok()?[4] == 1)
    }

    fn set_onboard_mode(&self, onboard: bool) -> Result<(), String> {
        let f = self.feature(F_ONBOARD_PROFILES).ok_or("onboard profiles not supported")?;
        self.request(f, 1, &[if onboard { 1 } else { 2 }]).map(|_| ())
    }
}

/// Rough Li-ion discharge curve (mV → %), in the spirit of Solaar's tables.
fn voltage_to_percent(mv: u16) -> u8 {
    const CURVE: &[(u16, u8)] = &[
        (4186, 100), (4067, 90), (3989, 80), (3922, 70), (3859, 60), (3811, 50),
        (3778, 40), (3751, 30), (3717, 20), (3671, 10), (3646, 5), (3579, 2), (3500, 0),
    ];
    if mv >= CURVE[0].0 {
        return 100;
    }
    for w in CURVE.windows(2) {
        let ((hv, hp), (lv, lp)) = (w[0], w[1]);
        if mv >= lv {
            let t = (mv - lv) as f32 / (hv - lv) as f32;
            return (lp as f32 + t * (hp - lp) as f32).round() as u8;
        }
    }
    0
}

/// Every reachable HID++ device: (connection label, product id, device index, handle).
fn open_all(api: &HidApi) -> Vec<(String, u16, Hidpp)> {
    let mut out = Vec::new();
    for info in api
        .device_list()
        .filter(|d| d.vendor_id() == VID && d.usage_page() == 0xff00 && d.usage() == 0x0002)
    {
        let pid = info.product_id();
        let receiver = RECEIVERS.contains(&pid);
        let indices: Vec<u8> = if receiver { (1..=6).collect() } else { vec![0xff] };
        for index in indices {
            let Ok(dev) = api.open_path(info.path()) else { continue };
            let h = Hidpp { dev, index, features: RefCell::default() };
            if h.ping() {
                let conn = if receiver { "Lightspeed" } else { "USB" };
                out.push((conn.to_string(), pid, h));
            }
        }
    }
    out
}

fn device_id(pid: u16, index: u8) -> String {
    format!("logitech:{pid:04x}:{index}")
}

impl Session {
    /// Returns the cached handle, rescanning once if the device is not known yet.
    fn get(&mut self, api: &HidApi, id: &str) -> Result<&Hidpp, String> {
        if !self.devices.contains_key(id) {
            self.rescan(api);
        }
        self.devices.get(id).ok_or_else(|| "device not found".into())
    }

    fn rescan(&mut self, api: &HidApi) -> Vec<(String, String)> {
        self.devices.clear();
        let mut out = Vec::new();
        for (conn, pid, h) in open_all(api) {
            let id = device_id(pid, h.index);
            out.push((id.clone(), conn));
            self.devices.insert(id, h);
        }
        out
    }

    /// Runs `f` on the device; on failure (e.g. receiver replugged) reopens it and retries once.
    fn with<T>(&mut self, api: &HidApi, id: &str, f: impl Fn(&Hidpp) -> Result<T, String>) -> Result<T, String> {
        match f(self.get(api, id)?) {
            Ok(v) => Ok(v),
            // A HID++ error means the device answered; only transport failures warrant reopening.
            Err(e) if e.starts_with("HID++") => Err(e),
            Err(_) => {
                self.rescan(api);
                f(self.get(api, id)?)
            }
        }
    }
}

pub fn list(api: &HidApi, session: &mut Session) -> Vec<DeviceInfo> {
    session
        .rescan(api)
        .into_iter()
        .map(|(id, connection)| {
            let h = &session.devices[&id];
            let dpi = h.dpi();
            DeviceInfo {
                name: h.name().unwrap_or_else(|| "Logitech device".into()),
                vendor: "Logitech".into(),
                kind: if dpi.is_some() { DeviceKind::Mouse } else { DeviceKind::Keyboard },
                connection,
                online: true,
                supported: true,
                battery: h.battery(),
                report_rate: h.report_rate(),
                onboard_mode: h.onboard_mode(),
                dpi,
                note: None,
                keyboard: None,
                led_zones: h.led_zones(),
                sensor: None,
                id,
            }
        })
        .collect()
}

pub fn set_dpi(api: &HidApi, session: &mut Session, id: &str, dpi: u16) -> Result<(), String> {
    session.with(api, id, |h| {
        let f = h.feature(F_ADJUSTABLE_DPI).ok_or("DPI not supported")?;
        let [hi, lo] = dpi.to_be_bytes();
        h.request(f, 3, &[0, hi, lo]).map(|_| ())
    })
}

/// Report rate can only be changed in host mode, so we leave onboard mode if needed.
pub fn set_report_rate(api: &HidApi, session: &mut Session, id: &str, hz: u16) -> Result<(), String> {
    let ms = (1000 / hz.max(1)) as u8;
    session.with(api, id, |h| {
        let f = h.feature(F_REPORT_RATE).ok_or("report rate not supported")?;
        if h.request(f, 2, &[ms]).is_ok() {
            return Ok(());
        }
        if h.onboard_mode() == Some(true) {
            h.set_onboard_mode(false)?;
        }
        h.request(f, 2, &[ms]).map(|_| ())
    })
}

/// LEDs follow the onboard profile, so (like report rate) this switches to host mode first.
pub fn set_led(api: &HidApi, session: &mut Session, id: &str, zone: u8, effect: &LedEffect) -> Result<(), String> {
    session.with(api, id, |h| {
        if h.onboard_mode() == Some(true) {
            h.set_onboard_mode(false)?;
        }
        h.set_led(zone, effect)
    })
}

pub fn set_onboard_mode(api: &HidApi, session: &mut Session, id: &str, onboard: bool) -> Result<(), String> {
    session.with(api, id, |h| h.set_onboard_mode(onboard))
}
