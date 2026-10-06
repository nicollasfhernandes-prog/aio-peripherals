//! BenQ ZOWIE mice (VID 1af3, Kingsis). Most models are driverless — DPI, polling rate and
//! lift-off are switched with buttons/switches under the mouse — and the protocol of the newer
//! wireless models is not public, so they are only detected.

use hidapi::HidApi;

use super::{DeviceInfo, DeviceKind};

const VID: u16 = 0x1af3;

pub fn list(api: &HidApi) -> Vec<DeviceInfo> {
    let mut seen = Vec::new();
    let mut out = Vec::new();
    for d in api.device_list().filter(|d| d.vendor_id() == VID) {
        if seen.contains(&d.product_id()) {
            continue;
        }
        seen.push(d.product_id());
        let name = d.product_string().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| "ZOWIE".into());
        out.push(DeviceInfo {
            id: format!("zowie:{:04x}", d.product_id()),
            name: name.trim_start_matches("ZOWIE ").to_string(),
            vendor: "ZOWIE".into(),
            kind: DeviceKind::Mouse,
            connection: "USB".into(),
            online: true,
            supported: false,
            battery: None,
            dpi: None,
            report_rate: None,
            onboard_mode: None,
            note: Some("Os mouses ZOWIE são configurados no próprio mouse (botões e chaves embaixo dele). Não há protocolo público para mudar isso pelo USB.".into()),
            keyboard: None,
            led_zones: None,
            sensor: None,
        });
    }
    out
}
