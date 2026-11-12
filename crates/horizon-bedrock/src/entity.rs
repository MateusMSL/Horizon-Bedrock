//! Client entities (`entity/*.json`), attachables (`attachables/*.json`) and render
//! controllers (`render_controllers/*.json`): which model, texture and material an entity
//! wears, which animations run on it, and what it sets off.
//!
//! [`Instance`] is one entity (or one held item) in the world: its variables, its running
//! animations. [`Instance::update`] runs a frame and gives a [`Frame`] the renderer draws.
//! The game answers `query.*` through [`Host`]; nothing here reads game state itself.

use crate::animation::{Children, Env, Event, Library, Pose, Run, Scalar, Scope, compile, compile_lines};
use crate::molang::{Host, Program, ResKind, Rng, Value, Vars};
use serde_json::Value as Json;
use std::collections::HashMap;

fn string_map(v: Option<&Json>) -> HashMap<String, String> {
    v.and_then(Json::as_object)
        .map(|o| {
            o.iter()
                .filter_map(|(k, v)| Some((k.to_ascii_lowercase(), v.as_str()?.to_owned())))
                .collect()
        })
        .unwrap_or_default()
}

/// A condition that is a string, a number or a bool.
fn condition(v: &Json) -> Option<Option<Program>> {
    match v {
        Json::String(s) => Some(compile(s)),
        Json::Number(n) => Some(compile(&n.to_string())),
        Json::Bool(b) => Some(compile(if *b { "1" } else { "0" })),
        Json::Null => Some(None),
        _ => None,
    }
}

