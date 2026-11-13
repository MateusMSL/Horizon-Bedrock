//! A software render of what a pack draws for one entity, to see it without the game: every
//! layer's model, posed by the pack's animations, textured, alpha-tested, from three sides.
//! Stencil layers (outlines) are skipped; lighting is a fixed shade.
//!
//! `cargo run --release -p horizon-bedrock --example render_entity -- <pack dir> <entity id> <out.png>`
//!
//! Environment: `BABY=1`, `VARIANT=n`, `TICKS=n` (default 40), `SIZE=n` px per view (default 256),
//! `VARS=a=1,b=0` variables set before the first tick, `ONLY=geometry-substring`,
//! `Q=name=value,...` answers for queries.
//!
//! `HOLD=<attachable id>` draws that attachable on the holder (`SLOT=main_hand|off_hand|head|torso`,
//! default `main_hand`; worn slots follow the holder's bone from rest): the holder's `rightitem` bone, with its pose, carries the attachable's
//! bound bones, the way the game does. `ANCHOR=auto` (default) puts the model's frame origin on the hand for a bound bone with no
//! parent and the bone's pivot for one under a parent; `ANCHOR=pivot` / `frame` force one. Prints where the item sits relative to the
//! hand, in blocks.

use horizon_bedrock::animation::Library;
use horizon_bedrock::entity::{Assets, ClientEntity, Instance, RenderController};
use horizon_bedrock::geometry::Part;
use horizon_bedrock::material::Materials;
use horizon_bedrock::molang::{self, Host, Value};
use horizon_bedrock::pack::Pack;
use std::collections::HashMap;
use std::path::Path;

struct World {
    tick: u32,
    baby: bool,
    variant: f32,
}

impl Host for World {
    /// `FP=1`: the first-person view model (`variable.is_first_person`, the player's item
    /// held in the main hand).
    fn variables(&mut self) -> Vec<(&'static str, Value)> {
        let fp = std::env::var_os("FP").is_some();
        vec![
            ("is_first_person", Value::bool(fp)),
            ("is_paperdoll", Value::Num(0.0)),
            ("is_holding_right", Value::bool(std::env::var_os("HOLD").is_some())),
            ("attack_time", Value::Num(0.0)),
        ]
    }
    fn query(&mut self, name: &str, args: &[Value]) -> Value {
        // `Q=target_x_rotation=30,is_sneaking=1` answers those queries.
        if let Ok(q) = std::env::var("Q") {
            for kv in q.split(',') {
                if let Some((k, v)) = kv.split_once('=')
                    && k == name
                {
                    return Value::Num(v.parse().unwrap_or(0.0));
                }
            }
        }
        // `NAME=Halloween`: the name tag `query.is_name_any` compares.
        if name == "is_name_any"
            && let Ok(n) = std::env::var("NAME")
        {
            return Value::bool(args.iter().any(|a| matches!(a, Value::Str(s) if **s == *n)));
        }
        if std::env::var_os("WALK").is_some() {
            match name {
                "ground_speed" => return Value::Num(4.3),
                "modified_move_speed" => return Value::Num(0.8),
                "modified_distance_moved" | "distance_moved" => {
                    return Value::Num(self.tick as f32 * 0.8 * 0.5);
                }
                _ => {}
            }
        }
        match name {
            "is_alive" | "is_on_ground" => Value::bool(true),
            "is_baby" => Value::bool(self.baby),
            "death_ticks" => Value::Num(0.0),
            "life_time" => Value::Num(self.tick as f32 / 20.0),
            "delta_time" => Value::Num(0.05),
            "frame_alpha" => Value::Num(0.0),
            "variant" => Value::Num(self.variant),
            "distance_from_camera" => Value::Num(3.0),
            "model_scale" => Value::Num(1.0),
            "health" | "max_health" => Value::Num(20.0),
            "time_stamp" => Value::Num(self.tick as f32),
            "graphics_mode_is_any" => Value::Num(1.0),
            "rotation_to_camera" => Value::Num(
                std::env::var(if args.first().map_or(0.0, |a| a.num()) < 0.5 {
                    "CAMPITCH"
                } else {
                    "CAMYAW"
                })
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(if args.first().map_or(0.0, |a| a.num()) < 0.5 {
                    10.0
                } else {
                    0.0
                }),
            ),
            "camera_rotation"
            | "body_y_rotation"
            | "target_y_rotation"
            | "target_x_rotation"
            | "ground_speed"
            | "modified_move_speed"
            | "vertical_speed"
            | "is_local_player"
            | "is_name_any"
            | "is_in_ui"
            | "is_attached"
            | "is_on_fire"
            | "is_in_water"
            | "is_in_lava"
            | "head_is_in_water" => Value::Num(0.0),
            _ => Value::Null,
        }
    }
}

