//! Razer mice and keyboards (VID 1532), using the protocol documented by the OpenRazer project.
//!
//! Every command is a 90-byte feature report (report id 0):
//!   [0] status  [1] transaction id  [2..4] remaining packets  [4] protocol type
//!   [5] data size  [6] command class  [7] command id  [8..88] arguments  [88] CRC  [89] 0
//! CRC = XOR of bytes 2..88. The device answers in the same report with status
//! 0x02 = ok, 0x01 = busy, 0x03 = failure, 0x04 = timeout, 0x05 = not supported.
//!
//! Untested on real hardware here: the transaction id, the HID collection and the LED ids
//! differ between models, so they are probed with read-only commands and cached.

use std::collections::HashMap;
use std::time::Duration;

use hidapi::{HidApi, HidDevice};

use super::logitech::{LedEffect, LedZone};
use super::{Battery, DeviceInfo, DeviceKind, DpiInfo, ReportRateInfo};

const VID: u16 = 0x1532;
const TRANSACTION_IDS: [u8; 4] = [0x1f, 0x3f, 0xff, 0x9f];
const REPORT_LEN: usize = 90;
const VARSTORE: u8 = 0x01;

const STATUS_OK: u8 = 0x02;
const STATUS_BUSY: u8 = 0x01;
const STATUS_FAIL: u8 = 0x03;
const STATUS_TIMEOUT: u8 = 0x04;
const STATUS_UNSUPPORTED: u8 = 0x05;

/// (led id, name) in the order we probe them.
const LEDS: &[(u8, &str)] = &[(0x05, "Iluminação"), (0x04, "Logo"), (0x01, "Scroll"), (0x00, "Tudo")];

const KEYBOARD_WORDS: &[&str] = &["huntsman", "blackwidow", "ornata", "cynosa", "deathstalker", "keyboard", "tartarus", "turret"];
/// Products on this VID that use other protocols (audio, controllers…).
const SKIP_WORDS: &[&str] = &["kraken", "barracuda", "nari", "kaira", "blackshark", "hammerhead", "wolverine", "kishi", "seiren", "audio"];

fn crc(p: &[u8; REPORT_LEN]) -> u8 {
    p[2..88].iter().fold(0, |a, b| a ^ b)
}

fn packet(tid: u8, class: u8, id: u8, size: u8, args: &[u8]) -> [u8; REPORT_LEN] {
    let mut p = [0u8; REPORT_LEN];
    p[1] = tid;
    p[5] = size;
    p[6] = class;
    p[7] = id;
    p[8..8 + args.len()].copy_from_slice(args);
    p[88] = crc(&p);
    p
}

struct Razer {
    dev: HidDevice,
    tid: u8,
}

impl Razer {
    fn transact_raw(dev: &HidDevice, tid: u8, class: u8, id: u8, size: u8, args: &[u8]) -> Result<[u8; REPORT_LEN], String> {
        let mut out = [0u8; REPORT_LEN + 1]; // leading report id 0
        out[1..].copy_from_slice(&packet(tid, class, id, size, args));
        dev.send_feature_report(&out).map_err(|e| e.to_string())?;
        for attempt in 0..12 {
            std::thread::sleep(Duration::from_millis(if attempt == 0 { 4 } else { 8 }));
            let mut buf = [0u8; REPORT_LEN + 1];
            let n = dev.get_feature_report(&mut buf).map_err(|e| e.to_string())?;
            if n < REPORT_LEN {
                continue;
            }
            let mut r = [0u8; REPORT_LEN];
            r.copy_from_slice(&buf[n - REPORT_LEN..n]);
            match r[0] {
                STATUS_OK if r[6] == class && r[7] == id => return Ok(r),
                STATUS_BUSY => continue,
                STATUS_UNSUPPORTED => return Err("comando não suportado por este modelo".into()),
                STATUS_FAIL => return Err("o aparelho recusou o comando".into()),
                STATUS_TIMEOUT => return Err("o aparelho não respondeu (desligado?)".into()),
                _ => continue,
            }
        }
        Err("sem resposta".into())
    }

    fn transact(&self, class: u8, id: u8, size: u8, args: &[u8]) -> Result<[u8; REPORT_LEN], String> {
        Self::transact_raw(&self.dev, self.tid, class, id, size, args)
    }

    /// Opens the first collection + transaction id that answers a firmware query.
    fn open(api: &HidApi, pid: u16) -> Option<Razer> {
        let mut paths: Vec<_> = api
            .device_list()
            .filter(|d| d.vendor_id() == VID && d.product_id() == pid)
            .map(|d| (d.interface_number(), d.path().to_owned()))
            .collect();
        paths.sort_by_key(|(iface, _)| *iface);
        for (_, path) in paths {
            let Ok(dev) = api.open_path(&path) else { continue };
            for tid in TRANSACTION_IDS {
                if Self::transact_raw(&dev, tid, 0x00, 0x81, 0x02, &[]).is_ok() {
                    return Some(Razer { dev, tid });
                }
            }
        }
        None
    }

