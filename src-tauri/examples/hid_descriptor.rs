//! Dev tool: `cargo run --example hid_descriptor -- 03f0 0f98` dumps the report descriptor of
//! every vendor (usage page >= 0xff00) interface of a device.
use hidapi::HidApi;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let vid = u16::from_str_radix(&args[1], 16).unwrap();
    let pid = u16::from_str_radix(&args[2], 16).unwrap();
    let api = HidApi::new().unwrap();
    for d in api.device_list().filter(|d| d.vendor_id() == vid && d.product_id() == pid && (d.usage_page() >= 0xff00 || std::env::var("ALL").is_ok())) {
        let dev = api.open_path(d.path()).unwrap();
        let mut buf = [0u8; 4096];
        let n = dev.get_report_descriptor(&mut buf).unwrap();
        println!("if {} up {:04x} len {}", d.interface_number(), d.usage_page(), n);
        println!("{}", buf[..n].iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" "));
    }
}