struct Tex {
    w: usize,
    h: usize,
    rgba: Vec<u8>,
}

fn load_tex(pack: &Pack, name: &str) -> Option<Tex> {
    let bytes = pack.read(&format!("{name}.png")).ok()?;
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut r = dec.read_info().ok()?;
    let mut buf = vec![0; r.output_buffer_size()?];
    let i = r.next_frame(&mut buf).ok()?;
    let src = &buf[..i.buffer_size()];
    let rgba = match i.color_type {
        png::ColorType::Rgba => src.to_vec(),
        png::ColorType::Rgb => src
            .chunks_exact(3)
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => src
            .chunks_exact(2)
            .flat_map(|c| [c[0], c[0], c[0], c[1]])
            .collect(),
        png::ColorType::Grayscale => src.iter().flat_map(|&c| [c, c, c, 255]).collect(),
        _ => return None,
    };
    Some(Tex {
        w: i.width as usize,
        h: i.height as usize,
        rgba,
    })
}

type M = [[f32; 4]; 4];

fn ident() -> M {
    let mut m = [[0.0; 4]; 4];
    for i in 0..4 {
        m[i][i] = 1.0;
    }
    m
}

fn mul(a: &M, b: &M) -> M {
    let mut o = [[0.0; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            for k in 0..4 {
                o[i][j] += a[i][k] * b[k][j];
            }
        }
    }
    o
}

fn translate(x: f32, y: f32, z: f32) -> M {
    let mut m = ident();
    m[0][3] = x;
    m[1][3] = y;
    m[2][3] = z;
    m
}

fn scale(x: f32, y: f32, z: f32) -> M {
    let mut m = ident();
    m[0][0] = x;
    m[1][1] = y;
    m[2][2] = z;
    m
}

fn rot_x(a: f32) -> M {
    let (s, c) = a.sin_cos();
    let mut m = ident();
    m[1][1] = c;
    m[1][2] = -s;
    m[2][1] = s;
    m[2][2] = c;
    m
}

fn rot_y(a: f32) -> M {
    let (s, c) = a.sin_cos();
    let mut m = ident();
    m[0][0] = c;
    m[0][2] = s;
    m[2][0] = -s;
    m[2][2] = c;
    m
}

fn rot_z(a: f32) -> M {
    let (s, c) = a.sin_cos();
    let mut m = ident();
    m[0][0] = c;
    m[0][1] = -s;
    m[1][0] = s;
    m[1][1] = c;
    m
}

fn apply(m: &M, p: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * p[0] + m[0][1] * p[1] + m[0][2] * p[2] + m[0][3],
        m[1][0] * p[0] + m[1][1] * p[1] + m[1][2] * p[2] + m[1][3],
        m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2] + m[2][3],
    ]
}

struct Tri {
    p: [[f32; 3]; 4],
    uv: [[f32; 2]; 4],
    shade: f32,
}

/// A part's pose with the animation's bone on it, in the game's frame: position, rotation, scale.
fn eff_pose(part: &Part, pose: &horizon_bedrock::animation::Pose) -> [f32; 9] {
    let mut p = part.pose;
    if let Some(b) = pose.get(&part.name) {
        let r = horizon_bedrock::geometry::rotation_to_java([
            b.rotation[0] + b.rotation_entity[0],
            b.rotation[1] + b.rotation_entity[1],
            b.rotation[2] + b.rotation_entity[2],
        ]);
        p[0] += b.position[0];
        p[1] -= b.position[1];
        p[2] += b.position[2];
        p[3] += r[0];
        p[4] += r[1];
        p[5] += r[2];
        p[6] *= b.scale[0];
        p[7] *= b.scale[1];
        p[8] *= b.scale[2];
    }
    p
}

