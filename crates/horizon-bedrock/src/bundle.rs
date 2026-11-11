//! Everything a converted pack holds, loaded for the renderer: the folder
//! [`crate::convert::to_java`] wrote (or any folder with `assets/horizon_bedrock/` in it, such as the
//! game's merged resource root).

use crate::animation::Library;
use crate::entity::{ClientEntity, RenderController};
use crate::geometry::{self, Geometry};
use crate::material::Materials;
use crate::particle::Effect;
use crate::sounds::{Definitions, EventSounds};
use crate::{convert::DATA_DIR, jsonc};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A pack's visual description.
#[derive(Default)]
pub struct Bundle {
    /// Client entities by identifier (`minecraft:zombie`), lower case.
    pub entities: HashMap<String, ClientEntity>,
    /// Attachables by the item they dress, lower case.
    pub attachables: HashMap<String, ClientEntity>,
    pub library: Library,
    pub render_controllers: HashMap<String, RenderController>,
    pub geometries: HashMap<String, Geometry>,
    pub materials: Materials,
    pub effects: HashMap<String, Arc<Effect>>,
    pub sound_definitions: Definitions,
    pub sound_events: EventSounds,
    /// The packs' options.
    pub settings: Vec<crate::settings::Def>,
    /// `fogs/`, by identifier (`custom:crystal_caverns_fog`).
    pub fogs: HashMap<String, crate::world_data::Fog>,
    /// What the packs say of a biome's look (`biomes/*.client_biome.json`, `biomes_client/`,
    /// `biomes_client.json`), by biome identifier; the highest pack wins.
    pub client_biomes: HashMap<String, crate::world_data::ClientBiome>,
    /// `blocks.json`: the textures, sound and flags a pack gives its blocks, by identifier.
    pub block_defs: HashMap<String, crate::world_data::BlockDef>,
    /// `block_culling/`, by identifier.
    pub culling_rules: HashMap<String, crate::world_data::CullRules>,
    /// Keys the packs carry that nothing reads (see [`crate::audit`]).
    pub ignored: crate::audit::Report,
}

/// A PNG's size and which pixels are not clear.
fn read_sprite(path: &Path) -> Option<geometry::Sprite> {
    let mut decoder = png::Decoder::new(std::io::BufReader::new(fs::File::open(path).ok()?));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width, info.height);
    let channels = info.color_type.samples();
    let alpha_at = |i: usize| -> bool {
        match info.color_type {
            png::ColorType::Rgba => buf[i * 4 + 3] > 8,
            png::ColorType::GrayscaleAlpha => buf[i * 2 + 1] > 8,
            _ => true,
        }
    };
    let _ = channels;
    Some(geometry::Sprite {
        size: [w, h],
        opaque: (0..(w * h) as usize).map(alpha_at).collect(),
    })
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        if p.is_dir() {
            files(&p, out);
        } else {
            out.push(p);
        }
    }
}

/// The top folders a pack's data holds (what is not a pack's own folder).
fn is_data_folder(name: &std::ffi::OsStr) -> bool {
    matches!(
        name.to_str().unwrap_or(""),
        "entity"
            | "attachables"
            | "animations"
            | "animation_controllers"
            | "render_controllers"
            | "models"
            | "particles"
            | "materials"
            | "sounds"
            | "fogs"
            | "biomes"
            | "biomes_client"
            | "block_culling"
    )
}

/// The packs' folders under `base`, lowest first: `order.txt`'s, then any others by name. The
/// empty name stands for `base` itself (files of a conversion that kept no folders).
fn namespaces(base: &Path) -> Vec<String> {
    let mut out = vec![String::new()];
    let listed: Vec<String> = fs::read_to_string(base.join("order.txt"))
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_owned())
        .filter(|l| !l.is_empty())
        .collect();
    let mut others: Vec<String> = fs::read_dir(base)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir() && !is_data_folder(&e.file_name()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !listed.contains(n))
        .collect();
    others.sort();
    for n in others.into_iter().chain(listed) {
        if base.join(&n).is_dir() {
            out.push(n);
        }
    }
    out
}