/// `["a", {"b": "cond"}]`: names, each with an optional condition.
fn named_conditions(v: Option<&Json>, lower: bool) -> Vec<(String, Option<Program>)> {
    let mut out = Vec::new();
    let one = |name: &str| {
        if lower {
            name.to_ascii_lowercase()
        } else {
            name.to_owned()
        }
    };
    let list: Vec<&Json> = match v {
        Some(Json::Array(a)) => a.iter().collect(),
        Some(other @ (Json::String(_) | Json::Object(_))) => vec![other],
        _ => return out,
    };
    for e in list {
        match e {
            Json::String(s) => out.push((one(s), None)),
            Json::Object(o) => {
                for (k, c) in o {
                    if let Some(c) = condition(c) {
                        // A condition that does not parse never holds.
                        out.push((
                            one(k),
                            Some(c.unwrap_or_else(|| compile("0").expect("literal"))),
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn scripts(v: Option<&Json>) -> Vec<Program> {
    match v {
        Some(Json::Array(a)) => compile_lines(a.iter().filter_map(Json::as_str)),
        Some(Json::String(s)) => compile(s).into_iter().collect(),
        _ => Vec::new(),
    }
}

/// The `animation_controllers` section: a list of one-entry maps, or one map.
fn controllers_of(d: &Json) -> HashMap<String, String> {
    let mut map = HashMap::new();
    match d.get("animation_controllers") {
        Some(Json::Array(a)) => {
            for item in a {
                map.extend(string_map(Some(item)));
            }
        }
        other => map.extend(string_map(other)),
    }
    map
}

/// What a client entity or attachable declares.
#[derive(Debug)]
pub struct ClientEntity {
    /// `minecraft:zombie`, or for an attachable the item it dresses.
    pub id: String,
    pub attachable: bool,
    pub materials: HashMap<String, String>,
    pub textures: HashMap<String, String>,
    pub geometry: HashMap<String, String>,
    /// Short name to `animation.x` or `controller.animation.x`.
    pub animations: HashMap<String, String>,
    /// The `animation_controllers` section apart (older layout, and the vanilla entities):
    /// short name to `controller.animation.x`. `animations` holds these too, where no
    /// animation takes the name.
    pub controllers: HashMap<String, String>,
    pub sounds: HashMap<String, String>,
    pub particles: HashMap<String, String>,
    pub initialize: Vec<Program>,
    pub pre_animation: Vec<Program>,
    pub parent_setup: Vec<Program>,
    pub animate: Vec<(String, Option<Program>)>,
    pub scale: Option<Program>,
    /// `scripts.scalex` / `scaley` / `scalez`: the scale along each axis, on top of `scale`.
    pub scale_axes: [Option<Program>; 3],
    /// `hide_armor`: the game's own armour is not drawn on the mob.
    pub hide_armor: bool,
    /// `scripts.hide_held_items`: nothing is drawn in the mob's hands while it holds.
    pub hide_held_items: Option<Program>,
    /// `held_item_scale`: how big what the mob holds is drawn.
    pub held_item_scale: Option<Program>,
    /// `held_item_ignores_lighting`: what the mob holds is lit in full.
    pub held_item_ignores_lighting: bool,
    pub render_controllers: Vec<(String, Option<Program>)>,
    /// Attachables only: items other than `id` it also dresses, with conditions.
    pub items: Vec<(String, Option<Program>)>,
    pub enable_attachables: bool,
    /// `min_engine_version`: a pack carries one file per version of the same entity; the game
    /// uses the highest one (none counts as 0.0.0).
    pub min_engine: [u32; 3],
}

impl ClientEntity {
    /// Reads a file with a `minecraft:client_entity` or `minecraft:attachable` root.
    pub fn parse(root: &Json) -> Option<ClientEntity> {
        let (attachable, body) = match root.get("minecraft:client_entity") {
            Some(b) => (false, b),
            None => (true, root.get("minecraft:attachable")?),
        };
        let d = body.get("description")?;
        let s = d.get("scripts");
        let sub = |k: &str| s.and_then(|s| s.get(k));
        Some(ClientEntity {
            id: d.get("identifier")?.as_str()?.to_owned(),
            attachable,
            materials: string_map(d.get("materials")),
            textures: string_map(d.get("textures")),
            geometry: string_map(d.get("geometry"))
                .into_iter()
                .map(|(k, v)| (k, v.to_ascii_lowercase()))
                .collect(),
            animations: {
                let mut map = string_map(d.get("animations"));
                for (k, v) in controllers_of(d) {
                    map.entry(k).or_insert(v);
                }
                map.into_iter()
                    .map(|(k, v)| (k, v.to_ascii_lowercase()))
                    .collect()
            },
            controllers: controllers_of(d)
                .into_iter()
                .map(|(k, v)| (k, v.to_ascii_lowercase()))
                .collect(),
            sounds: string_map(d.get("sound_effects")),
            particles: string_map(d.get("particle_effects")),
            initialize: scripts(sub("initialize")),
            pre_animation: scripts(sub("pre_animation")),
            parent_setup: scripts(sub("parent_setup")),
            animate: named_conditions(sub("animate"), true),
            scale: sub("scale").and_then(condition).flatten(),
            // Vanilla writes `scaleX`, other packs `scalex`.
            scale_axes: ["scalex", "scaley", "scalez"].map(|k| {
                s.and_then(Json::as_object)
                    .and_then(|o| o.iter().find(|(n, _)| n.eq_ignore_ascii_case(k)))
                    .and_then(|(_, v)| condition(v))
                    .flatten()
            }),
            hide_armor: d.get("hide_armor").and_then(Json::as_bool).unwrap_or(false),
            hide_held_items: sub("hide_held_items").and_then(condition).flatten(),
            held_item_scale: d.get("held_item_scale").and_then(condition).flatten(),
            held_item_ignores_lighting: d
                .get("held_item_ignores_lighting")
                .and_then(Json::as_bool)
                .unwrap_or(false),
            render_controllers: named_conditions(d.get("render_controllers"), true),
            items: d
                .get("item")
                .and_then(Json::as_object)
                .map(|o| {
                    o.iter()
                        .filter_map(|(k, v)| Some((k.clone(), condition(v)?)))
                        .collect()
                })
                .unwrap_or_default(),
            enable_attachables: d
                .get("enable_attachables")
                .and_then(Json::as_bool)
                .unwrap_or(false),
            min_engine: d
                .get("min_engine_version")
                .and_then(crate::manifest::Version::from_json)
                .map_or([0; 3], |v| v.0),
        })
    }
}

/// `*` matches any run of characters; the rest match case-insensitively.
pub fn glob(pattern: &str, name: &str) -> bool {
    fn go(p: &[u8], n: &[u8]) -> bool {
        match p.split_first() {
            None => n.is_empty(),
            Some((b'*', rest)) => (0..=n.len()).any(|i| go(rest, &n[i..])),
            Some((c, rest)) => {
                n.first().is_some_and(|m| m.eq_ignore_ascii_case(c)) && go(rest, &n[1..])
            }
        }
    }
    go(pattern.as_bytes(), name.as_bytes())
}

/// How a render controller picks what to draw.
#[derive(Debug, Default)]
pub struct RenderController {
    pub id: String,
    geometry: Option<Program>,
    textures: Vec<Program>,
    materials: Vec<(String, Program)>,
    part_visibility: Vec<(String, Program)>,
    /// `array.name` (without `array.`) to its elements.
    arrays: HashMap<String, Vec<Value>>,
    color: Option<[Scalar; 4]>,
    overlay: Option<[Scalar; 4]>,
    ignore_lighting: Option<Program>,
    light_multiplier: Option<Scalar>,
    /// `uv_anim`: offset and scale of the texture coordinates.
    uv_anim: Option<([Scalar; 2], [Scalar; 2])>,
    /// `on_fire_color`: the tint while the entity burns.
    on_fire_color: Option<[Scalar; 4]>,
    /// `is_hurt_color`: the overlay while the entity is hurt.
    hurt_color: Option<[Scalar; 4]>,
    /// `rebuild_animation_matrices`: read; the pose is rebuilt every frame here anyway, so it
    /// changes nothing.
    pub rebuild_animation_matrices: bool,
    /// `filter_lighting`: flat lighting on the layers it draws.
    pub filter_lighting: bool,
}

fn pair(v: Option<&Json>, d: f32) -> [Scalar; 2] {
    let a = v.and_then(Json::as_array);
    std::array::from_fn(|i| {
        a.and_then(|a| a.get(i))
            .and_then(Scalar::from_json)
            .unwrap_or(Scalar::Const(d))
    })
}

fn rgba(v: &Json) -> Option<[Scalar; 4]> {
    let o = v.as_object()?;
    let ch = |k: &str, d: f32| {
        o.get(k)
            .and_then(Scalar::from_json)
            .unwrap_or(Scalar::Const(d))
    };
    Some([ch("r", 1.0), ch("g", 1.0), ch("b", 1.0), ch("a", 1.0)])
}

fn resource(s: &str) -> Option<Value> {
    let (kind, name) = s.split_once('.')?;
    let kind = match kind.to_ascii_lowercase().as_str() {
        "geometry" => ResKind::Geometry,
        "texture" => ResKind::Texture,
        "material" => ResKind::Material,
        _ => return None,
    };
    Some(Value::Res(kind, name.to_ascii_lowercase().into()))
}

impl RenderController {
    pub fn parse(id: &str, v: &Json) -> RenderController {
        let mut rc = RenderController {
            id: id.to_ascii_lowercase(),
            ..Default::default()
        };
        rc.geometry = v.get("geometry").and_then(Json::as_str).and_then(compile);
        if let Some(a) = v.get("textures").and_then(Json::as_array) {
            rc.textures = a
                .iter()
                .filter_map(Json::as_str)
                .filter_map(compile)
                .collect();
        }
        if let Some(a) = v.get("materials").and_then(Json::as_array) {
            for e in a {
                for (bone, m) in e.as_object().into_iter().flatten() {
                    if let Some(p) = m.as_str().and_then(compile) {
                        rc.materials.push((bone.clone(), p));
                    }
                }
            }
        }
        match v.get("part_visibility") {
            Some(Json::Array(a)) => {
                for e in a {
                    for (bone, c) in e.as_object().into_iter().flatten() {
                        if let Some(Some(p)) = condition(c) {
                            rc.part_visibility.push((bone.clone(), p));
                        }
                    }
                }
            }
            Some(Json::Object(o)) => {
                for (bone, c) in o {
                    if let Some(Some(p)) = condition(c) {
                        rc.part_visibility.push((bone.clone(), p));
                    }
                }
            }
            _ => {}
        }
        if let Some(o) = v.get("arrays").and_then(Json::as_object) {
            for group in o.values().filter_map(Json::as_object) {
                for (name, list) in group {
                    let key = name.to_ascii_lowercase();
                    let key = key.strip_prefix("array.").unwrap_or(&key).to_owned();
                    let items = list
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Json::as_str)
                        .filter_map(resource)
                        .collect();
                    rc.arrays.insert(key, items);
                }
            }
        }
        rc.color = v.get("color").and_then(rgba);
        rc.overlay = v.get("overlay_color").and_then(rgba);
        rc.ignore_lighting = v.get("ignore_lighting").and_then(condition).flatten();
        rc.light_multiplier = v.get("light_color_multiplier").and_then(Scalar::from_json);
        rc.on_fire_color = v.get("on_fire_color").and_then(rgba);
        rc.hurt_color = v.get("is_hurt_color").and_then(rgba);
        rc.rebuild_animation_matrices = v
            .get("rebuild_animation_matrices")
            .and_then(Json::as_bool)
            .unwrap_or(false);
        rc.filter_lighting = v
            .get("filter_lighting")
            .and_then(Json::as_bool)
            .unwrap_or(false);
        rc.uv_anim = v
            .get("uv_anim")
            .filter(|u| u.is_object())
            .map(|u| (pair(u.get("offset"), 0.0), pair(u.get("scale"), 1.0)));
        rc
    }

    /// Adds the controllers in a `render_controllers/*.json` file.
    pub fn add_file(root: &Json, into: &mut HashMap<String, RenderController>) {
        if let Some(o) = root.get("render_controllers").and_then(Json::as_object) {
            for (id, v) in o {
                let rc = RenderController::parse(id, v);
                into.insert(rc.id.clone(), rc);
            }
        }
    }
}

/// One model, drawn with one texture.
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    /// `geometry.x`, lower case.
    pub geometry: String,
    /// The texture's path without extension.
    pub texture: String,
    /// The entity's name for the material (`materials` map value).
    pub material: String,
    /// Bone patterns and the material names for them.
    pub bone_materials: Vec<(String, String)>,
    /// Bone patterns in order and whether they show; the last that matches wins.
    pub visibility: Vec<(String, bool)>,
    pub color: [f32; 4],
    /// `overlay_color`: what the layer's texels are mixed toward (the alpha is how far).
    pub overlay: Option<[f32; 4]>,
    /// The overlay is the hurt colour (`is_hurt_color`): the game's own red flash covers it.
    pub hurt_overlay: bool,
    pub ignore_lighting: bool,
    pub light_multiplier: f32,
    /// Offset and scale of the texture coordinates (`uv_anim`), wrapping inside the texture.
    pub uv_anim: Option<([f32; 2], [f32; 2])>,
    /// `filter_lighting`: every face takes the same light, whichever way it points.
    pub flat_lighting: bool,
    /// Not the first of the render controller's textures: a multitexture material lays it
    /// over the ones before it.
    pub secondary: bool,
}

impl Layer {
    /// `color` with the overlay worked in, for a draw that can only multiply the texture: each
    /// channel moves toward the overlay's by its alpha (a black overlay darkens by that much).
    pub fn tint(&self) -> [f32; 4] {
        let mut c = self.color;
        if let Some(o) = self.overlay.filter(|_| !self.hurt_overlay) {
            let a = o[3].clamp(0.0, 1.0);
            for i in 0..3 {
                c[i] += (o[i].clamp(0.0, 1.0) - c[i]) * a;
            }
        }
        c
    }

    pub fn bone_visible(&self, bone: &str) -> bool {
        let bone = bone_of(bone);
        self.visibility
            .iter()
            .rev()
            .find(|(p, _)| glob(p, bone))
            .is_none_or(|(_, v)| *v)
    }

    pub fn material_for(&self, bone: &str) -> &str {
        let bone = bone_of(bone);
        self.bone_materials
            .iter()
            .rev()
            .find(|(p, _)| glob(p, bone))
            .map_or(&self.material, |(_, m)| m)
    }
}

/// The pack's bone a part belongs to: a rotated cube is a part of its own, `bone#cubeN`, which
/// follows the visibility and material the pack gives `bone`.
fn bone_of(part: &str) -> &str {
    part.find("#cube").map_or(part, |i| &part[..i])
}

/// Something a frame sets off, with the names the pack's maps resolved.
#[derive(Clone, Debug, PartialEq)]
pub enum Emit {
    Sound(String),
    Particle {
        effect: String,
        locator: Option<String>,
        /// `pre_effect_script`, run by the new emitter as it starts.
        pre: Option<String>,
    },
    /// The effect goes on while the state that set it off does.
    Hold {
        effect: String,
        locator: Option<String>,
        pre: Option<String>,
    },
}

#[derive(Debug, Default)]
pub struct Frame {
    pub scale: f32,
    /// `scalex` / `scaley` / `scalez` (1 when the pack gives none).
    pub scale_axes: [f32; 3],
    pub hide_armor: bool,
    pub hide_held_items: bool,
    /// How big what the mob holds is drawn (1 when the pack gives none).
    pub held_item_scale: f32,
    pub held_item_ignores_lighting: bool,
    pub layers: Vec<Layer>,
    pub pose: Pose,
    pub emits: Vec<Emit>,
}

/// What a pack provides, shared by all instances.
pub struct Assets<'a> {
    pub animations: &'a Library,
    pub render_controllers: &'a HashMap<String, RenderController>,
    /// The pack's models, for what asks where a bone is (`query.bone_origin`).
    pub geometries: Option<&'a HashMap<String, crate::geometry::Geometry>>,
}

struct WithArrays<'a> {
    inner: &'a mut dyn Host,
    arrays: &'a HashMap<String, Vec<Value>>,
}

impl Host for WithArrays<'_> {
    fn query(&mut self, name: &str, args: &[Value]) -> Value {
        self.inner.query(name, args)
    }
    fn context(&mut self, name: &str) -> Value {
        self.inner.context(name)
    }
    fn array(&self, name: &str) -> Option<Vec<Value>> {
        self.arrays
            .get(name)
            .cloned()
            .or_else(|| self.inner.array(name))
    }
    fn entity_var(&mut self, entity: u64, var: crate::molang::Sym) -> Value {
        self.inner.entity_var(entity, var)
    }
    fn entity_query(&mut self, entity: u64, name: &str, args: &[Value]) -> Value {
        self.inner.entity_query(entity, name, args)
    }
}

/// Answers `query.bone_origin` and `query.get_default_bone_pivot` from the model's bones, and
/// leaves the rest to the host. The origin follows the pose as the animations built so far leave
/// it, so an animation that moves a bone to a place reads where the bone is now.
struct WithBones<'a> {
    inner: &'a mut dyn Host,
    geometry: Option<&'a crate::geometry::Geometry>,
    /// Asked for once: the pose is then kept as it grows.
    wanted: &'a mut bool,
    origins: &'a HashMap<String, [f32; 3]>,
    /// `Instance::moved`.
    moved: &'a HashMap<String, [f32; 3]>,
    pivots: &'a HashMap<String, [f32; 3]>,
    rotations: &'a HashMap<String, [f32; 3]>,
    partial: Option<Pose>,
    fresh: Option<BoneTables>,
    /// `Instance::place`.
    place: Option<[[f32; 4]; 3]>,
}

type BoneTables = (
    HashMap<String, [f32; 3]>,
    HashMap<String, [f32; 3]>,
    HashMap<String, [f32; 3]>,
);

impl Host for WithBones<'_> {
    fn query(&mut self, name: &str, args: &[Value]) -> Value {
        if !matches!(
            name,
            "bone_origin" | "get_default_bone_pivot" | "bone_rotation" | "bone_orientation_trs"
        ) {
            return self.inner.query(name, args);
        }
        *self.wanted = true;
        let bone = match args.first() {
            Some(Value::Str(b)) => b.to_ascii_lowercase(),
            _ if name == "bone_orientation_trs" => {
                return Value::Trs([[0.0; 3], [0.0; 3], [1.0; 3]]);
            }
            _ => return Value::Vec3([0.0; 3]),
        };
        if let (Some(g), Some(pose)) = (self.geometry, &self.partial) {
            if self.fresh.is_none() {
                self.fresh = Some(bone_origins(g, pose));
            }
        }
        let (origins, pivots, rotations) = match &self.fresh {
            Some((o, p, r)) => (o, p, r),
            None => (self.origins, self.pivots, self.rotations),
        };
        let placed = |o: [f32; 3]| match &self.place {
            Some(m) => std::array::from_fn(|r| {
                m[r][0] * o[0] + m[r][1] * o[1] + m[r][2] * o[2] + m[r][3]
            }),
            None => o,
        };
        let value = match name {
            // The game's own frame: x and z as in the model, y counted down from the model's top.
            // The bone's pivot with the animated moves of it and of the bones above it, not
            // their turns (`bone_origin_moved`): a pack that holds an item at `a.z - b.z + 1` of
            // two bones' origins is asking where they were put, not where a turned arm swings
            // them to.
            "bone_origin" => match (self.geometry, &self.partial) {
                (Some(g), Some(pose)) => bone_origin_moved(g, pose, &bone)
                    .map(placed)
                    .map_or([0.0; 3], |o| [o[0], 24.0 - o[1], o[2]]),
                _ => self
                    .moved
                    .get(&bone)
                    .map(|o| placed(*o))
                    .map_or([0.0; 3], |o| [o[0], 24.0 - o[1], o[2]]),
            },
            "get_default_bone_pivot" => pivots.get(&bone).copied().unwrap_or([0.0; 3]),
            // Where the bone is in the entity's space (pixels, y up from the feet), how it is
            // turned, and its scale.
            "bone_orientation_trs" => {
                return Value::Trs([
                    origins.get(&bone).map_or([0.0; 3], |o| placed(*o)),
                    rotations.get(&bone).copied().unwrap_or([0.0; 3]),
                    [1.0; 3],
                ]);
            }
            _ => rotations.get(&bone).copied().unwrap_or([0.0; 3]),
        };
        Value::Vec3(value)
    }
    fn pose_changed(&mut self, pose: &Pose) {
        if *self.wanted && self.geometry.is_some() {
            self.partial = Some(pose.clone());
            self.fresh = None;
        }
    }
    fn variables(&mut self) -> Vec<(&'static str, Value)> {
        self.inner.variables()
    }
    fn context(&mut self, name: &str) -> Value {
        self.inner.context(name)
    }
    fn array(&self, name: &str) -> Option<Vec<Value>> {
        self.inner.array(name)
    }
    fn entity_var(&mut self, entity: u64, var: crate::molang::Sym) -> Value {
        self.inner.entity_var(entity, var)
    }
    fn entity_query(&mut self, entity: u64, name: &str, args: &[Value]) -> Value {
        self.inner.entity_query(entity, name, args)
    }
}

/// `deg` as the game turns a bone: the model's y runs down there, so turns about x and z read the
/// other way here; x first, then y, then z.
fn rotate_point(v: [f32; 3], deg: [f32; 3]) -> [f32; 3] {
    let [rx, ry, rz] = deg.map(f32::to_radians);
    let (rx, rz) = (-rx, -rz);
    let [mut x, mut y, mut z] = v;
    (y, z) = (y * rx.cos() - z * rx.sin(), y * rx.sin() + z * rx.cos());
    (x, z) = (x * ry.cos() + z * ry.sin(), -x * ry.sin() + z * ry.cos());
    (x, y) = (x * rz.cos() - y * rz.sin(), x * rz.sin() + y * rz.cos());
    [x, y, z]
}

pub type Mat3 = [[f32; 3]; 3];

pub(crate) fn mat_mul(a: &Mat3, b: &Mat3) -> Mat3 {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}

fn mat_transpose(a: &Mat3) -> Mat3 {
    std::array::from_fn(|i| std::array::from_fn(|j| a[j][i]))
}

/// The turn [`rotate_point`] makes for `deg`, as a matrix.
pub(crate) fn rotation_matrix(deg: [f32; 3]) -> Mat3 {
    let cols = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]].map(|v| rotate_point(v, deg));
    // `cols[j]` is the image of axis j: the matrix holds them as columns.
    std::array::from_fn(|i| std::array::from_fn(|j| cols[j][i]))
}

