//! HyperX Pulsefire Haste 2 (wired 03f0:0b97, wireless receiver 03f0:0f98).
//!
//! Protocol from fspy/haste2ctl (USB captures of NGENUITY), confirmed against the wireless
//! receiver: 64-byte reports on the vendor interface (usage page 0xff00), byte 0 = report id.
//!   32 02                 -> 33 02 RR MM AA [5 x (u16 code LE, R, G, B)]   polling + DPI table
//!   32 01 00 00 RR MM AA… live write of that table
//!   40 01 00 00 BB        set (volatile) brightness 0-255
//!   44 01 01 00 / 44 02 00 00 R G B   direct lighting frame for the single logo LED
//!   36 01 00 00 AS 00 00 LOD 01 / 36 02 -> 37 02 AS 00 00 LOD   angle snap + lift-off distance
//!   50 02                 -> 51 02 PCT …                      battery
//! Acks: ff 01 … [14] = report (or reply id), [15] = sub, [16] = result (0 = ok).
//! The 0x36 and 0x50 commands were captured from NGENUITY 3.0 talking to the wireless receiver.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use hidapi::{HidApi, HidDevice};

use super::logitech::{LedEffect, LedZone};
use super::{Battery, DeviceInfo, DeviceKind, DpiInfo, ReportRateInfo, SensorSettings};

const VID: u16 = 0x03f0;
/// (pid, connection, max polling rate)
const PIDS: &[(u16, &str, u16)] = &[(0x0f98, "2.4 GHz", 1000), (0x0b97, "USB", 8000)];

const TABLE_RATE: usize = 2;
const TABLE_ACTIVE: usize = 4;
const TABLE_STAGES: usize = 5;
const STAGE_LEN: usize = 5;
const DPI_STEP: u16 = 50;
const DPI_MAX: u16 = 26000;
/// Lift-off distance codes as NGENUITY sends them.
const LOD_OPTIONS: &[(u8, &str)] = &[(0, "1 mm"), (2, "2 mm")];

struct Haste {
    dev: HidDevice,
}

impl Haste {
    fn send(&self, bytes: &[u8]) -> Result<(), String> {
        let mut p = [0u8; 64];
        p[..bytes.len()].copy_from_slice(bytes);
        self.dev.write(&p).map(|_| ()).map_err(|e| e.to_string())
    }

    /// Sends a command and collects replies until `done` says so (or timeout).
    fn exchange(&self, bytes: &[u8], mut done: impl FnMut(&[u8]) -> bool) -> Result<(), String> {
        let mut buf = [0u8; 64];
        while matches!(self.dev.read_timeout(&mut buf, 0), Ok(n) if n > 0) {}
        self.send(bytes)?;
        let deadline = Instant::now() + Duration::from_millis(600);
        while Instant::now() < deadline {
            let n = self.dev.read_timeout(&mut buf, 50).map_err(|e| e.to_string())?;
            if n > 0 && done(&buf[..n]) {
                return Ok(());
            }
        }
        Err("mouse not responding (asleep or off?)".into())
    }

    /// Waits for the ack of `report sub` and checks its result byte.
    fn command(&self, bytes: &[u8]) -> Result<(), String> {
        let (report, sub) = (bytes[0], bytes[1]);
        let mut result = None;
        self.exchange(bytes, |r| {
            let is_ack = r.len() > 16 && r[0] == 0xff && (r[14] == report || r[14] == report + 1) && r[15] == sub;
            if is_ack {
                result = Some(r[16]);
            }
            is_ack
        })?;
        match result {
            Some(0) => Ok(()),
            Some(code) => Err(format!("mouse rejected the command (code {code})")),
            None => Err("no answer".into()),
        }
    }

    fn table(&self) -> Result<[u8; 64], String> {
        let mut table = None;
        self.exchange(&[0x32, 0x02], |r| {
            if r.len() >= 32 && r[0] == 0x33 && r[1] == 0x02 {
                let mut t = [0u8; 64];
                t[..r.len()].copy_from_slice(r);
                table = Some(t);
                true
            } else {
                false
            }
        })?;
        table.ok_or_else(|| "no DPI table".into())
    }