impl Bundle {
    /// What the game itself gives every pack: the vanilla animations, render controllers and
    /// geometries a pack refers to without defining them. A pack's own files, read after, win.
    pub fn add_builtins(&mut self) {
        let b = self;
        if let Ok(v) = jsonc::parse_bytes(include_bytes!("builtin_animations.json")) {
            b.library.add_file(&v);
        }
        // The vanilla models a pack names without shipping: the player's and the held items
        // (`item_sprite` of a thrown item, the bow, the crossbow, the trident), and the mobs
        // an entity of a pack may wear (the polar bear, the cat, the chicken).
        for bytes in [
            include_bytes!("builtin_geometries.json").as_slice(),
            include_bytes!("builtin_item_geometries.json").as_slice(),
            include_bytes!("builtin_vanilla_geometries.json").as_slice(),
        ] {
            if let Ok(v) = jsonc::parse_bytes(bytes)
                && let Ok(list) = geometry::parse_file(&v)
            {
                for g in list {
                    b.geometries.insert(g.id.to_ascii_lowercase(), g);
                }
            }
        }
        if let Ok(v) = jsonc::parse_bytes(include_bytes!("builtin_render_controllers.json")) {
            RenderController::add_file(&v, &mut b.render_controllers);
        }
    }

    /// Reads `<root>/assets/horizon_bedrock/`. A root without it gives an empty bundle.
    ///
    /// Each converted pack keeps its data in a folder of its own ([`crate::convert::namespace`]);
    /// `order.txt` there lists them from the lowest pack of the stack to the highest, so a pack
    /// above another wins an identifier both define. Files straight under the folder (an older
    /// conversion) count as the lowest.
    pub fn load(root: &Path) -> Bundle {
        let base = root.join(DATA_DIR);
        let mut all: Vec<(PathBuf, PathBuf)> = Vec::new();
        for ns in namespaces(&base) {
            let dir = if ns.is_empty() { base.clone() } else { base.join(&ns) };
            let mut found = Vec::new();
            files(&dir, &mut found);
            for path in found {
                let Ok(rel) = path.strip_prefix(&dir) else { continue };
                if ns.is_empty()
                    && rel
                        .components()
                        .next()
                        .is_some_and(|c| base.join(c.as_os_str()).is_dir() && !is_data_folder(c.as_os_str()))
                {
                    continue;
                }
                all.push((path.clone(), rel.to_path_buf()));
            }
        }
        let mut b = Bundle::default();
        b.add_builtins();
        for (path, rel) in all {
            let rel = rel.to_string_lossy().replace('\\', "/");
            let Ok(bytes) = fs::read(&path) else { continue };
            if rel.ends_with(".material") {
                if let Ok(v) = jsonc::parse_bytes(&bytes) {
                    b.materials.add_file(&v);
                }
                continue;
            }
            if !rel.ends_with(".json") {
                continue;
            }
            let v = match jsonc::parse_bytes(&bytes) {
                Ok(v) => v,
                Err(e) => {
                    log::warn!("Horizon: Bedrock file {rel} does not parse: {e}");
                    continue;
                }
            };
            crate::audit::scan(rel.split('/').next().unwrap_or(""), &v, &mut b.ignored);
            match rel.split('/').next().unwrap_or("") {
                "entity" => {
                    if let Some(d) = ClientEntity::parse(&v) {
                        let key = d.id.to_ascii_lowercase();
                        // One file per version of an entity: the highest `min_engine_version`
                        // stays, and of equal ones the later file.
                        if b.entities.get(&key).is_none_or(|old| old.min_engine <= d.min_engine) {
                            b.entities.insert(key, d);
                        }
                    }
                }
                "attachables" => {
                    if let Some(d) = ClientEntity::parse(&v) {
                        let key = d.id.to_ascii_lowercase();
                        if b.attachables.get(&key).is_none_or(|old| old.min_engine <= d.min_engine) {
                            b.attachables.insert(key, d);
                        }
                    }
                }
                "animations" | "animation_controllers" => b.library.add_file(&v),
                "render_controllers" => RenderController::add_file(&v, &mut b.render_controllers),
                "models" => {
                    if let Ok(list) = geometry::parse_file(&v) {
                        for g in list {
                            for variant in g.mesh_variants() {
                                b.geometries.insert(variant.id.clone(), variant);
                            }
                            b.geometries.insert(g.id.to_ascii_lowercase(), g);
                        }
                    }
                }
                "particles" => {
                    if let Some(fx) = Effect::parse(&v) {
                        b.effects.insert(fx.id.to_ascii_lowercase(), Arc::new(fx));
                    }
                }
                "sounds" if rel == "sounds/sound_definitions.json" => {
                    b.sound_definitions.add_file(&v)
                }
                "sounds.json" => b.sound_events.add_file(&v),
                "fogs" => {
                    if let Some(fog) = crate::world_data::Fog::parse(&v) {
                        b.fogs.insert(fog.id.clone(), fog);
                    }
                }
                "biomes" | "biomes_client" => {
                    if let Some(biome) = crate::world_data::ClientBiome::parse(&v) {
                        b.client_biomes.insert(biome.id.clone(), biome);
                    }
                }
                "biomes_client.json" => {
                    for biome in crate::world_data::ClientBiome::parse_legacy(&v) {
                        b.client_biomes.insert(biome.id.clone(), biome);
                    }
                }
                "blocks.json" => b.block_defs.extend(crate::world_data::BlockDef::parse_file(&v)),
                "block_culling" => {
                    if let Some(rules) = crate::world_data::CullRules::parse(&v) {
                        b.culling_rules.insert(rules.id.clone(), rules);
                    }
                }
                _ if rel == crate::settings::FILE => {
                    for d in crate::settings::from_json(&v) {
                        b.settings.retain(|o| o.name != d.name);
                        b.settings.push(d);
                    }
                }
                _ => {}
            }
        }
        b.fit_sprites(root);
        b
    }