/// The `deg` whose [`rotation_matrix`] is `m`.
pub(crate) fn rotation_degrees(m: &Mat3) -> [f32; 3] {
    let beta = (-m[2][0]).clamp(-1.0, 1.0).asin();
    let (alpha, gamma) = if beta.cos().abs() > 1e-5 {
        (m[2][1].atan2(m[2][2]), m[1][0].atan2(m[0][0]))
    } else {
        // Looking straight along y: the other two turns share an axis; keep it all in z.
        (0.0, (-m[0][1]).atan2(m[1][1]))
    };
    [-alpha, beta, -gamma].map(f32::to_degrees)
}

/// `relative_to: { rotation: "entity" }`: a bone turned in the entity's space ignores the turn of
/// the bones above it. Each such bone's turn (its own and the entity-space one, added) becomes the
/// turn that gives it that orientation in the model, and `rotation_entity` is spent.
/// Turns each bone the pose turns relative to the entity to face `camera` (pixels, y up, in the
/// model's space), the way a pack aims a glow at the camera: its pitch and yaw from the bone's
/// pivot, keeping the pack's roll.
fn face_camera(geometry: &crate::geometry::Geometry, pose: &mut Pose, camera: [f32; 3]) {
    if !pose.bones.values().any(|b| b.entity_frame || b.rotation_entity != [0.0; 3]) {
        return;
    }
    let (origins, _, _) = bone_origins(geometry, pose);
    for bone in &geometry.bones {
        let Some(o) = origins.get(&bone.name.to_ascii_lowercase()) else {
            continue;
        };
        let Some(b) = pose.bones.get_mut(&bone.name) else {
            continue;
        };
        if !b.entity_frame && b.rotation_entity == [0.0; 3] {
            continue;
        }
        let d: [f32; 3] = std::array::from_fn(|i| o[i] - camera[i]);
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        if len < 1e-3 {
            continue;
        }
        let pitch = (d[1] / len).clamp(-1.0, 1.0).asin().to_degrees();
        let yaw = d[0].atan2(d[2]).to_degrees();
        b.rotation_entity = [pitch - b.rotation[0], yaw - b.rotation[1], b.rotation_entity[2]];
    }
}