    fn write_table(&self, t: &[u8; 64]) -> Result<(), String> {
        let mut p = vec![0x32, 0x01, 0x00, 0x00];
        p.extend_from_slice(&t[2..62]);
        self.command(&p)
    }

    /// Reads a `report 02` query whose answer comes back as `report+1 02 …`.
    fn query(&self, report: u8) -> Result<[u8; 64], String> {
        let mut out = None;
        self.exchange(&[report, 0x02], |r| {
            if r.len() >= 8 && r[0] == report + 1 && r[1] == 0x02 {
                let mut t = [0u8; 64];
                t[..r.len()].copy_from_slice(r);
                out = Some(t);
                true
            } else {
                false
            }
        })?;
        out.ok_or_else(|| "no answer".into())
    }

    fn battery(&self) -> Option<Battery> {
        let r = self.query(0x50).ok()?;
        (r[2] <= 100).then(|| Battery { percent: r[2], charging: false, estimated: false })
    }

    /// (angle snap, lift-off code)
    fn sensor(&self) -> Option<(bool, u8)> {
        let r = self.query(0x36).ok()?;
        Some((r[2] == 1, r[5]))
    }

    fn set_sensor(&self, angle_snap: bool, lod: u8) -> Result<(), String> {
        self.command(&[0x36, 0x01, 0x00, 0x00, angle_snap as u8, 0x00, 0x00, lod, 0x01])
    }

    fn set_color(&self, [r, g, b]: [u8; 3]) -> Result<(), String> {
        self.command(&[0x44, 0x01, 0x01, 0x00])?;
        self.command(&[0x44, 0x02, 0x00, 0x00, r, g, b])
    }

    fn set_brightness(&self, level: u8) -> Result<(), String> {
        self.command(&[0x40, 0x01, 0x00, 0x00, level])
    }
}

fn stage_dpi(t: &[u8], stage: usize) -> u16 {
    let o = TABLE_STAGES + stage * STAGE_LEN;
    (u16::from_le_bytes([t[o], t[o + 1]]) + 1) * DPI_STEP
}

#[derive(Default)]
pub struct Session {
    devices: HashMap<u16, Haste>,
}

impl Session {
    fn get(&mut self, api: &HidApi, pid: u16) -> Result<&Haste, String> {
        if !self.devices.contains_key(&pid) {
            let info = api
                .device_list()
                .find(|d| d.vendor_id() == VID && d.product_id() == pid && d.usage_page() == 0xff00)
                .ok_or("mouse not found")?;
            let dev = api.open_path(info.path()).map_err(|e| e.to_string())?;
            self.devices.insert(pid, Haste { dev });
        }
        Ok(&self.devices[&pid])
    }

