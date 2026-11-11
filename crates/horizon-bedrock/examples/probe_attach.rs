//! An attachable held by an entity: its layers, vertices-bearing bones and pose.
//! `cargo run --release -p horizon-bedrock --example probe_attach -- <pack> <attachable id> [slot]`
use horizon_bedrock::animation::Library;
use horizon_bedrock::entity::{Assets, ClientEntity, Instance, RenderController};
use horizon_bedrock::molang::{self, Host, Value};
use horizon_bedrock::pack::Pack;
use std::collections::HashMap;
use std::path::Path;

struct H {
    slot: String,
}
impl Host for H {
    fn query(&mut self, name: &str, args: &[Value]) -> Value {
        match name {
            "is_attached" | "is_alive" | "is_on_ground" | "graphics_mode_is_any" => Value::Num(1.0),
            "delta_time" => Value::Num(0.05),
            "item_slot_to_bone_name" => Value::Str("rightitem".into()),
            "is_owner_identifier_any" => Value::Num(1.0),
            "is_name_any" | "is_in_ui" => Value::Num(0.0),
            _ => Value::Null,
        }
    }
    fn context(&mut self, name: &str) -> Value {
        match name {
            "item_slot" => Value::Str(self.slot.as_str().into()),
            "is_first_person" => Value::Num(0.0),
            "owning_entity" => Value::Entity(1),
            _ => Value::Null,
        }
    }
    fn entity_var(&mut self, _e: u64, _v: molang::Sym) -> Value {
        Value::Num(1.0)
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [pack_path, id, rest @ ..] = args.as_slice() else { return };
    let slot = rest.first().cloned().unwrap_or("main_hand".into());
    let pack = Pack::open(Path::new(pack_path)).expect("pack");
    let mut lib = Library::default();
    let mut rcs = HashMap::new();
    let mut defs = Vec::new();
    let mut geos = HashMap::new();
    for p in pack.paths().map(str::to_owned).collect::<Vec<_>>() {
        if !p.ends_with(".json") { continue }
        let Ok(v) = pack.read_json(&p) else { continue };
        if p.starts_with("animations/") || p.starts_with("animation_controllers/") { lib.add_file(&v) }
        if p.starts_with("render_controllers/") { RenderController::add_file(&v, &mut rcs) }
        if p.starts_with("models/") {
            for g in horizon_bedrock::geometry::parse_file(&v).unwrap_or_default() {
                for var in g.mesh_variants() { geos.insert(var.id.clone(), var); }
                geos.insert(g.id.to_ascii_lowercase(), g);
            }
        }
        if p.starts_with("attachables/") {
            if let Some(d) = ClientEntity::parse(&v) { defs.push(d) }
        }
    }
    let assets = Assets { animations: &lib, render_controllers: &rcs, geometries: Some(&geos) };
    let def = defs.iter().find(|d| d.id == *id).expect("attachable");
    let mut inst = Instance::new(1);
    let mut host = H { slot };
    let mut f = inst.update(def, &assets, &mut host, 0.05);
    for _ in 0..40 { f = inst.update(def, &assets, &mut host, 0.05); }
    for l in &f.layers {
        println!("layer geo={} tex={} mat={} color={:?}", l.geometry, l.texture, l.material, l.color);
        if let Some(g) = geos.get(&l.geometry) {
            for b in &g.bones {
                let p = f.pose.get(&b.name);
                println!("  bone {} parent={:?} cubes={} meshes={} vis={} pose={:?}", b.name, b.parent, b.cubes.len(), b.texture_meshes.len(), l.bone_visible(&b.name.to_ascii_lowercase()), p.map(|p| (p.position, p.rotation, p.scale)));
            }
        }
    }
}