pub fn resolve_entity_rotations(geometry: &crate::geometry::Geometry, pose: &mut Pose) {
    resolve_entity_rotations_under(geometry, pose, None);
}

/// `resolve_entity_rotations` for a model whose root hangs from a turned bone of another
/// (`root`: that bone's turn from the entity, an attachable's on its holder's hand): the bones
/// turned relative to the entity take that turn out, so a lantern hangs upright from a raised
/// arm, as Bedrock draws an attachable in its holder's skeleton.
pub fn resolve_entity_rotations_under(
    geometry: &crate::geometry::Geometry,
    pose: &mut Pose,
    root: Option<Mat3>,
) {
    if !pose.bones.values().any(|b| b.entity_frame || b.rotation_entity != [0.0; 3]) {
        return;
    }
    let index: HashMap<String, usize> = geometry
        .bones
        .iter()
        .enumerate()
        .map(|(i, b)| (b.name.to_ascii_lowercase(), i))
        .collect();
    let mut world: Vec<Option<Mat3>> = vec![None; geometry.bones.len()];
    // Parents come first; the walk climbs to the first one worked out, then comes back down.
    for start in 0..geometry.bones.len() {
        let mut chain = Vec::new();
        let mut cur = Some(start);
        while let Some(i) = cur {
            if world[i].is_some() || chain.len() > 64 {
                break;
            }
            chain.push(i);
            cur = geometry.bones[i]
                .parent
                .as_ref()
                .and_then(|n| index.get(&n.to_ascii_lowercase()))
                .copied();
        }
        let mut above = cur.and_then(|i| world[i]).or(root).unwrap_or([
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
        ]);
        for &i in chain.iter().rev() {
            let bone = &geometry.bones[i];
            let own = pose.bones.get_mut(&bone.name);
            let m = match own {
                Some(o) if o.entity_frame || o.rotation_entity != [0.0; 3] => {
                    // Turned relative to the entity, the bone's whole turn (every animation's,
                    // on its rest) counts in the entity's frame, as Bedrock sums the channels:
                    // A&S's head adds the look (relative) to its own body turn (`-q.body_y_rotation
                    // + v.nthjcy`), and its arms take the walk out with `-this`.
                    let total: [f32; 3] = std::array::from_fn(|k| {
                        bone.rotation[k] + o.rotation[k] + o.rotation_entity[k]
                    });
                    let wanted = rotation_matrix(total);
                    let local = mat_mul(&mat_transpose(&above), &wanted);
                    let deg = rotation_degrees(&local);
                    o.rotation = std::array::from_fn(|k| deg[k] - bone.rotation[k]);
                    o.rotation_entity = [0.0; 3];
                    o.entity_frame = false;
                    wanted
                }
                Some(o) => mat_mul(
                    &above,
                    &rotation_matrix(std::array::from_fn(|k| bone.rotation[k] + o.rotation[k])),
                ),
                None => mat_mul(&above, &rotation_matrix(bone.rotation)),
            };
            world[i] = Some(m);
            above = m;
        }
    }
}

/// Where each bone's pivot is in the model's space (pixels, y up) with `pose` applied, and
/// where it is at rest.
pub fn bone_origins(
    geometry: &crate::geometry::Geometry,
    pose: &Pose,
) -> (
    HashMap<String, [f32; 3]>,
    HashMap<String, [f32; 3]>,
    HashMap<String, [f32; 3]>,
) {
    let index: HashMap<String, usize> = geometry
        .bones
        .iter()
        .enumerate()
        .map(|(i, b)| (b.name.to_ascii_lowercase(), i))
        .collect();
    let mut origins = HashMap::new();
    let mut pivots = HashMap::new();
    let mut rotations = HashMap::new();
    for bone in &geometry.bones {
        let name = bone.name.to_ascii_lowercase();
        pivots.insert(name.clone(), bone.pivot);
        let own = pose.get(&bone.name).cloned().unwrap_or_default();
        rotations.insert(
            name.clone(),
            std::array::from_fn(|i| bone.rotation[i] + own.rotation[i]),
        );
        // The pivot, then up through each bone that holds it.
        let mut p = bone.pivot;
        let mut cur = Some(bone);
        let mut guard = 0;
        while let Some(b) = cur {
            let local = pose.get(&b.name).cloned().unwrap_or_default();
            let scale = if local.scale == [0.0; 3] {
                [1.0; 3]
            } else {
                local.scale
            };
            let rel = [
                (p[0] - b.pivot[0]) * scale[0],
                (p[1] - b.pivot[1]) * scale[1],
                (p[2] - b.pivot[2]) * scale[2],
            ];
            let turn = std::array::from_fn(|i| b.rotation[i] + local.rotation[i]);
            let r = rotate_point(rel, turn);
            p = std::array::from_fn(|i| b.pivot[i] + r[i] + local.position[i]);
            guard += 1;
            cur = b
                .parent
                .as_ref()
                .and_then(|n| index.get(&n.to_ascii_lowercase()))
                .map(|&i| &geometry.bones[i])
                .filter(|_| guard < 64);
        }
        origins.insert(name, p);
    }
    (origins, pivots, rotations)
}

