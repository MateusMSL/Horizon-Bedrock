//! What a pack draws for one entity: its layers (geometry, texture, material), whether the
//! geometry and texture exist, how many bones show, the scale and the first bones' pose.
//!
//! `cargo run --release -p horizon-bedrock --example inspect_entity -- <pack> <entity id> [name tag]`
//!
//! `Q=name=value,...` answers queries.
//!
//! `<entity id>` is a client entity identifier (`minecraft:spider`); `all` lists every id.

use horizon_bedrock::animation::Library;
use horizon_bedrock::entity::{Assets, ClientEntity, Instance, RenderController};
use horizon_bedrock::geometry;
use horizon_bedrock::material::Materials;
use horizon_bedrock::molang::{self, Host};
use horizon_bedrock::pack::Pack;
use std::collections::HashMap;
use std::path::Path;

struct Probe {
    name: String,
    baby: bool,
}

impl Host for Probe {
    fn query(&mut self, name: &str, args: &[molang::Value]) -> molang::Value {
        // `Q=is_powered=1,swell_amount=0.5` answers those queries.
        if let Ok(q) = std::env::var("Q") {
            for kv in q.split(',') {
                if let Some((k, v)) = kv.split_once('=')
                    && k == name
                {
                    return molang::Value::Num(v.parse().unwrap_or(0.0));
                }
            }
        }
        match name {
            "is_name_any" => {
                molang::Value::bool(args.iter().any(
                    |a| matches!(a, molang::Value::Str(s) if s.eq_ignore_ascii_case(&self.name)),
                ))
            }
            "is_baby" => molang::Value::bool(self.baby),
            "is_on_ground" => molang::Value::bool(true),
            _ => molang::Value::Null,
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [pack_path, id, rest @ ..] = args.as_slice() else {
        eprintln!("usage: inspect_entity <pack> <entity id|all> [name tag]");
        return;
    };
    let tag = rest.first().cloned().unwrap_or_default();
    let pack = Pack::open(Path::new(pack_path)).expect("pack");
    let mut lib = Library::default();
    let mut rcs = HashMap::new();
    // What the game gives every pack, under the pack's own files.
    let mut builtin = horizon_bedrock::bundle::Bundle::default();
    builtin.add_builtins();
    let mut geos: HashMap<String, geometry::Geometry> = builtin.geometries;
    let mut defs = Vec::new();
    let mut mats = Materials::default();
    for p in pack.paths().map(str::to_owned).collect::<Vec<_>>() {
        if p.ends_with(".material") {
            if let Ok(v) = pack.read_json(&p) {
                mats.add_file(&v);
            }
            continue;
        }
        if !p.ends_with(".json") {
            continue;
        }
        let Ok(v) = pack.read_json(&p) else { continue };
        if p.starts_with("animations/") || p.starts_with("animation_controllers/") {
            lib.add_file(&v);
        }
        if p.starts_with("render_controllers/") {
            RenderController::add_file(&v, &mut rcs);
        }
        if p.starts_with("models/") {
            if let Ok(list) = geometry::parse_file(&v) {
                for g in list {
                    geos.insert(g.id.to_ascii_lowercase(), g);
                }
            }
        }
        if p.starts_with("entity/") {
            if let Some(d) = ClientEntity::parse(&v) {
                defs.push(d);
            }
        }
    }
    if id == "all" {
        for d in &defs {
            println!("{}", d.id);
        }
        return;
    }
    let assets = Assets {
        animations: &lib,
        render_controllers: &rcs,
        geometries: None,
    };
    let Some(def) = defs.iter().find(|d| d.id == *id) else {
        eprintln!("no client entity {id}");
        return;
    };
    println!(
        "{}: {} animations, {} render controllers",
        def.id,
        def.animations.len(),
        def.render_controllers.len()
    );
    for baby in [false, true] {
        let mut inst = Instance::new(1);
        let mut host = Probe {
            name: tag.clone(),
            baby,
        };
        let mut frame = None;
        for _ in 0..4 {
            frame = Some(inst.update(def, &assets, &mut host, 0.05));
        }
        let f = frame.unwrap();
        println!("-- baby={baby} scale={} layers={}", f.scale, f.layers.len());
        for l in &f.layers {
            let tex_exists = ["png", "tga"]
                .iter()
                .any(|e| pack.contains(&format!("{}.{e}", l.texture)));
            print!(
                "   geometry {} texture {} ({}) material {} {:?} color {:?} overlay {:?}",
                l.geometry,
                l.texture,
                if tex_exists { "ok" } else { "MISSING" },
                l.material,
                mats.flags(&l.material),
                l.color,
                l.overlay
            );
            match geos.get(&l.geometry) {
                Some(g) => {
                    let shown = g.bones.iter().filter(|b| l.bone_visible(&b.name)).count();
                    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
                    for b in &g.bones {
                        for c in &b.cubes {
                            lo = lo.min(c.origin[1]);
                            hi = hi.max(c.origin[1] + c.size[1]);
                        }
                    }
                    println!(" bones {shown}/{} y {lo}..{hi}", g.bones.len());
                }
                None => println!(" geometry MISSING"),
            }
        }
        let mut bones: Vec<_> = f.pose.bones.iter().collect();
        bones.sort_by(|a, b| a.0.cmp(b.0));
        for (name, b) in bones.iter().take(8) {
            println!(
                "   pose {name}: pos {:?} rot {:?} scale {:?}",
                b.position, b.rotation, b.scale
            );
        }
    }
}