    fn with<T>(&mut self, api: &HidApi, id: &str, f: impl Fn(&Haste) -> Result<T, String>) -> Result<T, String> {
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

fn parse_id(id: &str) -> Result<u16, String> {
    id.strip_prefix("hyperx:")
        .and_then(|p| u16::from_str_radix(p, 16).ok())
        .ok_or_else(|| format!("not a HyperX device: {id}"))
}

pub fn list(api: &HidApi, session: &mut Session) -> Vec<DeviceInfo> {
    let mut out = Vec::new();
    for &(pid, connection, max_rate) in PIDS {
        let Some(info) = api.device_list().find(|d| d.vendor_id() == VID && d.product_id() == pid && d.usage_page() == 0xff00)
        else {
            continue;
        };
        let name = info
            .product_string()
            .map(|s| s.trim_start_matches("HyperX ").to_string())
            .unwrap_or_else(|| "Pulsefire Haste 2".into());
        session.devices.remove(&pid);
        let handle = session.get(api, pid).ok();
        let table = handle.and_then(|h| h.table().ok());
        let (battery, sensor) = match (handle, table.is_some()) {
            (Some(h), true) => (h.battery(), h.sensor()),
            _ => (None, None),
        };
        let (dpi, report_rate) = match &table {
            Some(t) => {
                let active = (t[TABLE_ACTIVE] as usize).min(4);
                let hz = 8000 / (t[TABLE_RATE].max(1) as u16);
                (
                    Some(DpiInfo { current: stage_dpi(t, active), min: DPI_STEP, max: DPI_MAX, step: DPI_STEP }),
                    Some(ReportRateInfo {
                        current_hz: hz,
                        supported_hz: [125, 250, 500, 1000, 2000, 4000, 8000].into_iter().filter(|r| *r <= max_rate).collect(),
                    }),
                )
            }
            None => (None, None),
        };
        out.push(DeviceInfo {
            id: format!("hyperx:{pid:04x}"),
            name,
            vendor: "HyperX".into(),
            kind: DeviceKind::Mouse,
            connection: connection.into(),
            online: table.is_some(),
            supported: table.is_some(),
            battery,
            dpi,
            report_rate,
            onboard_mode: None,
            note: table.is_none().then(|| "Receptor encontrado, mas o mouse está desligado ou dormindo. Mexa nele e atualize.".into()),
            keyboard: None,
            led_zones: table.is_some().then(|| {
                vec![LedZone { index: 0, name: "Logo".into(), effects: vec!["off".into(), "static".into()] }]
            }),
            sensor: sensor.map(|(angle, lod)| SensorSettings {
                angle_snap: Some(angle),
                lod: Some(lod),
                lod_options: LOD_OPTIONS.iter().map(|(v, l)| (*v, l.to_string())).collect(),
                ..Default::default()
            }),
        });
    }
    out
}

/// Sets the DPI of the active stage (read-modify-write of the whole table).
pub fn set_dpi(api: &HidApi, session: &mut Session, id: &str, dpi: u16) -> Result<(), String> {
    let code = (dpi / DPI_STEP).saturating_sub(1).to_le_bytes();
    session.with(api, id, |h| {
        let mut t = h.table()?;
        let o = TABLE_STAGES + (t[TABLE_ACTIVE] as usize).min(4) * STAGE_LEN;
        t[o] = code[0];
        t[o + 1] = code[1];
        h.write_table(&t)
    })
}

pub fn set_report_rate(api: &HidApi, session: &mut Session, id: &str, hz: u16) -> Result<(), String> {
    if ![125, 250, 500, 1000, 2000, 4000, 8000].contains(&hz) {
        return Err(format!("unsupported polling rate {hz} Hz"));
    }
    session.with(api, id, |h| {
        let mut t = h.table()?;
        t[TABLE_RATE] = (8000 / hz) as u8;
        h.write_table(&t)
    })
}

/// Logo colour. Like NGENUITY's, this is volatile: the mouse forgets it on reconnect.
pub fn set_led(api: &HidApi, session: &mut Session, id: &str, fx: &LedEffect) -> Result<(), String> {
    session.with(api, id, |h| {
        if fx.kind == "off" {
            return h.set_brightness(0);
        }
        h.set_color(fx.color)?;
        h.set_brightness(((fx.brightness.clamp(1, 100) as u16 * 255) / 100) as u8)
    })
}

/// `key`: angleSnap (0/1) or lod (lift-off code). The other setting is kept as it is.
pub fn set_sensor(api: &HidApi, session: &mut Session, id: &str, key: &str, value: u8) -> Result<(), String> {
    session.with(api, id, |h| {
        let (angle, lod) = h.sensor().ok_or("could not read sensor settings")?;
        match key {
            "angleSnap" => h.set_sensor(value != 0, lod),
            "lod" if LOD_OPTIONS.iter().any(|(v, _)| *v == value) => h.set_sensor(angle, value),
            _ => Err(format!("invalid sensor setting {key}={value}")),
        }
    })
}