/// `bone`'s pivot (pixels, y up, in the model's space) moved by its own `position` and those of
/// the bones above it, whatever they are turned by.
fn bone_origin_moved(
    geometry: &crate::geometry::Geometry,
    pose: &Pose,
    bone: &str,
) -> Option<[f32; 3]> {
    let start = geometry.bones.iter().find(|b| b.name.eq_ignore_ascii_case(bone))?;
    let mut p = start.pivot;
    let mut cur = Some(start);
    let mut guard = 0;
    while let Some(b) = cur {
        let local = pose.get(&b.name).cloned().unwrap_or_default();
        for i in 0..3 {
            p[i] += local.position[i];
        }
        guard += 1;
        cur = b
            .parent
            .as_ref()
            .and_then(|n| geometry.bones.iter().find(|x| x.name.eq_ignore_ascii_case(n)))
            .filter(|_| guard < 64);
    }
    Some(p)
}

/// Where `point` (pixels, y up, from the bone's pivot as modelled) of the bone `bone` is with
/// `pose` applied: in the model's space, turned and moved with the bone and its parents.
pub fn bone_point(
    geometry: &crate::geometry::Geometry,
    pose: &Pose,
    bone: &str,
    point: [f32; 3],
) -> Option<[f32; 3]> {
    let start = geometry.bones.iter().find(|b| b.name.eq_ignore_ascii_case(bone))?;
    let mut p = std::array::from_fn(|i| start.pivot[i] + point[i]);
    let mut cur = Some(start);
    let mut guard = 0;
    while let Some(b) = cur {
        let local = pose.get(&b.name).cloned().unwrap_or_default();
        let scale = if local.scale == [0.0; 3] { [1.0; 3] } else { local.scale };
        let rel = [
            (p[0] - b.pivot[0]) * scale[0],
            (p[1] - b.pivot[1]) * scale[1],
            (p[2] - b.pivot[2]) * scale[2],
        ];
        let turn = std::array::from_fn(|i| b.rotation[i] + local.rotation[i]);
        let r = rotate_point(rel, turn);
        p = std::array::from_fn(|i| b.pivot[i] + r[i] + local.position[i]);
        guard += 1;
        cur = b
            .parent
            .as_ref()
            .and_then(|n| geometry.bones.iter().find(|x| x.name.eq_ignore_ascii_case(n)))
            .filter(|_| guard < 64);
    }
    Some(p)
}

/// How `bone` is turned from the model's root with `pose` applied: its own turn and its parents',
/// as `resolve_entity_rotations` composes them.
pub fn bone_world_rotation(
    geometry: &crate::geometry::Geometry,
    pose: &Pose,
    bone: &str,
) -> Option<Mat3> {
    let mut chain = Vec::new();
    let mut cur = geometry.bones.iter().find(|b| b.name.eq_ignore_ascii_case(bone));
    while let Some(b) = cur {
        chain.push(b);
        if chain.len() > 64 {
            break;
        }
        cur = b
            .parent
            .as_ref()
            .and_then(|n| geometry.bones.iter().find(|x| x.name.eq_ignore_ascii_case(n)));
    }
    if chain.is_empty() {
        return None;
    }
    let mut m: Mat3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for b in chain.iter().rev() {
        let own = pose.get(&b.name).cloned().unwrap_or_default();
        let turn = std::array::from_fn(|k| b.rotation[k] + own.rotation[k] + own.rotation_entity[k]);
        m = mat_mul(&m, &rotation_matrix(turn));
    }
    Some(m)
}

/// The affine map `bone_point` is for `bone` (rows), from three points and its origin.
pub fn bone_place(
    geometry: &crate::geometry::Geometry,
    pose: &Pose,
    bone: &str,
) -> Option<[[f32; 4]; 3]> {
    let o = bone_point(geometry, pose, bone, [0.0; 3])?;
    let axes: [[f32; 3]; 3] = std::array::from_fn(|a| {
        let mut e = [0.0; 3];
        e[a] = 1.0;
        let p = bone_point(geometry, pose, bone, e).unwrap_or(o);
        std::array::from_fn(|i| p[i] - o[i])
    });
    Some(std::array::from_fn(|r| [axes[0][r], axes[1][r], axes[2][r], o[r]]))
}

/// One entity (or held item) in the world.
pub struct Instance {
    pub vars: Vars,
    rng: Rng,
    nodes: Children,
    initialized: bool,
    /// Each bone's pivot as the last frame's pose left it, and at rest.
    origins: HashMap<String, [f32; 3]>,
    /// The same bones' pivots moved by the animations' positions only (`bone_origin_moved`), as
    /// `origins` keeps them (the mean of the last two frames).
    moved: HashMap<String, [f32; 3]>,
    pivots: HashMap<String, [f32; 3]>,
    rotations: HashMap<String, [f32; 3]>,
    /// The model the last frame drew first.
    main_geometry: Option<String>,
    /// A pack asked where a bone is.
    uses_bones: bool,
    /// The bones' offsets from their parents' pivots, over the entity's models (`Pose::rest`).
    rest: Option<std::sync::Arc<HashMap<String, [f32; 3]>>>,
    /// Where an attachable's own space sits in its holder's (rows of an affine map, pixels,
    /// y up from the holder's feet): what asks where one of its bones is (`query.bone_origin`,
    /// `query.bone_orientation_trs`) is told where the bone is on the holder, as Bedrock, which
    /// draws the attachable as part of the holder's skeleton.
    pub place: Option<[[f32; 4]; 3]>,
    /// Where the camera really is in this model's own space (pixels, y up), when the pack's own
    /// idea of it is wrong: a first-person item, drawn in the hand's frame, which the pack's
    /// glows aim at as if the camera sat on the holder's eyes. Its bones turned relative to the
    /// entity are turned to face it instead.
    pub camera: Option<[f32; 3]>,
    /// For an attachable: how the holder's bone it hangs from is turned from the holder
    /// (`resolve_entity_rotations_under`).
    pub root_rotation: Option<Mat3>,
}

fn run(host: &mut dyn Host, vars: &mut Vars, rng: &mut Rng, p: &Program) -> Value {
    Env { host, vars, rng }.run(p, Scope::default(), Value::Null)
}

fn name_of(v: &Value) -> Option<String> {
    match v {
        Value::Res(_, n) => Some(n.to_string()),
        Value::Str(s) => Some(s.to_ascii_lowercase()),
        _ => None,
    }
}

impl Instance {
    pub fn new(seed: u64) -> Instance {
        Instance {
            vars: Vars::default(),
            rng: Rng::new(seed),
            nodes: Children::new(),
            initialized: false,
            origins: HashMap::new(),
            moved: HashMap::new(),
            pivots: HashMap::new(),
            rotations: HashMap::new(),
            main_geometry: None,
            uses_bones: false,
            rest: None,
            place: None,
            camera: None,
            root_rotation: None,
        }
    }

    /// What is running, as text for a log.
    pub fn describe(&self, def: &ClientEntity, assets: &Assets) -> String {
        let mut out = Vec::new();
        crate::animation::describe(&self.nodes, assets.animations, &def.animations, &mut out);
        out.join(" ")
    }

    /// Runs `initialize` again on the next frame and forgets running animations.
    pub fn reset(&mut self) {
        self.initialized = false;
        self.nodes.clear();
        self.vars.clear();
    }