    fn dpi(&self) -> Option<u16> {
        let r = self.transact(0x04, 0x85, 0x07, &[0x00]).ok()?;
        let x = u16::from_be_bytes([r[9], r[10]]);
        (x > 0).then_some(x)
    }

    fn set_dpi(&self, dpi: u16) -> Result<(), String> {
        let [h, l] = dpi.to_be_bytes();
        self.transact(0x04, 0x05, 0x07, &[VARSTORE, h, l, h, l, 0, 0]).map(|_| ())
    }

    /// (current Hz, supported Hz). Newer "HyperPolling" devices use a second command pair.
    fn polling(&self) -> Option<(u16, Vec<u16>)> {
        if let Ok(r) = self.transact(0x00, 0x85, 0x01, &[]) {
            let hz = match r[8] {
                0x01 => 1000,
                0x02 => 500,
                0x08 => 125,
                _ => return None,
            };
            return Some((hz, vec![125, 500, 1000]));
        }
        let r = self.transact(0x00, 0xc0, 0x01, &[]).ok()?;
        let hz = hyper_from_code(r[9]).or_else(|| hyper_from_code(r[8]))?;
        Some((hz, vec![125, 500, 1000, 2000, 4000, 8000]))
    }

    fn set_polling(&self, hz: u16) -> Result<(), String> {
        let legacy = match hz {
            1000 => Some(0x01),
            500 => Some(0x02),
            125 => Some(0x08),
            _ => None,
        };
        if let Some(code) = legacy {
            if self.transact(0x00, 0x05, 0x01, &[code]).is_ok() {
                return Ok(());
            }
        }
        let code = hyper_to_code(hz).ok_or_else(|| format!("{hz} Hz não suportado"))?;
        self.transact(0x00, 0x40, 0x02, &[0x00, code]).map(|_| ())
    }

    fn battery(&self) -> Option<Battery> {
        let r = self.transact(0x07, 0x80, 0x02, &[]).ok()?;
        let charging = self.transact(0x07, 0x84, 0x02, &[]).map(|c| c[9] == 1).unwrap_or(false);
        Some(Battery { percent: ((r[9] as u16 * 100 + 127) / 255) as u8, charging, estimated: false })
    }

    fn brightness(&self, led: u8) -> Option<u8> {
        self.transact(0x0f, 0x84, 0x03, &[VARSTORE, led]).ok().map(|r| r[10])
    }

    fn set_brightness(&self, led: u8, level: u8) -> Result<(), String> {
        self.transact(0x0f, 0x04, 0x03, &[VARSTORE, led, level]).map(|_| ())
    }

    /// Extended-matrix effects (class 0x0f, id 0x02).
    fn set_effect(&self, led: u8, fx: &LedEffect) -> Result<(), String> {
        let [r, g, b] = fx.color;
        match fx.kind.as_str() {
            "off" => self.transact(0x0f, 0x02, 0x06, &[VARSTORE, led, 0x00, 0, 0, 0]),
            "static" => self.transact(0x0f, 0x02, 0x09, &[VARSTORE, led, 0x01, 0, 0, 0x01, r, g, b]),
            "breathe" => self.transact(0x0f, 0x02, 0x09, &[VARSTORE, led, 0x02, 0x01, 0, 0x01, r, g, b]),
            "cycle" => self.transact(0x0f, 0x02, 0x06, &[VARSTORE, led, 0x03, 0, 0, 0]),
            k => return Err(format!("efeito desconhecido {k}")),
        }
        .map(|_| ())
    }
}

fn hyper_from_code(c: u8) -> Option<u16> {
    Some(match c {
        0x01 => 8000,
        0x02 => 4000,
        0x04 => 2000,
        0x08 => 1000,
        0x10 => 500,
        0x40 => 125,
        _ => return None,
    })
}

fn hyper_to_code(hz: u16) -> Option<u8> {
    Some(match hz {
        8000 => 0x01,
        4000 => 0x02,
        2000 => 0x04,
        1000 => 0x08,
        500 => 0x10,
        125 => 0x40,
        _ => return None,
    })
}

#[derive(Default)]
pub struct Session {
    devices: HashMap<u16, Razer>,
    /// Products that did not answer the probe, so we don't retry on every scan.
    dead: Vec<u16>,
}