fn local(p: &[f32; 9]) -> M {
    let m = translate(p[0] / 16.0, p[1] / 16.0, p[2] / 16.0);
    // `Quaternionf.rotationZYX(z, y, x)` is Rz * Ry * Rx.
    let m = mul(&m, &mul(&rot_z(p[5]), &mul(&rot_y(p[4]), &rot_x(p[3]))));
    mul(&m, &scale(p[6], p[7], p[8]))
}

fn emit(part: &Part, m: &M, visible: &dyn Fn(&str) -> bool, out: &mut Vec<(String, Tri)>) {
    if !visible(&part.name) || part.never_render {
        return;
    }
    for q in &part.quads {
        let v: Vec<[f32; 3]> = q
            .vertices
            .iter()
            .map(|v| apply(m, [v[0] / 16.0, v[1] / 16.0, v[2] / 16.0]))
            .collect();
        let n = [q.normal[0], q.normal[1], q.normal[2]];
        let shade = 0.62 + 0.38 * (-n[1] * 0.6 + n[2].abs() * 0.0 + (-n[2]) * 0.4).max(0.0);
        out.push((
            part.name.clone(),
            Tri {
                p: [v[0], v[1], v[2], v[3]],
                uv: std::array::from_fn(|i| [q.vertices[i][3], q.vertices[i][4]]),
                shade,
            },
        ));
    }
}

fn collect(
    part: &Part,
    parent: &M,
    pose: &horizon_bedrock::animation::Pose,
    visible: &dyn Fn(&str) -> bool,
    out: &mut Vec<(String, Tri)>,
) {
    let m = mul(parent, &local(&eff_pose(part, pose)));
    emit(part, &m, visible, out);
    for c in &part.children {
        collect(c, &m, pose, visible, out);
    }
}

/// Every bone's matrix, posed and at rest, under `now` / `rest`.
fn bone_mats(
    part: &Part,
    now: &M,
    rest: &M,
    pose: &horizon_bedrock::animation::Pose,
    out: &mut HashMap<String, (M, M)>,
) {
    let n = mul(now, &local(&eff_pose(part, pose)));
    let r = mul(rest, &local(&part.pose));
    out.insert(part.name.to_ascii_lowercase(), (n, r));
    for c in &part.children {
        bone_mats(c, &n, &r, pose, out);
    }
}

/// The rest position of the bone called `name`, its own and its ancestors' positions added
/// (blocks), as the game's `rest_pivot` finds it.
fn rest_pivot(roots: &[Part], name: &str) -> Option<[f32; 3]> {
    fn walk(part: &Part, from: [f32; 3], name: &str) -> Option<[f32; 3]> {
        let here = [
            from[0] + part.pose[0] / 16.0,
            from[1] + part.pose[1] / 16.0,
            from[2] + part.pose[2] / 16.0,
        ];
        if part.name.eq_ignore_ascii_case(name) {
            return Some(here);
        }
        part.children.iter().find_map(|c| walk(c, here, name))
    }
    roots.iter().find_map(|r| walk(r, [0.0; 3], name))
}

/// What an attachable in a hand needs: which holder bone each of its bones follows, the
/// holder's bones, how big what is held is drawn, and where the item sits on the hand.
struct Held {
    binds: HashMap<String, String>,
    mats: HashMap<String, (M, M)>,
    scale: f32,
    /// `Some(true)`: the frame origin on the hand; `Some(false)`: the bone's pivot; `None`: the
    /// frame origin for a bone with no parent, the pivot for one under another.
    frame_mode: Option<bool>,
    /// A hand holds the item; a worn piece follows only the bone's change from rest.
    in_hand: bool,
}

/// `collect` for an attachable: a bone with a binding follows the holder's bone of that name,
/// the others follow their parent; an attachable that binds bones draws only those and theirs.
#[allow(clippy::too_many_arguments)]
fn collect_held(
    part: &Part,
    chain: &M,
    inherited: Option<M>,
    h: &Held,
    roots: &[Part],
    pose: &horizon_bedrock::animation::Pose,
    visible: &dyn Fn(&str) -> bool,
    out: &mut Vec<(String, Tri)>,
) {
    let mine = mul(chain, &local(&eff_pose(part, pose)));
    let bound = h
        .binds
        .get(&part.name.to_ascii_lowercase())
        .and_then(|to| h.mats.get(to))
        .map(|(now, rest)| {
            if !h.in_hand {
                return mul(now, &mul(&scale(h.scale, h.scale, h.scale), &inverse(rest)));
            }
            let has_parent = !roots.iter().any(|r| r.name.eq_ignore_ascii_case(&part.name));
            let pv = if h.frame_mode.unwrap_or(!has_parent) {
                [0.0; 3]
            } else {
                rest_pivot(roots, &part.name).unwrap_or([0.0, 24.0 / 16.0, 0.0])
            };
            mul(
                now,
                &mul(&scale(h.scale, h.scale, h.scale), &translate(-pv[0], -pv[1], -pv[2])),
            )
        });
    let under = bound.or(inherited);
    if let Some(u) = &under {
        emit(part, &mul(u, &mine), visible, out);
    }
    for c in &part.children {
        collect_held(c, &mine, under, h, roots, pose, visible, out);
    }
}