    pub fn update(
        &mut self,
        def: &ClientEntity,
        assets: &Assets,
        host: &mut dyn Host,
        dt: f32,
    ) -> Frame {
        let mut frame = Frame {
            scale: 1.0,
            scale_axes: [1.0; 3],
            held_item_scale: 1.0,
            hide_armor: def.hide_armor,
            held_item_ignores_lighting: def.held_item_ignores_lighting,
            ..Frame::default()
        };
        if !self.initialized {
            self.initialized = true;
            for p in &def.initialize {
                run(host, &mut self.vars, &mut self.rng, p);
            }
        }
        for (name, value) in host.variables() {
            self.vars.insert(crate::molang::intern(name), value);
        }
        let mut events: Vec<Event> = Vec::new();
        {
            let geometry = self
                .main_geometry
                .as_deref()
                .zip(assets.geometries)
                .and_then(|(name, models)| models.get(&name.to_ascii_lowercase()));
            let mut bones = WithBones {
                inner: &mut *host,
                geometry,
                wanted: &mut self.uses_bones,
                origins: &self.origins,
                moved: &self.moved,
                pivots: &self.pivots,
                rotations: &self.rotations,
                partial: None,
                fresh: None,
                place: self.place,
            };
            let host: &mut dyn Host = &mut bones;
            for p in def.parent_setup.iter().chain(&def.pre_animation) {
                run(host, &mut self.vars, &mut self.rng, p);
            }
            if let Some(p) = &def.scale {
                let s = run(host, &mut self.vars, &mut self.rng, p).num();
                frame.scale = if s.is_finite() && s != 0.0 { s } else { 1.0 };
            }
            for (i, p) in def.scale_axes.iter().enumerate() {
                if let Some(p) = p {
                    let s = run(host, &mut self.vars, &mut self.rng, p).num();
                    frame.scale_axes[i] = if s.is_finite() && s != 0.0 { s } else { 1.0 };
                }
            }
            if let Some(p) = &def.hide_held_items {
                frame.hide_held_items = run(host, &mut self.vars, &mut self.rng, p).truthy();
            }
            if let Some(p) = &def.held_item_scale {
                let s = run(host, &mut self.vars, &mut self.rng, p).num();
                frame.held_item_scale = if s.is_finite() { s } else { 1.0 };
            }
            let mut env = Env {
                host: &mut *host,
                vars: &mut self.vars,
                rng: &mut self.rng,
            };
            let mut pose = Pose::default();
            if let Some(models) = assets.geometries {
                pose.rest = self
                    .rest
                    .get_or_insert_with(|| {
                        let mut rest = HashMap::new();
                        let mut names: Vec<&String> = def.geometry.values().collect();
                        names.sort();
                        for name in names {
                            let id = name.to_ascii_lowercase();
                            if let Some(g) = models.get(&id).or_else(|| {
                                id.split_once(':').and_then(|(own, _)| models.get(own))
                            }) {
                                g.rest_offsets(&mut rest);
                            }
                        }
                        std::sync::Arc::new(rest)
                    })
                    .clone();
            }
            let mut played: Vec<&str> = Vec::with_capacity(def.animate.len());
            for (name, cond) in &def.animate {
                // The expression is the animation's blend weight (`{"move.arms": "0.001"}` all but
                // turns the vanilla arm swing off): a number weighs, anything else true plays whole.
                let weight = match cond {
                    None => 1.0,
                    Some(c) => match env.run(c, Scope::default(), Value::Null) {
                        Value::Num(n) => n,
                        v if v.truthy() => 1.0,
                        _ => 0.0,
                    },
                };
                if weight > 0.0 && weight.is_finite() {
                    let mut r = Run {
                        lib: assets.animations,
                        names: &def.animations,
                        controllers: &def.controllers,
                        env: &mut env,
                        pose: &mut pose,
                        events: &mut events,
                        dt,
                    };
                    r.play(&mut self.nodes, name, weight);
                    played.push(name);
                }
            }
            self.nodes.retain(|k, _| played.contains(&k.as_str()));
            frame.pose = pose;
        }
        for e in events {
            frame.emits.push(match e {
                Event::Sound { effect } => Emit::Sound(
                    def.sounds
                        .get(&effect.to_ascii_lowercase())
                        .cloned()
                        .unwrap_or(effect),
                ),
                Event::Particle {
                    effect,
                    locator,
                    pre,
                } => Emit::Particle {
                    effect: def
                        .particles
                        .get(&effect.to_ascii_lowercase())
                        .cloned()
                        .unwrap_or(effect),
                    locator,
                    pre,
                },
                Event::Hold {
                    effect,
                    locator,
                    pre,
                } => Emit::Hold {
                    effect: def
                        .particles
                        .get(&effect.to_ascii_lowercase())
                        .cloned()
                        .unwrap_or(effect),
                    locator,
                    pre,
                },
            });
        }
        // What to draw.
        for (id, cond) in &def.render_controllers {
            if cond
                .as_ref()
                .is_some_and(|c| !run(host, &mut self.vars, &mut self.rng, c).truthy())
            {
                continue;
            }
            let Some(rc) = assets.render_controllers.get(id) else {
                continue;
            };
            self.layers(def, rc, host, assets.geometries, &mut frame.layers);
        }
        // Each model's bones turned relative to the entity: every layer's, not the first one's
        // alone (a torch's flame is a model of its own beside the stick's).
        if let Some(models) = assets.geometries {
            let mut seen: Vec<&str> = Vec::new();
            for layer in &frame.layers {
                if seen.contains(&layer.geometry.as_str()) {
                    continue;
                }
                seen.push(&layer.geometry);
                let Some(g) = models.get(&layer.geometry.to_ascii_lowercase()) else {
                    continue;
                };
                if let Some(camera) = self.camera {
                    face_camera(g, &mut frame.pose, camera);
                }
                resolve_entity_rotations_under(g, &mut frame.pose, self.root_rotation);
            }
        }
        if let (Some(models), Some(layer)) = (assets.geometries, frame.layers.first()) {
            if let Some(g) = models.get(&layer.geometry.to_ascii_lowercase()) {
                if self.main_geometry.as_deref() != Some(layer.geometry.as_str()) {
                    self.main_geometry = Some(layer.geometry.clone());
                }
                let (origins, pivots, rotations) = bone_origins(g, &frame.pose);
                // What `pre_animation` reads of the last frame is the mean of the last two: a
                // pack that lifts a bone by where another one was (`root` by the legs' height)
                // reads its own lift back with the sign turned, and alternates from frame to
                // frame (the rider flickering on its mount) unless it is averaged out.
                self.origins = origins
                    .into_iter()
                    .map(|(name, now)| {
                        let mean = self.origins.get(&name).map_or(now, |old| {
                            std::array::from_fn(|i| (now[i] + old[i]) * 0.5)
                        });
                        (name, mean)
                    })
                    .collect();
                self.moved = g
                    .bones
                    .iter()
                    .filter_map(|b| {
                        let name = b.name.to_ascii_lowercase();
                        let now = bone_origin_moved(g, &frame.pose, &b.name)?;
                        let mean = self.moved.get(&name).map_or(now, |old| {
                            std::array::from_fn(|i| (now[i] + old[i]) * 0.5)
                        });
                        Some((name, mean))
                    })
                    .collect();
                self.pivots = pivots;
                self.rotations = rotations;
            }
        }
        frame
    }

