//! Particle effects (`particles/*.json`) and a CPU simulation of them.
//!
//! An [`Effect`] is parsed once; an [`Emitter`] is one running instance. The game moves the
//! emitter ([`Emitter::set_transform`]), calls [`Emitter::update`] each frame, and draws the
//! [`Sprite`]s from [`Emitter::sprites`]. Collisions and block tests ask a [`World`].
//! Expressions see `variable.particle_age`, `particle_lifetime`, `particle_random_1..4`,
//! `emitter_age`, `emitter_lifetime`, `emitter_random_1..4`, and the host's `query.*`.

use crate::animation::{Env, Scalar, Scope, compile};
use crate::molang::{Host, Program, Rng, Sym, Value, Vars, intern};
use serde_json::Value as Json;
use std::sync::Arc;

type V3 = [f32; 3];

/// What the simulation asks of the level.
pub trait World {
    /// Whether the point is inside a solid block.
    fn solid(&self, _p: V3) -> bool {
        false
    }
    /// Width and height of the entity the emitter rides on (`emitter_shape_entity_aabb`).
    fn entity_size(&self) -> [f32; 2] {
        [0.6, 1.8]
    }
    /// The block id at a point, for `expire_if_in_blocks`.
    fn block_id(&self, _p: V3) -> Option<String> {
        None
    }
}

pub struct NoWorld;
impl World for NoWorld {}

#[derive(Clone, Debug)]
enum Rate {
    Instant { count: Scalar },
    Steady { per_second: Scalar, max: Scalar },
    Manual,
}

#[derive(Clone, Debug)]
enum Life {
    Once {
        active: Scalar,
    },
    Looping {
        active: Scalar,
        sleep: Scalar,
    },
    Expression {
        activation: Option<Program>,
        expiration: Option<Program>,
    },
}

#[derive(Clone, Debug)]
enum Direction {
    Outwards,
    Inwards,
    Fixed([Scalar; 3]),
}

#[derive(Clone, Debug)]
enum ShapeKind {
    Point,
    Sphere {
        radius: Scalar,
    },
    Box {
        half: [Scalar; 3],
    },
    Disc {
        radius: Scalar,
        normal: V3,
    },
    /// The box around the entity the emitter rides on.
    EntityBox,
}

#[derive(Clone, Debug)]
struct Shape {
    kind: ShapeKind,
    offset: [Scalar; 3],
    surface_only: bool,
    direction: Direction,
}

#[derive(Clone, Debug)]
enum Speed {
    Along(Scalar),
    Vector([Scalar; 3]),
}

#[derive(Clone, Debug)]
struct Dynamic {
    acceleration: [Scalar; 3],
    drag: Scalar,
    rotation_acceleration: Scalar,
    rotation_drag: Scalar,
}

#[derive(Clone, Debug)]
struct Collision {
    enabled: Option<Program>,
    drag: f32,
    restitution: f32,
    expire_on_contact: bool,
    /// How far from a particle's centre it touches a block.
    radius: f32,
    /// Effect events fired when it hits a block at least this fast (blocks per second).
    events: Vec<(String, f32)>,
}

#[derive(Clone, Debug)]
enum CurveKind {
    Linear,
    CatmullRom,
    Bezier,
    /// `bezier_chain`: `(time, value)` pairs.
    Chain,
}

/// A named curve (`curves`): a variable that follows `input / horizontal_range` along a shape.
#[derive(Clone, Debug)]
struct Curve {
    name: Sym,
    kind: CurveKind,
    nodes: Vec<f32>,
    times: Vec<f32>,
    input: Option<Program>,
    range: Option<Program>,
}

impl Curve {
    fn at(&self, t: f32) -> f32 {
        let n = &self.nodes;
        match (&self.kind, n.len()) {
            (_, 0) => 0.0,
            (_, 1) => n[0],
            (CurveKind::Linear, len) => {
                let x = t.clamp(0.0, 1.0) * (len - 1) as f32;
                let i = (x.floor() as usize).min(len - 2);
                n[i] + (n[i + 1] - n[i]) * (x - i as f32)
            }
            (CurveKind::Chain, len) => {
                let t = t.clamp(0.0, 1.0);
                let i = (0..len - 1)
                    .find(|&i| t <= self.times[i + 1])
                    .unwrap_or(len - 2);
                let (t0, t1) = (self.times[i], self.times[i + 1]);
                let k = if t1 > t0 { (t - t0) / (t1 - t0) } else { 0.0 };
                n[i] + (n[i + 1] - n[i]) * k
            }
            (CurveKind::Bezier, len) if len >= 4 => {
                let t = t.clamp(0.0, 1.0);
                let u = 1.0 - t;
                u * u * u * n[0]
                    + 3.0 * u * u * t * n[1]
                    + 3.0 * u * t * t * n[2]
                    + t * t * t * n[3]
            }
            (CurveKind::CatmullRom, len) if len >= 4 => {
                // The first and last nodes steer the ends; the rest lie on the curve.
                let points = len - 2;
                let x = t.clamp(0.0, 1.0) * (points - 1) as f32;
                let i = (x.floor() as usize).min(points - 2);
                let k = x - i as f32;
                let (p0, p1, p2, p3) = (n[i], n[i + 1], n[i + 2], n[(i + 3).min(len - 1)]);
                0.5 * (2.0 * p1
                    + (p2 - p0) * k
                    + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * k * k
                    + (3.0 * p1 - p0 - 3.0 * p2 + p3) * k * k * k)
            }
            (_, len) => n[len - 1],
        }
    }
}

/// How a sprite turns towards the camera.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facing {
    RotateXyz,
    RotateY,
    LookAtXyz,
    LookAtY,
    /// Faces the camera with its height along the sprite's direction.
    LookAtDirection,
    DirectionX,
    DirectionY,
    DirectionZ,
    EmitterXy,
    EmitterXz,
    EmitterYz,
}

#[derive(Clone, Debug)]
struct Flipbook {
    base: [Scalar; 2],
    size: [Scalar; 2],
    step: [Scalar; 2],
    fps: Scalar,
    max_frame: Scalar,
    stretch: bool,
    looping: bool,
}

#[derive(Clone, Debug)]
struct Billboard {
    size: [Scalar; 2],
    facing: Facing,
    texture: [f32; 2],
    uv: [Scalar; 2],
    uv_size: [Scalar; 2],
    flipbook: Option<Flipbook>,
    direction: Option<Direction3>,
}

/// Which way a direction-facing sprite points (`direction`).
#[derive(Clone, Debug)]
struct Direction3 {
    from_velocity: bool,
    custom: [Scalar; 3],
    min_speed: f32,
}

#[derive(Clone, Debug)]
enum Tint {
    Fixed([Scalar; 4]),
    Gradient {
        interpolant: Option<Program>,
        stops: Vec<(f32, [f32; 4])>,
    },
}

/// Something a particle effect asks the game to do.
#[derive(Clone, Debug, PartialEq)]
pub enum Spawn {
    /// Another effect, at a world position.
    Effect {
        effect: String,
        at: V3,
        /// The variables the event's `pre_effect_expression` gave the new emitter.
        seed: Vars,
    },
    Sound {
        event: String,
        at: V3,
    },
}

#[derive(Clone, Debug, Default)]
struct EventDef {
    expressions: Vec<Program>,
    effect: Option<String>,
    /// `particle_effect.type` is `emitter_bound`.
    bound: bool,
    /// `particle_effect.pre_effect_expression`: sets the new emitter's variables.
    pre: Option<Program>,
    sound: Option<String>,
    sequence: Vec<Step>,
    randomize: Vec<(u32, String)>,
}

/// An entry of an event's `sequence`: another event by name, or one written in place.
#[derive(Clone, Debug)]
enum Step {
    Named(String),
    Inline(Box<EventDef>),
}

#[derive(Clone, Debug, Default)]
struct Timeline {
    events: Vec<(f32, Vec<String>)>,
}

/// A parsed particle effect.
#[derive(Debug)]
pub struct Effect {
    pub id: String,
    pub material: String,
    /// Texture path without extension.
    pub texture: String,
    rate: Rate,
    life: Life,
    shape: Shape,
    lifetime: Scalar,
    expire: Option<Program>,
    speed: Speed,
    spin: Option<(Scalar, Scalar)>,
    dynamic: Option<Dynamic>,
    parametric: Option<[Scalar; 3]>,
    collision: Option<Collision>,
    billboard: Billboard,
    tint: Tint,
    pub lit: bool,
    pub local_position: bool,
    pub local_rotation: bool,
    in_blocks: Vec<String>,
    not_in_blocks: Vec<String>,
    events: std::collections::HashMap<String, EventDef>,
    /// An event reads `query.position(..)`: the world coordinates it hands an `emitter_bound`
    /// effect are measured from the world's origin, not from the emitter.
    events_use_world_position: bool,
    on_create: Vec<String>,
    on_expire: Vec<String>,
    timeline: Timeline,
    emitter_creation: Vec<Program>,
    emitter_update: Vec<Program>,
    particle_creation: Vec<Program>,
    particle_update: Vec<Program>,
    particle_render: Vec<Program>,
    kill_plane: Option<[f32; 4]>,
    curves: Vec<Curve>,
    emitter_create: Vec<String>,
    emitter_expire: Vec<String>,
    emitter_timeline: Timeline,
}

fn sc(v: Option<&Json>, d: f32) -> Scalar {
    v.and_then(Scalar::from_json).unwrap_or(Scalar::Const(d))
}