    /// Gives each `texture_meshes` sprite its texture's real size and, for `use_pixel_depth`
    /// ones, its silhouette, read from the converted PNGs.
    fn fit_sprites(&mut self, root: &Path) {
        let mut fitted: Vec<Geometry> = Vec::new();
        for def in self.entities.values().chain(self.attachables.values()) {
            for (key, path) in &def.textures {
                for geometry in def.geometry.values() {
                    let Some(base) = self.geometries.get(geometry) else {
                        continue;
                    };
                    if !self
                        .geometries
                        .contains_key(&geometry::mesh_id(geometry, key))
                    {
                        continue;
                    }
                    let png = root.join(format!(
                        "assets/minecraft/textures/{}.png",
                        crate::convert::atlas_name(path)
                    ));
                    if let Some(sprite) = read_sprite(&png) {
                        fitted.push(base.mesh_variant(key, Some(&sprite)));
                    }
                }
            }
        }
        for g in fitted {
            self.geometries.insert(g.id.clone(), g);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty() && self.attachables.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_what_to_java_wrote() {
        let src =
            std::env::temp_dir().join(format!("horizon-bedrock-bundle-{}", std::process::id()));
        let _ = fs::remove_dir_all(&src);
        let put = |rel: &str, text: &str| {
            let p = src.join(DATA_DIR).join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, text).unwrap();
        };
        put(
            "entity/z.json",
            r#"{"minecraft:client_entity":{"description":{"identifier":"minecraft:Zombie","geometry":{"default":"geometry.z"}}}}"#,
        );
        put(
            "attachables/a.json",
            r#"{"minecraft:attachable":{"description":{"identifier":"minecraft:stick"}}}"#,
        );
        put("animations/a.json", r#"{"animations":{"animation.x":{}}}"#);
        put(
            "render_controllers/r.json",
            r#"{"render_controllers":{"controller.render.x":{}}}"#,
        );
        put(
            "models/entity/z.geo.json",
            r#"{"format_version":"1.12.0","minecraft:geometry":[{"description":{"identifier":"geometry.Z","texture_width":16,"texture_height":16},"bones":[]}]}"#,
        );
        put(
            "particles/p.json",
            r#"{"particle_effect":{"description":{"identifier":"t:p"},"components":{}}}"#,
        );
        put(
            "materials/m.material",
            r#"{"materials":{"version":"1.0.0","x:entity":{}}}"#,
        );
        put(
            "sounds/sound_definitions.json",
            r#"{"sound_definitions":{"t:s":{"sounds":["sounds/a"]}}}"#,
        );
        put(
            "sounds.json",
            r#"{"entity_sounds":{"entities":{"zombie":{"events":{"step":""}}}}}"#,
        );
        let b = Bundle::load(&src);
        assert!(b.entities.contains_key("minecraft:zombie"));
        assert!(b.attachables.contains_key("minecraft:stick"));
        assert!(b.library.animations.contains_key("animation.x"));
        assert!(b.render_controllers.contains_key("controller.render.x"));
        assert!(b.geometries.contains_key("geometry.z"));
        assert!(b.effects.contains_key("t:p"));
        assert!(b.sound_definitions.get("t:s").is_some());
        assert!(!b.sound_events.is_empty());
        assert!(!b.is_empty());
        assert!(Bundle::load(&src.join("nothing")).is_empty());
        let _ = fs::remove_dir_all(&src);
    }

    #[test]
    fn the_game_gives_the_fang_and_the_bed_their_geometries() {
        let mut b = Bundle::default();
        b.add_builtins();
        let fang = &b.geometries["geometry.evocation_fang"];
        let (upper, lower) = (
            fang.bone("upper_jaw").unwrap(),
            fang.bone("lower_jaw").unwrap(),
        );
        // The base is the column under the ground; the jaws, side by side, stand on it
        // inside its footprint, the second turned half way round its pivot.
        let (u, l) = (&upper.cubes[0], &lower.cubes[0]);
        let base = &fang.bone("base").unwrap().cubes[0];
        assert_eq!(base.origin[1] + base.size[1], 0.0);
        assert_eq!(lower.rotation, [0.0, 180.0, 0.0]);
        let turned = |c: &crate::geometry::Cube, axis: usize| {
            let p = lower.pivot[axis];
            [
                2.0 * p - (c.origin[axis] + c.size[axis]),
                2.0 * p - c.origin[axis],
            ]
        };
        assert_eq!(turned(l, 0), [-2.5, 1.5]);
        assert_eq!(turned(l, 2), [-4.0, 4.0]);
        assert_eq!([u.origin[0], u.origin[0] + u.size[0]], [-1.5, 2.5]);
        assert_eq!(fang.texture_size, [64.0, 32.0]);
        let bed = &b.geometries["geometry.bed"];
        let cubes: usize = bed.bones.iter().map(|b| b.cubes.len()).sum();
        assert_eq!(cubes, 6);
    }

    #[test]
    fn the_game_gives_the_held_item_and_mob_models_a_pack_names_without_shipping() {
        let mut b = Bundle::default();
        b.add_builtins();
        for id in [
            "geometry.item_sprite",
            "geometry.bow_standby",
            "geometry.bow_pulling_2",
            "geometry.crossbow_standby",
            "geometry.crossbow_rocket",
            "geometry.polarbear",
            "geometry.cat",
            "geometry.chicken",
        ] {
            assert!(b.geometries.contains_key(id), "{id}");
        }
        // The sprite of a thrown item: one flat 8 x 8 quad.
        let sprite = &b.geometries["geometry.item_sprite"];
        assert_eq!(sprite.texture_size, [8.0, 8.0]);
        let cube = &sprite.bone("body").unwrap().cubes[0];
        assert_eq!(cube.size, [8.0, 8.0, 0.0]);
        // The bow is a texture mesh on the hand bone, the pack's own texture on it.
        assert!(!b.geometries["geometry.bow_standby"].bone("rightitem").unwrap().texture_meshes.is_empty());
    }

    #[test]
    fn every_stacked_packs_data_is_kept_and_the_top_one_wins() {
        let src = std::env::temp_dir().join(format!("horizon-bedrock-stack-{}", std::process::id()));
        let _ = fs::remove_dir_all(&src);
        let base = src.join(DATA_DIR);
        let put = |ns: &str, rel: &str, text: &str| {
            let p = base.join(ns).join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, text).unwrap();
        };
        let toggle = |name: &str| {
            serde_json::json!([{"type": "toggle", "name": name, "text": name, "default": true}])
                .to_string()
        };
        let entity = |id: &str, geometry: &str| {
            format!(
                r#"{{"format_version":"1.10.0","minecraft:client_entity":{{"description":{{"identifier":"{id}","geometry":{{"default":"{geometry}"}}}}}}}}"#
            )
        };
        // Two packs: both have a settings file and a material file of the same name; both
        // define `minecraft:cow`, under different file names.
        put("a", "pack_settings.json", &toggle("ns:a"));
        put("b", "pack_settings.json", &toggle("ns:b"));
        put("a", "entity/cow_a.json", &entity("minecraft:cow", "geometry.low"));
        put("b", "entity/cow_b.json", &entity("minecraft:cow", "geometry.high"));
        // `b` is listed first, so `a` is the pack on top.
        fs::write(base.join("order.txt"), "b\na\n").unwrap();
        let loaded = Bundle::load(&src);
        let mut names: Vec<_> = loaded.settings.iter().map(|d| d.name.clone()).collect();
        names.sort();
        assert_eq!(names, ["ns:a", "ns:b"], "{names:?}");
        let cow = &loaded.entities["minecraft:cow"];
        assert_eq!(cow.geometry.get("default").map(String::as_str), Some("geometry.low"));
        // And the other way round.
        fs::write(base.join("order.txt"), "a\nb\n").unwrap();
        let loaded = Bundle::load(&src);
        let cow = &loaded.entities["minecraft:cow"];
        assert_eq!(cow.geometry.get("default").map(String::as_str), Some("geometry.high"));
        let _ = fs::remove_dir_all(&src);
    }
}
