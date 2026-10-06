pub mod aula;
pub mod compx;
pub mod hyperx;
pub mod logitech;
pub mod lxd;
pub mod razer;
pub mod zowie;

use hidapi::HidApi;
use serde::Serialize;

#[derive(Serialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DeviceKind {
    Mouse,
    Keyboard,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Battery {
    pub percent: u8,
    pub charging: bool,
    /// true when the percentage is estimated from voltage
    pub estimated: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DpiInfo {
    pub current: u16,
    pub min: u16,
    pub max: u16,
    pub step: u16,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ReportRateInfo {
    pub current_hz: u16,
    pub supported_hz: Vec<u16>,
}

/// Advanced sensor options; `None` fields are not supported by the device.
#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct SensorSettings {
    pub angle_snap: Option<bool>,
    pub ripple_control: Option<bool>,
    pub motion_sync: Option<bool>,
    /// Lift-off distance: current value and (value, label) choices
    pub lod: Option<u8>,
    pub lod_options: Vec<(u8, String)>,
    pub debounce_ms: Option<u8>,
    pub debounce_max: u8,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub kind: DeviceKind,
    pub connection: String,
    pub online: bool,
    pub supported: bool,
    pub battery: Option<Battery>,
    pub dpi: Option<DpiInfo>,
    pub report_rate: Option<ReportRateInfo>,
    /// Some(true) = onboard profile active, Some(false) = host (software) mode
    pub onboard_mode: Option<bool>,
    pub note: Option<String>,
    pub keyboard: Option<aula::KeyboardInfo>,
    pub led_zones: Option<Vec<logitech::LedZone>>,
    pub sensor: Option<SensorSettings>,
}

#[derive(Default)]
pub struct Sessions {
    pub logitech: logitech::Session,
    pub aula: aula::Session,
    pub compx: compx::Session,
    pub hyperx: hyperx::Session,
    pub lxd: lxd::Session,
    pub razer: razer::Session,
}

pub fn list(api: &HidApi, s: &mut Sessions) -> Vec<DeviceInfo> {
    let mut out = logitech::list(api, &mut s.logitech);
    out.extend(aula::list(api, &mut s.aula));
    out.extend(compx::list(api, &mut s.compx));
    out.extend(hyperx::list(api, &mut s.hyperx));
    out.extend(lxd::list(api, &mut s.lxd));
    out.extend(razer::list(api, &mut s.razer));
    out.extend(zowie::list(api));
    out
}
