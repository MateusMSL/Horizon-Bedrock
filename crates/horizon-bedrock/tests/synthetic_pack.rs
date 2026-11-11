//! A pack written in the test, one small file per resource kind the roadmap lists, taken through
//! the whole way the game takes it: open the folder, convert it, load the bundle, run an entity
//! and an attachable. A resource that stops working breaks the one line that names it.

use horizon_bedrock::bundle::Bundle;
use horizon_bedrock::convert::to_java;
use horizon_bedrock::entity::{Assets, Instance};
use horizon_bedrock::molang::{Host, Value};
use horizon_bedrock::pack::Pack;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const MANIFEST: &str = r#"{"format_version":2,
  "header":{"name":"Synthetic","description":"test","uuid":"00000000-0000-4000-8000-000000000001","version":[1,0,0],"min_engine_version":[1,21,0]},
  "modules":[{"type":"resources","uuid":"00000000-0000-4000-8000-000000000002","version":[1,0,0]}]}"#;

const ENTITY: &str = r#"{"format_version":"1.10.0","minecraft:client_entity":{"description":{
  "identifier":"synth:beast",
  "materials":{"default":"entity_alphatest"},
  "textures":{"default":"textures/entity/beast","sheared":"textures/entity/beast_sheared"},
  "geometry":{"default":"geometry.synth.beast","baby":"geometry.synth.beast_baby"},
  "animations":{"idle":"animation.synth.idle","burn":"animation.synth.burn","ctl":"controller.animation.synth.main"},
  "scripts":{"initialize":["v.n = 3;"],"pre_animation":["v.big = q.is_saddled;"],"scale":"v.big ? 2 : 1",
    "animate":["idle","ctl"]},
  "render_controllers":["controller.render.synth"]}}}"#;

const GEOMETRY: &str = r#"{"format_version":"1.12.0","minecraft:geometry":[
  {"description":{"identifier":"geometry.synth.beast","texture_width":16,"texture_height":16},
   "bones":[{"name":"body","pivot":[0,12,0],"cubes":[{"origin":[-4,6,-3],"size":[8,6,6],"uv":[0,0]}]},
            {"name":"head","parent":"body","pivot":[0,12,0],"cubes":[{"origin":[-3,12,-3],"size":[6,6,6],"uv":[0,0]}]},
            {"name":"saddle","parent":"body","pivot":[0,12,0],"cubes":[{"origin":[-4,11,-3],"size":[8,1,6],"uv":[0,0]}]}]},
  {"description":{"identifier":"geometry.synth.beast_baby","texture_width":16,"texture_height":16},
   "bones":[{"name":"body","pivot":[0,6,0],"cubes":[{"origin":[-2,3,-2],"size":[4,3,4],"uv":[0,0]}]}]},
  {"description":{"identifier":"geometry.synth.staff","texture_width":16,"texture_height":16},
   "bones":[{"name":"root","pivot":[0,0,0]},
            {"name":"item","parent":"root","pivot":[0,0,0],"binding":"q.item_slot_to_bone_name(c.item_slot)",
             "cubes":[{"origin":[-1,0,-1],"size":[2,12,2],"uv":[0,0]}]}]}]}"#;

const RENDER: &str = r#"{"format_version":"1.8.0","render_controllers":{
  "controller.render.synth":{"geometry":"q.is_baby ? Geometry.baby : Geometry.default",
    "arrays":{"textures":{"Array.skins":["Texture.default","Texture.sheared"]}},
    "textures":["Array.skins[q.is_sheared ? 1 : 0]"],"materials":[{"*":"Material.default"}],
    "part_visibility":[{"*":true},{"saddle":"q.is_saddled"}]},
  "controller.render.staff":{"geometry":"Geometry.default","textures":["Texture.default"],"materials":[{"*":"Material.default"}]}}}"#;

const ANIMATIONS: &str = r#"{"format_version":"1.8.0","animations":{
  "animation.synth.idle":{"loop":true,"bones":{"head":{"rotation":[10,0,0],"position":[0,1,0]}}},
  "animation.synth.burn":{"loop":true,"bones":{"body":{"rotation":[0,0,45]}}}}}"#;

const CONTROLLER: &str = r#"{"format_version":"1.10.0","animation_controllers":{
  "controller.animation.synth.main":{"initial_state":"default","states":{
    "default":{"transitions":[{"on_fire":"q.is_on_fire"}]},
    "on_fire":{"animations":["burn"],"transitions":[{"default":"!q.is_on_fire"}]}}}}}"#;

const ATTACHABLE: &str = r#"{"format_version":"1.10.0","minecraft:attachable":{"description":{
  "identifier":"synth:staff","item":{"synth:staff":"q.is_attached"},
  "materials":{"default":"entity_alphatest"},"textures":{"default":"textures/entity/beast"},
  "geometry":{"default":"geometry.synth.staff"},
  "scripts":{"parent_setup":["v.held = 1;"],"animate":[]},
  "render_controllers":["controller.render.staff"]}}}"#;