fn slot_bone(slot: &str) -> &'static str {
    match slot {
        "off_hand" => "leftitem",
        "head" => "head",
        "torso" | "body" => "body",
        "legs" => "leftleg",
        _ => "rightitem",
    }
}

/// The inverse of a rotate, scale and translate matrix.
fn inverse(m: &M) -> M {
    // Gauss-Jordan on the 4x4.
    let mut a = *m;
    let mut inv = ident();
    for c in 0..4 {
        let p = (c..4)
            .max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs()))
            .unwrap_or(c);
        a.swap(c, p);
        inv.swap(c, p);
        let d = a[c][c];
        if d.abs() < 1e-9 {
            return ident();
        }
        for k in 0..4 {
            a[c][k] /= d;
            inv[c][k] /= d;
        }
        for r in 0..4 {
            if r != c {
                let f = a[r][c];
                for k in 0..4 {
                    a[r][k] -= f * a[c][k];
                    inv[r][k] -= f * inv[c][k];
                }
            }
        }
    }
    inv
}

/// The holder's bone a model bone's `binding` expression names, for this slot.
fn bound_to(binding: &str, slot: &str) -> Option<String> {
    if binding.contains("item_slot_to_bone_name") {
        return Some(slot_bone(slot).to_owned());
    }
    binding.split('\'').nth(1).map(str::to_ascii_lowercase)
}

/// What an attachable's queries answer when it is held.
struct AttHost {
    slot: String,
}
impl Host for AttHost {
    fn query(&mut self, name: &str, args: &[Value]) -> Value {
        match name {
            "is_attached" | "is_alive" | "is_on_ground" | "graphics_mode_is_any" => Value::Num(1.0),
            "delta_time" => Value::Num(0.05),
            "item_slot_to_bone_name" => Value::Str(slot_bone(&self.slot).into()),
            "is_owner_identifier_any" => Value::Num(1.0),
            _ => {
                let _ = args;
                Value::Null
            }
        }
    }
    fn context(&mut self, name: &str) -> Value {
        match name {
            "item_slot" => Value::Str(self.slot.as_str().into()),
            "is_first_person" => Value::bool(std::env::var_os("FP").is_some()),
            "owning_entity" => Value::Entity(1),
            _ => Value::Null,
        }
    }
    fn entity_var(&mut self, _e: u64, _v: molang::Sym) -> Value {
        Value::Num(1.0)
    }
}

/// One layer's quads, ready to draw.
struct Item {
    tris: Vec<(Tri, (bool, bool, bool))>,
    texture: String,
    color: [f32; 4],
}

