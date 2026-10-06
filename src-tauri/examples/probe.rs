//! Dev tool: `cargo run --example probe` prints every detected device as JSON.
use hidapi::HidApi;
use openmouse_lib::devices;

fn main() {
    let api = HidApi::new().expect("hidapi");
    let t = std::time::Instant::now();
    let mut s = devices::Sessions::default();
    let list = devices::list(&api, &mut s);
    println!("{}", serde_json::to_string_pretty(&list).unwrap());
    eprintln!("listed in {:?}", t.elapsed());
    // Rewrites each mouse's current DPI to time a cached write.
    for d in list.iter().filter(|d| d.dpi.is_some()) {
        let t = std::time::Instant::now();
        let r = devices::logitech::set_dpi(&api, &mut s.logitech, &d.id, d.dpi.as_ref().unwrap().current);
        eprintln!("set_dpi {} -> {:?} in {:?}", d.id, r, t.elapsed());
    }
    // Rewrites each AULA keyboard's current settings (no visible change) to exercise the write path.
    let write = std::env::var("PROBE_WRITE").is_ok();
    for d in list.iter().filter(|d| write && d.id.starts_with("aula:")) {
        let Some(kb) = &d.keyboard else { continue };
        let t = std::time::Instant::now();
        if let Some(l) = &kb.lighting {
            eprintln!("set_lighting -> {:?}", devices::aula::set_lighting(&api, &mut s.aula, &d.id, l));
        }
        if let Some(k) = kb.actuation.as_ref().and_then(|a| a.keys.first()) {
            let r = devices::aula::set_trigger(&api, &mut s.aula, &d.id, &k.trigger, &[k.index]);
            eprintln!("set_trigger key {} -> {:?}", k.index, r);
        }
        if let Some(r) = &d.report_rate {
            eprintln!("set_report_rate -> {:?}", devices::aula::set_report_rate(&api, &mut s.aula, &d.id, r.current_hz));
        }
        eprintln!("aula writes in {:?}", t.elapsed());
    }
}
