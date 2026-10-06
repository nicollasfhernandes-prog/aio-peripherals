pub mod devices;

use std::sync::Mutex;

use devices::{aula, compx, hyperx, logitech, lxd, razer, DeviceInfo, Sessions};
use hidapi::HidApi;
use tauri::{AppHandle, Emitter, Manager, State};

struct Backend {
    api: HidApi,
    sessions: Sessions,
    monitor: Option<aula::Monitor>,
}

struct Hid(Mutex<Backend>);

fn with_backend<T>(hid: &State<Hid>, f: impl FnOnce(&HidApi, &mut Sessions) -> Result<T, String>) -> Result<T, String> {
    let mut b = hid.0.lock().map_err(|e| e.to_string())?;
    let Backend { api, sessions, .. } = &mut *b;
    f(api, sessions)
}

#[tauri::command]
async fn list_devices(hid: State<'_, Hid>) -> Result<Vec<DeviceInfo>, String> {
    let mut b = hid.0.lock().map_err(|e| e.to_string())?;
    let Backend { api, sessions, .. } = &mut *b;
    api.refresh_devices().map_err(|e| e.to_string())?;
    Ok(devices::list(api, sessions))
}

#[tauri::command]
async fn set_dpi(hid: State<'_, Hid>, id: String, dpi: u16) -> Result<(), String> {
    with_backend(&hid, |api, s| {
        if id.starts_with("compx:") {
            compx::set_dpi(api, &mut s.compx, &id, dpi)
        } else if id.starts_with("hyperx:") {
            hyperx::set_dpi(api, &mut s.hyperx, &id, dpi)
        } else if id.starts_with("lxd:") {
            lxd::set_dpi(api, &mut s.lxd, dpi)
        } else if id.starts_with("razer:") {
            razer::set_dpi(api, &mut s.razer, &id, dpi)
        } else {
            logitech::set_dpi(api, &mut s.logitech, &id, dpi)
        }
    })
}

/// Returns true when the device is about to reconnect, so the UI should wait before rescanning.
#[tauri::command]
async fn set_report_rate(hid: State<'_, Hid>, id: String, hz: u16) -> Result<bool, String> {
    with_backend(&hid, |api, s| {
        if id.starts_with("aula:") {
            aula::set_report_rate(api, &mut s.aula, &id, hz)
        } else if id.starts_with("compx:") {
            compx::set_report_rate(api, &mut s.compx, &id, hz).map(|_| false)
        } else if id.starts_with("hyperx:") {
            hyperx::set_report_rate(api, &mut s.hyperx, &id, hz).map(|_| false)
        } else if id.starts_with("lxd:") {
            lxd::set_report_rate(api, &mut s.lxd, hz).map(|_| false)
        } else if id.starts_with("razer:") {
            razer::set_report_rate(api, &mut s.razer, &id, hz).map(|_| false)
        } else {
            logitech::set_report_rate(api, &mut s.logitech, &id, hz).map(|_| false)
        }
    })
}

#[tauri::command]
async fn set_lighting(hid: State<'_, Hid>, id: String, lighting: aula::Lighting) -> Result<(), String> {
    with_backend(&hid, |api, s| aula::set_lighting(api, &mut s.aula, &id, &lighting))
}

#[tauri::command]
async fn set_trigger(hid: State<'_, Hid>, id: String, trigger: aula::Trigger, keys: Vec<u8>) -> Result<(), String> {
    with_backend(&hid, |api, s| aula::set_trigger(api, &mut s.aula, &id, &trigger, &keys))
}

#[tauri::command]
async fn set_custom_colors(hid: State<'_, Hid>, id: String, colors: Vec<aula::KeyColor>) -> Result<(), String> {
    with_backend(&hid, |api, s| aula::set_custom_colors(api, &mut s.aula, &id, &colors))
}

#[tauri::command]
async fn set_led(hid: State<'_, Hid>, id: String, zone: u8, effect: logitech::LedEffect) -> Result<(), String> {
    with_backend(&hid, |api, s| {
        if id.starts_with("compx:") {
            compx::set_led(api, &mut s.compx, &id, &effect)
        } else if id.starts_with("hyperx:") {
            hyperx::set_led(api, &mut s.hyperx, &id, &effect)
        } else if id.starts_with("razer:") {
            razer::set_led(api, &mut s.razer, &id, zone, &effect)
        } else {
            logitech::set_led(api, &mut s.logitech, &id, zone, &effect)
        }
    })
}

#[tauri::command]
async fn set_sensor(hid: State<'_, Hid>, id: String, key: String, value: u8) -> Result<(), String> {
    with_backend(&hid, |api, s| {
        if id.starts_with("compx:") {
            compx::set_sensor(api, &mut s.compx, &id, &key, value)
        } else if id.starts_with("hyperx:") {
            hyperx::set_sensor(api, &mut s.hyperx, &id, &key, value)
        } else if id.starts_with("lxd:") {
            lxd::set_sensor(api, &mut s.lxd, &key, value)
        } else {
            Err("this device has no adjustable sensor settings".into())
        }
    })
}

/// Streams `key-travel` events ([[matrix index, travel], ...]) until `stop_key_monitor`.
#[tauri::command]
async fn start_key_monitor(app: AppHandle, hid: State<'_, Hid>, id: String) -> Result<(), String> {
    let mut b = hid.0.lock().map_err(|e| e.to_string())?;
    b.monitor = None; // stop any previous stream first
    let monitor = aula::start_monitor(&b.api, &id, move |batch| {
        let _ = app.emit("key-travel", batch);
    })?;
    b.monitor = Some(monitor);
    Ok(())
}

#[tauri::command]
async fn stop_key_monitor(hid: State<'_, Hid>) -> Result<(), String> {
    hid.0.lock().map_err(|e| e.to_string())?.monitor = None;
    Ok(())
}

#[tauri::command]
async fn set_onboard_mode(hid: State<'_, Hid>, id: String, onboard: bool) -> Result<(), String> {
    with_backend(&hid, |api, s| logitech::set_onboard_mode(api, &mut s.logitech, &id, onboard))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let api = HidApi::new().expect("failed to initialise hidapi");
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(Hid(Mutex::new(Backend { api, sessions: Sessions::default(), monitor: None })))
        .on_window_event(|window, event| {
            // Never leave the keyboard in test mode after the app closes.
            if let tauri::WindowEvent::Destroyed = event {
                if let Ok(mut b) = window.state::<Hid>().0.lock() {
                    b.monitor = None;
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            list_devices,
            set_dpi,
            set_report_rate,
            set_onboard_mode,
            set_lighting,
            set_trigger,
            start_key_monitor,
            stop_key_monitor,
            set_custom_colors,
            set_led,
            set_sensor
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