impl Session {
    fn get(&mut self, api: &HidApi, pid: u16) -> Result<&Razer, String> {
        if !self.devices.contains_key(&pid) {
            let r = Razer::open(api, pid).ok_or("o aparelho não respondeu ao protocolo da Razer")?;
            self.devices.insert(pid, r);
        }
        Ok(&self.devices[&pid])
    }

    fn with<T>(&mut self, api: &HidApi, id: &str, f: impl Fn(&Razer) -> Result<T, String>) -> Result<T, String> {
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
    id.strip_prefix("razer:")
        .and_then(|p| u16::from_str_radix(p, 16).ok())
        .ok_or_else(|| format!("not a Razer device: {id}"))
}

pub fn list(api: &HidApi, session: &mut Session) -> Vec<DeviceInfo> {
    let mut products: Vec<(u16, String)> = Vec::new();
    for d in api.device_list().filter(|d| d.vendor_id() == VID) {
        if !products.iter().any(|(p, _)| *p == d.product_id()) {
            products.push((d.product_id(), d.product_string().unwrap_or("Razer").trim().to_string()));
        }
    }
    let mut out = Vec::new();
    for (pid, product) in products {
        let lower = product.to_lowercase();
        if SKIP_WORDS.iter().any(|w| lower.contains(w)) || session.dead.contains(&pid) {
            continue;
        }
        let Ok(r) = session.get(api, pid) else {
            session.dead.push(pid);
            continue;
        };
        let keyboard = KEYBOARD_WORDS.iter().any(|w| lower.contains(w));
        let dpi = if keyboard { None } else { r.dpi() };
        let polling = r.polling();
        let battery = r.battery();
        let zones: Vec<LedZone> = LEDS
            .iter()
            .filter(|(led, _)| r.brightness(*led).is_some())
            .map(|(led, name)| LedZone {
                index: *led,
                name: name.to_string(),
                effects: vec!["off".into(), "static".into(), "breathe".into(), "cycle".into()],
            })
            .collect();
        let wireless = lower.contains("receiver") || lower.contains("dongle") || lower.contains("hyperspeed") || battery.is_some();
        out.push(DeviceInfo {
            id: format!("razer:{pid:04x}"),
            name: product.trim_start_matches("Razer ").to_string(),
            vendor: "Razer".into(),
            kind: if keyboard { DeviceKind::Keyboard } else { DeviceKind::Mouse },
            connection: if wireless { "2.4 GHz".into() } else { "USB".into() },
            online: true,
            supported: true,
            battery,
            dpi: dpi.map(|current| DpiInfo { current, min: 100, max: 30000, step: 50 }),
            report_rate: polling.map(|(current_hz, supported_hz)| ReportRateInfo { current_hz, supported_hz }),
            onboard_mode: None,
            note: Some("Suporte experimental (protocolo OpenRazer), ainda não testado com este modelo.".into()),
            keyboard: None,
            led_zones: (!zones.is_empty()).then_some(zones),
            sensor: None,
        });
    }
    out
}

pub fn set_dpi(api: &HidApi, s: &mut Session, id: &str, dpi: u16) -> Result<(), String> {
    s.with(api, id, |r| r.set_dpi(dpi))
}

pub fn set_report_rate(api: &HidApi, s: &mut Session, id: &str, hz: u16) -> Result<(), String> {
    s.with(api, id, |r| r.set_polling(hz))
}

pub fn set_led(api: &HidApi, s: &mut Session, id: &str, zone: u8, fx: &LedEffect) -> Result<(), String> {
    s.with(api, id, |r| {
        r.set_effect(zone, fx)?;
        if fx.kind != "off" {
            r.set_brightness(zone, ((fx.brightness.clamp(1, 100) as u16 * 255) / 100) as u8)?;
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// "Get firmware" for transaction id 0x1f, as documented by OpenRazer: CRC = 0x02 ^ 0x00 ^ 0x81.
    #[test]
    fn firmware_query_crc() {
        let p = packet(0x1f, 0x00, 0x81, 0x02, &[]);
        assert_eq!(&p[..8], &[0x00, 0x1f, 0x00, 0x00, 0x00, 0x02, 0x00, 0x81]);
        assert_eq!(p[88], 0x02 ^ 0x81);
    }

    #[test]
    fn dpi_packet() {
        let p = packet(0x1f, 0x04, 0x05, 0x07, &[VARSTORE, 0x06, 0x40, 0x06, 0x40, 0, 0]);
        assert_eq!(&p[8..13], &[0x01, 0x06, 0x40, 0x06, 0x40]); // 1600 DPI
        assert_eq!(p[88], crc(&p));
    }

    #[test]
    fn hyperpolling_roundtrip() {
        for hz in [125, 500, 1000, 2000, 4000, 8000] {
            assert_eq!(hyper_from_code(hyper_to_code(hz).unwrap()), Some(hz));
        }
    }
}