/// The vanilla layout: controllers listed apart, with the short name of the animation they play.
const OLD_ENTITY: &str = r#"{"format_version":"1.10.0","minecraft:client_entity":{"description":{
  "identifier":"synth:old","materials":{"default":"entity_alphatest"},"textures":{"default":"textures/entity/beast"},
  "geometry":{"default":"geometry.synth.beast"},
  "animations":{"move":"animation.synth.move"},
  "animation_controllers":[{"move":"controller.animation.synth.move"}],
  "scripts":{"animate":["move"]},"render_controllers":["controller.render.synth"]}}}"#;

const OLD_ANIMATIONS: &str = r#"{"format_version":"1.8.0","animations":{
  "animation.synth.move":{"loop":true,"bones":{"body":{"position":[0,3,0]}}}}}"#;

const OLD_CONTROLLER: &str = r#"{"format_version":"1.10.0","animation_controllers":{
  "controller.animation.synth.move":{"initial_state":"default","states":{"default":{"animations":["move"]}}}}}"#;

fn png(path: &Path) {
    let file = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(file, 16, 16);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .unwrap()
        .write_image_data(&[200u8; 16 * 16 * 4])
        .unwrap();
}

struct Dirs {
    root: PathBuf,
}
impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Writes the pack, converts it and loads what the game loads.
fn load() -> (Dirs, Bundle) {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("horizon-synth-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let src = root.join("pack");
    for (path, text) in [
        ("manifest.json", MANIFEST),
        ("entity/beast.entity.json", ENTITY),
        ("models/entity/beast.geo.json", GEOMETRY),
        ("render_controllers/synth.rc.json", RENDER),
        ("animations/synth.anim.json", ANIMATIONS),
        ("animation_controllers/synth.ac.json", CONTROLLER),
        ("attachables/staff.json", ATTACHABLE),
        (
            "textures/terrain_texture.json",
            r#"{"texture_data":{"stone":{"textures":"textures/blocks/custom_rock"},"grass_top":{"textures":["textures/blocks/lawn","textures/blocks/lawn"]}}}"#,
        ),
        (
            "textures/item_texture.json",
            r#"{"texture_data":{"apple":{"textures":"textures/items/fruit"}}}"#,
        ),
        (
            "textures/flipbook_textures.json",
            r#"[{"flipbook_texture":"textures/blocks/water_still","atlas_tile":"water_still","ticks_per_frame":4,"frames":[0,1,1,0],"blend_frames":false}]"#,
        ),
        ("entity/old.entity.json", OLD_ENTITY),
        ("animations/old.anim.json", OLD_ANIMATIONS),
        ("animation_controllers/old.ac.json", OLD_CONTROLLER),
    ] {
        let p = src.join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    for t in [
        "textures/entity/beast.png",
        "textures/entity/beast_sheared.png",
        "textures/blocks/custom_rock.png",
        "textures/blocks/lawn.png",
        "textures/blocks/water_still.png",
        "textures/items/fruit.png",
    ] {
        let p = src.join(t);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        png(&p);
    }
    let out = root.join("java");
    let pack = Pack::open(&src).expect("the folder opens as a pack");
    to_java(&pack, &out, (64, 0)).expect("the pack converts");
    let bundle = Bundle::load(&out);
    (Dirs { root }, bundle)
}

#[derive(Default)]
struct World {
    saddled: bool,
    sheared: bool,
    baby: bool,
    fire: bool,
}
impl Host for World {
    fn query(&mut self, name: &str, _args: &[Value]) -> Value {
        match name {
            "is_saddled" => Value::bool(self.saddled),
            "is_sheared" => Value::bool(self.sheared),
            "is_baby" => Value::bool(self.baby),
            "is_on_fire" => Value::bool(self.fire),
            "is_alive" | "is_on_ground" => Value::bool(true),
            _ => Value::Null,
        }
    }
}

fn assets(b: &Bundle) -> Assets<'_> {
    Assets {
        animations: &b.library,
        render_controllers: &b.render_controllers,
        geometries: Some(&b.geometries),
    }
}

#[test]
fn the_pack_loads_each_kind_of_resource() {
    let (_dirs, b) = load();
    assert!(b.entities.contains_key("synth:beast"), "client entity");
    assert!(b.attachables.contains_key("synth:staff"), "attachable");
    assert!(b.geometries.contains_key("geometry.synth.beast"), "geometry");
    assert!(b.render_controllers.contains_key("controller.render.synth"), "render controller");
    assert!(b.library.animations.get("animation.synth.idle").is_some(), "animation");
}

#[test]
fn an_entity_draws_its_geometry_texture_and_visible_bones() {
    let (_dirs, b) = load();
    let def = &b.entities["synth:beast"];
    let mut inst = Instance::new(1);
    let f = inst.update(def, &assets(&b), &mut World::default(), 0.05);
    assert_eq!(f.layers.len(), 1);
    let l = &f.layers[0];
    assert_eq!(l.geometry, "geometry.synth.beast");
    assert_eq!(l.texture, "textures/entity/beast");
    assert!(l.bone_visible("head") && !l.bone_visible("saddle"), "saddle hidden unsaddled");
}

#[test]
fn queries_pick_geometry_texture_scale_and_parts() {
    let (_dirs, b) = load();
    let def = &b.entities["synth:beast"];
    let mut inst = Instance::new(1);
    let mut w = World { saddled: true, sheared: true, baby: true, ..World::default() };
    let f = inst.update(def, &assets(&b), &mut w, 0.05);
    assert_eq!(f.layers[0].geometry, "geometry.synth.beast_baby", "baby model");
    assert_eq!(f.layers[0].texture, "textures/entity/beast_sheared", "sheared texture");
    assert_eq!(f.scale, 2.0, "scale from a variable set in pre_animation");
    assert!(f.layers[0].bone_visible("saddle"), "saddle shown when saddled");
}

#[test]
fn animations_and_controllers_pose_bones() {
    let (_dirs, b) = load();
    let def = &b.entities["synth:beast"];
    let mut inst = Instance::new(1);
    let mut w = World::default();
    let f = inst.update(def, &assets(&b), &mut w, 0.05);
    assert_eq!(f.pose.get("head").unwrap().rotation, [10.0, 0.0, 0.0], "looping animation");
    assert!(f.pose.get("body").is_none_or(|p| p.rotation == [0.0; 3]), "controller idle");
    w.fire = true;
    inst.update(def, &assets(&b), &mut w, 0.05);
    let f = inst.update(def, &assets(&b), &mut w, 0.05);
    assert_eq!(f.pose.get("body").unwrap().rotation[2], 45.0, "controller state on a query");
}

struct Held;
impl Host for Held {
    fn query(&mut self, name: &str, _args: &[Value]) -> Value {
        match name {
            "is_attached" => Value::bool(true),
            "item_slot_to_bone_name" => Value::Str("rightitem".into()),
            _ => Value::Null,
        }
    }
    fn context(&mut self, name: &str) -> Value {
        match name {
            "item_slot" => Value::Str("main_hand".into()),
            _ => Value::Null,
        }
    }
}

#[test]
fn an_attachable_names_the_holders_bone_it_binds() {
    let (_dirs, b) = load();
    let def = &b.attachables["synth:staff"];
    assert!(def.attachable);
    assert_eq!(def.parent_setup.len(), 1, "parent_setup");
    let mut inst = Instance::new(2);
    let f = inst.update(def, &assets(&b), &mut Held, 0.05);
    assert_eq!(f.layers[0].geometry, "geometry.synth.staff");
    let bones: HashMap<_, _> = b.geometries["geometry.synth.staff"]
        .bones
        .iter()
        .map(|g| (g.name.as_str(), g.binding.as_deref()))
        .collect();
    assert!(bones["item"].is_some_and(|x| x.contains("item_slot_to_bone_name")), "binding kept");
    assert!(bones["root"].is_none());
}

#[test]
fn a_controller_and_the_animation_it_plays_can_share_a_short_name() {
    let (_dirs, b) = load();
    let def = &b.entities["synth:old"];
    let mut inst = Instance::new(1);
    let f = inst.update(def, &assets(&b), &mut World::default(), 0.05);
    assert_eq!(f.pose.get("body").unwrap().position, [0.0, 3.0, 0.0]);
}

#[test]
fn block_and_item_textures_follow_the_texture_lists_and_flipbooks_animate() {
    let (dirs, _b) = load();
    let tex = dirs.root.join("java/assets/minecraft/textures");
    let src = dirs.root.join("pack/textures");
    let same = |java: &str, bedrock: &str| {
        std::fs::read(tex.join(java)).ok() == std::fs::read(src.join(bedrock)).ok()
            && tex.join(java).exists()
    };
    assert!(same("block/stone.png", "blocks/custom_rock.png"), "terrain_texture.json renames a block texture");
    assert!(same("block/grass_block_top.png", "blocks/lawn.png"), "a list of the same file is one file");
    assert!(same("item/apple.png", "items/fruit.png"), "item_texture.json renames an item texture");
    let meta = std::fs::read_to_string(tex.join("block/water_still.png.mcmeta")).unwrap();
    let meta: serde_json::Value = serde_json::from_str(&meta).unwrap();
    assert_eq!(meta["animation"]["frametime"], 4);
    assert_eq!(meta["animation"]["frames"], serde_json::json!([0, 1, 1, 0]));
    assert_eq!(meta["animation"]["interpolate"], false);
}