fn sc3(v: Option<&Json>, d: f32) -> [Scalar; 3] {
    match v {
        Some(Json::Array(a)) if a.len() >= 3 => std::array::from_fn(|i| sc(a.get(i), d)),
        _ => std::array::from_fn(|_| Scalar::Const(d)),
    }
}

fn sc2(v: Option<&Json>, d: f32) -> [Scalar; 2] {
    match v {
        Some(Json::Array(a)) if a.len() >= 2 => std::array::from_fn(|i| sc(a.get(i), d)),
        _ => std::array::from_fn(|_| Scalar::Const(d)),
    }
}

/// A component's expression (or list of them), compiled.
fn exprs(v: Option<&Json>) -> Vec<Program> {
    match v {
        Some(Json::String(s)) => compile(s).into_iter().collect(),
        Some(Json::Array(a)) => a
            .iter()
            .filter_map(Json::as_str)
            .filter_map(compile)
            .collect(),
        _ => Vec::new(),
    }
}

fn parse_curves(body: &Json) -> Vec<Curve> {
    let Some(o) = body.get("curves").and_then(Json::as_object) else {
        return Vec::new();
    };
    o.iter()
        .filter_map(|(key, c)| {
            let name = key
                .strip_prefix("variable.")
                .or_else(|| key.strip_prefix("v."))
                .unwrap_or(key)
                .to_ascii_lowercase();
            let kind = match c.get("type").and_then(Json::as_str) {
                Some("catmull_rom") => CurveKind::CatmullRom,
                Some("bezier") => CurveKind::Bezier,
                Some("bezier_chain") => CurveKind::Chain,
                _ => CurveKind::Linear,
            };
            let (nodes, times): (Vec<f32>, Vec<f32>) = match c.get("nodes")? {
                Json::Array(a) => (
                    a.iter()
                        .filter_map(Json::as_f64)
                        .map(|n| n as f32)
                        .collect(),
                    Vec::new(),
                ),
                Json::Object(o) => {
                    let mut pairs: Vec<(f32, f32)> = o
                        .iter()
                        .filter_map(|(k, v)| {
                            let value = v
                                .get("value")
                                .and_then(Json::as_f64)
                                .or_else(|| v.as_f64())?;
                            Some((k.trim().parse().ok()?, value as f32))
                        })
                        .collect();
                    pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
                    pairs.into_iter().unzip()
                }
                _ => return None,
            };
            let program = |k: &str| match c.get(k) {
                Some(Json::String(s)) => compile(s),
                Some(Json::Number(n)) => compile(&n.to_string()),
                _ => None,
            };
            Some(Curve {
                name: intern(&name),
                kind,
                nodes,
                times,
                input: program("input"),
                range: program("horizontal_range"),
            })
        })
        .collect()
}

fn hex(s: &str) -> Option<[f32; 4]> {
    let h = s.strip_prefix('#')?;
    let n = u32::from_str_radix(h, 16).ok()?;
    let (a, r, g, b) = match h.len() {
        8 => ((n >> 24) & 255, (n >> 16) & 255, (n >> 8) & 255, n & 255),
        6 => (255, (n >> 16) & 255, (n >> 8) & 255, n & 255),
        _ => return None,
    };
    Some([
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        a as f32 / 255.0,
    ])
}

fn color_stop(v: &Json) -> Option<[f32; 4]> {
    match v {
        Json::String(s) => hex(s),
        Json::Array(a) if a.len() >= 3 => Some(std::array::from_fn(|i| {
            a.get(i).and_then(Json::as_f64).unwrap_or(1.0) as f32
        })),
        _ => None,
    }
}