    fn layers(
        &mut self,
        def: &ClientEntity,
        rc: &RenderController,
        host: &mut dyn Host,
        models: Option<&HashMap<String, crate::geometry::Geometry>>,
        out: &mut Vec<Layer>,
    ) {
        let mut host = WithArrays {
            inner: host,
            arrays: &rc.arrays,
        };
        let host: &mut dyn Host = &mut host;
        let key = rc
            .geometry
            .as_ref()
            .and_then(|g| name_of(&run(host, &mut self.vars, &mut self.rng, g)))
            .unwrap_or_else(|| "default".into());
        let Some(geometry) = def.geometry.get(&key) else {
            return;
        };
        let mut bone_materials = Vec::new();
        let mut material = String::new();
        for (bone, p) in &rc.materials {
            let Some(m) = name_of(&run(host, &mut self.vars, &mut self.rng, p))
                .and_then(|k| def.materials.get(&k).cloned())
            else {
                continue;
            };
            if bone == "*" {
                material = m.clone();
            }
            bone_materials.push((bone.clone(), m));
        }
        let mut visibility = Vec::new();
        for (bone, p) in &rc.part_visibility {
            visibility.push((
                bone.clone(),
                run(host, &mut self.vars, &mut self.rng, p).truthy(),
            ));
        }
        let on_fire = rc.on_fire_color.is_some() && host.query("is_on_fire", &[]).truthy();
        let hurt = rc.hurt_color.is_some() && host.query("hurt_time", &[]).num() > 0.0;
        let mut env = Env {
            host: &mut *host,
            vars: &mut self.vars,
            rng: &mut self.rng,
        };
        let mut colour = |c: &Option<[Scalar; 4]>, this: f32| {
            let scope = Scope {
                this: Some(this),
                ..Scope::default()
            };
            c.as_ref()
                .map(|c| std::array::from_fn(|i| c[i].eval(&mut env, scope)))
        };
        // `this` in `color` is the colour's own default, white; in the others, none.
        let mut color = colour(&rc.color, 1.0).unwrap_or([1.0; 4]);
        if on_fire && let Some(fire) = colour(&rc.on_fire_color, 0.0) {
            // The fire colour's alpha is how much of it shows over the entity, not how see-through
            // the entity gets: a pack that flickers it would make the mob blink in and out.
            let k = fire[3].clamp(0.0, 1.0);
            for i in 0..3 {
                color[i] += (fire[i] - color[i]) * k;
            }
        }
        let overlay = colour(if hurt { &rc.hurt_color } else { &rc.overlay }, 0.0);
        let light_multiplier = rc
            .light_multiplier
            .as_ref()
            .map_or(1.0, |s| s.eval(&mut env, Scope::default()));
        let uv_anim = rc.uv_anim.as_ref().map(|(o, s)| {
            (
                std::array::from_fn(|i| o[i].eval(&mut env, Scope::default())),
                std::array::from_fn(|i| s[i].eval(&mut env, Scope::default())),
            )
        });
        let ignore_lighting = rc
            .ignore_lighting
            .as_ref()
            .is_some_and(|p| env.run(p, Scope::default(), Value::Null).truthy());
        let start = out.len();
        for (index, t) in rc.textures.iter().enumerate() {
            let v = run(host, &mut self.vars, &mut self.rng, t);
                let Some(texture) = name_of(&v).and_then(|k| def.textures.get(&k).cloned()) else {
                continue;
            };
            out.push(Layer {
                geometry: geometry.clone(),
                texture,
                material: material.clone(),
                bone_materials: bone_materials.clone(),
                visibility: visibility.clone(),
                color,
                overlay,
                hurt_overlay: hurt,
                ignore_lighting,
                light_multiplier,
                uv_anim,
                flat_lighting: rc.filter_lighting,
                secondary: index > 0,
            });
        }
        // The flat sprites the model's bones show, each in the texture it names.
        if let (Some(models), Some(first)) = (models, out.get(start).cloned()) {
            for (key, path) in &def.textures {
                let fitted = crate::geometry::fitted_mesh_id(geometry, key, path);
                let id = if models.contains_key(&fitted) {
                    fitted
                } else {
                    crate::geometry::mesh_id(geometry, key)
                };
                if models.contains_key(&id) {
                    out.push(Layer {
                        geometry: id,
                        texture: path.clone(),
                        secondary: false,
                        ..first.clone()
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jsonc;

    struct Burning(bool);
    impl Host for Burning {
        fn query(&mut self, name: &str, _args: &[Value]) -> Value {
            match name {
                "is_on_fire" => Value::bool(self.0),
                _ => Value::Null,
            }
        }
    }

    #[test]
    fn a_burning_entity_takes_the_on_fire_color() {
        let (def, lib, _) = setup();
        let mut rcs = HashMap::new();
        RenderController::add_file(
            &jsonc::parse(
                r#"{"render_controllers":{"controller.render.main":{"geometry":"Geometry.default",
                  "textures":["Texture.default"],"materials":[{"*":"Material.default"}],
                  "on_fire_color":{"r":1,"g":0.5,"b":0,"a":1},"filter_lighting":true}}}"#,
            )
            .unwrap(),
            &mut rcs,
        );
        let assets = Assets {
            animations: &lib,
            render_controllers: &rcs,
            geometries: None,
        };
        let mut inst = Instance::new(1);
        let f = inst.update(&def, &assets, &mut Burning(true), 0.1);
        assert_eq!(f.layers[0].color, [1.0, 0.5, 0.0, 1.0]);
        let f = inst.update(&def, &assets, &mut Burning(false), 0.1);
        assert_eq!(f.layers[0].color, [1.0; 4]);
        assert!(rcs["controller.render.main"].filter_lighting);
    }

    #[test]
    fn a_fire_color_with_no_alpha_leaves_the_entity_opaque() {
        let (def, lib, _) = setup();
        let mut rcs = HashMap::new();
        RenderController::add_file(
            &jsonc::parse(
                r#"{"render_controllers":{"controller.render.main":{"geometry":"Geometry.default",
                  "textures":["Texture.default"],"materials":[{"*":"Material.default"}],
                  "on_fire_color":{"r":1,"g":0.4,"b":0,"a":0}}}}"#,
            )
            .unwrap(),
            &mut rcs,
        );
        let assets = Assets {
            animations: &lib,
            render_controllers: &rcs,
            geometries: None,
        };
        let mut inst = Instance::new(1);
        let f = inst.update(&def, &assets, &mut Burning(true), 0.1);
        assert_eq!(f.layers[0].color, [1.0; 4]);
    }

    struct H {
        name: String,
        speed: f32,
    }
    impl Host for H {
        fn query(&mut self, name: &str, args: &[Value]) -> Value {
            match name {
                "is_name_any" => Value::bool(
                    args.iter()
                        .any(|a| matches!(a, Value::Str(s) if s.eq_ignore_ascii_case(&self.name))),
                ),
                "speed" => Value::Num(self.speed),
                _ => Value::Null,
            }
        }
    }

