//! Client entities whose declared materials glow (additive, emissive, blend, no depth write).
//!
//! `cargo run --release -p horizon-bedrock --example list_glow -- <pack>`

use horizon_bedrock::entity::ClientEntity;
use horizon_bedrock::material::Materials;
use horizon_bedrock::pack::Pack;
use std::path::Path;

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: list_glow <pack>");
        return;
    };
    let pack = Pack::open(Path::new(&path)).expect("pack");
    let mut mats = Materials::default();
    let mut defs = Vec::new();
    for p in pack.paths().map(str::to_owned).collect::<Vec<_>>() {
        if p.ends_with(".material") {
            if let Ok(v) = pack.read_json(&p) {
                mats.add_file(&v);
            }
        } else if (p.starts_with("entity/") || p.starts_with("attachables/"))
            && p.ends_with(".json")
            && let Ok(v) = pack.read_json(&p)
            && let Some(d) = ClientEntity::parse(&v)
        {
            defs.push(d);
        }
    }
    defs.sort_by(|a, b| a.id.cmp(&b.id));
    for d in &defs {
        let mut tags: Vec<String> = Vec::new();
        for (short, name) in &d.materials {
            let f = mats.flags(name);
            let mut t = Vec::new();
            if f.additive {
                t.push("add");
            }
            if f.alpha_test {
                t.push("at");
            }
            if f.emissive {
                t.push("emis");
            }
            if f.emissive_alpha {
                t.push("emisA");
            }
            if f.blend {
                t.push("blend");
            }
            if f.no_depth_write {
                t.push("nodw");
            }
            if f.energy_swirl {
                t.push("swirl");
            }
            if f.stencil_write != 0 || f.stencil_not_equal != 0 {
                t.push("stencil");
            }
            if !t.is_empty() {
                tags.push(format!("{short}={name}[{}]", t.join(",")));
            }
        }
        if !tags.is_empty() {
            tags.sort();
            println!(
                "{}{}\t{}",
                d.id,
                if d.attachable { " (att)" } else { "" },
                tags.join(" ")
            );
        }
    }
}
