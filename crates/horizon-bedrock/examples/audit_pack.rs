// Prints the keys a pack carries that the engine ignores.
//   cargo run --release -p horizon-bedrock --example audit_pack -- <pack>...
use horizon_bedrock::{audit, pack::Pack};
use std::path::Path;

fn main() {
    let mut report = audit::Report::new();
    for arg in std::env::args().skip(1) {
        let Ok(pack) = Pack::open(Path::new(&arg)) else {
            eprintln!("cannot open {arg}");
            continue;
        };
        for p in pack.paths().map(str::to_owned).collect::<Vec<_>>() {
            if !p.ends_with(".json") {
                continue;
            }
            let Ok(v) = pack.read_json(&p) else { continue };
            audit::scan(p.split('/').next().unwrap_or(""), &v, &mut report);
        }
    }
    for line in audit::lines(&report) {
        println!("{line}");
    }
}