    const ENTITY: &str = r#"{"minecraft:client_entity":{"description":{
      "identifier":"minecraft:zombie",
      "materials":{"default":"zombie","glow":"entity_emissive"},
      "textures":{"default":"textures/z","dark":"textures/z_dark","a":"textures/skin_a","b":"textures/skin_b"},
      "geometry":{"default":"geometry.Z","big":"geometry.Z_Big"},
      "animations":{"walk":"animation.t.walk"},
      "sound_effects":{"step":"mob.zombie.step"},
      "particle_effects":{"puff":"mypack:puff"},
      "scripts":{"initialize":["v.n = 5;"],"pre_animation":["v.big = q.is_name_any('Giant');"],
        "scale":"v.big ? 2 : 1","animate":["walk"]},
      "render_controllers":["controller.render.main",{"controller.render.dark":"q.is_name_any('dark')"}]}}}"#;

    const RCS: &str = r#"{"render_controllers":{
      "controller.render.main":{"geometry":"v.big ? Geometry.big : Geometry.default",
        "arrays":{"textures":{"Array.skins":["Texture.a","Texture.b"]}},
        "textures":["Array.skins[v.n - 4]"],"materials":[{"*":"Material.default"},{"head":"Material.glow"}],
        "part_visibility":[{"*":true},{"hat":"!v.big"}],"color":{"r":1,"g":0.5,"b":"v.n / 10","a":1}},
      "controller.render.dark":{"geometry":"Geometry.default","textures":["Texture.dark"],"materials":[{"*":"Material.default"}]}}}"#;

    const ANIMS: &str = r#"{"animations":{"animation.t.walk":{"loop":true,"animation_length":1,
      "bones":{"leg":{"rotation":["q.speed * 10",0,0]}},"sound_effects":{"0.5":{"effect":"step"}},
      "particle_effects":{"0.0":{"effect":"puff","locator":"foot"}}}}}"#;

    fn setup() -> (ClientEntity, Library, HashMap<String, RenderController>) {
        let def = ClientEntity::parse(&jsonc::parse(ENTITY).unwrap()).unwrap();
        let mut lib = Library::default();
        lib.add_file(&jsonc::parse(ANIMS).unwrap());
        let mut rcs = HashMap::new();
        RenderController::add_file(&jsonc::parse(RCS).unwrap(), &mut rcs);
        (def, lib, rcs)
    }

    #[test]
    fn parses_the_declaration() {
        let (def, _, rcs) = setup();
        assert_eq!(def.id, "minecraft:zombie");
        assert_eq!(def.animate.len(), 1);
        assert_eq!(def.render_controllers.len(), 2);
        assert!(rcs.contains_key("controller.render.main"));
    }

    #[test]
    fn name_tag_picks_geometry_scale_and_extra_layer() {
        let (def, lib, rcs) = setup();
        let assets = Assets {
            animations: &lib,
            render_controllers: &rcs,
            geometries: None,
        };
        let mut inst = Instance::new(1);
        let mut h = H {
            name: "Giant".into(),
            speed: 1.0,
        };
        let f = inst.update(&def, &assets, &mut h, 0.1);
        assert_eq!(f.scale, 2.0);
        assert_eq!(f.layers.len(), 1);
        let l = &f.layers[0];
        assert_eq!(
            (l.geometry.as_str(), l.texture.as_str(), l.material.as_str()),
            ("geometry.z_big", "textures/skin_b", "zombie")
        );
        assert_eq!(l.color, [1.0, 0.5, 0.5, 1.0].map(|x: f32| x));
        assert!(!l.bone_visible("hat") && l.bone_visible("head"));
        assert!(!l.bone_visible("hat#cube1") && l.bone_visible("head#cube0"));
        assert_eq!(l.material_for("head"), "entity_emissive");
        assert_eq!(l.material_for("body"), "zombie");

        let mut h = H {
            name: "dark".into(),
            speed: 1.0,
        };
        let f = inst.update(&def, &assets, &mut h, 0.1);
        assert_eq!(f.scale, 1.0);
        assert_eq!(f.layers.len(), 2);
        assert_eq!(f.layers[0].geometry, "geometry.z");
        assert_eq!(f.layers[0].texture, "textures/skin_b");
        assert_eq!(f.layers[1].texture, "textures/z_dark");
        assert!(f.layers[0].bone_visible("hat"));
    }

    #[test]
    fn animates_and_resolves_event_names() {
        let (def, lib, rcs) = setup();
        let assets = Assets {
            animations: &lib,
            render_controllers: &rcs,
            geometries: None,
        };
        let mut inst = Instance::new(1);
        let mut h = H {
            name: String::new(),
            speed: 2.0,
        };
        let f = inst.update(&def, &assets, &mut h, 0.0);
        assert_eq!(f.pose.get("leg").unwrap().rotation, [20.0, 0.0, 0.0]);
        assert_eq!(
            f.emits,
            [Emit::Particle {
                effect: "mypack:puff".into(),
                locator: Some("foot".into()),
                pre: None
            }]
        );
        let f = inst.update(&def, &assets, &mut h, 0.6);
        assert_eq!(f.emits, [Emit::Sound("mob.zombie.step".into())]);
    }

    #[test]
    fn missing_pieces_draw_nothing() {
        let (def, lib, _) = setup();
        let empty = HashMap::new();
        let assets = Assets {
            animations: &lib,
            render_controllers: &empty,
            geometries: None,
        };
        let mut inst = Instance::new(1);
        let mut h = H {
            name: String::new(),
            speed: 0.0,
        };
        assert!(inst.update(&def, &assets, &mut h, 0.1).layers.is_empty());
    }

    #[test]
    fn globs() {
        assert!(glob("*", "x"));
        assert!(glob("Arm*", "arm_left"));
        assert!(!glob("arm", "arm_left"));
        assert!(glob("*_left", "ARM_LEFT"));
        assert!(glob("a*b*c", "aXXbYYc"));
    }

    #[test]
    fn attachable_declares_items_and_parent_setup() {
        let v = jsonc::parse(
            r#"{"minecraft:attachable":{"description":{"identifier":"minecraft:stick","item":{"minecraft:stick":"q.is_attached","x:y":true},
               "materials":{"default":"entity_alphatest"},"textures":{"default":"textures/s"},"geometry":{"default":"geometry.s"},
               "scripts":{"parent_setup":["v.a = 1;"],"animate":[],"initialize":[]},"render_controllers":["controller.render.s"]}}}"#,
        )
        .unwrap();
        let d = ClientEntity::parse(&v).unwrap();
        assert!(d.attachable);
        assert_eq!(d.items.len(), 2);
        assert_eq!(d.parent_setup.len(), 1);
    }

    #[test]
    fn rotations_make_the_round_trip_through_a_matrix() {
        for deg in [[10.0, 20.0, 30.0], [-90.0, 17.0, 0.0], [45.0, -60.0, 120.0]] {
            let back = rotation_degrees(&rotation_matrix(deg));
            let again = rotation_matrix(back);
            let first = rotation_matrix(deg);
            for i in 0..3 {
                for j in 0..3 {
                    assert!((again[i][j] - first[i][j]).abs() < 1e-4, "{deg:?} {back:?}");
                }
            }
        }
    }

    #[test]
    fn an_entity_space_turn_ignores_the_bones_above() {
        let g = crate::geometry::parse_file(
            &jsonc::parse(
                r#"{"format_version":"1.12.0","minecraft:geometry":[{"description":{"identifier":"geometry.t",
                "texture_width":16,"texture_height":16},"bones":[
                {"name":"body","pivot":[0,12,0]},
                {"name":"arm","parent":"body","pivot":[5,22,0]}]}]}"#,
            )
            .unwrap(),
        )
        .unwrap()
        .remove(0);
        let mut pose = Pose::default();
        pose.bones.entry("body".into()).or_default().rotation = [0.0, -45.0, 0.0];
        pose.bones.entry("arm".into()).or_default().rotation_entity = [-90.0, 0.0, 0.0];
        resolve_entity_rotations(&g, &mut pose);
        let arm = &pose.bones["arm"];
        assert_eq!(arm.rotation_entity, [0.0; 3]);
        // The body's turn then the arm's own give back what was asked in the entity's space.
        let world = mat_mul(
            &rotation_matrix([0.0, -45.0, 0.0]),
            &rotation_matrix(arm.rotation),
        );
        let wanted = rotation_matrix([-90.0, 0.0, 0.0]);
        for i in 0..3 {
            for j in 0..3 {
                assert!((world[i][j] - wanted[i][j]).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn scale_per_axis_and_what_is_held_are_read() {
        let def = ClientEntity::parse(
            &jsonc::parse(
                r#"{"minecraft:client_entity":{"description":{"identifier":"a:b","hide_armor":true,
                  "held_item_ignores_lighting":true,"held_item_scale":"0.5",
                  "scripts":{"scalex":"2","scaley":"3","hide_held_items":"1"}}}}"#,
            )
            .unwrap(),
        )
        .unwrap();
        let lib = Library::default();
        let rcs = HashMap::new();
        let assets = Assets {
            animations: &lib,
            render_controllers: &rcs,
            geometries: None,
        };
        let f = Instance::new(1).update(
            &def,
            &assets,
            &mut H {
                name: String::new(),
                speed: 0.0,
            },
            0.1,
        );
        assert_eq!(f.scale_axes, [2.0, 3.0, 1.0]);
        assert!(f.hide_armor && f.hide_held_items && f.held_item_ignores_lighting);
        assert_eq!(f.held_item_scale, 0.5);
    }

    #[test]
    fn an_overlay_moves_the_tint_toward_it_but_the_hurt_flash_is_left_to_the_game() {
        let mut layer = Layer {
            geometry: String::new(),
            texture: String::new(),
            material: String::new(),
            bone_materials: Vec::new(),
            visibility: Vec::new(),
            color: [1.0, 1.0, 1.0, 0.5],
            overlay: Some([0.0, 0.0, 0.0, 0.25]),
            hurt_overlay: false,
            ignore_lighting: false,
            light_multiplier: 1.0,
            uv_anim: None,
            flat_lighting: false,
            secondary: false,
        };
        assert_eq!(layer.tint(), [0.75, 0.75, 0.75, 0.5]);
        layer.hurt_overlay = true;
        assert_eq!(layer.tint(), [1.0, 1.0, 1.0, 0.5]);
    }
}
