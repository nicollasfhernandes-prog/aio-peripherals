//! DeLUX M900 Pro 8K on the "LXDDZ 2.4G 8K HS Receiver" (1d57:fa65).
//!
//! Same framing as the Attack Shark X8 Pro (OpenMouse-Project/mouse-protocol#171), but this
//! firmware (model id 0x30) never answers reads or acks writes, so — like DeLUX's own app —
//! we keep the profile on disk and rewrite whole blocks. Field layout was captured from the
//! official "DELUX Gaming Driver" talking to the receiver.
//!
//! Frame (vendor collection, report 4): `04 len 00 <block> chkHi chkLo`, checksum = BE sum of
//! all preceding bytes. Notifications arrive on the 0x000a collection as `03 model type p1 p2`.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use hidapi::{HidApi, HidDevice};
use serde::{Deserialize, Serialize};

use super::logitech::LedZone;
use super::{Battery, DeviceInfo, DeviceKind, DpiInfo, ReportRateInfo, SensorSettings};

const VID: u16 = 0x1d57;
const PID: u16 = 0xfa65;
const DELUX_MODEL: u8 = 0x30;
const DPI_STEP: u16 = 50;
const DPI_MAX: u16 = 26000;
/// Gap between frames; the receiver has no flow control and the official app waits ~1 s.
const FRAME_GAP: Duration = Duration::from_millis(120);

/// Everything we write, persisted because the mouse cannot be read back.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Profile {
    pub stages: Vec<u16>,
    /// 1-based
    pub active: u8,
    pub colors: Vec<[u8; 3]>,
    pub polling_hz: u16,
    pub lod_2mm: bool,
    pub ripple: bool,
    pub angle_snap: bool,
    pub motion_sync: bool,
    /// Block 0x05 bytes 3..=10 (lighting, sleep timers, key response) as last written.
    pub block05: [u8; 8],
}

impl Default for Profile {
    /// What DeLUX's app wrote to this mouse when it was captured.
    fn default() -> Self {
        Profile {
            stages: vec![400, 800, 1200, 1600, 3200, 5000],
            active: 4,
            colors: vec![
                [0xff, 0, 0], [0, 0xff, 0], [0, 0, 0xff], [0xff, 0, 0xff],
                [0xff, 0xff, 0], [0xff, 0xff, 0xff], [0xff, 0x40, 0], [0xff, 0xff, 0xff],
            ],
            polling_hz: 8000,
            lod_2mm: false,
            ripple: false,
            angle_snap: false,
            motion_sync: false,
            block05: [0x00, 0x03, 0x78, 0x00, 0x00, 0xff, 0x02, 0x02],
        }
    }
}

fn frame(block: &[u8]) -> Vec<u8> {
    let mut f = vec![0x04, (block.len() + 5) as u8, 0x00];
    f.extend_from_slice(block);
    let sum: u16 = f.iter().map(|b| *b as u16).sum();
    f.extend_from_slice(&sum.to_be_bytes());
    f
}

/// Sent by the official app before every block ("select profile 1").
fn select_block() -> Vec<u8> {
    vec![0x0c, 0x0a, 0x01, 0xfe, 0x01, 0xfe, 0x00, 0x00, 0x00, 0x00]
}

fn dpi_block(p: &Profile) -> Vec<u8> {
    let mut b = vec![0u8; 56];
    b[0..3].copy_from_slice(&[0x04, 0x38, 0x01]);
    b[3] = p.lod_2mm as u8;
    b[4] = p.ripple as u8;
    b[5] = 0x3f; // stages 1-6 enabled
    b[6] = p.angle_snap as u8;
    b[7] = (!p.motion_sync) as u8; // inverted on this firmware
    for (i, dpi) in p.stages.iter().take(8).enumerate() {
        let n = (dpi.clamp(&DPI_STEP, &DPI_MAX) / DPI_STEP) - 1;
        b[8 + i] = n as u8;
        b[16 + i] = (n >> 8) as u8;
    }
    b[24] = p.active;
    for (i, c) in p.colors.iter().take(8).enumerate() {
        b[25 + i * 3..28 + i * 3].copy_from_slice(c);
    }
    b[49] = 0x03; // indication type, as DeLUX writes it
    let sum: u16 = b[3..50].iter().map(|x| *x as u16).sum();
    b[50..52].copy_from_slice(&sum.to_be_bytes());
    b
}

fn block05(p: &Profile) -> Vec<u8> {
    let mut b = vec![0x05, 0x0f, 0x01];
    b.extend_from_slice(&p.block05);
    let sum: u16 = p.block05.iter().map(|x| *x as u16).sum();
    b.extend_from_slice(&sum.to_be_bytes());
    b.extend_from_slice(&[0, 0]);
    b
}

