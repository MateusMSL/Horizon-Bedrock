//! Every attachable of a pack, evaluated once as a held item: which draw nothing, which name a
//! geometry or a texture the pack lacks, which have no visible bone with cubes or meshes.
//!   cargo run --release -p horizon-bedrock --example audit_attachables -- <pack> [slot]
//! Prints `id<TAB>problem` per failing attachable, then the totals.
use horizon_bedrock::animation::Library;
use horizon_bedrock::entity::{Assets, ClientEntity, Instance, RenderController};
use horizon_bedrock::molang::{self, Host, Value};
use horizon_bedrock::pack::Pack;
use std::collections::{HashMap, HashSet};
use std::path::Path;

struct H {
    slot: String,
}
impl Host for H {
    fn query(&mut self, name: &str, _args: &[Value]) -> Value {
        match name {
            "is_attached" | "is_alive" | "is_on_ground" | "graphics_mode_is_any"
            | "is_pack_setting_enabled" | "is_owner_identifier_any" => Value::Num(1.0),
            "delta_time" => Value::Num(0.05),
            "item_slot_to_bone_name" => Value::Str("rightitem".into()),
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
    let [pack_path, rest @ ..] = args.as_slice() else {
        return;
    };
    let slot = rest.first().cloned().unwrap_or("main_hand".into());
    let pack = Pack::open(Path::new(pack_path)).expect("pack");
    let paths: HashSet<String> = pack.paths().map(str::to_owned).collect();
    let has_texture = |t: &str| {
        ["png", "tga", "jpg", "jpeg", "texture_set.json"]
            .iter()
            .any(|e| paths.contains(&format!("{t}.{e}")))
    };
    let mut lib = Library::default();
    let mut rcs = HashMap::new();
    let mut defs = Vec::new();
    let mut geos = HashMap::new();
    for p in pack.paths().map(str::to_owned).collect::<Vec<_>>() {
        if !p.ends_with(".json") {
            continue;
        }
        let Ok(v) = pack.read_json(&p) else { continue };
        if p.starts_with("animations/") || p.starts_with("animation_controllers/") {
            lib.add_file(&v)
        }
        if p.starts_with("render_controllers/") {
            RenderController::add_file(&v, &mut rcs)
        }
        if p.starts_with("models/") {
            for g in horizon_bedrock::geometry::parse_file(&v).unwrap_or_default() {
                for var in g.mesh_variants() {
                    geos.insert(var.id.clone(), var);
                }
                geos.insert(g.id.to_ascii_lowercase(), g);
            }
        }
        if p.starts_with("attachables/") {
            if let Some(d) = ClientEntity::parse(&v) {
                defs.push(d)
            }
        }
    }
    let assets = Assets {
        animations: &lib,
        render_controllers: &rcs,
        geometries: Some(&geos),
    };
    let (mut ok, mut bad) = (0, 0);
    defs.sort_by(|a, b| a.id.cmp(&b.id));
    for def in &defs {
        let mut inst = Instance::new(1);
        let mut host = H { slot: slot.clone() };
        let mut f = inst.update(def, &assets, &mut host, 0.05);
        for _ in 0..40 {
            f = inst.update(def, &assets, &mut host, 0.05);
        }
        let mut problems: Vec<String> = Vec::new();
        if f.layers.is_empty() {
            problems.push("sem camada (render controller/condição)".into());
        }
        let mut drawable = false;
        for l in &f.layers {
            match geos.get(&l.geometry.to_ascii_lowercase()).or_else(|| geos.get(&l.geometry)) {
                None => problems.push(format!("geometria ausente {}", l.geometry)),
                Some(g) => {
                    if g.bones.iter().any(|b| {
                        (!b.cubes.is_empty() || !b.texture_meshes.is_empty())
                            && l.bone_visible(&b.name.to_ascii_lowercase())
                    }) {
                        drawable = true;
                    }
                }
            }
            if !has_texture(&l.texture) {
                problems.push(format!("textura ausente {}", l.texture));
            }
        }
        if !f.layers.is_empty() && !drawable {
            problems.push("nenhum osso visível com cubos/meshes".into());
        }
        if problems.is_empty() {
            ok += 1;
        } else {
            bad += 1;
            problems.sort();
            problems.dedup();
            println!("{}\t{}", def.id, problems.join("; "));
        }
    }
    println!("total {} ok {} com problema {}", defs.len(), ok, bad);
}