#[allow(clippy::too_many_arguments)]
fn raster(
    color: &mut [[f32; 4]],
    depth: &mut [f32],
    size: usize,
    tri: &Tri,
    view: &M,
    tex: Option<&Tex>,
    tint: [f32; 4],
    mode: (bool, bool, bool),
    s: f32,
) {
    let (alpha_test, blend, additive) = mode;
    let pts: Vec<[f32; 3]> = tri.p.iter().map(|p| apply(view, *p)).collect();
    // Orthographic: x, y on the image (a block is `size / 3` pixels), z is the depth.
    let to_px = |p: &[f32; 3]| {
        [
            size as f32 / 2.0 + p[0] * s,
            size as f32 * 0.5 + p[1] * s,
            p[2],
        ]
    };
    for idx in [[0usize, 1, 2], [0, 2, 3]] {
        let a = to_px(&pts[idx[0]]);
        let b = to_px(&pts[idx[1]]);
        let c = to_px(&pts[idx[2]]);
        let (minx, maxx) = (
            a[0].min(b[0]).min(c[0]).floor().max(0.0) as usize,
            (a[0].max(b[0]).max(c[0]).ceil().min(size as f32 - 1.0)).max(0.0) as usize,
        );
        let (miny, maxy) = (
            a[1].min(b[1]).min(c[1]).floor().max(0.0) as usize,
            (a[1].max(b[1]).max(c[1]).ceil().min(size as f32 - 1.0)).max(0.0) as usize,
        );
        let den = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
        if den.abs() < 1e-6 {
            continue;
        }
        for y in miny..=maxy {
            for x in minx..=maxx {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let w0 = ((b[1] - c[1]) * (px - c[0]) + (c[0] - b[0]) * (py - c[1])) / den;
                let w1 = ((c[1] - a[1]) * (px - c[0]) + (a[0] - c[0]) * (py - c[1])) / den;
                let w2 = 1.0 - w0 - w1;
                if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                    continue;
                }
                let z = a[2] * w0 + b[2] * w1 + c[2] * w2;
                let i = y * size + x;
                if z > depth[i] {
                    continue;
                }
                let u = tri.uv[idx[0]][0] * w0 + tri.uv[idx[1]][0] * w1 + tri.uv[idx[2]][0] * w2;
                let v = tri.uv[idx[0]][1] * w0 + tri.uv[idx[1]][1] * w1 + tri.uv[idx[2]][1] * w2;
                let mut texel = [128.0, 128.0, 128.0, 255.0];
                if let Some(t) = tex {
                    let tx = ((u * t.w as f32) as usize).min(t.w - 1);
                    let ty = ((v * t.h as f32) as usize).min(t.h - 1);
                    let o = (ty * t.w + tx) * 4;
                    texel = [
                        f32::from(t.rgba[o]),
                        f32::from(t.rgba[o + 1]),
                        f32::from(t.rgba[o + 2]),
                        f32::from(t.rgba[o + 3]),
                    ];
                }
                let alpha = texel[3] / 255.0 * tint[3];
                if alpha_test && alpha < 0.1 {
                    continue;
                }
                if !alpha_test && !blend && !additive && texel[3] == 0.0 {
                    continue;
                }
                if (blend || additive) && alpha < 0.01 {
                    continue;
                }
                let rgb = [
                    texel[0] / 255.0 * tint[0] * tri.shade,
                    texel[1] / 255.0 * tint[1] * tri.shade,
                    texel[2] / 255.0 * tint[2] * tri.shade,
                ];
                if additive {
                    for k in 0..3 {
                        color[i][k] = (color[i][k] + rgb[k] * alpha).min(1.0);
                    }
                } else if blend && alpha < 0.99 {
                    for k in 0..3 {
                        color[i][k] = color[i][k] * (1.0 - alpha) + rgb[k] * alpha;
                    }
                    color[i][3] = 1.0;
                } else {
                    color[i] = [rgb[0], rgb[1], rgb[2], 1.0];
                    depth[i] = z;
                }
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [pack_path, id, out_path, ..] = args.as_slice() else {
        eprintln!("usage: render_entity <pack dir> <entity id> <out.png>");
        return;
    };
    let pack = Pack::open(Path::new(pack_path)).expect("pack");
    let mut builtin = horizon_bedrock::bundle::Bundle::default();
    builtin.add_builtins();
    let mut lib: Library = builtin.library;
    let mut rcs = builtin.render_controllers;
    let mut geos = builtin.geometries;
    let mut defs = Vec::new();
    let mut att_defs = Vec::new();
    let mut materials = Materials::default();
    for p in pack.paths().map(str::to_owned).collect::<Vec<_>>() {
        if p.ends_with(".material") {
            if let Ok(v) = pack.read_json(&p) {
                materials.add_file(&v);
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
            for g in horizon_bedrock::geometry::parse_file(&v).unwrap_or_default() {
                for var in g.mesh_variants() {
                    geos.insert(var.id.clone(), var);
                }
                geos.insert(g.id.to_ascii_lowercase(), g);
            }
        }
        if p.starts_with("attachables/") {
            if let Some(d) = ClientEntity::parse(&v) {
                att_defs.push(d);
            }
        }
        if p.starts_with("entity/") {
            if let Some(d) = ClientEntity::parse(&v) {
                defs.push(d);
            }
        }
    }
    let assets = Assets {
        animations: &lib,
        render_controllers: &rcs,
        geometries: Some(&geos),
    };
    let def = defs
        .iter()
        .filter(|d| d.id == *id)
        .max_by_key(|d| d.min_engine)
        .expect("entity");
    let env = |k: &str| std::env::var(k).ok();
    let mut host = World {
        tick: 0,
        baby: env("BABY").is_some_and(|v| v == "1"),
        variant: env("VARIANT").and_then(|v| v.parse().ok()).unwrap_or(0.0),
    };
    let mut inst = Instance::new(1);
    if let Some(vars) = env("VARS") {
        for kv in vars.split(',') {
            if let Some((k, v)) = kv.split_once('=') {
                inst.vars
                    .insert(molang::intern(k), Value::Num(v.parse().unwrap_or(0.0)));
            }
        }
    }
    let ticks: u32 = env("TICKS").and_then(|v| v.parse().ok()).unwrap_or(40);
    let mut frame = None;
    for t in 0..ticks {
        host.tick = t;
        frame = Some(inst.update(def, &assets, &mut host, 0.05));
        if std::env::var_os("EMITS").is_some()
            && let Some(f) = &frame
        {
            for e in &f.emits {
                println!("tick {t}: emit {e:?}");
            }
        }
    }
    let mut frame = frame.expect("frame");
    // `FORCE=bone=sx,sy,sz;bone=..` sets those bones' scale after the last tick.
    if let Ok(f) = std::env::var("FORCE") {
        for item in f.split(';').filter(|i| !i.is_empty()) {
            if let Some((b, v)) = item.split_once('=') {
                let v: Vec<f32> = v.split(',').filter_map(|n| n.parse().ok()).collect();
                if v.len() == 3 {
                    frame
                        .pose
                        .bones
                        .entry(b.to_ascii_lowercase())
                        .or_default()
                        .scale = [v[0], v[1], v[2]];
                }
            }
        }
    }
    for n in env("BONES")
        .unwrap_or_default()
        .split(',')
        .filter(|n| !n.is_empty())
    {
        println!(
            "  bone {n}: {:?}",
            frame.pose.get(n).map(|b| (b.position, b.rotation, b.scale))
        );
    }
    if env("DESCRIBE").is_some() {
        println!("{}", inst.describe(def, &assets));
    }
    let size: usize = env("SIZE").and_then(|v| v.parse().ok()).unwrap_or(256);
    let only = env("ONLY");
    let mut views = vec![(25.0_f32, 15.0_f32), (-50.0, 15.0), (160.0, 15.0)];
    // `VIEWS=yaw:pitch;yaw:pitch` replaces the three views.
    if let Some(v) = env("VIEWS") {
        views = v
            .split(';')
            .filter_map(|p| {
                let (a, b) = p.split_once(':')?;
                Some((a.parse().ok()?, b.parse().ok()?))
            })
            .collect();
    }
    let mut sheet = vec![0u8; size * views.len() * size * 4];
    let mut cache: HashMap<String, Option<Tex>> = HashMap::new();
    let mut items: Vec<Item> = Vec::new();
    let build = |frame: &horizon_bedrock::entity::Frame,
                 held: Option<(&HashMap<String, (M, M)>, f32, Option<bool>, &str)>,
                 items: &mut Vec<Item>| {
        for layer in &frame.layers {
            if layer.color[3] <= 0.0 {
                continue;
            }
            if let Some(o) = &only
                && held.is_none()
                && !layer.geometry.contains(o.as_str())
            {
                continue;
            }
            if std::env::var("LAYERS").is_ok() {
                println!("layer {} texture {} material {}", layer.geometry, layer.texture, layer.material);
            }
            let Some(g) = geos.get(&layer.geometry.to_ascii_lowercase()) else {
                continue;
            };
            if !g.bones.iter().any(|b| layer.bone_visible(&b.name)) {
                continue;
            }
            let roots = g.to_tree();
            let mut tris = Vec::new();
            for root in &roots {
                match held {
                    None => collect(root, &ident(), &frame.pose, &|n| layer.bone_visible(n), &mut tris),
                    Some((mats, hand_scale, frame_mode, slot)) => {
                        let mut binds: HashMap<String, String> = g
                            .bones
                            .iter()
                            .filter_map(|b| {
                                Some((
                                    b.name.to_ascii_lowercase(),
                                    bound_to(b.binding.as_deref()?, slot)?,
                                ))
                            })
                            .collect();
                        if binds.is_empty() && matches!(slot, "main_hand" | "off_hand") {
                            for b in g.bones.iter().filter(|b| b.parent.is_none()) {
                                binds.insert(b.name.to_ascii_lowercase(), slot_bone(slot).to_owned());
                            }
                        }
                        let h = Held {
                            binds,
                            mats: mats.clone(),
                            scale: if matches!(slot, "main_hand" | "off_hand") { hand_scale } else { 1.0 },
                            frame_mode,
                            in_hand: matches!(slot, "main_hand" | "off_hand"),
                        };
                        collect_held(
                            root,
                            &ident(),
                            None,
                            &h,
                            &roots,
                            &frame.pose,
                            &|n| layer.bone_visible(n),
                            &mut tris,
                        );
                    }
                }
            }
            if env("PROBE").is_some() && layer.geometry.contains("rduxhj") {
                for (bone, t) in &tris {
                    if matches!(bone.as_str(), "hmhadk" | "6q6k" | "6q6da5k_lgh" | "6q6da5k_3gllge" | "head") {
                        let lo: Vec<f32> = (0..3).map(|a| t.p.iter().map(|p| p[a]).fold(f32::MAX, f32::min)).collect();
                        let hi: Vec<f32> = (0..3).map(|a| t.p.iter().map(|p| p[a]).fold(f32::MIN, f32::max)).collect();
                        let flat = (0..3).min_by(|a, b| (hi[*a] - lo[*a]).partial_cmp(&(hi[*b] - lo[*b])).unwrap());
                        if let Some(a) = flat {
                            println!("{bone:16} flat axis {a} at {:.5}  x {:.3}..{:.3} y {:.3}..{:.3} z {:.3}..{:.3} (px: y {:.2}..{:.2})", lo[a], lo[0], hi[0], lo[1], hi[1], lo[2], hi[2], (1.5-hi[1])*16.0, (1.5-lo[1])*16.0);
                        }
                    }
                }
            }
            let tris = tris
                .into_iter()
                .filter_map(|(bone, tri)| {
                    let f = materials.flags(layer.material_for(&bone));
                    (f.stencil_not_equal == 0 && !f.no_color)
                        .then_some((tri, (f.alpha_test, f.blend && !f.additive, f.additive)))
                })
                .collect();
            items.push(Item {
                tris,
                texture: layer.texture.clone(),
                color: layer.color,
            });
        }
    };
    build(&frame, None, &mut items);
    let holder_items = items.len();
    if let Some(att_id) = env("HOLD") {
        let slot = env("SLOT").unwrap_or_else(|| "main_hand".into());
        let frame_mode = match env("ANCHOR").as_deref() {
            Some("frame") => Some(true),
            Some("pivot") => Some(false),
            _ => None,
        };
        let adef = att_defs
            .iter()
            .find(|d| d.id == att_id)
            .or_else(|| att_defs.iter().find(|d| d.items.iter().any(|(k, _)| *k == att_id)))
            .expect("attachable");
        let mut mats = HashMap::new();
        for layer in &frame.layers {
            if let Some(g) = geos.get(&layer.geometry.to_ascii_lowercase()) {
                for root in g.to_tree() {
                    bone_mats(&root, &ident(), &ident(), &frame.pose, &mut mats);
                }
            }
        }
        let bone = slot_bone(&slot);
        if env("DEBUG_MATS").is_some() {
            for (k, (m, _)) in &mats {
                eprintln!("mat {k}: {m:?}");
            }
            for k in ["root", "waist", "body", "rightarm", "rightitem", "leftarm"] {
                eprintln!("pose {k}: {:?}", frame.pose.get(k));
            }
        }
        match mats.get(bone) {
            None => eprintln!("the holder has no `{bone}` bone"),
            Some((now, _)) => {
                let mut ainst = Instance::new(2);
                let mut ahost = AttHost { slot: slot.clone() };
                let mut af = ainst.update(adef, &assets, &mut ahost, 0.05);
                for _ in 0..ticks {
                    af = ainst.update(adef, &assets, &mut ahost, 0.05);
                }
                build(
                    &af,
                    Some((&mats, frame.held_item_scale, frame_mode, slot.as_str())),
                    &mut items,
                );
                if items[holder_items..].iter().all(|i| i.tris.is_empty()) {
                    println!("{att_id}: its layers draw no quads (every bone hidden or scaled to zero)");
                }
                let hand = apply(now, [0.0; 3]);
                // The eye: 1.62 above the feet, the frame's origin 1.5 above them, y down.
                println!(
                    "hand bone `{bone}` from the eye, blocks (x left+, up, z back+): x {:.2} up {:.2} z {:.2}",
                    hand[0],
                    -hand[1] - 0.12,
                    hand[2]
                );
                let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
                for it in &items[holder_items..] {
                    for (t, _) in &it.tris {
                        for p in t.p {
                            for k in 0..3 {
                                lo[k] = lo[k].min(p[k] - hand[k]);
                                hi[k] = hi[k].max(p[k] - hand[k]);
                            }
                        }
                    }
                }
                // The game's frame is y down: above the hand is minus y.
                println!(
                    "{att_id} ({slot}, anchor {}): {} layers; relative to the hand, in blocks: x {:.2}..{:.2}, up {:.2}..{:.2}, z {:.2}..{:.2}",
                    match frame_mode {
                        Some(true) => "frame",
                        Some(false) => "pivot",
                        None => "auto",
                    },
                    items.len() - holder_items,
                    lo[0],
                    hi[0],
                    -hi[1],
                    -lo[1],
                    lo[2],
                    hi[2]
                );
                println!(
                    "item from the eye: x {:.2}..{:.2} up {:.2}..{:.2} z {:.2}..{:.2}",
                    lo[0] + hand[0],
                    hi[0] + hand[0],
                    -hi[1] - hand[1] - 0.12,
                    -lo[1] - hand[1] - 0.12,
                    lo[2] + hand[2],
                    hi[2] + hand[2]
                );
            }
        }
    }
    // Fit: the posed model's extent decides the centre and zoom.
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for it in &items {
        for (t, _) in &it.tris {
            for p in t.p {
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
        }
    }
    let centre = [
        (lo[0] + hi[0]) / 2.0,
        (lo[1] + hi[1]) / 2.0,
        (lo[2] + hi[2]) / 2.0,
    ];
    let extent = (hi[0] - lo[0])
        .max(hi[1] - lo[1])
        .max(hi[2] - lo[2])
        .max(0.05);
    let (centre, extent) = match env("CROP").map(|c| c.split(',').filter_map(|n| n.parse::<f32>().ok()).collect::<Vec<_>>()) {
        Some(c) if c.len() == 4 => ([c[0], c[1], c[2]], c[3]),
        _ => (centre, extent),
    };
    let zoom = size as f32 * 0.8 / (extent * 1.2);
    for (vi, (yaw, pitch)) in views.iter().enumerate() {
        let mut color = vec![[0.55, 0.75, 0.55, 0.0_f32]; size * size];
        let mut depth = vec![f32::MAX; size * size];
        // Model space is y down with the model's feet at 1.5; the view turns about the middle of
        // the body. Screen x is mirrored so the entity's front shows to the camera at yaw 0.
        let view = mul(
            &rot_x(pitch.to_radians()),
            &mul(
                &rot_y(yaw.to_radians()),
                &mul(
                    &scale(-1.0, 1.0, 1.0),
                    &translate(-centre[0], -centre[1], -centre[2]),
                ),
            ),
        );
        for it in &items {
            let tex = cache
                .entry(it.texture.clone())
                .or_insert_with(|| load_tex(&pack, &it.texture))
                .as_ref();
            for (tri, mode) in &it.tris {
                raster(&mut color, &mut depth, size, tri, &view, tex, it.color, *mode, zoom);
            }
        }
        for y in 0..size {
            for x in 0..size {
                let c = color[y * size + x];
                let o = (y * size * views.len() + vi * size + x) * 4;
                for k in 0..3 {
                    sheet[o + k] = (c[k] * 255.0) as u8;
                }
                sheet[o + 3] = 255;
            }
        }
    }
    let file = std::fs::File::create(out_path).expect("out");
    let mut enc = png::Encoder::new(file, (size * views.len()) as u32, size as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .expect("header")
        .write_image_data(&sheet)
        .expect("data");
    println!("{} layers; wrote {out_path}", items.len());
}