fn polling_block(hz: u16) -> Vec<u8> {
    let rate = (hz / 125).clamp(1, 64) as u8; // one-hot: 125 Hz * value
    vec![0x06, 0x09, 0x01, rate, 0xff ^ rate, 0, 0, 0, 0]
}

fn profile_path() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("AIO Peripherals").join("delux-m900-fa65.json")
}

fn load_profile() -> Profile {
    std::fs::read_to_string(profile_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_profile(p: &Profile) {
    let path = profile_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(s) = serde_json::to_string_pretty(p) {
        let _ = std::fs::write(path, s);
    }
}

struct Link {
    cfg: HidDevice,
    notify: Option<HidDevice>,
}

#[derive(Default)]
pub struct Session {
    link: Option<Link>,
    profile: Option<Profile>,
    battery: Option<Battery>,
    last_heartbeat: Option<Instant>,
}

impl Session {
    fn open(&mut self, api: &HidApi) -> Result<(), String> {
        if self.link.is_some() {
            return Ok(());
        }
        let find = |page: u16| {
            api.device_list()
                .find(|d| d.vendor_id() == VID && d.product_id() == PID && d.usage_page() == page)
                .and_then(|d| api.open_path(d.path()).ok())
        };
        let cfg = find(0xff00).ok_or("receiver not found")?;
        self.link = Some(Link { cfg, notify: find(0x000a) });
        Ok(())
    }

    fn profile(&mut self) -> &mut Profile {
        self.profile.get_or_insert_with(load_profile)
    }

    /// Consumes queued notifications (battery heartbeat every 2 s, DPI button presses).
    fn pump(&mut self, wait: Duration) {
        let Some(n) = self.link.as_ref().and_then(|l| l.notify.as_ref()) else { return };
        let deadline = Instant::now() + wait;
        let mut buf = [0u8; 16];
        let mut events = Vec::new();
        loop {
            match n.read_timeout(&mut buf, 20) {
                Ok(len) if len >= 5 && buf[0] == 0x03 && buf[1] == DELUX_MODEL => events.push([buf[2], buf[3], buf[4]]),
                Ok(0) if Instant::now() >= deadline => break,
                Ok(_) => {}
                Err(_) => {
                    self.link = None;
                    break;
                }
            }
        }
        for [kind, p1, p2] in events {
            match kind {
                0x40 | 0x41 => {
                    self.battery = Some(Battery { percent: p2.min(100), charging: p1 == 3, estimated: false });
                    self.last_heartbeat = Some(Instant::now());
                }
                0x10 if (1..=6).contains(&p1) => {
                    self.profile().active = p1;
                    let p = self.profile().clone();
                    save_profile(&p);
                }
                _ => {}
            }
        }
    }

    fn send(&mut self, blocks: &[Vec<u8>]) -> Result<(), String> {
        let link = self.link.as_ref().ok_or("receiver not open")?;
        for block in blocks {
            for b in [select_block(), block.clone()] {
                let mut report = frame(&b);
                report.resize(64, 0);
                link.cfg.write(&report).map_err(|e| e.to_string())?;
                std::thread::sleep(FRAME_GAP);
            }
        }
        Ok(())
    }

    fn apply(&mut self, api: &HidApi, edit: impl FnOnce(&mut Profile), blocks: impl FnOnce(&Profile) -> Vec<Vec<u8>>) -> Result<(), String> {
        self.open(api)?;
        edit(self.profile());
        let p = self.profile().clone();
        let result = self.send(&blocks(&p));
        if result.is_err() {
            self.link = None;
        }
        save_profile(&p);
        result
    }
}

pub fn list(api: &HidApi, s: &mut Session) -> Vec<DeviceInfo> {
    if !api.device_list().any(|d| d.vendor_id() == VID && d.product_id() == PID) {
        s.link = None;
        return Vec::new();
    }
    if s.open(api).is_err() {
        return Vec::new();
    }
    // First scan: wait for one heartbeat (sent every 2 s) to learn battery/online state.
    let wait = if s.last_heartbeat.is_none() { Duration::from_millis(2300) } else { Duration::ZERO };
    s.pump(wait);
    let online = s.last_heartbeat.is_some_and(|t| t.elapsed() < Duration::from_secs(10));
    let p = s.profile().clone();
    let current = p.stages.get(p.active.saturating_sub(1) as usize).copied().unwrap_or(800);
    vec![DeviceInfo {
        id: format!("lxd:{PID:04x}"),
        name: "DeLUX M900 Pro 8K".into(),
        vendor: "DeLUX".into(),
        kind: DeviceKind::Mouse,
        connection: "2.4 GHz".into(),
        online,
        supported: true,
        battery: s.battery.clone(),
        dpi: Some(DpiInfo { current, min: 100, max: DPI_MAX, step: DPI_STEP }),
        report_rate: Some(ReportRateInfo {
            current_hz: p.polling_hz,
            supported_hz: vec![125, 250, 500, 1000, 2000, 4000, 8000],
        }),
        onboard_mode: None,
        note: (!online).then(|| "O mouse está desligado ou dormindo. Mexa nele e atualize.".into()),
        keyboard: None,
        led_zones: None::<Vec<LedZone>>,
        sensor: Some(SensorSettings {
            angle_snap: Some(p.angle_snap),
            ripple_control: Some(p.ripple),
            motion_sync: Some(p.motion_sync),
            lod: Some(p.lod_2mm as u8),
            lod_options: vec![(0, "1 mm".into()), (1, "2 mm".into())],
            debounce_ms: Some(p.block05[7] * 2),
            debounce_max: 50,
        }),
    }]
}

pub fn set_dpi(api: &HidApi, s: &mut Session, dpi: u16) -> Result<(), String> {
    let dpi = (dpi / DPI_STEP * DPI_STEP).clamp(100, DPI_MAX);
    s.apply(api, |p| {
        let i = p.active.saturating_sub(1) as usize;
        if let Some(stage) = p.stages.get_mut(i) {
            *stage = dpi;
        }
    }, |p| vec![dpi_block(p)])
}

pub fn set_report_rate(api: &HidApi, s: &mut Session, hz: u16) -> Result<(), String> {
    if ![125, 250, 500, 1000, 2000, 4000, 8000].contains(&hz) {
        return Err(format!("unsupported polling rate {hz} Hz"));
    }
    s.apply(api, |p| p.polling_hz = hz, |p| vec![polling_block(p.polling_hz)])
}

pub fn set_sensor(api: &HidApi, s: &mut Session, key: &str, value: u8) -> Result<(), String> {
    let on = value != 0;
    match key {
        "angleSnap" => s.apply(api, |p| p.angle_snap = on, |p| vec![dpi_block(p)]),
        "rippleControl" => s.apply(api, |p| p.ripple = on, |p| vec![dpi_block(p)]),
        "motionSync" => s.apply(api, |p| p.motion_sync = on, |p| vec![dpi_block(p)]),
        "lod" if value <= 1 => s.apply(api, |p| p.lod_2mm = on, |p| vec![dpi_block(p)]),
        // Key response time: stored as ms / 2, 4-50 ms.
        "debounceMs" => s.apply(api, |p| p.block05[7] = (value.clamp(4, 50) / 2).max(2), |p| vec![block05(p)]),
        _ => Err(format!("invalid sensor setting {key}={value}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ")
    }

    /// Frames captured from DeLUX's app (00:45:43 and 00:46:21 in the session log).
    #[test]
    fn reproduces_captured_frames() {
        let p = Profile { active: 1, ..Default::default() };
        let f = frame(&dpi_block(&p));
        assert_eq!(
            hex(&f),
            "04 3d 00 04 38 01 00 00 3f 00 01 07 0f 17 1f 3f 63 00 00 00 00 00 00 00 00 00 00 01 ff 00 00 00 ff 00 00 00 ff ff 00 ff ff ff 00 ff ff ff ff 40 00 ff ff ff 03 0f 64 00 00 00 00 10 55"
        );
        assert_eq!(hex(&frame(&polling_block(8000))), "04 0e 00 06 09 01 40 bf 00 00 00 00 01 21");
        assert_eq!(hex(&frame(&polling_block(125))), "04 0e 00 06 09 01 01 fe 00 00 00 00 01 21");
        assert_eq!(hex(&frame(&select_block())), "04 0f 00 0c 0a 01 fe 01 fe 00 00 00 00 02 27");
        assert_eq!(
            hex(&frame(&block05(&Profile::default()))),
            "04 14 00 05 0f 01 00 03 78 00 00 ff 02 02 01 7e 00 00 02 2a"
        );
    }

    #[test]
    fn sensor_bits() {
        let p = Profile { angle_snap: true, ripple: true, lod_2mm: true, motion_sync: true, ..Default::default() };
        let b = dpi_block(&p);
        assert_eq!((b[3], b[4], b[6], b[7]), (1, 1, 1, 0));
    }
}