fn names(v: Option<&Json>) -> Vec<String> {
    match v {
        Some(Json::String(s)) => vec![s.clone()],
        Some(Json::Array(a)) => a
            .iter()
            .filter_map(Json::as_str)
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

/// `time: [event names]` pairs, in time order.
fn timeline(v: Option<&Json>) -> Vec<(f32, Vec<String>)> {
    let mut t: Vec<(f32, Vec<String>)> = v
        .and_then(Json::as_object)
        .map(|o| {
            o.iter()
                .filter_map(|(k, v)| Some((k.trim().parse().ok()?, names(Some(v)))))
                .collect()
        })
        .unwrap_or_default();
    t.sort_by(|a, b| a.0.total_cmp(&b.0));
    t
}

fn parse_event(e: &Json) -> EventDef {
    EventDef {
        expressions: e
            .get("expression")
            .and_then(Json::as_str)
            .and_then(compile)
            .into_iter()
            .collect(),
        effect: e
            .pointer("/particle_effect/effect")
            .and_then(Json::as_str)
            .map(str::to_owned),
        bound: e.pointer("/particle_effect/type").and_then(Json::as_str) == Some("emitter_bound"),
        pre: e
            .pointer("/particle_effect/pre_effect_expression")
            .and_then(Json::as_str)
            .and_then(compile),
        sound: e
            .pointer("/sound_effect/event_name")
            .and_then(Json::as_str)
            .map(str::to_owned),
        sequence: e
            .get("sequence")
            .and_then(Json::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|s| match s.get("event").and_then(Json::as_str) {
                        Some(name) => Some(Step::Named(name.to_owned())),
                        None if s.is_object() => Some(Step::Inline(Box::new(parse_event(s)))),
                        None => None,
                    })
                    .collect()
            })
            .unwrap_or_default(),
        randomize: e
            .get("randomize")
            .and_then(Json::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|s| {
                        Some((
                            s.get("weight").and_then(Json::as_u64).unwrap_or(1) as u32,
                            s.get("event")?.as_str()?.to_owned(),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn parse_events(root: &Json) -> std::collections::HashMap<String, EventDef> {
    root.get("events")
        .and_then(Json::as_object)
        .into_iter()
        .flatten()
        .map(|(name, e)| (name.clone(), parse_event(e)))
        .collect()
}

impl Effect {
    /// Reads a file with a `particle_effect` root.
    pub fn parse(root: &Json) -> Option<Effect> {
        let body = root.get("particle_effect")?;
        let desc = body.get("description")?;
        let render = desc.get("basic_render_parameters");
        let c = body.get("components").and_then(Json::as_object)?;
        let comp = |k: &str| c.get(&format!("minecraft:{k}"));

        let rate = if let Some(r) = comp("emitter_rate_steady") {
            Rate::Steady {
                per_second: sc(r.get("spawn_rate"), 1.0),
                max: sc(r.get("max_particles"), 50.0),
            }
        } else if comp("emitter_rate_manual").is_some() {
            Rate::Manual
        } else {
            Rate::Instant {
                count: sc(
                    comp("emitter_rate_instant").and_then(|r| r.get("num_particles")),
                    10.0,
                ),
            }
        };
        let life = if let Some(l) = comp("emitter_lifetime_looping") {
            Life::Looping {
                active: sc(l.get("active_time"), 10.0),
                sleep: sc(l.get("sleep_time"), 0.0),
            }
        } else if let Some(l) = comp("emitter_lifetime_expression") {
            Life::Expression {
                activation: l
                    .get("activation_expression")
                    .and_then(Json::as_str)
                    .and_then(compile),
                expiration: l
                    .get("expiration_expression")
                    .and_then(Json::as_str)
                    .and_then(compile),
            }
        } else {
            Life::Once {
                active: sc(
                    comp("emitter_lifetime_once").and_then(|l| l.get("active_time")),
                    10.0,
                ),
            }
        };

        let mut shape = Shape {
            kind: ShapeKind::Point,
            offset: sc3(None, 0.0),
            surface_only: false,
            direction: Direction::Fixed(sc3(None, 0.0)),
        };
        for (key, make) in [
            ("emitter_shape_point", 0),
            ("emitter_shape_custom", 0),
            ("emitter_shape_sphere", 1),
            ("emitter_shape_box", 2),
            ("emitter_shape_disc", 3),
            ("emitter_shape_entity_aabb", 4),
        ] {
            let Some(s) = comp(key) else { continue };
            shape.offset = sc3(s.get("offset"), 0.0);
            shape.surface_only = s
                .get("surface_only")
                .and_then(Json::as_bool)
                .unwrap_or(false);
            shape.direction = match s.get("direction") {
                Some(Json::String(d)) if d == "inwards" => Direction::Inwards,
                Some(Json::String(d)) if d == "outwards" => Direction::Outwards,
                Some(d @ Json::Array(_)) => Direction::Fixed(sc3(Some(d), 0.0)),
                _ => Direction::Fixed(sc3(None, 0.0)),
            };
            shape.kind = match make {
                1 => ShapeKind::Sphere {
                    radius: sc(s.get("radius"), 1.0),
                },
                2 => ShapeKind::Box {
                    half: sc3(s.get("half_dimensions"), 1.0),
                },
                3 => ShapeKind::Disc {
                    radius: sc(s.get("radius"), 1.0),
                    normal: match s.get("plane_normal") {
                        Some(Json::String(p)) if p == "x" => [1.0, 0.0, 0.0],
                        Some(Json::String(p)) if p == "z" => [0.0, 0.0, 1.0],
                        Some(Json::Array(a)) if a.len() >= 3 => {
                            std::array::from_fn(|i| a[i].as_f64().unwrap_or(0.0) as f32)
                        }
                        _ => [0.0, 1.0, 0.0],
                    },
                },
                4 => ShapeKind::EntityBox,
                _ => ShapeKind::Point,
            };
            break;
        }

        let lifetime_c = comp("particle_lifetime_expression");
        let speed = match comp("particle_initial_speed") {
            Some(Json::Array(a)) if a.len() >= 3 => {
                Speed::Vector(sc3(comp("particle_initial_speed"), 0.0))
            }
            Some(v) => Speed::Along(sc(Some(v), 0.0)),
            None => Speed::Along(Scalar::Const(0.0)),
        };
        let bb = comp("particle_appearance_billboard");
        let uv = bb.and_then(|b| b.get("uv"));
        let texture = [
            uv.and_then(|u| u.get("texture_width"))
                .and_then(Json::as_f64)
                .unwrap_or(1.0) as f32,
            uv.and_then(|u| u.get("texture_height"))
                .and_then(Json::as_f64)
                .unwrap_or(1.0) as f32,
        ];
        let flipbook = uv.and_then(|u| u.get("flipbook")).map(|f| Flipbook {
            base: sc2(f.get("base_UV"), 0.0),
            size: sc2(f.get("size_UV"), 1.0),
            step: sc2(f.get("step_UV"), 0.0),
            fps: sc(f.get("frames_per_second"), 0.0),
            max_frame: sc(f.get("max_frame"), 1.0),
            stretch: f
                .get("stretch_to_lifetime")
                .and_then(Json::as_bool)
                .unwrap_or(false),
            looping: f.get("loop").and_then(Json::as_bool).unwrap_or(false),
        });
        let facing = match bb
            .and_then(|b| b.get("facing_camera_mode"))
            .and_then(Json::as_str)
        {
            Some("rotate_xyz") => Facing::RotateXyz,
            Some("rotate_y") => Facing::RotateY,
            Some("lookat_y") => Facing::LookAtY,
            Some("lookat_direction") => Facing::LookAtDirection,
            Some("direction_x") => Facing::DirectionX,
            Some("direction_y") => Facing::DirectionY,
            Some("direction_z") => Facing::DirectionZ,
            Some("emitter_transform_xy") => Facing::EmitterXy,
            Some("emitter_transform_xz") => Facing::EmitterXz,
            Some("emitter_transform_yz") => Facing::EmitterYz,
            _ => Facing::LookAtXyz,
        };
        let tint = match comp("particle_appearance_tinting").and_then(|t| t.get("color")) {
            Some(Json::Array(a)) if a.len() >= 3 => {
                Tint::Fixed(std::array::from_fn(|i| sc(a.get(i), 1.0)))
            }
            Some(Json::String(s)) => {
                let c = hex(s).unwrap_or([1.0; 4]);
                Tint::Fixed(c.map(Scalar::Const))
            }
            Some(Json::Object(o)) => {
                let mut stops: Vec<(f32, [f32; 4])> = match o.get("gradient") {
                    Some(Json::Object(g)) => g
                        .iter()
                        .filter_map(|(k, v)| Some((k.trim().parse().ok()?, color_stop(v)?)))
                        .collect(),
                    Some(Json::Array(g)) => {
                        let n = g.len().max(2) - 1;
                        g.iter()
                            .enumerate()
                            .filter_map(|(i, v)| Some((i as f32 / n as f32, color_stop(v)?)))
                            .collect()
                    }
                    _ => Vec::new(),
                };
                stops.sort_by(|a, b| a.0.total_cmp(&b.0));
                if stops.is_empty() {
                    Tint::Fixed([1.0; 4].map(Scalar::Const))
                } else {
                    Tint::Gradient {
                        interpolant: o
                            .get("interpolant")
                            .and_then(Json::as_str)
                            .and_then(compile),
                        stops,
                    }
                }
            }
            _ => Tint::Fixed([1.0; 4].map(Scalar::Const)),
        };

        let local = comp("emitter_local_space");
        let flag = |k: &str| {
            local
                .and_then(|l| l.get(k))
                .and_then(Json::as_bool)
                .unwrap_or(false)
        };
        let le = comp("particle_lifetime_events");
        Some(Effect {
            id: desc.get("identifier")?.as_str()?.to_owned(),
            material: render
                .and_then(|r| r.get("material"))
                .and_then(Json::as_str)
                .unwrap_or("particles_alpha")
                .to_owned(),
            texture: render
                .and_then(|r| r.get("texture"))
                .and_then(Json::as_str)
                .unwrap_or("")
                .to_owned(),
            rate,
            life,
            shape,
            lifetime: sc(lifetime_c.and_then(|l| l.get("max_lifetime")), 1.0),
            expire: lifetime_c
                .and_then(|l| l.get("expiration_expression"))
                .and_then(Json::as_str)
                .and_then(compile),
            speed,
            spin: comp("particle_initial_spin")
                .map(|s| (sc(s.get("rotation"), 0.0), sc(s.get("rotation_rate"), 0.0))),
            dynamic: comp("particle_motion_dynamic").map(|d| Dynamic {
                acceleration: sc3(d.get("linear_acceleration"), 0.0),
                drag: sc(d.get("linear_drag_coefficient"), 0.0),
                rotation_acceleration: sc(d.get("rotation_acceleration"), 0.0),
                rotation_drag: sc(d.get("rotation_drag_coefficient"), 0.0),
            }),
            parametric: comp("particle_motion_parametric")
                .map(|p| sc3(p.get("relative_position"), 0.0)),
            collision: comp("particle_motion_collision").map(|c| Collision {
                enabled: c.get("enabled").and_then(|e| match e {
                    Json::String(s) => compile(s),
                    Json::Bool(b) => compile(if *b { "1" } else { "0" }),
                    _ => None,
                }),
                drag: c
                    .get("collision_drag")
                    .and_then(Json::as_f64)
                    .unwrap_or(0.0) as f32,
                restitution: c
                    .get("coefficient_of_restitution")
                    .and_then(Json::as_f64)
                    .unwrap_or(0.0) as f32,
                expire_on_contact: c
                    .get("expire_on_contact")
                    .and_then(Json::as_bool)
                    .unwrap_or(false),
                radius: c
                    .get("collision_radius")
                    .and_then(Json::as_f64)
                    .unwrap_or(0.1) as f32,
                events: c
                    .get("events")
                    .and_then(Json::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(|e| {
                                Some((
                                    e.get("event")?.as_str()?.to_owned(),
                                    e.get("min_speed").and_then(Json::as_f64).unwrap_or(2.0) as f32,
                                ))
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            }),
            billboard: Billboard {
                size: sc2(bb.and_then(|b| b.get("size")), 0.1),
                facing,
                texture,
                uv: sc2(uv.and_then(|u| u.get("uv")), 0.0),
                uv_size: sc2(uv.and_then(|u| u.get("uv_size")), texture[0]),
                flipbook,
                direction: bb.and_then(|b| b.get("direction")).map(|d| Direction3 {
                    from_velocity: d.get("mode").and_then(Json::as_str) != Some("custom"),
                    custom: sc3(d.get("custom_direction"), 0.0),
                    min_speed: d
                        .get("min_speed_threshold")
                        .and_then(Json::as_f64)
                        .unwrap_or(0.01) as f32,
                }),
            },
            tint,
            lit: comp("particle_appearance_lighting").is_some(),
            local_position: flag("position"),
            local_rotation: flag("rotation"),
            in_blocks: names(comp("particle_expire_if_in_blocks")),
            not_in_blocks: names(comp("particle_expire_if_not_in_blocks")),
            events: parse_events(body),
            events_use_world_position: body.get("events").is_some_and(|ev| {
                let text = ev.to_string().to_ascii_lowercase();
                text.contains("q.position(") || text.contains("query.position(")
            }),
            on_create: names(le.and_then(|l| l.get("creation_event"))),
            on_expire: names(le.and_then(|l| l.get("expiration_event"))),
            timeline: Timeline {
                events: {
                    let mut t: Vec<(f32, Vec<String>)> = le
                        .and_then(|l| l.get("timeline"))
                        .and_then(Json::as_object)
                        .map(|o| {
                            o.iter()
                                .filter_map(|(k, v)| Some((k.trim().parse().ok()?, names(Some(v)))))
                                .collect()
                        })
                        .unwrap_or_default();
                    t.sort_by(|a, b| a.0.total_cmp(&b.0));
                    t
                },
            },
            emitter_creation: exprs(
                comp("emitter_initialization").and_then(|i| i.get("creation_expression")),
            ),
            emitter_update: exprs(
                comp("emitter_initialization").and_then(|i| i.get("per_update_expression")),
            ),
            particle_creation: exprs(
                comp("particle_initialization").and_then(|i| i.get("creation_expression")),
            ),
            particle_update: exprs(
                comp("particle_initialization").and_then(|i| i.get("per_update_expression")),
            ),
            particle_render: exprs(
                comp("particle_initialization").and_then(|i| i.get("per_render_expression")),
            ),
            kill_plane: match comp("particle_kill_plane") {
                Some(Json::Array(a)) if a.len() >= 4 => {
                    Some(std::array::from_fn(|i| a[i].as_f64().unwrap_or(0.0) as f32))
                }
                _ => None,
            },
            curves: parse_curves(body),
            emitter_create: names(
                comp("emitter_lifetime_events").and_then(|l| l.get("creation_event")),
            ),
            emitter_expire: names(
                comp("emitter_lifetime_events").and_then(|l| l.get("expiration_event")),
            ),
            emitter_timeline: Timeline {
                events: timeline(comp("emitter_lifetime_events").and_then(|l| l.get("timeline"))),
            },
        })
    }
}

#[derive(Clone, Debug, Default)]
struct Particle {
    pos: V3,
    vel: V3,
    age: f32,
    lifetime: f32,
    rotation: f32,
    spin: f32,
    random: [f32; 4],
    /// Relative position origin for parametric motion.
    origin: V3,
    timeline_at: usize,
    dead: bool,
    /// The variables this particle's own expressions set; the emitter's are not touched.
    vars: Vars,
}

/// One particle to draw.
#[derive(Clone, Debug, PartialEq)]
pub struct Sprite {
    /// World position (the emitter's transform already applied).
    pub pos: V3,
    /// Half-extents in blocks.
    pub size: [f32; 2],
    /// Degrees around the view axis.
    pub rotation: f32,
    /// `[u0, v0, u1, v1]` as fractions of the texture.
    pub uv: [f32; 4],
    pub color: [f32; 4],
    pub facing: Facing,
    /// Velocity, for the direction facing modes.
    pub velocity: V3,
}

struct Names {
    age: Sym,
    lifetime: Sym,
    random: [Sym; 4],
    e_age: Sym,
    e_lifetime: Sym,
    e_random: [Sym; 4],
}

fn names_table() -> &'static Names {
    static N: std::sync::OnceLock<Names> = std::sync::OnceLock::new();
    N.get_or_init(|| Names {
        age: intern("particle_age"),
        lifetime: intern("particle_lifetime"),
        random: [
            intern("particle_random_1"),
            intern("particle_random_2"),
            intern("particle_random_3"),
            intern("particle_random_4"),
        ],
        e_age: intern("emitter_age"),
        e_lifetime: intern("emitter_lifetime"),
        e_random: [
            intern("emitter_random_1"),
            intern("emitter_random_2"),
            intern("emitter_random_3"),
            intern("emitter_random_4"),
        ],
    })
}

/// One running effect.
pub struct Emitter {
    effect: Arc<Effect>,
    pos: V3,
    yaw: f32,
    age: f32,
    particles: Vec<Particle>,
    vars: Vars,
    rng: Rng,
    owed: f32,
    started: bool,
    cycle: i64,
    /// Where the current loop began, how long it is active and how long it lasts.
    loop_at: (f32, f32, f32),
    finished_emitting: bool,
    random: [f32; 4],
    max_seen: usize,
    created: bool,
    entity_size: [f32; 2],
    timeline_at: usize,
    expired: bool,
    /// What the pack's `pre_effect_script` and `pre_effect_expression` set, applied as the
    /// emitter starts.
    init: Vec<Program>,
    seed: Vars,
}

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn len(a: V3) -> f32 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

fn rotate_yaw(p: V3, yaw_deg: f32) -> V3 {
    let (s, c) = yaw_deg.to_radians().sin_cos();
    [p[0] * c + p[2] * s, p[1], -p[0] * s + p[2] * c]
}

impl Emitter {
    pub fn new(effect: Arc<Effect>, seed: u64) -> Emitter {
        let mut rng = Rng::new(seed);
        let random = [rng.unit(), rng.unit(), rng.unit(), rng.unit()];
        Emitter {
            effect,
            pos: [0.0; 3],
            yaw: 0.0,
            age: 0.0,
            particles: Vec::new(),
            vars: Vars::default(),
            rng,
            owed: 0.0,
            started: false,
            cycle: 0,
            loop_at: (0.0, 0.0, 0.0),
            finished_emitting: false,
            random,
            max_seen: 0,
            created: false,
            entity_size: [0.6, 1.8],
            timeline_at: 0,
            expired: false,
            init: Vec::new(),
            seed: Vars::default(),
        }
    }

    /// Has the emitter run `script` (a `pre_effect_script`) and take `seed` as variables
    /// when it starts.
    pub fn set_init(&mut self, script: Option<&str>, seed: Vars) {
        self.init = script.and_then(compile).into_iter().collect();
        self.seed = seed;
    }

    pub fn effect(&self) -> &Effect {
        &self.effect
    }

    /// Where the emitter is (feet of the entity, or the locator) and which way it faces.
    pub fn set_transform(&mut self, pos: V3, yaw_degrees: f32) {
        self.pos = pos;
        self.yaw = yaw_degrees;
    }

    pub fn alive(&self) -> usize {
        self.particles.len()
    }

    /// Whether the emitter starts over when its active time ends (an aura, a flame).
    pub fn is_looping(&self) -> bool {
        matches!(self.effect.life, Life::Looping { .. })
    }

    /// Whether the emitter goes on until told to stop (looping, or ended by its own expression),
    /// as the effect of a controller state does.
    pub fn is_sustained(&self) -> bool {
        matches!(
            self.effect.life,
            Life::Looping { .. } | Life::Expression { .. }
        )
    }

    /// Whether the emitter still makes new particles.
    pub fn emitting(&self) -> bool {
        !self.finished_emitting
    }

    /// Stops emitting; the particles already out live their lives.
    pub fn stop(&mut self) {
        self.finished_emitting = true;
    }

    /// No more particles will appear and none are left.
    pub fn finished(&self) -> bool {
        self.finished_emitting && self.particles.is_empty()
    }

    fn eval(&mut self, host: &mut dyn Host, s: &Scalar) -> f32 {
        s.eval(
            &mut Env {
                host,
                vars: &mut self.vars,
                rng: &mut self.rng,
            },
            Scope::default(),
        )
    }

    fn run(&mut self, host: &mut dyn Host, p: &Program) -> f32 {
        Env {
            host,
            vars: &mut self.vars,
            rng: &mut self.rng,
        }
        .run(p, Scope::default(), Value::Null)
        .num()
    }

    fn set_emitter_vars(&mut self, lifetime: f32) {
        let n = names_table();
        self.vars.insert(n.e_age, Value::Num(self.age));
        self.vars.insert(n.e_lifetime, Value::Num(lifetime));
        for i in 0..4 {
            self.vars.insert(n.e_random[i], Value::Num(self.random[i]));
        }
    }

    /// Makes the variables those of `p` over the emitter's; hands back the emitter's.
    fn enter(&mut self, p: &mut Particle) -> Vars {
        let emitter = self.vars.clone();
        for (k, v) in p.vars.drain() {
            self.vars.insert(k, v);
        }
        emitter
    }

    /// Keeps in `p` what it set, and gives the emitter its variables back.
    fn leave(&mut self, p: &mut Particle, emitter: Vars) {
        p.vars = self
            .vars
            .drain()
            .filter(|(k, v)| emitter.get(k) != Some(v))
            .collect();
        self.vars = emitter;
    }

    fn set_particle_vars(&mut self, p: &Particle) {
        let n = names_table();
        self.vars.insert(n.age, Value::Num(p.age));
        self.vars.insert(n.lifetime, Value::Num(p.lifetime));
        for i in 0..4 {
            self.vars.insert(n.random[i], Value::Num(p.random[i]));
        }
    }

    /// Sets each curve's variable from its input.
    fn apply_curves(&mut self, host: &mut dyn Host) {
        let e = Arc::clone(&self.effect);
        for c in &e.curves {
            let input = c.input.as_ref().map_or(0.0, |p| self.run(host, p));
            let range = c.range.as_ref().map_or(1.0, |p| self.run(host, p));
            let t = if range.abs() > 1e-6 {
                input / range
            } else {
                0.0
            };
            self.vars.insert(c.name, Value::Num(c.at(t)));
        }
    }

    fn to_store(&self, world_pos: V3) -> V3 {
        if self.effect.local_position {
            let rel = [
                world_pos[0] - self.pos[0],
                world_pos[1] - self.pos[1],
                world_pos[2] - self.pos[2],
            ];
            if self.effect.local_rotation {
                rotate_yaw(rel, -self.yaw)
            } else {
                rel
            }
        } else {
            world_pos
        }
    }

    fn to_world(&self, stored: V3) -> V3 {
        if self.effect.local_position {
            let r = if self.effect.local_rotation {
                rotate_yaw(stored, self.yaw)
            } else {
                stored
            };
            add(self.pos, r)
        } else {
            stored
        }
    }

    fn spawn(&mut self, host: &mut dyn Host, out: &mut Vec<Spawn>) {
        let e = Arc::clone(&self.effect);
        let offset: V3 = std::array::from_fn(|i| self.eval(host, &e.shape.offset[i]));
        let mut local = match &e.shape.kind {
            ShapeKind::Point => [0.0; 3],
            ShapeKind::Sphere { radius } => {
                let r = self.eval(host, radius);
                let mut d = [
                    self.rng.unit() * 2.0 - 1.0,
                    self.rng.unit() * 2.0 - 1.0,
                    self.rng.unit() * 2.0 - 1.0,
                ];
                let l = len(d);
                if l < 1e-4 {
                    d = [0.0, 1.0, 0.0];
                } else {
                    d = d.map(|x| x / l);
                }
                let k = if e.shape.surface_only {
                    r
                } else {
                    r * self.rng.unit().cbrt()
                };
                d.map(|x| x * k)
            }
            ShapeKind::Box { half } => {
                let h: V3 = std::array::from_fn(|i| self.eval(host, &half[i]));
                let mut p: V3 = std::array::from_fn(|i| (self.rng.unit() * 2.0 - 1.0) * h[i]);
                if e.shape.surface_only {
                    let face = (self.rng.unit() * 3.0) as usize % 3;
                    p[face] = if self.rng.unit() < 0.5 {
                        -h[face]
                    } else {
                        h[face]
                    };
                }
                p
            }
            ShapeKind::EntityBox => {
                let [w, h] = self.entity_size;
                let half = [w / 2.0, h / 2.0, w / 2.0];
                let mut p: V3 = std::array::from_fn(|i| (self.rng.unit() * 2.0 - 1.0) * half[i]);
                if e.shape.surface_only {
                    let face = (self.rng.unit() * 3.0) as usize % 3;
                    p[face] = if self.rng.unit() < 0.5 {
                        -half[face]
                    } else {
                        half[face]
                    };
                }
                [p[0], p[1] + h / 2.0, p[2]]
            }
            ShapeKind::Disc { radius, normal } => {
                let r = self.eval(host, radius);
                let k = if e.shape.surface_only {
                    r
                } else {
                    r * self.rng.unit().sqrt()
                };
                let a = self.rng.unit() * std::f32::consts::TAU;
                let (u, v) = (a.cos() * k, a.sin() * k);
                if normal[0].abs() > 0.5 {
                    [0.0, u, v]
                } else if normal[2].abs() > 0.5 {
                    [u, v, 0.0]
                } else {
                    [u, 0.0, v]
                }
            }
        };
        let dir = match &e.shape.direction {
            Direction::Fixed(d) => {
                let v: V3 = std::array::from_fn(|i| self.eval(host, &d[i]));
                let l = len(v);
                if l > 1e-5 { v.map(|x| x / l) } else { v }
            }
            Direction::Outwards | Direction::Inwards => {
                let l = len(local);
                let d = if l > 1e-5 {
                    local.map(|x| x / l)
                } else {
                    [0.0, 1.0, 0.0]
                };
                if matches!(e.shape.direction, Direction::Inwards) {
                    d.map(|x| -x)
                } else {
                    d
                }
            }
        };
        local = add(local, offset);
        let mut p = Particle {
            random: [
                self.rng.unit(),
                self.rng.unit(),
                self.rng.unit(),
                self.rng.unit(),
            ],
            ..Particle::default()
        };
        let emitter_vars = self.vars.clone();
        self.vars.insert(names_table().age, Value::Num(0.0));
        self.set_particle_vars(&p);
        for x in &e.particle_creation {
            self.run(host, x);
        }
        self.apply_curves(host);
        p.lifetime = self.eval(host, &e.lifetime).max(0.0);
        self.set_particle_vars(&p);
        let velocity = match &e.speed {
            Speed::Along(s) => {
                let s = self.eval(host, s);
                dir.map(|x| x * s)
            }
            Speed::Vector(v) => std::array::from_fn(|i| self.eval(host, &v[i])),
        };
        let world_vel = if e.local_position {
            velocity
        } else {
            rotate_yaw(velocity, 0.0)
        };
        // Spawn in world axes: the emitter's own turn only matters with `local_space.rotation`.
        let world_pos = add(
            self.pos,
            if e.local_rotation {
                rotate_yaw(local, self.yaw)
            } else {
                local
            },
        );
        p.pos = self.to_store(world_pos);
        p.origin = p.pos;
        p.vel = world_vel;
        if let Some((rot, rate)) = &e.spin {
            p.rotation = self.eval(host, rot);
            p.spin = self.eval(host, rate);
        }
        self.leave(&mut p, emitter_vars);
        for name in &e.on_create {
            self.fire(name, p.pos, host, out);
        }
        self.particles.push(p);
    }

    /// Runs an event; its steps share their `temp.*` variables.
    fn fire(&mut self, name: &str, at: V3, host: &mut dyn Host, out: &mut Vec<Spawn>) {
        crate::molang::shared_temps(|| self.fire_in(name, at, host, out));
    }

    fn fire_in(&mut self, name: &str, at: V3, host: &mut dyn Host, out: &mut Vec<Spawn>) {
        let e = Arc::clone(&self.effect);
        let Some(ev) = e.events.get(name) else { return };
        self.run_event(&e, ev, at, host, out);
    }

    fn run_event(
        &mut self,
        e: &Effect,
        ev: &EventDef,
        at: V3,
        host: &mut dyn Host,
        out: &mut Vec<Spawn>,
    ) {
        for p in &ev.expressions {
            self.run(host, p);
        }
        let at = self.to_world(at);
        if let Some(effect) = &ev.effect {
            // The new emitter's variables, from `pre_effect_expression` (which reads the
            // event's `temp.*`).
            let mut seed = Vars::default();
            if let Some(p) = &ev.pre {
                Env {
                    host: &mut *host,
                    vars: &mut seed,
                    rng: &mut self.rng,
                }
                .run(p, Scope::default(), Value::Null);
            }
            out.push(Spawn::Effect {
                effect: effect.clone(),
                // Coordinates the pack worked out in the world go to a bound emitter that
                // starts at the world's origin.
                at: if ev.bound && e.events_use_world_position {
                    [0.0; 3]
                } else {
                    at
                },
                seed,
            });
        }
        if let Some(event) = &ev.sound {
            out.push(Spawn::Sound {
                event: event.clone(),
                at,
            });
        }
        for s in &ev.sequence {
            match s {
                Step::Named(n) => self.fire_in(n, at, host, out),
                Step::Inline(d) => self.run_event(e, d, at, host, out),
            }
        }
        if !ev.randomize.is_empty() {
            let total: u32 = ev.randomize.iter().map(|r| r.0.max(1)).sum();
            let mut roll = (self.rng.unit() * total as f32) as u32;
            for (w, name) in &ev.randomize {
                if roll < (*w).max(1) {
                    self.fire_in(name, at, host, out);
                    break;
                }
                roll -= (*w).max(1);
            }
        }
    }

    /// Advances by `dt` seconds; effects and sounds the particles ask for go to `out`.
    pub fn update(
        &mut self,
        dt: f32,
        host: &mut dyn Host,
        world: &dyn World,
        out: &mut Vec<Spawn>,
    ) {
        let e = Arc::clone(&self.effect);
        self.age += dt;
        self.entity_size = world.entity_size();
        self.set_emitter_vars(0.0);
        if !self.created {
            self.created = true;
            for x in &e.emitter_creation {
                self.run(host, x);
            }
            for (k, v) in std::mem::take(&mut self.seed) {
                self.vars.insert(k, v);
            }
            for x in std::mem::take(&mut self.init) {
                self.run(host, &x);
            }
            let at = self.to_store(self.pos);
            for n in &e.emitter_create {
                self.fire(n, at, host, out);
            }
        }
        for x in &e.emitter_update {
            self.run(host, x);
        }
        // Whether the emitter emits.
        let active = match &e.life {
            Life::Once { active } => {
                let a = self.eval(host, active);
                self.set_emitter_vars(a);
                let on = self.age <= a;
                if !on {
                    self.finished_emitting = true;
                }
                on
            }
            Life::Looping { active, sleep } => {
                // The times are worked out once per loop (a random active time is not drawn
                // anew every frame).
                if self.cycle == 0 || self.age >= self.loop_at.0 + self.loop_at.2 {
                    let (a, s) = (
                        self.eval(host, active).max(1e-3),
                        self.eval(host, sleep).max(0.0),
                    );
                    let begin = if self.cycle == 0 {
                        0.0
                    } else {
                        self.loop_at.0 + self.loop_at.2
                    };
                    self.loop_at = (begin, a, a + s);
                    self.cycle += 1;
                    self.started = false;
                    // What was owed at the end of a loop is spent in the next: a rate whose
                    // loop gives one particle at most would otherwise never spawn any.
                }
                self.set_emitter_vars(self.loop_at.1);
                (self.age - self.loop_at.0) <= self.loop_at.1
            }
            Life::Expression {
                activation,
                expiration,
            } => {
                if let Some(x) = expiration
                    && self.run(host, x) != 0.0
                {
                    self.finished_emitting = true;
                }
                !self.finished_emitting
                    && activation.as_ref().is_none_or(|a| self.run(host, a) != 0.0)
            }
        };
        let at = self.to_store(self.pos);
        while let Some((t, evs)) = e.emitter_timeline.events.get(self.timeline_at) {
            if self.age < *t {
                break;
            }
            self.timeline_at += 1;
            for n in evs {
                self.fire(n, at, host, out);
            }
        }
        if self.finished_emitting && !self.expired {
            self.expired = true;
            for n in &e.emitter_expire {
                self.fire(n, at, host, out);
            }
        }
        if active && !self.finished_emitting {
            match &e.rate {
                Rate::Instant { count } => {
                    if !self.started {
                        self.started = true;
                        let n = self.eval(host, count).max(0.0) as usize;
                        for _ in 0..n.min(4096) {
                            self.spawn(host, out);
                        }
                    }
                }
                Rate::Steady { per_second, max } => {
                    self.started = true;
                    let rate = self.eval(host, per_second).max(0.0);
                    let cap = self.eval(host, max).max(0.0) as usize;
                    self.owed += rate * dt;
                    while self.owed >= 1.0 {
                        self.owed -= 1.0;
                        if self.particles.len() < cap.min(4096) {
                            self.spawn(host, out);
                        }
                    }
                }
                Rate::Manual => {}
            }
        }
        self.step(dt, host, world, out);
        self.max_seen = self.max_seen.max(self.particles.len());
    }

    /// Spawns `n` particles at once (for `emitter_rate_manual`).
    pub fn burst(&mut self, n: usize, host: &mut dyn Host, out: &mut Vec<Spawn>) {
        for _ in 0..n.min(4096) {
            self.spawn(host, out);
        }
    }

    fn step(&mut self, dt: f32, host: &mut dyn Host, world: &dyn World, out: &mut Vec<Spawn>) {
        let e = Arc::clone(&self.effect);
        let mut particles = std::mem::take(&mut self.particles);
        for p in &mut particles {
            p.age += dt;
            let emitter_vars = self.enter(p);
            self.set_particle_vars(p);
            self.apply_curves(host);
            for x in &e.particle_update {
                self.run(host, x);
            }
            if let Some(par) = &e.parametric {
                let rel: V3 = std::array::from_fn(|i| self.eval(host, &par[i]));
                p.pos = add(p.origin, rel);
            } else if let Some(d) = &e.dynamic {
                let acc: V3 = std::array::from_fn(|i| self.eval(host, &d.acceleration[i]));
                let drag = self.eval(host, &d.drag);
                for i in 0..3 {
                    p.vel[i] += (acc[i] - drag * p.vel[i]) * dt;
                }
                let racc = self.eval(host, &d.rotation_acceleration);
                let rdrag = self.eval(host, &d.rotation_drag);
                p.spin += (racc - rdrag * p.spin) * dt;
            }
            p.rotation += p.spin * dt;
            if e.parametric.is_none() {
                let enabled = e
                    .collision
                    .as_ref()
                    .is_some_and(|c| c.enabled.as_ref().is_none_or(|x| self.run(host, x) != 0.0));
                if enabled {
                    let c = e.collision.as_ref().expect("checked");
                    for axis in 0..3 {
                        let mut next = p.pos;
                        next[axis] += p.vel[axis] * dt;
                        let mut probe = next;
                        probe[axis] += c.radius * p.vel[axis].signum();
                        if p.vel[axis] != 0.0 && world.solid(self.to_world(probe)) {
                            let speed = p.vel[axis].abs();
                            for (name, min) in &c.events {
                                if speed >= *min {
                                    self.fire(name, p.pos, host, out);
                                }
                            }
                            p.vel[axis] = -p.vel[axis] * c.restitution;
                            for o in 0..3 {
                                if o != axis {
                                    p.vel[o] *= (1.0 - c.drag * dt).clamp(0.0, 1.0);
                                }
                            }
                            if c.expire_on_contact {
                                p.dead = true;
                            }
                        } else {
                            p.pos = next;
                        }
                    }
                } else {
                    for i in 0..3 {
                        p.pos[i] += p.vel[i] * dt;
                    }
                }
            }
            if p.age >= p.lifetime {
                p.dead = true;
            }
            if let Some(k) = e.kill_plane {
                let w = self.to_world(p.pos);
                let rel = [w[0] - self.pos[0], w[1] - self.pos[1], w[2] - self.pos[2]];
                if k[0] * rel[0] + k[1] * rel[1] + k[2] * rel[2] + k[3] < 0.0 {
                    p.dead = true;
                }
            }
            if let Some(x) = &e.expire
                && self.run(host, x) != 0.0
            {
                p.dead = true;
            }
            if !e.in_blocks.is_empty() || !e.not_in_blocks.is_empty() {
                let block = world.block_id(self.to_world(p.pos));
                let named = |list: &[String]| {
                    block
                        .as_ref()
                        .is_some_and(|b| list.iter().any(|n| n.eq_ignore_ascii_case(b)))
                };
                if (!e.in_blocks.is_empty() && named(&e.in_blocks))
                    || (!e.not_in_blocks.is_empty() && !named(&e.not_in_blocks))
                {
                    p.dead = true;
                }
            }
            while let Some((t, evs)) = e.timeline.events.get(p.timeline_at) {
                if p.age < *t {
                    break;
                }
                p.timeline_at += 1;
                for n in evs {
                    self.fire(n, p.pos, host, out);
                }
            }
            let mut emitter_vars = emitter_vars;
            if p.dead && !e.on_expire.is_empty() {
                // What the expiration event sets belongs to the emitter: the next particle
                // reads it (a trail remembers where the last one ended).
                let before = self.vars.clone();
                for n in &e.on_expire {
                    self.fire(n, p.pos, host, out);
                }
                for (k, v) in &self.vars {
                    if before.get(k) != Some(v) {
                        emitter_vars.insert(*k, v.clone());
                    }
                }
            }
            self.leave(p, emitter_vars);
        }
        particles.retain(|p| !p.dead);
        self.particles = particles;
    }

    /// What to draw now.
    pub fn sprites(&mut self, host: &mut dyn Host) -> Vec<Sprite> {
        let e = Arc::clone(&self.effect);
        let mut out = Vec::with_capacity(self.particles.len());
        let mut particles = std::mem::take(&mut self.particles);
        for p in &mut particles {
            let emitter_vars = self.enter(p);
            self.set_particle_vars(p);
            self.apply_curves(host);
            for x in &e.particle_render {
                self.run(host, x);
            }
            let size = [
                self.eval(host, &e.billboard.size[0]),
                self.eval(host, &e.billboard.size[1]),
            ];
            let (tw, th) = (
                e.billboard.texture[0].max(1.0),
                e.billboard.texture[1].max(1.0),
            );
            let uv = match &e.billboard.flipbook {
                Some(f) => {
                    let base = [self.eval(host, &f.base[0]), self.eval(host, &f.base[1])];
                    let sz = [self.eval(host, &f.size[0]), self.eval(host, &f.size[1])];
                    let step = [self.eval(host, &f.step[0]), self.eval(host, &f.step[1])];
                    let max = self.eval(host, &f.max_frame).max(1.0);
                    let frame = if f.stretch {
                        p.age / p.lifetime.max(1e-4) * max
                    } else {
                        self.eval(host, &f.fps) * p.age
                    };
                    let frame = if f.looping {
                        frame.rem_euclid(max)
                    } else {
                        frame.min(max - 1.0)
                    };
                    let frame = frame.floor().max(0.0);
                    let (u, v) = (base[0] + step[0] * frame, base[1] + step[1] * frame);
                    [u / tw, v / th, (u + sz[0]) / tw, (v + sz[1]) / th]
                }
                None => {
                    let u = [
                        self.eval(host, &e.billboard.uv[0]),
                        self.eval(host, &e.billboard.uv[1]),
                    ];
                    let s = [
                        self.eval(host, &e.billboard.uv_size[0]),
                        self.eval(host, &e.billboard.uv_size[1]),
                    ];
                    [u[0] / tw, u[1] / th, (u[0] + s[0]) / tw, (u[1] + s[1]) / th]
                }
            };
            let color = match &e.tint {
                Tint::Fixed(c) => std::array::from_fn(|i| self.eval(host, &c[i])),
                Tint::Gradient { interpolant, stops } => {
                    let t = interpolant.as_ref().map_or(0.0, |i| self.run(host, i));
                    gradient(stops, t)
                }
            };
            out.push(Sprite {
                pos: self.to_world(p.pos),
                size,
                rotation: p.rotation,
                uv,
                color,
                facing: e.billboard.facing,
                velocity: {
                    let v = if e.local_rotation && e.local_position {
                        rotate_yaw(p.vel, self.yaw)
                    } else {
                        p.vel
                    };
                    match &e.billboard.direction {
                        Some(d) if !d.from_velocity || len(v) < d.min_speed => {
                            std::array::from_fn(|i| self.eval(host, &d.custom[i]))
                        }
                        _ => v,
                    }
                },
            });
            self.leave(p, emitter_vars);
        }
        self.particles = particles;
        out
    }
}

fn gradient(stops: &[(f32, [f32; 4])], t: f32) -> [f32; 4] {
    let first = stops[0];
    if t <= first.0 {
        return first.1;
    }
    let last = stops[stops.len() - 1];
    if t >= last.0 {
        return last.1;
    }
    let i = stops.partition_point(|s| s.0 <= t) - 1;
    let (a, b) = (stops[i], stops[i + 1]);
    let f = (t - a.0) / (b.0 - a.0).max(1e-6);
    std::array::from_fn(|c| a.1[c] + (b.1[c] - a.1[c]) * f)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jsonc;
    use crate::molang::NoHost;

    fn effect(components: &str) -> Arc<Effect> {
        let text = format!(
            r#"{{"format_version":"1.10.0","particle_effect":{{"description":{{"identifier":"t:p","basic_render_parameters":{{"material":"particles_alpha","texture":"textures/particle/dust"}}}},"components":{{{components}}}}}}}"#
        );
        Arc::new(Effect::parse(&jsonc::parse(&text).unwrap()).unwrap())
    }

    fn run(e: &mut Emitter, seconds: f32, dt: f32) -> Vec<Spawn> {
        let mut out = Vec::new();
        let mut t = 0.0;
        while t < seconds {
            e.update(dt, &mut NoHost, &NoWorld, &mut out);
            t += dt;
        }
        out
    }

    #[test]
    fn an_expiration_event_chain_seeds_the_effect_it_spawns() {
        let text = r#"{"format_version":"1.10.0","particle_effect":{"description":{"identifier":"t:p","basic_render_parameters":{"material":"particles_alpha","texture":"textures/particle/dust"}},
            "components":{"minecraft:emitter_rate_steady":{"spawn_rate":100,"max_particles":10},
              "minecraft:particle_lifetime_expression":{"max_lifetime":0},
              "minecraft:particle_lifetime_events":{"expiration_event":"go"},
              "minecraft:particle_appearance_billboard":{"size":[0,0]}},
            "events":{"go":{"sequence":[
              {"expression":"v.n=(v.n??0)+1;t.a=v.n*10;t.where=q.position(0);"},
              {"particle_effect":{"effect":"t:child","type":"emitter_bound","pre_effect_expression":"v.k=t.a;"}}]}}}}"#;
        let fx = Arc::new(Effect::parse(&jsonc::parse(text).unwrap()).unwrap());
        let mut e = Emitter::new(fx, 1);
        let out = run(&mut e, 0.1, 0.02);
        let k = intern("k");
        let seeds: Vec<f32> = out
            .iter()
            .filter_map(|s| match s {
                Spawn::Effect { seed, at, .. } => {
                    // Reads `q.position(`, bound: the child's origin is the world's.
                    assert_eq!(*at, [0.0; 3]);
                    Some(seed.get(&k)?.num())
                }
                _ => None,
            })
            .collect();
        assert!(seeds.len() >= 4, "{seeds:?}");
        assert_eq!(&seeds[..3], &[10.0, 20.0, 30.0]);
    }

    #[test]
    fn instant_burst_spawns_once_and_expires() {
        let fx = effect(
            r#""minecraft:emitter_rate_instant":{"num_particles":7},"minecraft:emitter_lifetime_once":{"active_time":1},
               "minecraft:emitter_shape_point":{},"minecraft:particle_lifetime_expression":{"max_lifetime":0.5},
               "minecraft:particle_appearance_billboard":{"size":[0.1,0.1],"facing_camera_mode":"lookat_xyz","uv":{"texture_width":16,"texture_height":16,"uv":[4,8],"uv_size":[8,4]}}"#,
        );
        assert_eq!(fx.texture, "textures/particle/dust");
        let mut e = Emitter::new(fx, 1);
        run(&mut e, 0.1, 0.05);
        assert_eq!(e.alive(), 7);
        let s = e.sprites(&mut NoHost);
        assert_eq!(s[0].uv, [0.25, 0.5, 0.75, 0.75]);
        assert_eq!(s[0].size, [0.1, 0.1]);
        run(&mut e, 0.6, 0.05);
        assert_eq!(e.alive(), 0);
        assert!(!e.finished());
        run(&mut e, 1.0, 0.1);
        assert!(e.finished());
    }

    #[test]
    fn a_looping_emitter_stops_when_told_to() {
        let fx = effect(
            r#""minecraft:emitter_rate_steady":{"spawn_rate":20,"max_particles":50},"minecraft:emitter_lifetime_looping":{"active_time":1},
               "minecraft:particle_lifetime_expression":{"max_lifetime":0.5}"#,
        );
        let mut e = Emitter::new(fx, 1);
        assert!(e.is_looping());
        run(&mut e, 3.0, 0.05);
        assert!(e.alive() > 0 && !e.finished());
        e.stop();
        run(&mut e, 1.0, 0.05);
        assert_eq!(e.alive(), 0);
        assert!(e.finished());
    }

    #[test]
    fn a_slow_rate_in_short_loops_still_spawns() {
        let fx = effect(
            r#""minecraft:emitter_rate_steady":{"spawn_rate":1,"max_particles":10},"minecraft:emitter_lifetime_looping":{"active_time":1},
               "minecraft:particle_lifetime_expression":{"max_lifetime":5}"#,
        );
        let mut e = Emitter::new(fx, 1);
        run(&mut e, 5.0, 1.0 / 60.0);
        assert!(e.alive() >= 3);
    }

    #[test]
    fn steady_rate_respects_the_cap() {
        let fx = effect(
            r#""minecraft:emitter_rate_steady":{"spawn_rate":20,"max_particles":5},"minecraft:emitter_lifetime_looping":{"active_time":10},
               "minecraft:particle_lifetime_expression":{"max_lifetime":100}"#,
        );
        let mut e = Emitter::new(fx, 1);
        run(&mut e, 1.0, 0.05);
        assert_eq!(e.alive(), 5);
        let fx = effect(
            r#""minecraft:emitter_rate_steady":{"spawn_rate":10,"max_particles":1000},"minecraft:particle_lifetime_expression":{"max_lifetime":100}"#,
        );
        let mut e = Emitter::new(fx, 1);
        run(&mut e, 1.0, 0.05);
        assert!((9..=11).contains(&e.alive()), "{}", e.alive());
    }

    #[test]
    fn motion_gravity_drag_and_initial_direction() {
        let fx = effect(
            r#""minecraft:emitter_rate_instant":{"num_particles":1},"minecraft:emitter_shape_point":{"offset":[1,2,3],"direction":[0,1,0]},
               "minecraft:particle_initial_speed":4,"minecraft:particle_lifetime_expression":{"max_lifetime":10},
               "minecraft:particle_motion_dynamic":{"linear_acceleration":[0,-10,0]}"#,
        );
        let mut e = Emitter::new(fx, 1);
        e.set_transform([10.0, 0.0, 0.0], 0.0);
        run(&mut e, 0.5, 0.01);
        let p = e.sprites(&mut NoHost)[0].pos;
        assert!(
            (p[0] - 11.0).abs() < 1e-3 && (p[2] - 3.0).abs() < 1e-3,
            "{p:?}"
        );
        // y = 2 + 4t - 5t^2 at t = 0.5 is 2.75 (explicit Euler is a little off).
        assert!((p[1] - 2.75).abs() < 0.1, "{p:?}");
    }

    #[test]
    fn expressions_see_particle_and_emitter_variables() {
        let fx = effect(
            r##""minecraft:emitter_rate_instant":{"num_particles":1},"minecraft:particle_lifetime_expression":{"max_lifetime":2},
               "minecraft:particle_appearance_billboard":{"size":["v.particle_age * 2","v.particle_lifetime"],"uv":{"texture_width":1,"texture_height":1,"uv":[0,0],"uv_size":[1,1]}},
               "minecraft:particle_appearance_tinting":{"color":{"interpolant":"v.particle_age/v.particle_lifetime","gradient":{"0.0":"#FFFF0000","1.0":"#FF0000FF"}}}"##,
        );
        let mut e = Emitter::new(fx, 1);
        run(&mut e, 1.0, 0.1);
        let s = &e.sprites(&mut NoHost)[0];
        assert!(
            (s.size[0] - 2.0).abs() < 0.2 && s.size[1] == 2.0,
            "{:?}",
            s.size
        );
        assert!(
            (s.color[0] - 0.5).abs() < 0.06 && (s.color[2] - 0.5).abs() < 0.06 && s.color[3] == 1.0,
            "{:?}",
            s.color
        );
    }

    #[test]
    fn flipbook_stretches_over_lifetime() {
        let fx = effect(
            r#""minecraft:emitter_rate_instant":{"num_particles":1},"minecraft:particle_lifetime_expression":{"max_lifetime":1},
               "minecraft:particle_appearance_billboard":{"size":[1,1],"uv":{"texture_width":64,"texture_height":16,
               "flipbook":{"base_UV":[0,0],"size_UV":[16,16],"step_UV":[16,0],"max_frame":4,"stretch_to_lifetime":true}}}"#,
        );
        let mut e = Emitter::new(fx, 1);
        run(&mut e, 0.6, 0.05);
        let uv = e.sprites(&mut NoHost)[0].uv;
        // 0.6 of 4 frames: frame 2.
        assert!(
            (uv[0] - 0.5).abs() < 1e-4 && (uv[2] - 0.75).abs() < 1e-4,
            "{uv:?}"
        );
    }

    #[test]
    fn each_particle_keeps_the_variables_it_set() {
        let text = r##"{"particle_effect":{"description":{"identifier":"t:v"},
          "components":{"minecraft:emitter_rate_instant":{"num_particles":3},
            "minecraft:particle_initialization":{"creation_expression":"v.mine = v.particle_random_1 * 10;","per_update_expression":"v.mine = v.mine + 1;"},
            "minecraft:particle_lifetime_expression":{"max_lifetime":5},
            "minecraft:particle_appearance_billboard":{"size":["v.mine","v.mine"],"uv":{"texture_width":1,"texture_height":1,"uv":[0,0],"uv_size":[1,1]}}}}}"##;
        let mut e = Emitter::new(
            Arc::new(Effect::parse(&jsonc::parse(text).unwrap()).unwrap()),
            7,
        );
        e.update(0.0, &mut NoHost, &NoWorld, &mut Vec::new());
        let before: Vec<f32> = e.sprites(&mut NoHost).iter().map(|s| s.size[0]).collect();
        assert!(
            before[0] != before[1] && before[1] != before[2],
            "{before:?}"
        );
        e.update(0.05, &mut NoHost, &NoWorld, &mut Vec::new());
        let after: Vec<f32> = e.sprites(&mut NoHost).iter().map(|s| s.size[0]).collect();
        // One update each: every particle is one above its own start.
        for (b, a) in before.iter().zip(&after) {
            assert!((a - b - 1.0).abs() < 1e-4, "{before:?} {after:?}");
        }
    }

    #[test]
    fn curves_initialization_kill_plane_and_entity_box() {
        // A curve over the particle's age drives the size; creation and per-update expressions
        // run; a kill plane removes what crosses it.
        let text = r##"{"particle_effect":{"description":{"identifier":"t:c"},
          "curves":{"variable.grow":{"type":"linear","nodes":[0,2,0],"input":"v.particle_age","horizontal_range":"v.particle_lifetime"}},
          "components":{"minecraft:emitter_rate_instant":{"num_particles":1},
            "minecraft:emitter_initialization":{"creation_expression":"v.base = 4;"},
            "minecraft:particle_initialization":{"per_render_expression":"v.shown = v.base + 1;"},
            "minecraft:particle_lifetime_expression":{"max_lifetime":2},
            "minecraft:particle_appearance_billboard":{"size":["v.grow","v.shown"],"uv":{"texture_width":1,"texture_height":1,"uv":[0,0],"uv_size":[1,1]}}}}}"##;
        let mut e = Emitter::new(
            Arc::new(Effect::parse(&jsonc::parse(text).unwrap()).unwrap()),
            1,
        );
        run(&mut e, 1.0, 0.1);
        let s = &e.sprites(&mut NoHost)[0];
        assert!(
            (s.size[0] - 2.0).abs() < 0.2 && s.size[1] == 5.0,
            "{:?}",
            s.size
        );

        let fx = effect(
            r#""minecraft:emitter_rate_instant":{"num_particles":1},"minecraft:particle_lifetime_expression":{"max_lifetime":10},
               "minecraft:particle_initial_speed":[0,-1,0],"minecraft:particle_motion_dynamic":{},
               "minecraft:particle_kill_plane":[0,1,0,0.5]"#,
        );
        let mut e = Emitter::new(fx, 1);
        let mut out = Vec::new();
        e.update(0.01, &mut NoHost, &NoWorld, &mut out);
        assert_eq!(e.alive(), 1);
        for _ in 0..100 {
            e.update(0.01, &mut NoHost, &NoWorld, &mut out);
        }
        assert_eq!(e.alive(), 0);

        let fx = effect(
            r#""minecraft:emitter_rate_instant":{"num_particles":50},"minecraft:emitter_shape_entity_aabb":{},
               "minecraft:particle_lifetime_expression":{"max_lifetime":10}"#,
        );
        let mut e = Emitter::new(fx, 1);
        e.update(0.0, &mut NoHost, &NoWorld, &mut Vec::new());
        for s in e.sprites(&mut NoHost) {
            assert!(
                s.pos[1] >= 0.0 && s.pos[1] <= 1.8 && s.pos[0].abs() <= 0.3,
                "{:?}",
                s.pos
            );
        }
    }

    #[test]
    fn emitter_lifetime_events_fire_on_creation_timeline_and_expiry() {
        let text = r#"{"particle_effect":{"description":{"identifier":"t:e"},"components":{
          "minecraft:emitter_rate_instant":{"num_particles":0},"minecraft:emitter_lifetime_once":{"active_time":0.5},
          "minecraft:particle_lifetime_expression":{"max_lifetime":1},
          "minecraft:emitter_lifetime_events":{"creation_event":"born","expiration_event":"bye","timeline":{"0.2":["tick"]}}},
          "events":{"born":{"sound_effect":{"event_name":"a"}},"tick":{"sound_effect":{"event_name":"b"}},"bye":{"sound_effect":{"event_name":"c"}}}}}"#;
        let mut e = Emitter::new(
            Arc::new(Effect::parse(&jsonc::parse(text).unwrap()).unwrap()),
            1,
        );
        let mut out = Vec::new();
        for _ in 0..80 {
            e.update(0.01, &mut NoHost, &NoWorld, &mut out);
        }
        let sounds: Vec<&str> = out
            .iter()
            .filter_map(|s| match s {
                Spawn::Sound { event, .. } => Some(event.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(sounds, ["a", "b", "c"]);
    }

    struct Floor;
    impl World for Floor {
        fn solid(&self, p: V3) -> bool {
            p[1] < 0.0
        }
    }

    #[test]
    fn collision_bounces_and_expires_on_contact() {
        let fx = effect(
            r#""minecraft:emitter_rate_instant":{"num_particles":1},"minecraft:emitter_shape_point":{"offset":[0,1,0]},
               "minecraft:particle_lifetime_expression":{"max_lifetime":10},
               "minecraft:particle_motion_dynamic":{"linear_acceleration":[0,-10,0]},
               "minecraft:particle_motion_collision":{"enabled":true,"coefficient_of_restitution":0.5}"#,
        );
        let mut e = Emitter::new(fx, 1);
        let mut out = Vec::new();
        for _ in 0..200 {
            e.update(0.01, &mut NoHost, &Floor, &mut out);
        }
        let y = e.sprites(&mut NoHost)[0].pos[1];
        assert!((-1e-3..1.0).contains(&y), "{y}");
        let fx = effect(
            r#""minecraft:emitter_rate_instant":{"num_particles":1},"minecraft:emitter_shape_point":{"offset":[0,0.1,0]},
               "minecraft:particle_lifetime_expression":{"max_lifetime":10},"minecraft:particle_initial_speed":[0,-5,0],
               "minecraft:particle_motion_dynamic":{},"minecraft:particle_motion_collision":{"expire_on_contact":true}"#,
        );
        let mut e = Emitter::new(fx, 1);
        for _ in 0..30 {
            e.update(0.01, &mut NoHost, &Floor, &mut out);
        }
        assert_eq!(e.alive(), 0);
    }

    #[test]
    fn shapes_place_particles() {
        let mk = |shape: &str| {
            let fx = effect(&format!(
                r#""minecraft:emitter_rate_instant":{{"num_particles":200}},{shape},"minecraft:particle_lifetime_expression":{{"max_lifetime":10}},
                   "minecraft:particle_initial_speed":1"#
            ));
            let mut e = Emitter::new(fx, 3);
            run(&mut e, 0.01, 0.01);
            e.sprites(&mut NoHost)
        };
        let sphere = mk(
            r#""minecraft:emitter_shape_sphere":{"radius":2,"surface_only":true,"direction":"outwards"}"#,
        );
        assert!(sphere.iter().all(|s| (len(s.pos) - 2.0).abs() < 0.05));
        // Moving outwards: farther than the start.
        let b = mk(
            r#""minecraft:emitter_shape_box":{"half_dimensions":[1,2,3],"direction":"outwards"}"#,
        );
        assert!(
            b.iter()
                .all(|s| s.pos[0].abs() <= 1.1 && s.pos[1].abs() <= 2.1 && s.pos[2].abs() <= 3.1)
        );
        let d = mk(
            r#""minecraft:emitter_shape_disc":{"radius":1,"plane_normal":"y","direction":[0,1,0]}"#,
        );
        assert!(
            d.iter()
                .all(|s| s.pos[1] > 0.0 && (s.pos[0].powi(2) + s.pos[2].powi(2)).sqrt() <= 1.05)
        );
    }

    #[test]
    fn local_space_follows_the_emitter() {
        let fx = effect(
            r#""minecraft:emitter_rate_instant":{"num_particles":1},"minecraft:emitter_shape_point":{"offset":[1,0,0]},
               "minecraft:emitter_local_space":{"position":true,"rotation":false},"minecraft:particle_lifetime_expression":{"max_lifetime":10}"#,
        );
        let mut e = Emitter::new(fx, 1);
        e.set_transform([5.0, 0.0, 0.0], 0.0);
        run(&mut e, 0.02, 0.01);
        e.set_transform([8.0, 0.0, 0.0], 0.0);
        assert!((e.sprites(&mut NoHost)[0].pos[0] - 9.0).abs() < 1e-3);
    }

    #[test]
    fn lifetime_events_chain_effects_and_sounds() {
        let v = jsonc::parse(
            r#"{"particle_effect":{"description":{"identifier":"t:e"},"components":{
              "minecraft:emitter_rate_instant":{"num_particles":1},"minecraft:particle_lifetime_expression":{"max_lifetime":0.2},
              "minecraft:particle_lifetime_events":{"creation_event":"born","expiration_event":"bye"}},
              "events":{"born":{"sound_effect":{"event_name":"pop"}},
                        "bye":{"particle_effect":{"effect":"t:other","type":"emitter"},"sequence":[{"event":"born"}]}}}}"#,
        )
        .unwrap();
        let fx = Arc::new(Effect::parse(&v).unwrap());
        let mut e = Emitter::new(fx, 1);
        let out = run(&mut e, 0.5, 0.05);
        assert!(out.contains(&Spawn::Sound {
            event: "pop".into(),
            at: [0.0; 3]
        }));
        assert!(
            out.iter()
                .any(|s| matches!(s, Spawn::Effect { effect, .. } if effect == "t:other"))
        );
        assert_eq!(
            out.iter()
                .filter(|s| matches!(s, Spawn::Sound { .. }))
                .count(),
            2
        );
    }

    #[test]
    fn looping_emitter_restarts_its_burst() {
        let fx = effect(
            r#""minecraft:emitter_rate_instant":{"num_particles":3},"minecraft:emitter_lifetime_looping":{"active_time":0.2,"sleep_time":0.3},
               "minecraft:particle_lifetime_expression":{"max_lifetime":0.12}"#,
        );
        let mut e = Emitter::new(fx, 1);
        let mut bursts = 0;
        let mut before = 0;
        for _ in 0..30 {
            let mut out = Vec::new();
            e.update(0.05, &mut NoHost, &NoWorld, &mut out);
            if e.alive() == 3 && before == 0 {
                bursts += 1;
            }
            before = e.alive();
        }
        // 1.5 s with a 0.5 s cycle: three bursts.
        assert_eq!(bursts, 3);
        assert!(!e.finished());
    }

    #[test]
    fn hex_colours() {
        assert_eq!(hex("#FF00FF00"), Some([0.0, 1.0, 0.0, 1.0]));
        assert_eq!(hex("#00FF00").unwrap()[3], 1.0);
        assert_eq!(hex("nope"), None);
    }
}
