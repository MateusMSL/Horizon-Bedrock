//! Bedrock resource packs' look for mobs: a client entity's geometry, textures, animations and
//! render controllers ([`horizon_bedrock`]) over a mob the Java logic runs.
//!
//! Only what is drawn and heard comes from the pack. The mob's state is read, never written:
//! [`MobHost`] answers a pack's `query.*` from the `Mob` (its name tag, whether it is a baby,
//! how fast it walks...), and nothing in the pack reaches back into the game. Without a pack
//! the bundle is empty and every mob keeps its vanilla model.
//!
//! The pack's files reach this module through the resource root (`assets/horizon_bedrock/`,
//! written by `horizon_bedrock::convert`); its textures are in the entity atlas as
//! `entity/bedrock/...`.

use super::{Mob, Part, Polygon};
use horizon_bedrock::{
    animation::Pose,
    bundle::Bundle,
    convert::atlas_name,
    entity::{Assets, Emit, Instance, Layer},
    geometry::{self, Geometry},
    molang::{Host, Value},
    particle::{Emitter, Spawn, Sprite, World},
};
use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex, PoisonError},
    time::Instant,
};

/// A model baked for the game with the names of its parts.
pub struct Built {
    pub part: Arc<Part>,
    /// Every part's name (lower case), for hiding bones.
    pub names: Vec<String>,
    pub geometry: String,
    /// The model's texture is one texel (a flat colour): every face samples that texel.
    pub single_texel: bool,
}

/// What one mob draws this frame.
pub struct MobFrame {
    pub layers: Vec<DrawLayer>,
    pub pose: Pose,
    pub scale: f32,
    /// `scalex` / `scaley` / `scalez`, on top of `scale`.
    pub scale_axes: [f32; 3],
    /// How the pack has what the mob holds drawn.
    pub held: Held,
    /// What the mob holds or wears that the pack dresses (a bow in hand, a helmet).
    pub attached: Vec<Attached>,
}

/// `hide_held_items`, `held_item_scale` and `held_item_ignores_lighting` of a frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Held {
    pub hidden: bool,
    pub scale: f32,
    pub full_bright: bool,
    /// The pack dresses the item in the main hand (an attachable draws it): the game's own
    /// model of it stays out.
    pub main_replaced: bool,
    /// The same for the off hand.
    pub off_replaced: bool,
}

impl Default for Held {
    fn default() -> Held {
        Held {
            hidden: false,
            scale: 1.0,
            full_bright: false,
            main_replaced: false,
            off_replaced: false,
        }
    }
}

/// A held or worn item's look: its layers, and the bone of the holder each of its bones follows.
pub struct Attached {
    pub slot: &'static str,
    /// The item (`minecraft:clock`, lower case), for the debug file's `hide_item_parts`.
    pub item: String,
    pub layers: Vec<DrawLayer>,
    pub pose: Pose,
    /// The attachable's bone (lower case) to the holder's bone it follows.
    pub binds: HashMap<String, String>,
    /// What its animations set off this frame (an amethyst shard's sparkle, an ender pearl's
    /// swirl, a sound), each with where on the holder it starts (blocks from the feet, in the
    /// holder model's axes).
    pub emits: Vec<(Emit, [f64; 3])>,
}

pub struct DrawLayer {
    pub built: Arc<Built>,
    /// The texture's atlas name (`entity/bedrock/...`).
    pub texture: String,
    /// The vanilla Java texture standing in when the pack does not ship `texture`.
    pub fallback: Option<String>,
    pub layer: Layer,
}

/// Something the frames set off, for the game to play or spawn.
#[derive(Clone, Debug, PartialEq)]
pub enum Emitted {
    Sound {
        event: String,
        at: [f64; 3],
    },
    Particle {
        effect: String,
        at: [f64; 3],
        yaw: f32,
        baby: bool,
        name: Option<String>,
        size: [f32; 2],
        /// The mob the effect is attached to, which it follows.
        follow: Option<Follow>,
        init: Init,
    },
}

/// What a new emitter starts with besides its effect: the pack's `pre_effect_script` and the
/// variables an event's `pre_effect_expression` gave it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Init {
    pub script: Option<String>,
    pub seed: horizon_bedrock::molang::Vars,
}

/// Where on a mob an effect is attached.
#[derive(Clone, Debug, PartialEq)]
pub struct Follow {
    pub mob: u32,
    /// The locator's name, which with the effect tells an effect already running from a new one.
    pub locator: Option<String>,
    /// From the mob's feet in the model's own axes (x, y up, z with the front at -z), in blocks.
    pub offset: [f64; 3],
}

/// A model-space offset of a mob standing at `pos` and turned `yaw` degrees, in the world: as the
/// model is drawn (`LivingEntityRenderer`: `rotateY(180 - yaw)`, `scale(-1, -1, 1)`, the Bedrock
/// point in Java's frame), so the front (-z) is ahead and the right side (-x) is on the right.
/// The turn and the mirror together are their own inverse.
fn attach_point(pos: [f64; 3], offset: [f64; 3], yaw: f32) -> [f64; 3] {
    let [x, y, z] = model_to_world(offset, yaw);
    [pos[0] + x, pos[1] + y, pos[2] + z]
}

/// [`attach_point`] from the feet, without the position.
fn model_to_world(offset: [f64; 3], yaw: f32) -> [f64; 3] {
    let yaw = f64::from(yaw).to_radians();
    let [x, y, z] = offset;
    [
        x * yaw.cos() + z * yaw.sin(),
        y,
        x * yaw.sin() - z * yaw.cos(),
    ]
}

/// An offset on the first-person view model (`render::first_person_item`) carried to where it
/// shows in the world, as an offset from the player's feet in the body's axes (what [`Follow`]
/// keeps). The view model stands with its feet `eye` below the camera, turned half round from
/// the body (+z behind) and tilted with the look's pitch about the eye; a particle set off by the
/// item in hand starts there, before the camera, as Bedrock shows it.
fn first_person_offset(m: &Mob, offset: [f64; 3]) -> [f64; 3] {
    let eye = f64::from(
        super::render::first_person_eye().unwrap_or(m.kind.eye_height as f32 * m.model_scale()),
    );
    let (_, pitch) = camera_rotation();
    let yaw = m.body_yaw;
    let [x, y, z] = offset;
    let q = attach_point([0.0, -eye, 0.0], [-x, y, -z], yaw);
    let level = glam::DQuat::from_rotation_y(std::f64::consts::PI - f64::from(yaw).to_radians());
    let tilt = level * glam::DQuat::from_rotation_x(-f64::from(pitch).to_radians()) * level.inverse();
    let world = tilt * glam::DVec3::from_array(q) + glam::DVec3::new(0.0, eye, 0.0);
    // Back into the body's axes (the turn and mirror undo themselves).
    model_to_world(world.to_array(), yaw)
}

struct MobState {
    instance: Instance,
    /// The mob's age in ticks (with the partial tick) at the last update.
    time: f32,
    frame: Option<Arc<MobFrame>>,
    seen: Instant,
    /// When `HORIZON_BEDROCK_DUMP` last logged this mob (its age in ticks).
    dumped: f32,
    /// The mob as the last frame saw it: what an effect that starts to follow it asks of it
    /// before the next frame (a trail's first segment would run from the world's origin).
    live: Option<Live>,
    water: WaterEntry,
}

/// A held item's running animations.
struct AttState {
    /// The attachable it runs.
    id: String,
    instance: Instance,
    time: f32,
    seen: Instant,
    /// The holder's bone its bones bound to last frame.
    bound_to: Option<String>,
}

struct Active {
    /// When an animation last asked for this effect.
    requested: Instant,
    /// Set off by a controller state, which asks for it every frame it lasts.
    held: bool,
    emitter: Emitter,
    host: Snapshot,
    follow: Option<Follow>,
    /// The `pre_effect_script` it started with: tells two of one effect on one mob apart.
    script: Option<String>,
}

/// A mob's facts, kept for a particle effect that outlives the frame that asked for it.
#[derive(Clone, Default)]
struct Snapshot {
    baby: bool,
    name: Option<String>,
    age: f32,
    /// Width and height of the mob's box, for `emitter_shape_entity_aabb`.
    size: [f32; 2],
    /// The mob as it is now, while an effect follows one.
    live: Option<Live>,
}

/// What an effect that follows a mob asks of it each frame.
#[derive(Clone, Default)]
struct Live {
    position: [f64; 3],
    /// Blocks moved in the last tick.
    delta: [f64; 3],
    body_yaw: f32,
    /// The head's yaw against the body's, and its pitch.
    target: [f32; 2],
    local_player: bool,
    sneaking: bool,
    gliding: bool,
    from_camera: f32,
    /// Blocks a second up (`q.vertical_speed`): a splash reads how fast the mob hit the water.
    vertical_speed: f32,
}

/// An effect's sprites for one frame.
pub struct Drawn {
    /// The atlas name of its texture.
    pub texture: Arc<str>,
    pub sprites: Vec<Sprite>,
    /// Takes the light where it is (`particle_appearance_lighting`).
    pub lit: bool,
    /// Its material adds its colour to what is behind (`particles_add`), not covers it.
    pub additive: bool,
}

/// The blocks around the effects, as the game knows them: what a particle runs into
/// (`particle_motion_collision`) and the block it is in (`particle_expire_if_in_blocks`).
pub trait Ground {
    /// Whether the point is inside a block that stops a particle.
    fn solid(&self, p: [f64; 3]) -> bool;
    /// The Bedrock name of the block at the point (`minecraft:water` for water).
    fn block_id(&self, p: [f64; 3]) -> Option<String>;
}

/// The level an effect rides in: the size of the mob that carries it and the blocks around.
struct Carrier<'a>([f32; 2], Option<&'a dyn Ground>);

impl World for Carrier<'_> {
    fn entity_size(&self) -> [f32; 2] {
        if self.0[0] > 0.0 && self.0[1] > 0.0 {
            self.0
        } else {
            [0.6, 1.8]
        }
    }
    fn solid(&self, p: [f32; 3]) -> bool {
        self.1.is_some_and(|g| g.solid(p.map(f64::from)))
    }
    fn block_id(&self, p: [f32; 3]) -> Option<String> {
        self.1.and_then(|g| g.block_id(p.map(f64::from)))
    }
}

/// The size the game gives a mob's model before any `scale` script: the vanilla rabbit's script
/// reads `1 / 0.6` (`1 / 0.4` for a baby) to cancel it, and a pack that scales the rabbit writes
/// its own factor on top of the same.
fn builtin_scale(id: &str, baby: bool) -> f32 {
    match id {
        "minecraft:rabbit" if baby => 0.4,
        "minecraft:rabbit" => 0.6,
        _ => 1.0,
    }
}

/// `q.vertical_speed`: blocks a second up. Gravity keeps pulling a mob on the ground (-0.0784 a
/// tick, -1.57 a second) with nothing moving it: that is not a fall to the packs. A passenger
/// rides, it does not fall.
fn vertical_speed(m: &Mob) -> f32 {
    if m.extra.passenger {
        return 0.0;
    }
    let (v, d) = (m.velocity.y, m.position.y - m.previous.y);
    let speed = if m.on_ground && v < 0.0 {
        d
    } else if v.abs() >= d.abs() {
        v
    } else {
        d
    };
    speed as f32 * 20.0
}

/// A mob's box as `[width, height]`.
fn box_size(m: &Mob) -> [f32; 2] {
    let (w, h, _) = m.dims();
    [w as f32, h as f32]
}

impl Host for Snapshot {
    fn query(&mut self, name: &str, args: &[Value]) -> Value {
        match name {
            "is_baby" => Value::bool(self.baby),
            "is_name_any" => Value::bool(name_matches(self.name.as_deref(), args)),
            "life_time" => Value::Num(self.age / 20.0),
            _ => match &self.live {
                Some(m) => live_query(m, name, args).unwrap_or_else(|| common_query(name, args)),
                None => common_query(name, args),
            },
        }
    }
}

fn live_query(m: &Live, name: &str, args: &[Value]) -> Option<Value> {
    let axis = args.first().map_or(0.0, Value::num).max(0.0) as usize;
    Some(match name {
        "position" => Value::Num(m.position[axis.min(2)] as f32),
        "position_delta" => Value::Num(m.delta[axis.min(2)] as f32),
        "body_y_rotation" => Value::Num(m.body_yaw),
        "target_y_rotation" => Value::Num(m.target[0]),
        "target_x_rotation" => Value::Num(m.target[1]),
        "is_local_player" => Value::bool(m.local_player),
        "is_sneaking" | "is_crouching" => Value::bool(m.sneaking),
        "is_gliding" => Value::bool(m.gliding),
        "distance_from_camera" => Value::Num(m.from_camera),
        "vertical_speed" => Value::Num(m.vertical_speed),
        _ => return None,
    })
}

fn name_matches(name: Option<&str>, args: &[Value]) -> bool {
    let Some(name) = name else { return false };
    args.iter()
        .any(|a| matches!(a, Value::Str(s) if &**s == name))
}

/// What a pack may ask about the world around a mob, sampled in the mob's tick (the render has
/// no world to ask): the blocks near it, its biome, the time and the weather.
#[derive(Debug, Default)]
pub struct Surroundings {
    /// Index into `palette` of each block of the box centred on the mob's block, x fastest, then
    /// z, then y from `below` blocks under it.
    cells: Vec<u16>,
    /// Block names (`minecraft:stone`), each once.
    palette: Vec<Box<str>>,
    /// Blocks sampled each side on x and z, below and above the mob's block.
    side: i32,
    below: i32,
    above: i32,
    biome: Box<str>,
    day_time: i64,
    raining: bool,
    thundering: bool,
}

impl Surroundings {
    /// The name of the block `(dx, dy, dz)` from the mob's, when the sample reaches it.
    fn block(&self, dx: i32, dy: i32, dz: i32) -> Option<&str> {
        if dx.abs() > self.side || dz.abs() > self.side || dy < -self.below || dy > self.above {
            return None;
        }
        let width = 2 * self.side + 1;
        let i = ((dy + self.below) * width + (dz + self.side)) * width + (dx + self.side);
        let cell = *self.cells.get(i as usize)?;
        self.palette.get(cell as usize).map(|b| &**b)
    }
}

const REACH: i32 = 2;
/// What the local player's sample reaches: the packs' fall prediction (the pose a body takes in
/// the air) walks the path it will fall along block by block, to where it lands.
const PLAYER_SIDE: i32 = 6;
const PLAYER_BELOW: i32 = 48;

/// Set the first time a pack asks something the sample answers.
static WANTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// The game's block tags, kept from the first sample, for `relative_block_has_any_tag`.
static BLOCK_TAGS: std::sync::OnceLock<HashMap<String, std::collections::HashSet<String>>> =
    std::sync::OnceLock::new();

/// Whether any pack expression asked about the world: ticks sample only then.
pub fn wants_surroundings() -> bool {
    WANTED.load(std::sync::atomic::Ordering::Relaxed)
}

/// The sample of the world around `m`.
pub fn sample_surroundings(m: &Mob, w: &super::MobWorld) -> Surroundings {
    let _ = BLOCK_TAGS.set(w.block_tags.clone());
    let at = [
        m.position.x.floor() as i32,
        m.position.y.floor() as i32,
        m.position.z.floor() as i32,
    ];
    let (side, below, above) = if m.kind.id == "minecraft:player" {
        (PLAYER_SIDE, PLAYER_BELOW, REACH)
    } else {
        (REACH, REACH, REACH)
    };
    let mut palette: Vec<Box<str>> = Vec::new();
    let mut known: HashMap<&str, u16> = HashMap::new();
    let mut cells = Vec::with_capacity(((2 * side + 1) * (2 * side + 1) * (below + above + 1)) as usize);
    for dy in -below..=above {
        for dz in -side..=side {
            for dx in -side..=side {
                let state = w.world.block(at[0] + dx, at[1] + dy, at[2] + dz);
                let name: &str = &w.facts.get(state).name;
                let cell = *known.entry(name).or_insert_with(|| {
                    palette.push(Box::from(name));
                    (palette.len() - 1) as u16
                });
                cells.push(cell);
            }
        }
    }
    let biome = w
        .world
        .chunks
        .get(&(at[0] >> 4, at[2] >> 4))
        .map(|c| {
            w.biomes
                .get(c.biome(at[0], at[1], at[2]))
                .name
                .as_str()
                .into()
        })
        .unwrap_or_default();
    Surroundings {
        cells,
        palette,
        side,
        below,
        above,
        biome,
        day_time: w.day_time,
        raining: w.raining,
        thundering: w.thundering,
    }
}

/// Whether `name` has `tag` (a block tag of the game, with or without its namespace), or the
/// Bedrock tag the packs ask by: the mineable tags of the tools, `water`, `not_feature_replaceable`
/// and the rest, each the game's own tags or names that mean the same.
static PACK_LIES_DOWN: std::sync::Mutex<Vec<u32>> = std::sync::Mutex::new(Vec::new());

fn set_pack_lies_down(id: u32, on: bool) {
    let mut ids = lock(&PACK_LIES_DOWN);
    match (ids.iter().position(|i| *i == id), on) {
        (None, true) => ids.push(id),
        (Some(at), false) => {
            ids.swap_remove(at);
        }
        _ => {}
    }
}

/// The pack's own animations lay this player's body down (`root` turned 15 degrees or more
/// over): the swimming pose's turn is theirs, not the game's.
pub fn pack_lies_down(id: u32) -> bool {
    lock(&PACK_LIES_DOWN).contains(&id)
}

static PACK_GLIDES: std::sync::Mutex<Vec<u32>> = std::sync::Mutex::new(Vec::new());

fn set_pack_glides(id: u32) {
    let mut ids = lock(&PACK_GLIDES);
    if !ids.contains(&id) {
        ids.push(id);
    }
}

/// The pack's gliding animation turns this player's `root` by the look pitch itself, on top of
/// the game's own glide pitch: the game's counts, the pack's `root` turn is dropped.
pub fn pack_glides(id: u32) -> bool {
    lock(&PACK_GLIDES).contains(&id)
}

fn block_has_tag(name: &str, tag: &str) -> bool {
    let Some(tags) = BLOCK_TAGS.get() else {
        return false;
    };
    let java = |t: &str| {
        let full = if t.contains(':') { t.to_owned() } else { format!("minecraft:{t}") };
        tags.get(full.as_str()).is_some_and(|set| set.contains(name))
    };
    let air = matches!(name, "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air");
    let tag = tag.to_ascii_lowercase();
    match tag.trim_start_matches("minecraft:") {
        "water" => matches!(
            name,
            "minecraft:water" | "minecraft:bubble_column" | "minecraft:kelp"
                | "minecraft:kelp_plant" | "minecraft:seagrass" | "minecraft:tall_seagrass"
        ),
        "lava" => name == "minecraft:lava",
        "is_pickaxe_item_destructible" | "iron_tier_destructible" | "stone_tier_destructible" => {
            java("mineable/pickaxe")
        }
        "is_axe_item_destructible" => java("mineable/axe"),
        "is_shovel_item_destructible" => java("mineable/shovel"),
        "is_hoe_item_destructible" => java("mineable/hoe"),
        "is_shears_item_destructible" => {
            java("leaves") || java("wool") || name == "minecraft:cobweb"
        }
        // What world generation does not replace: every block but the air, the fluids and what
        // the game itself marks replaceable (grass, flowers, snow layers).
        "not_feature_replaceable" => {
            !air && !java("replaceable") && !matches!(name, "minecraft:water" | "minecraft:lava")
        }
        "wood" => java("logs") || java("planks"),
        "snow" => matches!(name, "minecraft:snow" | "minecraft:snow_block" | "minecraft:powder_snow"),
        "rail" => java("rails"),
        "trapdoors" => java("trapdoors"),
        "dirt" => java("dirt"),
        "stone" => java("base_stone_overworld") || java("stone_bricks"),
        other => java(other),
    }
}

/// The answers that come from the sampled world.
fn world_query(m: &Mob, name: &str, args: &[Value]) -> Option<Value> {
    let asks = matches!(
        name,
        "relative_block_has_any_tag"
            | "entity_biome_has_any_identifier"
            | "time_of_day"
            | "moon_phase"
            | "is_raining"
            | "is_thundering"
    );
    if !asks {
        return None;
    }
    WANTED.store(true, std::sync::atomic::Ordering::Relaxed);
    let Some(s) = m.extra.surroundings.as_deref() else {
        return Some(Value::Num(0.0));
    };
    Some(match name {
        "relative_block_has_any_tag" => {
            let n = |i: usize| args.get(i).map_or(0.0, Value::num).floor() as i32;
            let (dx, dy, dz) = (n(0), n(1), n(2));
            let block = s.block(dx, dy, dz).unwrap_or("");
            Value::bool(args.iter().skip(3).any(|a| match a {
                Value::Str(t) => block_has_tag(block, t),
                _ => false,
            }))
        }
        "entity_biome_has_any_identifier" => Value::bool(args.iter().any(|a| match a {
            Value::Str(t) => {
                let t = t.trim_start_matches("minecraft:");
                let b = s.biome.trim_start_matches("minecraft:");
                t.eq_ignore_ascii_case(b)
            }
            _ => false,
        })),
        // Bedrock: midnight 0.0, sunrise 0.25, noon 0.5. Java's tick 6000 is noon, 0 sunrise.
        "time_of_day" => Value::Num(((s.day_time + 6000).rem_euclid(24000)) as f32 / 24000.0),
        "moon_phase" => Value::Num(s.day_time.div_euclid(24000).rem_euclid(8) as f32),
        "is_raining" => Value::bool(s.raining),
        _ => Value::bool(s.thundering),
    })
}

/// Whether the pack dresses players: its player animations run on the game's own humanoid
/// model and skin. On unless `HORIZON_BEDROCK_PLAYERS` is `0` or `off`.
fn players_enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        !std::env::var("HORIZON_BEDROCK_PLAYERS")
            .is_ok_and(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "0" | "off" | "false"))
    })
}

/// A player or mannequin: drawn on the game's skin.
pub fn is_avatar(id: &str) -> bool {
    matches!(id, "minecraft:player" | "minecraft:mannequin")
}

static COMPASS_TARGET: std::sync::Mutex<Option<[f64; 2]>> = std::sync::Mutex::new(None);

/// Where a spawn compass points (x and z of the world spawn), or None where it spins (another
/// dimension): for `query.get_animation_frame` of a held compass.
pub fn set_compass_target(target: Option<[f64; 2]>) {
    *COMPASS_TARGET.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = target;
}

/// `CompassAngleState`: the needle's turn, 0..1 (0 points ahead of the holder), toward
/// `target` for a holder at `pos` facing `yaw` degrees; spinning without one.
fn compass_turn(pos: [f64; 2], yaw: f32, target: Option<[f64; 2]>) -> f64 {
    let Some(t) = target else {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0.0, |d| d.as_secs_f64());
        return (secs * 0.8).rem_euclid(1.0);
    };
    let facing = f64::from(yaw).rem_euclid(360.0) / 360.0;
    let to_target = (t[1] - pos[1]).atan2(t[0] - pos[0]) / std::f64::consts::TAU;
    (0.5 - (facing - 0.25 - to_target)).rem_euclid(1.0)
}

/// The compass texture's frame for a needle turn (the game's `compass` model: turn 0, ahead,
/// is `compass_16`, each 1/32 turn the next frame).
fn compass_frame(turn: f64) -> f32 {
    (((turn * 32.0).round() as i64 + 16).rem_euclid(32)) as f32
}

/// The clock's frame at `day` ticks: the sun's angle (`DimensionType.timeOfDay`, 0 at noon) in 64
/// steps, as the item's own texture turns. Noon is frame 0, the sun at the top of the dial;
/// midnight is 32.
fn clock_frame(day: i64) -> f32 {
    let d = (day.rem_euclid(24000) as f64 / 24000.0 - 0.25).rem_euclid(1.0);
    let eased = 0.5 - (d * std::f64::consts::PI).cos() / 2.0;
    let angle = (d * 2.0 + eased) / 3.0;
    ((angle * 64.0).floor() as i64).rem_euclid(64) as f32
}

/// The frame of a held clock or compass, which a pack reads as `query.get_animation_frame`:
/// the clock's 64 frames by the time of day, the compass's 32 by the needle's turn (toward the
/// world spawn; a recovery compass, whose death point the game does not keep here, spins).
fn item_frame(m: &Mob, item: &str) -> f32 {
    let day = m.extra.surroundings.as_deref().map_or(0, |s| s.day_time);
    let pos = [m.position.x, m.position.z];
    match item.trim_start_matches("minecraft:") {
        "clock" => clock_frame(day),
        "compass" => {
            let target = *COMPASS_TARGET.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            compass_frame(compass_turn(pos, m.yaw, target))
        }
        "recovery_compass" => compass_frame(compass_turn(pos, m.yaw, None)),
        _ => 0.0,
    }
}

/// Answers shared by every host: the parts of the world the game does not tell a pack.
fn common_query(name: &str, args: &[Value]) -> Value {
    match name {
        "is_in_ui" | "is_attached" | "is_spectator" | "is_local_player" | "is_emoting"
        | "vertical_speed" | "mark_variant" | "get_animation_frame" | "is_in_water" | "is_swimming" => {
            Value::Num(0.0)
        }
        "is_pack_setting_enabled" => Value::bool(
            matches!(args.first(), Some(Value::Str(n)) if horizon_bedrock::settings::is_enabled(n)),
        ),
        "is_pack_setting_selected" => Value::bool(matches!(
            (args.first(), args.get(1)),
            (Some(Value::Str(n)), Some(Value::Str(o))) if horizon_bedrock::settings::is_selected(n, o)
        )),
        "graphics_mode_is_any" => Value::bool(
            args.iter()
                .any(|a| matches!(a, Value::Str(s) if s.eq_ignore_ascii_case("fancy"))),
        ),
        "in_range" => {
            let n = |i: usize| args.get(i).map_or(0.0, Value::num);
            Value::bool(n(0) >= n(1) && n(0) <= n(2))
        }
        "is_owner_identifier_any" => Value::Num(0.0),
        // What the game keeps no equivalent of (spear timings, vibrations, timers, the skin
        // kind, the particle taken from a block): 0.
        "timer_flag_1"
        | "timer_flag_2"
        | "timer_flag_3"
        | "structural_integrity"
        | "key_frame_lerp_time"
        | "surface_particle_texture_size"
        | "surface_particle_texture_coordinate"
        | "surface_particle_color"
        | "kinetic_weapon_delay"
        | "kinetic_weapon_dismount_duration"
        | "kinetic_weapon_knockback_duration"
        | "kinetic_weapon_damage_duration"
        | "ticks_since_last_kinetic_weapon_hit"
        | "bone_aabb"
        | "tail_angle"
        | "show_bottom"
        | "heartbeat_phase"
        | "cape_flap_amount"
        | "life_span"
        | "spellcolor"
        | "get_root_locator_offset"
        | "texture_frame_index"
        | "is_persona_or_premium_skin"
        | "is_attached_to_entity"
        | "time_since_last_vibration_detection"
        // Logic the game keeps and a resource pack only reads for flavour: 0.
        | "actor_count"
        | "anger_level"
        | "can_climb"
        | "can_dash"
        | "can_fly"
        | "can_power_jump"
        | "combine_entities"
        | "cooldown_time"
        | "cooldown_time_remaining"
        | "is_cooldown_category"
        | "effect_emitter_count"
        | "effect_particle_count"
        | "had_component_group"
        | "has_any_family"
        | "heartbeat_interval"
        | "heightmap"
        | "is_avoiding_block"
        | "is_breathing"
        | "is_bribed"
        | "is_critical"
        | "is_enchanted"
        | "is_idling"
        | "is_ignited"
        | "is_illager_captain"
        | "is_laying_down"
        | "is_lingering"
        | "is_orphaned"
        | "is_ram_attacking"
        | "is_rising"
        | "is_rolling"
        | "is_scenting"
        | "is_selected_item"
        | "is_silent"
        | "is_sneezing"
        | "is_stackable"
        | "is_transforming"
        | "item_in_use_duration"
        | "item_max_use_duration"
        | "last_hit_by_player"
        | "log"
        | "max_durability"
        | "max_trade_tier"
        | "noise"
        | "out_of_control"
        | "player_level"
        | "remaining_durability"
        | "rider_body_x_rotation"
        | "rider_body_y_rotation"
        | "rider_head_x_rotation"
        | "rider_head_y_rotation"
        | "scoreboard"
        | "shield_blocking_bob"
        | "get_actor_info_id"
        | "get_locator_offset"
        | "bone_orientation_matrix"
        | "block_face"
        | "block_state"
        | "has_block_state"
        | "cardinal_facing"
        | "cardinal_facing_2d"
        | "cardinal_player_facing"
        | "block_has_all_tags"
        | "block_has_any_tag"
        | "block_neighbor_has_all_tags"
        | "block_neighbor_has_any_tag"
        | "relative_block_has_all_tags"
        | "entity_biome_has_all_tags"
        | "entity_biome_has_any_tags"
        | "has_all_biome_tags"
        | "has_any_biome_tags"
        | "has_biome_tag"
        | "equipped_item_all_tags"
        | "equipped_item_is_attachable"
        | "all_tags"
        | "any_tag"
        | "get_pack_setting" => Value::Num(0.0),
        "item_slot_to_bone_name" | "get_equipped_item_name" => Value::Str("".into()),
        other => {
            note_unanswered(other);
            Value::Null
        }
    }
}

/// The engine's build, logged once with `HORIZON_BEDROCK_DEBUG`: tells whether the game runs the
/// files the last overlay carried.
pub const ENGINE_BUILD: &str = "v234-stencil-replay-bias";

/// `HORIZON_BEDROCK_DUMP=skeleton,creeper`: the kinds (by part of the id) whose state is logged
/// once a second while within twelve blocks of the camera.
fn dump_wanted(kind: &str) -> bool {
    super::bedrock_debug::settings().dumps(kind)
}

/// One line of what a mob's pack is doing: what the game answers, what runs, the bones moved
/// and what is drawn.
/// `trace_rider`: the bones of a mount and of its rider, every frame.
fn trace_pose(m: &Mob, alpha: f32, time: f32, dt: f32, frame: &horizon_bedrock::entity::Frame) {
    use std::fmt::Write;
    let settings = super::bedrock_debug::settings();
    if !settings.trace_rider {
        return;
    }
    let wanted: Vec<String> = if settings.dump_bones.is_empty() {
        ["root", "waist", "body", "head"].map(String::from).to_vec()
    } else {
        settings.dump_bones.iter().map(|n| n.to_ascii_lowercase()).collect()
    };
    let mut bones: Vec<_> = frame
        .pose
        .bones
        .iter()
        .filter(|(name, _)| {
            let name = name.to_ascii_lowercase();
            wanted.iter().any(|w| name.contains(w.as_str()))
        })
        .collect();
    bones.sort_by(|a, b| a.0.cmp(b.0));
    let mut b = String::new();
    for (name, p) in bones.iter().take(16) {
        let _ = write!(
            b,
            " {name}(rot={:?} pos={:?})",
            p.rotation.map(|v| (v * 100.0).round() / 100.0),
            p.position.map(|v| (v * 100.0).round() / 100.0),
        );
    }
    super::bedrock_debug::trace(&format!(
        "rider-trace-pose {}#{} age={} a={alpha:.3} time={time:.3} dt={dt:.4}{b}",
        m.kind.id, m.id, m.age
    ));
}

fn dump_line(m: &Mob, host: &mut MobHost, frame: &horizon_bedrock::entity::Frame, running: &str) -> String {
    use std::fmt::Write;
    let mut q = String::new();
    for name in [
        "has_target",
        "facing_target_to_range_attack",
        "is_using_item",
        "target_x_rotation",
        "target_y_rotation",
        "modified_move_speed",
        "modified_distance_moved",
        "ground_speed",
        "vertical_speed",
        "is_swimming",
        "is_in_water",
        "is_sprinting",
        "is_on_ground",
        "swell_amount",
        "is_powered",
        "is_saddled",
        "is_baby",
        "is_tamed",
        "has_rider",
        "is_riding",
        "is_charged",
        "is_angry",
        "is_sheared",
        "is_sleeping",
        "is_sitting",
        "is_alive",
        "is_on_fire",
        "is_attached",
        "variant",
        "mark_variant",
        "skin_id",
        "body_x_rotation",
        "body_y_rotation",
        "yaw_speed",
        "life_time",
    ] {
        let v = host.query(name, &[]);
        let _ = write!(q, " {name}={}", v.num());
    }
    let _ = write!(
        q,
        " main_hand={:?} body={:?} using_item={} hurt={} death={}",
        m.extra.main_hand,
        m.extra.body,
        m.extra.using_item,
        m.hurt_time,
        m.death_time
    );
    // Where the mob is: the position the packs differentiate for their own speed.
    let _ = write!(
        q,
        " pos=[{:.4},{:.4},{:.4}] prev=[{:.4},{:.4},{:.4}] vel=[{:.4},{:.4},{:.4}] alpha={:.3} dt={:.4} yaw_prev={:.2} yaw_cur={:.2} yaw_draw={:.2}",
        m.position.x,
        m.position.y,
        m.position.z,
        m.previous.x,
        m.previous.y,
        m.previous.z,
        m.velocity.x,
        m.velocity.y,
        m.velocity.z,
        host.alpha,
        host.dt,
        m.previous_body_yaw,
        m.body_yaw,
        m.previous_body_yaw + wrap_degrees(m.body_yaw - m.previous_body_yaw) * host.alpha
    );
    let settings = super::bedrock_debug::settings();
    let wanted: Vec<String> = settings
        .dump_bones
        .iter()
        .map(|n| n.to_ascii_lowercase())
        .collect();
    let mut bones: Vec<_> = frame
        .pose
        .bones
        .iter()
        .filter(|(name, b)| {
            if !wanted.is_empty() {
                let name = name.to_ascii_lowercase();
                return wanted.iter().any(|w| name.contains(w.as_str()));
            }
            b.rotation
                .iter()
                .chain(&b.rotation_entity)
                .any(|v| v.abs() > 0.5)
                || b.position.iter().any(|v| v.abs() > 0.02)
                || b.scale.iter().any(|v| (v - 1.0).abs() > 0.01)
        })
        .collect();
    bones.sort_by(|a, b| a.0.cmp(b.0));
    let mut b = String::new();
    for (name, p) in bones.iter().take(if wanted.is_empty() { 30 } else { 80 }) {
        let _ = write!(
            b,
            " {name}(rot={:?} ent={:?} pos={:?} sc={:?})",
            p.rotation.map(|v| (v * 10.0).round() / 10.0),
            p.rotation_entity.map(|v| (v * 10.0).round() / 10.0),
            p.position.map(|v| (v * 10.0).round() / 10.0),
            p.scale.map(|v| (v * 100.0).round() / 100.0),
        );
    }
    // How many bones the animations touched at all, and the biggest turn among them: tells an
    // animation that never ran from one that ran with nothing to move.
    let touched = frame.pose.bones.len();
    let biggest = frame
        .pose
        .bones
        .values()
        .flat_map(|b| b.rotation.iter().chain(&b.rotation_entity))
        .fold(0.0f32, |a, v| a.max(v.abs()));
    let _ = write!(b, " [touched={touched} max_rot={biggest:.2}]");
    let layers: Vec<String> = frame
        .layers
        .iter()
        .enumerate()
        .map(|(i, l)| {
            format!(
                "L{i} {}|{}|mat={}|col={:?}|ov={:?}|uv={:?}|hide={:?}",
                l.geometry.trim_start_matches("geometry.oreville_ans."),
                l.texture.rsplit('/').next().unwrap_or(""),
                l.material,
                l.color,
                l.overlay,
                l.uv_anim,
                l.visibility
                    .iter()
                    .filter(|(_, v)| !*v)
                    .map(|(p, _)| p.as_str())
                    .collect::<Vec<_>>()
            )
        })
        .collect();
    format!(
        "q:{q} || running: {running} || bones:{b} || layers: {}",
        layers.join(" ; ")
    )
}

/// With `HORIZON_BEDROCK_DEBUG`, each query nothing answers is logged once: a pack's animation
/// that never starts is often waiting on one of them.
fn note_unanswered(name: &str) {
    if !super::bedrock_debug::debug() {
        return;
    }
    static SEEN: Mutex<Option<std::collections::HashSet<String>>> = Mutex::new(None);
    if lock(&SEEN)
        .get_or_insert_with(Default::default)
        .insert(name.to_owned())
    {
        log::info!("bedrock: query.{name} has no answer (reads as 0)");
    }
}

/// Answers a pack's queries from a mob.
pub struct MobHost<'a> {
    pub m: &'a Mob,
    pub alpha: f32,
    pub dt: f32,
    /// The mob's position minus the camera's, in blocks.
    pub from_camera: [f32; 3],
    /// `q.vertical_speed` in the tick the mob went into water ([`WaterEntry`]).
    pub entry_speed: Option<f32>,
}

/// When a mob went into water and how fast. Bedrock knows a mob is in water in the tick that
/// takes it there, so `q.vertical_speed` then is the speed it hit the water with (the A&S
/// splash grows with it). The Java game finds the water at the start of the next tick, after
/// the fall has stopped on the bottom of a shallow pool: the speed of the tick before stands in.
#[derive(Default)]
struct WaterEntry {
    /// The mob's age at the last look.
    tick: u32,
    in_water: bool,
    /// `q.vertical_speed` at the last look.
    speed: f32,
    /// The tick it went into water, and the speed it went in with.
    entered: Option<(u32, f32)>,
}

impl WaterEntry {
    /// Looks at a mob once a tick (its `age`, whether it is in water, its `q.vertical_speed`);
    /// the speed it went into water with, during that tick.
    fn look(&mut self, age: u32, in_water: bool, speed: f32) -> Option<f32> {
        if age != self.tick {
            if in_water && !self.in_water {
                let hit = if self.speed.abs() > speed.abs() { self.speed } else { speed };
                self.entered = Some((age, hit));
            }
            self.tick = age;
            self.in_water = in_water;
            self.speed = speed;
        }
        self.entered.filter(|&(t, _)| t == age).map(|(_, v)| v)
    }
}

/// With `HORIZON_BEDROCK_DEBUG`, logs `what` the first `times` times it comes.
fn log_first(what: &str, times: u32) {
    static SEEN: std::sync::Mutex<Option<HashMap<String, u32>>> = std::sync::Mutex::new(None);
    let mut seen = lock(&SEEN);
    let n = seen.get_or_insert_with(HashMap::new).entry(what.to_owned()).or_insert(0);
    if *n < times {
        *n += 1;
        log::info!("bedrock-debug: {what}");
    }
}

static CAMERA_ROTATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The camera's `yaw` and `pitch` in degrees, for `query.camera_rotation`.
pub fn set_camera_rotation(yaw: f32, pitch: f32) {
    let bits = (u64::from(yaw.to_bits()) << 32) | u64::from(pitch.to_bits());
    CAMERA_ROTATION.store(bits, std::sync::atomic::Ordering::Relaxed);
}

fn camera_rotation() -> (f32, f32) {
    let bits = CAMERA_ROTATION.load(std::sync::atomic::Ordering::Relaxed);
    (f32::from_bits((bits >> 32) as u32), f32::from_bits(bits as u32))
}

/// `query.rotation_to_camera`: the (pitch, yaw) in degrees that would face the camera from a
/// mob `from_camera` blocks away from it.
fn rotation_to_camera(from_camera: [f32; 3]) -> (f32, f32) {
    let [x, y, z] = from_camera.map(|v| -v);
    let flat = x.hypot(z);
    if flat < 1e-4 && y.abs() < 1e-4 {
        return (0.0, 0.0);
    }
    ((-y.atan2(flat)).to_degrees(), (-x).atan2(z).to_degrees())
}

fn is_villager(id: &str) -> bool {
    matches!(id, "minecraft:villager" | "minecraft:zombie_villager")
}

/// Bedrock's profession number for the Java profession at `index` of
/// [`super::render::PROFESSIONS`].
fn bedrock_profession(index: u8) -> u32 {
    let java = super::render::PROFESSIONS[usize::from(index) % super::render::PROFESSIONS.len()];
    const BEDROCK: [&str; 15] = [
        "none",
        "farmer",
        "fisherman",
        "shepherd",
        "fletcher",
        "librarian",
        "cartographer",
        "cleric",
        "armorer",
        "weaponsmith",
        "toolsmith",
        "butcher",
        "leatherworker",
        "mason",
        "nitwit",
    ];
    BEDROCK.iter().position(|b| *b == java).unwrap_or(0) as u32
}

fn wrap_degrees(a: f32) -> f32 {
    let a = a % 360.0;
    if a > 180.0 {
        a - 360.0
    } else if a < -180.0 {
        a + 360.0
    } else {
        a
    }
}

impl Host for MobHost<'_> {
    fn context(&mut self, name: &str) -> Value {
        match name {
            // The off hand's `v.player_arm_height` (vanilla `swap_item`).
            "player_offhand_arm_height" => Value::Num(super::render::first_person_arm_heights()[1]),
            "is_first_person" => Value::bool(self.m.extra.first_person),
            _ => Value::Null,
        }
    }
    fn variables(&mut self) -> Vec<(&'static str, Value)> {
        let m = self.m;
        // The swing's progress, 0 at rest to 1 at its end, as the Java mob keeps it.
        let a = self.alpha;
        let mut swing = m.anim.attack_anim - m.anim.attack_anim_o;
        if swing < 0.0 {
            swing += 1.0;
        }
        let mut swing = (m.anim.attack_anim_o + swing * a).clamp(0.0, 1.0);
        // A golem's swing is its own ten-tick clock.
        if m.kind.id == "minecraft:iron_golem" && m.extra.anim_ticks > 0 {
            swing = ((10.0 - m.extra.anim_ticks as f32 + a) / 10.0).clamp(0.0, 1.0);
        }
        // Ticks left of a golem's ten-tick swing (`attackAnimationTick`), 0 at rest.
        let ticks = if m.kind.id == "minecraft:iron_golem" {
            m.extra.anim_ticks.max(0) as f32
        } else {
            0.0
        };
        let holds = |slot: &str| slot_items(m, false).iter().any(|(s, _)| *s == slot);
        let using = |what: &str| {
            m.extra.using_item > 0
                && slot_items(m, false)
                    .iter()
                    .any(|(s, i)| matches!(*s, "main_hand" | "off_hand") && i.contains(what))
        };
        let pitch = m.previous_head_pitch + (m.head_pitch - m.previous_head_pitch) * a;
        // What the game keeps for every entity a pack dresses (`variable.*` of vanilla's own
        // resources): the swing, the hands, the head's tilt, and the screens it is not drawn in.
        let mut vars = vec![
            ("attack_time", Value::Num(swing)),
            ("attack_animation_tick", Value::Num(ticks)),
            ("is_paperdoll", Value::bool(super::cem_anim::is_in_gui())),
            ("is_holding_right", Value::bool(holds("main_hand"))),
            ("is_holding_left", Value::bool(holds("off_hand"))),
            ("player_x_rotation", Value::Num(pitch)),
            ("map_face_icon", Value::Num(0.0)),
            ("short_arm_offset_left", Value::Num(0.0)),
            ("short_arm_offset_right", Value::Num(0.0)),
            ("bob_animation", Value::Num(0.0)),
            ("has_trim", Value::Num(0.0)),
            ("rail_rotation", Value::Num(0.0)),
            ("liedownamount", Value::Num(0.0)),
            ("is_enchanted", Value::Num(0.0)),
            ("is_first_person", Value::bool(m.extra.first_person)),
            // The hand raised (1) or lowered while the item is swapped (`swap_item`: the arm
            // drops 10 px at 0). Undefined, it read 0 and the vanilla view model held every
            // item 10 px too low.
            ("player_arm_height", Value::Num(super::render::first_person_arm_heights()[0])),
            // Using what the main hand holds: the spyglass up to the eye, the brush, the horn.
            ("is_holding_spyglass", Value::bool(using("spyglass"))),
            ("is_using_brush", Value::bool(using("brush"))),
            ("is_tooting_goat_horn", Value::bool(using("goat_horn"))),
            ("goat_has_left_horn", Value::Num(1.0)),
            ("goat_has_right_horn", Value::Num(1.0)),
            // What the walk swing (`v.tcos0 = cos(..) * q.modified_move_speed /
            // v.gliding_speed_value`) is divided by: 1 on foot, more while gliding fast
            // (HumanoidModel.setupAnim: `(speed² / 0.2)³`, never below 1). The speed itself
            // made the swing huge standing still (0 or nearly).
            ("gliding_speed_value", Value::Num(if m.extra.fall_flying {
                ((m.velocity.length_squared() as f32 / 0.2).powi(3)).max(1.0)
            } else {
                1.0
            })),
            // Arms out in front: a zombie, a skeleton or a husk with something to attack.
            ("raise_arms", Value::bool(m.ai.target.is_some() || {
                let id: &str = &m.kind.id;
                id.contains("zombie") || id.contains("husk") || id.contains("drowned")
            })),
            // What vanilla's resources keep and the game has no word for: at rest.
            ("is_patterned", Value::Num(0.0)),
            ("ischarging", Value::bool(m.extra.charged)),
            ("lieonplayer", Value::Num(0.0)),
            ("offer_flower_tick", Value::Num(0.0)),
            ("shake_tail", Value::Num(0.0)),
            ("should_bow_head", Value::Num(0.0)),
            ("stand_anim", Value::Num(0.0)),
            ("use_blinking_animation", Value::Num(0.0)),
            ("glow_alpha", Value::Num(0.0)),
            ("trim_path", Value::Num(0.0)),
        ];
        // A dragon's flight, ticks back.
        if let Some(history) = &m.extra.history {
            for (names, [y, yaw]) in HISTORY_NAMES.iter().zip(history.iter()) {
                vars.push((names[0], Value::Num(*y)));
                vars.push((names[1], Value::Num(*yaw)));
            }
        }
        vars
    }

    fn query(&mut self, name: &str, args: &[Value]) -> Value {
        if let Some(v) = super::bedrock_debug::query(name) {
            return Value::Num(v);
        }
        let m = self.m;
        if let Some(v) = world_query(m, name, args) {
            return v;
        }
        let a = self.alpha;
        let lerp = |p: f32, c: f32| p + (c - p) * a;
        match name {
            "life_time" => Value::Num((m.age as f32 + a) / 20.0),
            "is_in_ui" => Value::bool(super::cem_anim::is_in_gui()),
            "delta_time" => Value::Num(self.dt),
            "frame_alpha" => Value::Num(a),
            "is_name_any" => Value::bool(name_matches(m.custom_name.as_deref(), args)),
            "get_name" => Value::Str(m.custom_name.as_deref().unwrap_or("").into()),
            "is_baby" => Value::bool(m.is_baby()),
            "is_local_player" => Value::bool(m.extra.local_player),
            "is_on_ground" => Value::bool(m.on_ground),
            "is_in_water" | "is_in_contact_with_water" => Value::bool(m.fluids.in_water),
            // A player swims in the swimming pose only (sprinting through water): standing still
            // in water it floats upright. Any other mob swims in water.
            // Never while gliding: a glide that runs into water keeps the swimming pose (the
            // game picks it before the glide) and the packs' two animations would both turn
            // the body (`is_gliding` and `is_swimming` do not hold together in Bedrock).
            "is_swimming" if m.kind.id == "minecraft:player" => Value::bool(
                m.extra.avatar_pose == crate::pose::Pose::Swimming && !m.extra.fall_flying,
            ),
            "is_swimming" => Value::bool(m.fluids.in_water),
            "is_alive" => Value::bool(m.alive()),
            "health" => Value::Num(m.health),
            "max_health" => Value::Num(m.kind.max_health),
            "hurt_time" => Value::Num(m.hurt_time as f32),
            "is_on_fire" => Value::bool(m.fire_ticks > 0),
            "is_riding" => Value::bool(m.extra.passenger),
            // A mob the server runs has no target here: one drawing a bow or crossbow has one.
            "has_target" => Value::bool(
                m.ai.target.is_some()
                    // Mob.isAggressive, which the server sends: an illager's raised axe, a
                    // zombie's raised arms.
                    || m.ai.aggressive
                    || m.extra.using_item > 0
                    || (m.species() == super::Species::Blaze && super::nether::blaze_attacking(m)),
            ),
            // A rider's limbs do not swing (`LivingEntityRenderer`: a sitting passenger's walk
            // animation is zero), whatever the mount's speed carries it at.
            "modified_move_speed" if m.extra.passenger => Value::Num(0.0),
            "ground_speed" if m.extra.passenger => Value::Num(0.0),
            "modified_distance_moved" | "distance_moved" if m.extra.passenger => Value::Num(0.0),
            "modified_move_speed" => Value::Num(lerp(m.previous_limb_speed, m.limb_speed).min(1.0)),
            // Blocks a second. A player's own velocity is not the game's to track (the client
            // drives it), so what it moved in the last tick counts too.
            // Swimming: the way the body moved in the last tick, for the pitch the pack takes
            // from `vertical_speed / ground_speed`. A player's velocity is a stale one (the
            // client drives it): taken with the move, a climb on the space key read as a dive,
            // and the body turned head down.
            // Gliding the same: the wings' size follows it, and a climb on a firework reads
            // as no speed at all when only the flat part counts.
            "ground_speed" if m.kind.id == "minecraft:player"
                && (m.extra.avatar_pose == crate::pose::Pose::Swimming || m.extra.fall_flying) =>
            {
                let (dx, dy, dz) = (
                    m.position.x - m.previous.x,
                    m.position.y - m.previous.y,
                    m.position.z - m.previous.z,
                );
                Value::Num((dx * dx + dy * dy + dz * dz).sqrt() as f32 * 20.0)
            }
            "vertical_speed" if m.kind.id == "minecraft:player"
                && m.extra.avatar_pose == crate::pose::Pose::Swimming
                && !m.extra.fall_flying =>
            {
                Value::Num((m.position.y - m.previous.y) as f32 * 20.0)
            }
            "ground_speed" => {
                let v = m.velocity;
                let (dx, dz) = (m.position.x - m.previous.x, m.position.z - m.previous.z);
                let by_velocity = (v.x * v.x + v.z * v.z).sqrt();
                let by_moving = (dx * dx + dz * dz).sqrt();
                let flat = by_velocity.max(by_moving);
                // Swimming leans the body along the way it goes (`-asin(vertical_speed /
                // ground_speed)` is the pitch only if the speed counts the climb too).
                let speed = if m.extra.avatar_pose == crate::pose::Pose::Swimming {
                    let dy = m.position.y - m.previous.y;
                    let by_moving = (dx * dx + dy * dy + dz * dz).sqrt();
                    let by_velocity = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
                    by_velocity.max(by_moving)
                } else {
                    flat
                };
                Value::Num(speed as f32 * 20.0)
            }
            "vertical_speed" => Value::Num(self.entry_speed.unwrap_or_else(|| vertical_speed(m))),
            "body_y_rotation" => Value::Num(wrap_degrees(
                m.previous_body_yaw + wrap_degrees(m.body_yaw - m.previous_body_yaw) * a,
            )),
            // The head's yaw in the world, as `body_y_rotation` is the body's: the packs take
            // `head_y_rotation - target_y_rotation` for the body's.
            "head_y_rotation" => Value::Num(wrap_degrees(
                m.previous_head_yaw + wrap_degrees(m.head_yaw - m.previous_head_yaw) * a,
            )),
            "head_x_rotation" => Value::Num(lerp(m.previous_head_pitch, m.head_pitch)),
            // The mount's pose a rider takes (`passengers.rs` gives it the vehicle's yaw): 0
            // off one.
            "ride_body_y_rotation" if m.extra.passenger => {
                Value::Num(wrap_degrees(lerp(m.previous_body_yaw, m.body_yaw)))
            }
            "ride_head_y_rotation" if m.extra.passenger => {
                Value::Num(wrap_degrees(lerp(m.previous_head_yaw, m.head_yaw)))
            }
            "ride_head_x_rotation" if m.extra.passenger => {
                Value::Num(lerp(m.previous_head_pitch, m.head_pitch))
            }
            "ride_body_x_rotation" | "ride_body_y_rotation" | "ride_head_x_rotation"
            | "ride_head_y_rotation" => Value::Num(0.0),
            "position" => {
                // Per frame, like Bedrock: the packs' speed from successive reads is smooth
                // instead of a spike on each tick's first frame and zero between.
                let axis = (args.first().map_or(0.0, Value::num) as usize).min(2);
                let now = [m.position.x, m.position.y, m.position.z][axis];
                let before = [m.previous.x, m.previous.y, m.previous.z][axis];
                Value::Num((before + (now - before) * f64::from(a)) as f32)
            }
            "distance_from_camera" => {
                let [x, y, z] = self.from_camera;
                Value::Num((x * x + y * y + z * z).sqrt())
            }
            "rotation_to_camera" => {
                let (pitch, yaw) = rotation_to_camera(self.from_camera);
                Value::Num(if args.first().map_or(0.0, Value::num) as i32 == 0 { pitch } else { yaw })
            }
            "camera_rotation" => {
                let (yaw, pitch) = camera_rotation();
                Value::Num(if args.first().map_or(0.0, Value::num) as i32 == 0 { pitch } else { yaw })
            }
            "swell_amount" => Value::Num(if m.max_swell > 0 {
                (lerp(m.previous_swell as f32, m.swell as f32) / m.max_swell as f32).clamp(0.0, 1.0)
            } else {
                0.0
            }),
            "is_item_name_any"
            | "get_equipped_item_name"
            | "equipped_item_any_tag"
            | "is_item_equipped" => {
                let item = item_at(m, args.first());
                let rest = || {
                    args.iter().skip(1).filter_map(|v| match v {
                        Value::Str(s) => {
                            Some(s.trim_start_matches("minecraft:").to_ascii_lowercase())
                        }
                        _ => None,
                    })
                };
                match name {
                    "get_equipped_item_name" => Value::Str(
                        item.as_deref()
                            .map_or("", |i| i.trim_start_matches("minecraft:"))
                            .into(),
                    ),
                    "is_item_equipped" => Value::bool(item.is_some()),
                    "equipped_item_any_tag" => Value::bool(item.as_deref().is_some_and(|i| {
                        args.iter()
                            .skip(1)
                            .any(|v| matches!(v, Value::Str(t) if item_has_tag(i, t)))
                    })),
                    _ => Value::bool(item.as_deref().is_some_and(|i| {
                        let short = bedrock_item_name(i);
                        rest().any(|n| n == short)
                    })),
                }
            }
            // The walk cycle: Java's limb position and speed are the numbers a Bedrock walk
            // animation reads as `modified_distance_moved` and `modified_move_speed`.
            "modified_distance_moved" | "distance_moved" => {
                Value::Num(m.limb_position - m.limb_speed * (1.0 - a))
            }
            // The head's turn and tilt from the body, in degrees.
            "target_y_rotation" => Value::Num(wrap_degrees(
                lerp(m.previous_head_yaw, m.head_yaw) - lerp(m.previous_body_yaw, m.body_yaw),
            )),
            "target_x_rotation" => Value::Num(lerp(m.previous_head_pitch, m.head_pitch)),
            "body_x_rotation" => Value::Num(0.0),
            "yaw_speed" => Value::Num(wrap_degrees(m.body_yaw - m.previous_body_yaw) * 20.0),
            // A swing lasts a third of a second in the game that made these packs.
            "base_swing_duration" | "modified_swing_duration" => Value::Num(0.3),
            "is_delayed_attacking" => {
                Value::bool(m.kind.id == "minecraft:iron_golem" && m.extra.anim_ticks > 0)
            }
            "death_ticks" => Value::Num(m.death_time as f32),
            "is_in_lava" => Value::bool(m.fluids.in_lava),
            "head_is_in_water" => Value::bool(m.fluids.eye_in_water),
            "is_gliding" => Value::bool(m.extra.fall_flying),
            "is_jumping" => Value::bool(!m.on_ground && m.velocity.y > 0.0),
            "model_scale" => Value::Num(1.0),
            // The world's clock is not the mob's to know: noon.
            "time_of_day" => Value::Num(0.5),
            "is_sheared" => Value::bool(m.extra.sheared),
            "is_powered" | "is_charged" => Value::bool(if m.species() == super::Species::Blaze {
                super::nether::blaze_charging(m)
            } else {
                m.extra.charged
            }),
            // A camel sits by its own pose state (`Camel.isCamelSitting`), not a pet's `sitting`.
            "is_sitting" => Value::bool(
                m.extra.sitting || (super::camel::is_camel(m.species()) && super::camel::sitting(m)),
            ),
            "sit_amount" => Value::Num(
                if m.extra.sitting
                    || (super::camel::is_camel(m.species()) && super::camel::sitting(m))
                {
                    1.0
                } else {
                    0.0
                },
            ),
            "is_tamed" => Value::bool(m.extra.tame),
            "is_saddled" => Value::bool(m.extra.saddled || m.extra.saddle_stack.is_some()),
            "is_chested" => Value::bool(m.extra.chested),
            "is_sneaking" | "is_crouching" => Value::bool(m.extra.crouching || m.extra.sneaking),
            "is_sprinting" => Value::bool(m.extra.sprinting),
            "is_angry" => Value::bool(
                m.extra.anger > 0 || (m.species() == super::Species::Enderman && m.extra.flag),
            ),
            "is_wall_climbing" => Value::bool(m.climbing),
            "is_carrying_block" => Value::bool(m.extra.carried.is_some()),
            "has_rider" => Value::bool(m.rider.is_some()),
            "is_using_item" => Value::bool(m.extra.using_item > 0),
            // Ticks left of the item in use, as Bedrock counts: 0 when none is in use.
            "main_hand_item_use_duration" => {
                Value::Num(if m.extra.using_item > 0 && !m.extra.using_off_hand {
                    (max_use_ticks(m.extra.main_hand.as_deref()) - m.extra.using_item as f32)
                        .max(1.0)
                } else {
                    0.0
                })
            }
            "main_hand_item_max_duration" => {
                Value::Num(max_use_ticks(m.extra.main_hand.as_deref()))
            }
            // Seconds left of the item in use.
            "item_remaining_use_duration" => {
                let off = matches!(args.first(), Some(Value::Str(s)) if s.contains("off"));
                let held = if off {
                    &m.extra.off_hand
                } else {
                    &m.extra.main_hand
                };
                let using = m.extra.using_item > 0 && m.extra.using_off_hand == off;
                Value::Num(if using {
                    ((max_use_ticks(held.as_deref()) - m.extra.using_item as f32) / 20.0).max(0.0)
                } else {
                    0.0
                })
            }
            "previous_squish_value" => Value::Num(m.anim.vo[0]),
            "current_squish_value" => Value::Num(m.anim.v[0]),
            "fuse_time" => Value::Num(if m.species() == super::Species::SulfurCube {
                // Seconds left on a lit fuse; 0 while the cube is not primed.
                super::sulfur_cube::fuse(m).max(0) as f32 / 20.0
            } else {
                m.max_swell as f32 / 20.0
            }),
            "wing_flap_position" => Value::Num(lerp(m.anim.flap_o, m.anim.flap)),
            "wing_flap_speed" => Value::Num(lerp(m.anim.flap_speed_o, m.anim.flap_speed)),
            "movement_direction" => {
                let axis = args.first().map_or(0.0, Value::num) as usize;
                let mut v = [m.velocity.x, m.velocity.y, m.velocity.z];
                let mut len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                // A player's own velocity is not the game's to track (the client drives it):
                // what it moved in the last tick says the way it goes.
                if len < 1e-4 {
                    v = [
                        m.position.x - m.previous.x,
                        m.position.y - m.previous.y,
                        m.position.z - m.previous.z,
                    ];
                    len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                }
                Value::Num(if len > 1e-6 {
                    (v[axis.min(2)] / len) as f32
                } else {
                    0.0
                })
            }
            "eye_target_x_rotation" => Value::Num(lerp(m.previous_head_pitch, m.head_pitch)),
            "eye_target_y_rotation" => Value::Num(wrap_degrees(
                lerp(m.previous_head_yaw, m.head_yaw) - lerp(m.previous_body_yaw, m.body_yaw),
            )),
            "has_head_gear" => Value::bool(m.extra.head.is_some()),
            "equipment_count" => {
                Value::Num(m.equipment_stacks().iter().filter(|s| s.is_some()).count() as f32)
            }
            "has_any_leashed_entity_of_type" => Value::bool(m.extra.leash.is_some()),
            "armor_texture_slot" => {
                let slot = args.first().map_or(0.0, Value::num) as usize;
                Value::Num(armor_texture_index(m, slot))
            }
            "armor_color_slot" => {
                let slot = args.first().map_or(0.0, Value::num) as usize;
                let channel = args.get(1).map_or(0.0, Value::num) as usize;
                Value::Num(armor_color(m, slot, channel))
            }
            "overlay_alpha" => Value::Num(if m.hurt_time > 0 { 0.3 } else { 0.0 }),
            "owner_identifier" => Value::Str(if m.extra.tame { "minecraft:player" } else { "" }.into()),
            // A pet's owner is a player.
            "is_owner_identifier_any" => Value::bool(
                m.extra.tame
                    && args.iter().any(|a| {
                        matches!(a, Value::Str(s) if s.trim().eq_ignore_ascii_case("minecraft:player"))
                    }),
            ),
            "invulnerable_ticks" => Value::Num(m.invulnerable as f32),
            // A villager's look: Bedrock keeps the profession in `variant`, the biome in
            // `mark_variant` and the trade tier in `trade_tier`; a skin is picked per mob.
            "variant" if is_villager(&m.kind.id) => {
                Value::Num(bedrock_profession(m.extra.variant2) as f32)
            }
            "mark_variant" if is_villager(&m.kind.id) => Value::Num(f32::from(m.extra.variant)),
            "trade_tier" if is_villager(&m.kind.id) => {
                Value::Num((i32::from(m.extra.variant3) - 1).clamp(0, 4) as f32)
            }
            "skin_id" if is_villager(&m.kind.id) => Value::Num((m.id % 6) as f32),
            // A pufferfish keeps its puff state (0 small, 1 half, 2 full) apart from its skin.
            "variant" if m.species() == super::Species::Pufferfish => {
                Value::Num(m.extra.size as f32)
            }
            // A shulker's variant is its shell colour: 0 to 15 the dyes, 16 the purple one it
            // has by default.
            "variant" if m.species() == super::Species::Shulker => Value::Num(f32::from(m.extra.color)),
            "variant" => Value::Num(f32::from(m.extra.variant)),
            "swelling_dir" => Value::Num(match m.swell.cmp(&m.previous_swell) {
                std::cmp::Ordering::Greater => 1.0,
                std::cmp::Ordering::Less => -1.0,
                std::cmp::Ordering::Equal => 0.0,
            }),
            "is_on_screen" => Value::Num(1.0),
            "is_invisible" => Value::bool(super::trader::is_invisible(m)),
            "is_leashed" => Value::bool(m.extra.leash.is_some()),
            "leashed_entity_count" => Value::Num(f32::from(u8::from(m.extra.leash.is_some()))),
            "is_in_love" => Value::bool(m.extra.in_love > 0),
            "is_moving" => Value::bool(m.velocity.x.abs() + m.velocity.z.abs() > 0.003),
            "position_delta" => {
                let axis = args.first().map_or(0.0, Value::num) as usize;
                Value::Num([m.velocity.x, m.velocity.y, m.velocity.z][axis.min(2)] as f32)
            }
            // Interpolated between ticks like `modified_distance_moved`: a pack scales it up (the
            // first-person walk sway reads `q.walk_distance*100`), so a step per tick shook the arm.
            // In blocks, as Bedrock: the limb cycle moves four times a block walked (`limb_speed`
            // is `4 * blocks per tick`, up to 1), so the sway's phase ran four times too fast.
            "walk_distance" => Value::Num((m.limb_position - m.limb_speed * (1.0 - a)) * 0.25),
            "has_player_rider" => Value::bool(m.rider.is_some()),
            "is_eating" | "is_grazing" => Value::bool(
                (m.species() == super::Species::Sheep && m.extra.anim_ticks > 0)
                    || m.extra.using_item > 0,
            ),
            "is_roaring" => {
                Value::bool(m.species() == super::Species::Ravager && m.extra.anim_ticks > 0)
            }
            "is_stunned" => {
                Value::bool(m.species() == super::Species::Ravager && m.extra.counter > 0)
            }
            "hurt_direction" | "head_roll_angle" | "sleep_rotation" | "roll_counter"
            | "sneeze_counter" | "unhappy_counter" | "shake_angle" | "shake_time" => {
                Value::Num(0.0)
            }
            "swim_amount" => Value::Num(f32::from(u8::from(m.fluids.in_water && !m.on_ground))),
            "lie_amount" => Value::Num(0.0),
            "standing_scale" => Value::Num(if m.species() == super::Species::PolarBear {
                super::polar_bear::standing_scale(m, self.alpha)
            } else {
                0.0
            }),
            // A vex flies at its target (`flag`): the pack's charge pose and attack trail.
            "is_charging" if m.species() == super::Species::Vex => Value::bool(m.extra.flag),
            // A parrot (or other mob) dancing to the jukebox it hears.
            "is_dancing" if m.extra.party_jukebox.is_some() => Value::bool(true),
            "is_celebrating"
            | "is_celebrating_special"
            | "is_admiring"
            | "is_dancing"
            | "is_playing_dead"
            | "is_casting"
            | "is_charging"
            | "is_digging"
            | "is_crawling"
            | "is_standing"
            | "is_interested"
            | "is_scared"
            | "is_stalking"
            | "is_searching"
            | "is_emerging"
            | "is_pregnant"
            | "is_laying_egg"
            | "is_levitating"
            | "is_croaking"
            | "is_sniffing"
            | "is_sonic_boom"
            | "is_ghost"
            | "is_elder"
            | "is_resting"
            | "is_jump_goal_jumping"
            | "has_dash_cooldown"
            | "is_shield_powered"
            | "is_shaking_wetness"
            | "is_eating_mob"
            | "can_damage_nearby_mobs" => Value::Num(0.0),
            "is_riding_any_entity_of_type" => Value::bool(
                m.extra.passenger
                    && super::riders::mount_kind(m.id).is_some_and(|k| {
                        args.iter()
                            .any(|a| matches!(a, Value::Str(s) if s.eq_ignore_ascii_case(&k)))
                    }),
            ),
            "is_sleeping" | "is_avoiding_mobs" | "is_shaking" => {
                Value::Num(0.0)
            }
            "time_stamp" => Value::Num(m.age as f32),
            "facing_target_to_range_attack" => Value::bool(if m.species() == super::Species::Blaze {
                super::nether::blaze_attacking(m)
            } else {
                m.extra.using_item > 0
            }),
            "item_is_charged" => Value::bool(m.extra.flag2),
            "blocking" => Value::bool(m.extra.using_item > 0 && m.extra.off_hand.is_some()),
            // The properties vanilla's own entities carry, from the Java mob's state.
            "property" => match args.first() {
                Some(Value::Str(p)) if &**p == "minecraft:armadillo_state" => Value::Str(
                    match m.extra.armadillo.state {
                        super::armadillo::IDLE => "unrolled",
                        super::armadillo::UNROLLING => "rolled_up_unrolling",
                        _ => "rolled_up",
                    }
                    .into(),
                ),
                // The block a sulfur cube holds sets its archetype; `none` with nothing held (a
                // pack tests `!= 'none'` to tell a cube with a block from an empty one).
                Some(Value::Str(p)) if &**p == "minecraft:sulfur_cube_archetype" => Value::Str(
                    if m.species() != super::Species::SulfurCube || m.extra.body.is_none() {
                        "none"
                    } else {
                        super::sulfur_cube::archetype(m).map_or("regular", |a| a.name)
                    }
                    .into(),
                ),
                _ => Value::Null,
            },
            "has_property" => Value::bool(matches!(
                args.first(),
                Some(Value::Str(p)) if matches!(
                    &**p,
                    "minecraft:armadillo_state" | "minecraft:sulfur_cube_archetype"
                )
            )),
            // The stack size of what is held: one when there is something.
            "count" => Value::Num(f32::from(u8::from(item_at(m, args.first()).is_some()))),
            "is_onfire" => Value::bool(m.fire_ticks > 0),
            "on_fire_time" => Value::Num(m.fire_ticks.max(0) as f32 / 20.0),
            "is_in_water_or_rain" => Value::bool(m.fluids.in_water),
            "is_fire_immune" => Value::bool(m.kind.flags.fire_immune),
            "has_owner" => Value::bool(m.extra.tame),
            "has_gravity" | "has_collision" | "can_walk" | "can_swim" => Value::Num(1.0),
            "has_cape" => Value::bool(m.cape.is_some()),
            // The slot counts 0 head, 1 chest, 2 legs, 3 feet, 4 body.
            "has_armor_slot" => {
                let slot = args.first().map_or(0.0, Value::num) as usize;
                Value::bool(m.equipment_stacks().get(slot).is_some_and(|s| s.is_some()))
            }
            "armor_material_slot" => {
                let slot = args.first().map_or(0.0, Value::num) as usize;
                Value::Num(armor_texture_index(m, slot))
            }
            "armor_damage_slot" => Value::Num(0.0),
            // 0 up to the first distance from the camera, 1 from the second, linear between.
            "camera_distance_range_lerp" => {
                let [x, y, z] = self.from_camera;
                let d = (x * x + y * y + z * z).sqrt();
                let (lo, hi) = (
                    args.first().map_or(0.0, Value::num),
                    args.get(1).map_or(1.0, Value::num),
                );
                Value::Num(if hi > lo { ((d - lo) / (hi - lo)).clamp(0.0, 1.0) } else { 0.0 })
            }
            "get_level_seed_based_fraction" => {
                Value::Num(((m.id.wrapping_mul(2654435761)) >> 8 & 0xFFFF) as f32 / 65536.0)
            }
            "average_frame_time" | "last_frame_time" | "minimum_frame_time"
            | "maximum_frame_time" => Value::Num(self.dt),
            "client_max_render_distance" => Value::Num(128.0),
            "client_memory_tier" | "server_memory_tier" => Value::Num(3.0),
            "last_input_mode_is_any" => Value::bool(
                args.iter()
                    .any(|a| matches!(a, Value::Str(s) if s.eq_ignore_ascii_case("keyboard_and_mouse"))),
            ),
            "day" => Value::Num(
                m.extra
                    .surroundings
                    .as_deref()
                    .map_or(0, |s| s.day_time.div_euclid(24000)) as f32,
            ),
            "moon_brightness" => {
                let phase = m
                    .extra
                    .surroundings
                    .as_deref()
                    .map_or(0, |s| s.day_time.div_euclid(24000).rem_euclid(8));
                Value::Num([1.0, 0.75, 0.5, 0.25, 0.0, 0.25, 0.5, 0.75][phase as usize])
            }
            other => common_query(other, args),
        }
    }
}

/// What `m` holds or wears in the slot a query names: `slot.weapon.mainhand`, `main_hand`, 0 (the
/// main hand) or 1 (the off hand).
/// The name a Bedrock pack knows a Java item by, without the namespace: the blocks Bedrock
/// names otherwise (`magma_block` is `magma`, `jack_o_lantern` is `lit_pumpkin`, the snow
/// block is `snow` and the snow layer `snow_layer`), else the Java name.
fn bedrock_item_name(java: &str) -> String {
    let short = java.trim_start_matches("minecraft:").to_ascii_lowercase();
    match short.as_str() {
        "magma_block" => "magma",
        "jack_o_lantern" => "lit_pumpkin",
        "snow_block" => "snow",
        "snow" => "snow_layer",
        "rooted_dirt" => "dirt_with_roots",
        "slime_block" => "slime",
        "cobweb" => "web",
        "lily_pad" => "waterlily",
        "dead_bush" => "deadbush",
        "nether_quartz_ore" => "quartz_ore",
        "melon" => "melon_block",
        "note_block" => "noteblock",
        "powered_rail" => "golden_rail",
        "end_stone_bricks" => "end_bricks",
        "terracotta" => "hardened_clay",
        "bricks" => "brick_block",
        "nether_bricks" => "nether_brick",
        "red_nether_bricks" => "red_nether_brick",
        "sugar_cane" => "reeds",
        "spawner" => "mob_spawner",
        _ => return short,
    }
    .to_owned()
}

/// The pack's attachables for a held Java item: by its own id, else by the id Bedrock gives it.
fn attachables_for<'a>(
    index: &'a HashMap<String, Vec<String>>,
    item_lc: &str,
) -> Option<&'a Vec<String>> {
    index
        .get(item_lc)
        .or_else(|| index.get(&format!("minecraft:{}", bedrock_item_name(item_lc))))
}

fn item_at(m: &Mob, slot: Option<&Value>) -> Option<Arc<str>> {
    let slot = match slot? {
        Value::Str(s) => {
            let s = s.to_ascii_lowercase();
            slot_of(&s).or(match s.as_str() {
                "main_hand" => Some("main_hand"),
                "off_hand" => Some("off_hand"),
                "head" => Some("head"),
                "chest" | "torso" => Some("torso"),
                "legs" => Some("legs"),
                "feet" => Some("feet"),
                "body" => Some("body"),
                _ => None,
            })?
        }
        Value::Num(n) if *n < 0.5 => "main_hand",
        Value::Num(_) => "off_hand",
        _ => return None,
    };
    slot_items(m, false)
        .into_iter()
        .find_map(|(s, i)| (s == slot).then_some(i))
}

/// The ticks an item can be used for at most (`query.main_hand_item_max_duration`).
fn max_use_ticks(item: Option<&str>) -> f32 {
    let name = item.map_or("", |i| i.trim_start_matches("minecraft:"));
    match name {
        "crossbow" => 25.0,
        "potion" | "milk_bucket" | "honey_bottle" | "ominous_bottle" => 32.0,
        "dried_kelp" => 16.0,
        "" => 0.0,
        n if item_has_tag(n, "is_food") => 32.0,
        _ => 72000.0,
    }
}

/// What a worn piece in `slot` (0 head, 1 chest, 2 legs, 3 feet, 4 body armour) is made of, as
/// the index a pack's armour texture array uses: a horse's armour 1 leather, 2 iron, 3 gold,
/// 4 diamond; other armour 0 leather, 1 chain, 2 iron, 3 diamond, 4 gold, 5 netherite.
fn armor_texture_index(m: &Mob, slot: usize) -> f32 {
    let stacks = m.equipment_stacks();
    let index = match slot {
        0 => 5,
        1 => 4,
        2 => 3,
        3 => 2,
        _ => 6,
    };
    let Some(stack) = &stacks[index] else {
        return if slot >= 4 { 0.0 } else { -1.0 };
    };
    let name = stack.item.trim_start_matches("minecraft:");
    let kind = |k: &str| name.starts_with(k);
    if slot >= 4 {
        return if kind("leather") {
            1.0
        } else if kind("iron") {
            2.0
        } else if kind("golden") {
            3.0
        } else if kind("diamond") {
            4.0
        } else {
            0.0
        };
    }
    if kind("leather") {
        0.0
    } else if kind("chainmail") {
        1.0
    } else if kind("iron") {
        2.0
    } else if kind("diamond") {
        3.0
    } else if kind("golden") {
        4.0
    } else if kind("netherite") {
        5.0
    } else {
        -1.0
    }
}

/// A worn piece's dye (`query.armor_color_slot(slot, channel)`, channel 0..3 for r, g, b, a):
/// leather's own brown when undyed, white for any other material.
pub(super) fn armor_color(m: &Mob, slot: usize, channel: usize) -> f32 {
    let stacks = m.equipment_stacks();
    let index = match slot {
        0 => 5,
        1 => 4,
        2 => 3,
        3 => 2,
        _ => 6,
    };
    let Some(stack) = &stacks[index] else {
        return 1.0;
    };
    if channel >= 3 {
        return 1.0;
    }
    let rgb = if stack.item.contains("leather") {
        stack
            .data()
            .and_then(|d| d.dyed_color)
            .unwrap_or(0xA0_65_40)
    } else {
        0x_FF_FF_FF
    };
    ((rgb >> (16 - 8 * channel as i32)) & 255) as f32 / 255.0
}

/// Whether the item carries the tag a pack asks about. A pack's own tags (`oreville:is_sword`)
/// come from its behaviour pack, which is not here: they read as the kind of item their name says.
fn item_has_tag(item: &str, tag: &str) -> bool {
    let path = item.rsplit(':').next().unwrap_or(item).to_ascii_lowercase();
    let tag = tag.rsplit(':').next().unwrap_or(tag).to_ascii_lowercase();
    let ends = |w: &str| path == w || path.ends_with(&format!("_{w}"));
    const GEAR: [&str; 10] = [
        "sword",
        "axe",
        "pickaxe",
        "shovel",
        "hoe",
        "spear",
        "helmet",
        "chestplate",
        "leggings",
        "boots",
    ];
    if let Some(tier) = tag.strip_suffix("_tier") {
        return path
            .strip_prefix(tier)
            .and_then(|r| r.strip_prefix('_'))
            .is_some_and(|r| GEAR.contains(&r));
    }
    let Some(word) = tag.strip_prefix("is_") else {
        return match tag.as_str() {
            "harness" => ends("harness"),
            "music_disc" => path.starts_with("music_disc"),
            "sign" => ends("sign") && !path.contains("hanging"),
            "hanging_sign" => ends("hanging_sign"),
            "nautilus_armor" => ends("nautilus_armor"),
            _ => false,
        };
    };
    match word {
        "armor" => ["helmet", "chestplate", "leggings", "boots"]
            .iter()
            .any(|w| ends(w)),
        "tool" => [
            "sword", "axe", "pickaxe", "shovel", "hoe", "spear", "shears",
        ]
        .iter()
        .any(|w| ends(w)),
        "soup" => ends("soup") || ends("stew"),
        "milk" => path == "milk_bucket",
        "drink" => {
            ends("potion")
                || path == "milk_bucket"
                || ends("bottle") && !path.contains("experience")
        }
        "horn" => path == "goat_horn",
        "readable" => ends("book"),
        "throwable" => matches!(
            path.as_str(),
            "snowball"
                | "egg"
                | "ender_pearl"
                | "wind_charge"
                | "splash_potion"
                | "lingering_potion"
                | "experience_bottle"
        ),
        "seeds" => path.ends_with("seeds"),
        "petals" => path.ends_with("petals"),
        "on_a_stick" => path.ends_with("_on_a_stick"),
        "rod" => path.ends_with("_rod") || path == "rod",
        w => ends(w),
    }
}

/// What a mob holds or wears, by slot (the names Bedrock's `context.item_slot` takes).
fn held_items(m: &Mob) -> Vec<(&'static str, Arc<str>)> {
    slot_items(m, true)
}

/// What each slot holds: as the pack's queries see it (`for_attachables` false: the sulfur
/// cube's block in its main hand) or as attachables dress it.
fn slot_items(m: &Mob, for_attachables: bool) -> Vec<(&'static str, Arc<str>)> {
    let e = &m.extra;
    // A sulfur cube's block is the Java mob's body slot. Bedrock draws it as a block on the
    // cube's `rightitem` bone, never through an attachable (the generic held-block one would
    // shrink and tilt it by the hand's animation): the game draws it there, and the pack's
    // queries see it in the main hand (`item_at`).
    let cube = m.species() == super::Species::SulfurCube;
    [
        (
            "main_hand",
            match (cube, for_attachables) {
                (true, true) => &None,
                (true, false) => &e.body,
                (false, _) => &e.main_hand,
            },
        ),
        ("off_hand", &e.off_hand),
        ("head", &e.head),
        ("torso", &e.chest),
        ("legs", &e.legs),
        ("feet", &e.feet),
        // What the body slot holds: a harness, a horse's armour, a llama's carpet.
        ("body", if cube { &None } else { &e.body }),
    ]
    .into_iter()
    .filter_map(|(slot, item)| Some((slot, item.clone()?)))
    .collect()
}

/// `variable.historical_frame_<n>`'s members, as the game sets them on a dragon.
const HISTORY_NAMES: [[&str; 2]; super::HISTORY] = [
    ["historical_frame_0.pos_y", "historical_frame_0.rot_y"],
    ["historical_frame_1.pos_y", "historical_frame_1.rot_y"],
    ["historical_frame_2.pos_y", "historical_frame_2.rot_y"],
    ["historical_frame_3.pos_y", "historical_frame_3.rot_y"],
    ["historical_frame_4.pos_y", "historical_frame_4.rot_y"],
    ["historical_frame_5.pos_y", "historical_frame_5.rot_y"],
    ["historical_frame_6.pos_y", "historical_frame_6.rot_y"],
    ["historical_frame_7.pos_y", "historical_frame_7.rot_y"],
    ["historical_frame_8.pos_y", "historical_frame_8.rot_y"],
    ["historical_frame_9.pos_y", "historical_frame_9.rot_y"],
    ["historical_frame_10.pos_y", "historical_frame_10.rot_y"],
    ["historical_frame_11.pos_y", "historical_frame_11.rot_y"],
    ["historical_frame_12.pos_y", "historical_frame_12.rot_y"],
    ["historical_frame_13.pos_y", "historical_frame_13.rot_y"],
    ["historical_frame_14.pos_y", "historical_frame_14.rot_y"],
    ["historical_frame_15.pos_y", "historical_frame_15.rot_y"],
];

/// A Java armor stand's pose over a pack's stand (`ArmorStandModel.setupAnim`): each part's
/// rotation is the stand's own, in place of what the pack's pose animation gave it. Bedrock's
/// default pose holds the same numbers as Java's (`hlnmov`: left arm -10, 0, -10), so the degrees
/// carry over as they are. The body turns the waist, which carries the torso's sticks.
pub(super) fn pose_stand(pose: &mut horizon_bedrock::animation::Pose, stand: &super::StandPose) {
    const BONES: [&[&str]; 6] = [
        &["head"],
        &["waist"],
        &["leftarm"],
        &["rightarm"],
        &["leftleg"],
        &["rightleg"],
    ];
    for (names, r) in BONES.iter().zip(stand.pose) {
        for name in *names {
            let b = pose.bones.entry((*name).to_owned()).or_default();
            b.rotation = r;
            b.rotation_entity = [0.0; 3];
        }
    }
    let mut hide = |name: &str| {
        pose.bones.entry(name.to_owned()).or_default().scale = [0.0; 3];
    };
    if !stand.arms {
        hide("leftarm");
        hide("rightarm");
    }
    if !stand.base_plate {
        hide("baseplate");
    }
}

/// Items worn on the body that no hand holds as a model: armour, elytra, heads.
fn is_wearable(item: &str) -> bool {
    let name = item.trim_start_matches("minecraft:");
    ["_helmet", "_chestplate", "_leggings", "_boots", "_head", "_skull"]
        .iter()
        .any(|s| name.ends_with(s))
        || matches!(name, "elytra" | "carved_pumpkin" | "turtle_helmet")
}

/// The holder's bone an item in `slot` follows (`query.item_slot_to_bone_name`).
pub(super) fn slot_bone(slot: &str) -> &'static str {
    match slot {
        "main_hand" => "rightitem",
        "off_hand" => "leftitem",
        "head" => "head",
        "torso" | "body" => "body",
        "legs" => "leftleg",
        _ => "leftleg",
    }
}

/// The slot a `slot.weapon.mainhand`-style name means.
fn slot_of(name: &str) -> Option<&'static str> {
    Some(match name {
        "slot.weapon.mainhand" => "main_hand",
        "slot.weapon.offhand" => "off_hand",
        "slot.armor.head" => "head",
        "slot.armor.chest" => "torso",
        "slot.armor.legs" => "legs",
        "slot.armor.body" => "body",
        "slot.armor.feet" => "feet",
        _ => return None,
    })
}

/// Answers an attachable's expressions: the holder's own answers, plus what concerns the item.
struct AttachHost<'a> {
    inner: MobHost<'a>,
    slot: &'static str,
    /// The holder's variables, for `context.owning_entity->variable.x`.
    owner: &'a horizon_bedrock::molang::Vars,
}

/// Whether what `m` holds or wears in `slot` shows the enchantment glint
/// (`variable.is_enchanted` of an attachable).
fn slot_enchanted(m: &Mob, slot: &str) -> bool {
    use super::gear;
    let (index, stack) = match slot {
        "main_hand" => (gear::MAIN, m.extra.main_hand_stack()),
        "off_hand" => (gear::OFF, None),
        "head" => (gear::HEAD, m.extra.armor[0].as_ref()),
        "torso" => (gear::CHEST, m.extra.armor[1].as_ref()),
        "legs" => (gear::LEGS, m.extra.armor[2].as_ref()),
        "feet" => (gear::FEET, m.extra.armor[3].as_ref()),
        _ => return false,
    };
    stack.is_some_and(|s| s.has_foil(false)) || gear::enchanted(m, index)
}

impl Host for AttachHost<'_> {
    fn variables(&mut self) -> Vec<(&'static str, Value)> {
        let enchanted = slot_enchanted(self.inner.m, self.slot);
        let mut vars = self.inner.variables();
        for (name, value) in &mut vars {
            if *name == "is_enchanted" {
                *value = Value::bool(enchanted);
            }
        }
        vars
    }
    fn query(&mut self, name: &str, args: &[Value]) -> Value {
        match name {
            "is_attached" => Value::Num(1.0),
            // The entity that holds the item is the attachable's owner.
            "is_owner_identifier_any" => super::bedrock_debug::query(name)
                .map(Value::Num)
                .unwrap_or_else(|| Value::bool(holder_is_any(self.inner.m, args))),
            "get_animation_frame" => {
                let m = self.inner.m;
                let slot = self.slot;
                let item = held_items(m)
                    .into_iter()
                    .find_map(|(s, i)| (s == slot).then_some(i));
                Value::Num(item.map_or(0.0, |i| item_frame(m, &i)))
            }
            "item_slot_to_bone_name" => {
                let slot = match args.first() {
                    Some(Value::Str(s)) => s.to_string(),
                    _ => self.slot.to_owned(),
                };
                Value::Str(slot_bone(&slot).into())
            }
            _ => self.inner.query(name, args),
        }
    }
    fn context(&mut self, name: &str) -> Value {
        match name {
            "item_slot" => Value::Str(self.slot.into()),
            "is_first_person" => Value::bool(self.inner.m.extra.first_person),
            "owning_entity" => Value::Entity(u64::from(self.inner.m.id)),
            _ => Value::Null,
        }
    }
    fn entity_var(&mut self, _entity: u64, var: horizon_bedrock::molang::Sym) -> Value {
        self.owner.get(&var).cloned().unwrap_or_default()
    }
    fn entity_query(&mut self, _entity: u64, name: &str, args: &[Value]) -> Value {
        self.inner.query(name, args)
    }
}

/// Whether `m`, as the holder of an attachable, is one of the entities `args` names
/// (`query.is_owner_identifier_any` on an attachable: the identifier of what wears or holds it, in
/// Bedrock's names). A mannequin stands for a player and for an armor stand.
fn holder_is_any(m: &Mob, args: &[Value]) -> bool {
    let java = m.kind.id.to_ascii_lowercase();
    let bedrock = bedrock_id(&java).unwrap_or(&java).to_owned();
    let mannequin = java == "minecraft:mannequin";
    args.iter().any(|a| {
        let Value::Str(s) = a else { return false };
        let want = s.trim().to_ascii_lowercase();
        let want = if want.contains(':') { want } else { format!("minecraft:{want}") };
        want == bedrock
            || want == java
            || (mannequin && matches!(want.as_str(), "minecraft:player" | "minecraft:armor_stand"))
    })
}

/// The packs' mob look: everything the bundle holds and what is running.
#[derive(Default)]
pub struct BedrockMobs {
    bundle: Bundle,
    parts: Mutex<HashMap<String, Option<Arc<Built>>>>,
    states: Mutex<HashMap<u32, MobState>>,
    queue: Mutex<Vec<Emitted>>,
    emitters: Mutex<Vec<Active>>,
    clock: Mutex<Option<Instant>>,
    /// The attachables' animations, by mob and slot.
    attached: Mutex<HashMap<(u32, &'static str), AttState>>,
    /// Item (lower case) to the attachables that dress it.
    item_index: HashMap<String, Vec<String>>,
    /// A geometry's bone bindings, parsed once.
    bindings: Mutex<HashMap<String, Arc<Vec<(String, horizon_bedrock::molang::Program)>>>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// How long a looping effect bound to a mob goes on after its animation last asked for it.
/// How long a state-held looping effect outlasts the state's last frame.
const HOLD_GRACE: std::time::Duration = std::time::Duration::from_millis(400);
const LOOP_GRACE: std::time::Duration = std::time::Duration::from_millis(2500);

/// How long a mob's running animations are kept without a draw.
const KEEP_SECONDS: u64 = 5;
/// The warm-up of a first-person view model that was not running: `WARM_STEPS` steps of
/// `WARM_STEP` seconds (the pack's slow transitions settle in a few seconds).
const WARM_STEP: f32 = 0.05;
const WARM_STEPS: usize = 120;

/// Java entities Bedrock calls something else (the identifier the other game gives the same mob).
const BEDROCK_IDS: &[(&str, &str)] = &[
    ("minecraft:villager", "minecraft:villager_v2"),
    ("minecraft:zombie_villager", "minecraft:zombie_villager_v2"),
    ("minecraft:zombified_piglin", "minecraft:zombie_pigman"),
    ("minecraft:evoker", "minecraft:evocation_illager"),
    ("minecraft:evoker_fangs", "minecraft:evocation_fang"),
    ("minecraft:illusioner", "minecraft:illusioner"),
    ("minecraft:end_crystal", "minecraft:ender_crystal"),
    ("minecraft:experience_bottle", "minecraft:xp_bottle"),
    ("minecraft:experience_orb", "minecraft:xp_orb"),
    ("minecraft:eye_of_ender", "minecraft:eye_of_ender_signal"),
    ("minecraft:trident", "minecraft:thrown_trident"),
    ("minecraft:wind_charge", "minecraft:wind_charge_projectile"),
    ("minecraft:firework_rocket", "minecraft:fireworks_rocket"),
    ("minecraft:tropical_fish", "minecraft:tropicalfish"),
    ("minecraft:snow_golem", "minecraft:snow_golem"),
    ("minecraft:potion", "minecraft:splash_potion"),
    ("minecraft:ender_pearl", "minecraft:ender_pearl"),
    ("minecraft:chest_minecart", "minecraft:chest_minecart"),
    ("minecraft:furnace_minecart", "minecraft:minecart"),
    ("minecraft:spawner_minecart", "minecraft:minecart"),
];

fn bedrock_id(java: &str) -> Option<&'static str> {
    BEDROCK_IDS
        .iter()
        .find_map(|(j, b)| (*j == java && j != b).then_some(*b))
}

/// The invisible part above a geometry's bones.
const ROOT: &str = "<root>";

/// Draws a face's texels from inside its rectangle: a pack's face edges sit on texel borders (or
/// between texels, with fractional sizes), and a sample taken right on one picks the neighbour's
/// colour, a thin line along the edge. Each edge moves a twentieth of a texel inward (more than the half pixel a multisampled edge reaches past it up close).
fn inset_uv(vertices: &mut [[f32; 5]; 4], texture: [f32; 2]) {
    const INSET: f32 = 0.05;
    for axis in 0..2 {
        let size = texture[axis].max(1.0);
        let i = 3 + axis;
        let (lo, hi) = vertices
            .iter()
            .fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v[i]), hi.max(v[i])));
        let span = (hi - lo) * size;
        let step = INSET.min(span * 0.25) / size;
        let mid = (lo + hi) / 2.0;
        for v in vertices.iter_mut() {
            if v[i] <= lo + f32::EPSILON {
                v[i] += step;
            } else if v[i] >= hi - f32::EPSILON {
                v[i] -= step;
            } else if v[i] < mid {
                v[i] += step;
            } else {
                v[i] -= step;
            }
        }
    }
}

fn bake(g: &Geometry) -> Built {
    fn convert(p: geometry::Part, names: &mut Vec<String>, texture: [f32; 2]) -> Part {
        let name = p.name.to_ascii_lowercase();
        names.push(name.clone());
        let polygons = if p.never_render {
            Vec::new()
        } else {
            p.quads
                .into_iter()
                .map(|mut q| {
                    inset_uv(&mut q.vertices, texture);
                    Polygon {
                        normal: q.normal,
                        vertices: q.vertices,
                        cull_back: false,
                    }
                })
                .collect()
        };
        Part {
            name,
            pose: p.pose,
            polygons,
            children: p
                .children
                .into_iter()
                .map(|c| convert(c, names, texture))
                .collect(),
        }
    }
    let mut names = Vec::new();
    // The tree's roots become children of one invisible root part.
    let children: Vec<Part> = g
        .to_tree()
        .into_iter()
        .map(|p| convert(p, &mut names, g.texture_size))
        .collect();
    // Not `root`: a pack's own `root` bone has its animation, and a shared name would apply it twice.
    let root = Part {
        name: ROOT.into(),
        pose: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        polygons: Vec::new(),
        children,
    };
    names.push(ROOT.into());
    Built {
        part: Arc::new(root),
        names,
        geometry: g.id.clone(),
        single_texel: g.texture_size == [1.0, 1.0],
    }
}

impl BedrockMobs {
    /// The bundle of the resource root `root` (empty without a Bedrock pack).
    pub fn load(root: &Path) -> BedrockMobs {
        let bundle = Bundle::load(root);
        if super::bedrock_debug::debug() && !bundle.ignored.is_empty() {
            let lines = horizon_bedrock::audit::lines(&bundle.ignored);
            log::info!(
                "bedrock: {} keys the pack carries and the engine ignores:\n{}",
                lines.len(),
                lines.join("\n")
            );
        }
        // The options the player chose for the packs, in the game folder.
        let game = std::env::current_dir().unwrap_or_default();
        horizon_bedrock::settings::install(
            &bundle.settings,
            &horizon_bedrock::settings::Config::read(&game).choices(),
        );
        let mut item_index: HashMap<String, Vec<String>> = HashMap::new();
        for (id, def) in &bundle.attachables {
            item_index.entry(id.clone()).or_default().push(id.clone());
            for (item, _) in &def.items {
                item_index
                    .entry(item.to_ascii_lowercase())
                    .or_default()
                    .push(id.clone());
            }
        }
        for ids in item_index.values_mut() {
            ids.sort();
            ids.dedup();
        }
        BedrockMobs {
            bundle,
            item_index,
            ..Default::default()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.bundle.is_empty()
    }

    /// Hands the game's own particles that this pack draws itself (its
    /// `minecraft:rain_splash_particle`) to it: once for the pack in use, every frame cheap.
    pub fn take_game_particles(&self) {
        let key = std::ptr::from_ref(self) as usize;
        if let Some(ids) = horizon_client_render::bedrock_particles::install_for(key, |id| {
            self.bundle.effects.get(id).is_some_and(|fx| fx.one_per_request())
        }) {
            log::info!(
                "bedrock: {} game particles drawn by the pack: {}",
                ids.len(),
                ids.join(", ")
            );
        }
    }

    pub fn bundle(&self) -> &Bundle {
        &self.bundle
    }

    /// The client entity that dresses the Java entity `id` (`minecraft:villager`): the one with
    /// the same identifier, or the one Bedrock calls it by.
    fn entity_def(&self, id: &str) -> Option<&horizon_bedrock::entity::ClientEntity> {
        let id = id.to_ascii_lowercase();
        self.bundle
            .entities
            .get(&id)
            .or_else(|| self.bundle.entities.get(bedrock_id(&id)?))
    }

    /// Whether the pack draws its own flames on `entity`: it names an effect for fire, so the
    /// game's flame quads stay out of the way.
    pub fn replaces_flames(&self, entity: &str) -> bool {
        !self.bundle.entities.is_empty()
            && self.entity_def(entity).is_some_and(|d| {
                d.particles.keys().any(|k| {
                    let k = k.to_ascii_lowercase();
                    k.contains("fire") || k.contains("flame") || k.contains("burn")
                })
            })
    }

    /// Whether the pack dresses `entity` (`minecraft:zombie`).
    /// How the pack has what `m` holds drawn, as of its last frame (`None` before the first or
    /// when no pack dresses it).
    pub fn held(&self, m: &Mob) -> Option<Held> {
        lock(&self.states)
            .get(&m.id)
            .and_then(|s| s.frame.as_ref())
            .map(|f| f.held)
    }

    pub fn has_entity(&self, entity: &str) -> bool {
        !self.bundle.entities.is_empty()
            && (players_enabled() || !is_avatar(entity))
            && self.entity_def(entity).is_some()
    }

    /// Whether the geometry has a cube turned inside out (a negative size: a hollow shell, an
    /// inverted core), whose far walls a single-sided material would hide behind its near ones.
    pub(super) fn has_inside_out(&self, geometry: &str) -> bool {
        static CACHE: Mutex<Option<HashMap<String, bool>>> = Mutex::new(None);
        let mut cache = lock(&CACHE);
        *cache
            .get_or_insert_with(HashMap::new)
            .entry(geometry.to_owned())
            .or_insert_with(|| {
                self.bundle.geometries.get(geometry).is_some_and(|g| {
                    g.bones
                        .iter()
                        .flat_map(|b| &b.cubes)
                        .any(|c| c.size.iter().any(|s| *s < 0.0))
                })
            })
    }

    fn built(&self, geometry: &str) -> Option<Arc<Built>> {
        let mut parts = lock(&self.parts);
        parts
            .entry(geometry.to_owned())
            .or_insert_with(|| {
                self.bundle
                    .geometries
                    .get(geometry)
                    .map(|g| Arc::new(bake(g)))
            })
            .clone()
    }

    /// Every texture the entity may wear, as atlas names (so the game can ask for them).
    pub fn textures_of(&self, entity: &str) -> Vec<String> {
        self.entity_def(entity)
            .map(|d| d.textures.values().map(|t| atlas_name(t)).collect())
            .unwrap_or_default()
    }

    /// This frame of `m`, run once per `(mob, age + alpha)`; `None` when the pack does not dress
    /// the mob.
    pub fn frame(&self, m: &Mob, alpha: f32, from_camera: [f32; 3]) -> Option<Arc<MobFrame>> {
        let avatar = matches!(&*m.kind.id, "minecraft:player" | "minecraft:mannequin");
        if avatar && !players_enabled() {
            return None;
        }
        let def = self.entity_def(&m.kind.id)?;
        let time = m.age as f32 + alpha;
        // A screen's preview (the inventory's player box) keeps its own state, so it neither
        // reads nor replaces the world's frame, and it re-poses every draw: the cursor moves it.
        let gui = super::cem_anim::is_in_gui();
        let key = if gui { m.id | 0x8000_0000 } else { m.id };
        let mut states = lock(&self.states);
        // The first-person view model runs only while the camera is in first person; Bedrock keeps
        // its controllers running all along, so a state that is new or was left (third person) is
        // brought to where it would have settled, not shown at its first state.
        let mut warm = !gui && m.extra.first_person && !states.contains_key(&key);
        let st = states.entry(key).or_insert_with(|| MobState {
            instance: Instance::new(u64::from(m.id) + 1),
            time,
            frame: None,
            seen: Instant::now(),
            dumped: f32::NEG_INFINITY,
            live: None,
            water: WaterEntry::default(),
        });
        let since = st.seen.elapsed().as_secs_f32();
        warm |= !gui && m.extra.first_person && since > 0.3;
        st.seen = Instant::now();
        if !gui
            && st.time == time
            && let Some(f) = &st.frame
        {
            return Some(f.clone());
        }
        let dt = if gui {
            since.clamp(0.0, 0.1)
        } else {
            ((time - st.time) / 20.0).clamp(0.0, 0.1)
        };
        let assets = Assets {
            animations: &self.bundle.library,
            render_controllers: &self.bundle.render_controllers,
            geometries: Some(&self.bundle.geometries),
        };
        let entry_speed = if gui { None } else { st.water.look(m.age, m.fluids.in_water, vertical_speed(m)) };
        let mut host = MobHost {
            m,
            alpha,
            dt,
            from_camera,
            entry_speed,
        };
        if warm {
            let mut host = MobHost {
                m,
                alpha,
                dt: WARM_STEP,
                from_camera,
                entry_speed: None,
            };
            for _ in 0..WARM_STEPS {
                st.instance.update(def, &assets, &mut host, WARM_STEP);
            }
        }
        let mut frame = st.instance.update(def, &assets, &mut host, dt);
        if let Some(stand) = &m.extra.stand_pose {
            pose_stand(&mut frame.pose, stand);
        }
        st.time = time;
        if !gui && avatar && m.extra.fall_flying {
            // The same for a glide: a pack that follows the look with `root` itself (the
            // gliding animation turns it by `-target_x_rotation`) turns the body a second time
            // over the game's pitch, and a climb straight up lies it back down.
            let turned = frame.pose.get("root").is_some_and(|b| {
                (b.rotation[0] + b.rotation_entity[0]).abs() >= 5.0
            });
            if turned {
                set_pack_glides(m.id);
            }
        }
        if !gui && avatar && m.extra.avatar_pose == crate::pose::Pose::Swimming {
            // A pack that lies the body down itself (the swim animations turn `root` over) does
            // not want the game's own swimming turn on top: that is a body tilted twice. The
            // animations blend by the way the body goes (a diagonal swim weighs the forward
            // one at 0.7, a root 55 degrees over), so the first time any of the turn shows
            // while swimming settles it for the pack.
            let turned = frame.pose.get("root").is_some_and(|b| {
                (b.rotation[0] + b.rotation_entity[0]).abs() >= 15.0
            });
            if turned {
                set_pack_lies_down(m.id, true);
            }
        }
        if !gui && super::camel::is_camel(m.species()) {
            let moved = |n: &str| frame.pose.get(n).map_or(0.0, |b| b.position[1]);
            super::camel::set_model_drop(m.id, (moved("root") + moved("body")) / 16.0);
        }
        if !gui
            && (m.extra.passenger || m.rider.is_some() || super::bedrock_debug::is_traced_mount(m.id))
        {
            trace_pose(m, alpha, time, dt, &frame);
        }
        if dump_wanted(&m.kind.id)
            && time - st.dumped
                >= match super::bedrock_debug::settings().dump_every {
                    e if e > 0.0 => e,
                    _ => 20.0,
                }
            && from_camera.iter().map(|v| v * v).sum::<f32>() < 144.0
        {
            st.dumped = time;
            log::info!(
                "bedrock-dump {}#{} age={} {}",
                m.kind.id,
                m.id,
                m.age,
                dump_line(m, &mut host, &frame, &st.instance.describe(def, &assets)),
            );
            let vars: Vec<String> = super::bedrock_debug::settings()
                .dump_vars
                .iter()
                .map(|v| {
                    let v = v.as_str();
                    format!(
                        "v.{v}={:?}",
                        st.instance.vars.get(&horizon_bedrock::molang::intern(v))
                    )
                })
                .collect();
            if !vars.is_empty() {
                log::info!("bedrock-dump-vars {}#{} {}", m.kind.id, m.id, vars.join(" "));
            }
        }
        let layers: Vec<DrawLayer> = frame
            .layers
            .into_iter()
            .filter_map(|layer| {
                // The player's body is the skin's own: its model and texture are the game's.
                let geometry = if avatar && m.slim && layer.geometry == "geometry.humanoid.custom" {
                    "geometry.humanoid.customslim"
                } else {
                    layer.geometry.as_str()
                };
                let built = self.built(geometry)?;
                let skinned = avatar && layer.texture == "textures/entity/steve";
                // A pack's model samples the skin as it was drawn (its unused base-layer
                // texels transparent): the game's own opaque-base copy is the fallback.
                let skin = skinned
                    .then(|| m.skin.clone().unwrap_or_else(|| m.kind.texture.to_string()));
                Some(DrawLayer {
                    built,
                    texture: match &skin {
                        Some(skin) => crate::skins::raw_skin_location(skin),
                        None => atlas_name(&layer.texture),
                    },
                    fallback: match skin {
                        Some(skin) => Some(skin),
                        None => horizon_bedrock::convert::java_texture(&layer.texture),
                    },
                    layer,
                })
            })
            .collect();
        // The mob as it is now, for the effects that follow it.
        let lerp = |a: f64, b: f64| a + (b - a) * f64::from(alpha);
        let [cx, cy, cz] = from_camera;
        let live = Live {
            // Bedrock's position of a player is at its eyes, which the packs
            // take 1.62 from to get back to the feet.
            position: [
                lerp(m.previous.x, m.position.x),
                lerp(m.previous.y, m.position.y)
                    + if m.kind.id == "minecraft:player" { 1.62 } else { 0.0 },
                lerp(m.previous.z, m.position.z),
            ],
            delta: [
                m.position.x - m.previous.x,
                m.position.y - m.previous.y,
                m.position.z - m.previous.z,
            ],
            body_yaw: wrap_degrees(m.previous_body_yaw + (m.body_yaw - m.previous_body_yaw) * alpha),
            target: [
                wrap_degrees(
                    m.previous_head_yaw + (m.head_yaw - m.previous_head_yaw) * alpha
                        - (m.previous_body_yaw + (m.body_yaw - m.previous_body_yaw) * alpha),
                ),
                m.previous_head_pitch + (m.head_pitch - m.previous_head_pitch) * alpha,
            ],
            local_player: m.extra.local_player,
            sneaking: m.extra.crouching || m.extra.sneaking,
            gliding: m.extra.fall_flying,
            vertical_speed: entry_speed.unwrap_or_else(|| vertical_speed(m)),
            from_camera: (cx * cx + cy * cy + cz * cz).sqrt(),
        };
        let pos = [m.position.x, m.position.y, m.position.z];
        let mut queue = lock(&self.queue);
        for e in frame.emits {
            let offset = match &e {
                Emit::Particle { locator, .. } | Emit::Hold { locator, .. } => locator
                    .as_deref()
                    .and_then(|l| {
                        layers.iter().find_map(|d| {
                            self.bundle.geometries.get(&d.layer.geometry)?.locator(l)
                        })
                    })
                    .map_or([0.0; 3], |l| l.1.offset.map(|v| f64::from(v) / 16.0)),
                Emit::Sound(_) => [0.0; 3],
            };
            self.send_emit(&mut queue, m, &live, e, offset, None);
        }
        let scale = frame.scale * builtin_scale(&m.kind.id, m.is_baby());
        let mut held = Held {
            hidden: frame.hide_held_items,
            scale: frame.held_item_scale,
            full_bright: frame.held_item_ignores_lighting,
            main_replaced: false,
            off_replaced: false,
        };
        self.parent_setups(m, alpha, dt, from_camera, &mut st.instance.vars);
        let mut attached =
            self.attachments(m, alpha, dt, from_camera, &st.instance.vars, (&layers, &frame.pose));
        for a in &mut attached {
            for (e, offset) in std::mem::take(&mut a.emits) {
                self.send_emit(&mut queue, m, &live, e, offset, Some(a.slot));
            }
        }
        let dressed = |slot: &str| attached.iter().any(|a| a.slot == slot && !a.layers.is_empty());
        held.main_replaced = dressed("main_hand");
        held.off_replaced = dressed("off_hand");
        if held.hidden {
            attached.retain(|a| !matches!(a.slot, "main_hand" | "off_hand"));
        }
        // An attachable's animations may name bones of its holder that its own model lacks (the
        // arm a held item raises, the hand bone it moves): they drive the holder's bones, added
        // to what the holder's own animations gave, as animations of one entity stack.
        let mut holder_pose = frame.pose;
        for a in &attached {
            for (bone, drive) in &a.pose.bones {
                let own = a.layers.iter().any(|d| d.built.names.iter().any(|n| n == bone));
                let holder_has = layers.iter().any(|d| d.built.names.iter().any(|n| n == bone));
                if own || !holder_has {
                    continue;
                }
                let at = holder_pose.bones.entry(bone.clone()).or_default();
                for k in 0..3 {
                    at.rotation[k] += drive.rotation[k];
                    at.rotation_entity[k] += drive.rotation_entity[k];
                    at.position[k] += drive.position[k];
                    at.scale[k] *= drive.scale[k];
                }
                at.entity_frame |= drive.entity_frame;
            }
        }
        let out = Arc::new(MobFrame {
            layers,
            pose: holder_pose,
            scale,
            scale_axes: frame.scale_axes,
            held,
            attached,
        });
        st.frame = Some(out.clone());
        drop(queue);
        st.live = Some(live.clone());
        for a in lock(&self.emitters).iter_mut() {
            if let Some(f) = &a.follow
                && f.mob == m.id
            {
                // A dead mob burns no more.
                if !m.alive() && a.emitter.is_looping() {
                    a.emitter.stop();
                }
                let at = attach_point(pos, f.offset, m.body_yaw);
                a.emitter
                    .set_transform([at[0] as f32, at[1] as f32, at[2] as f32], m.body_yaw);
                a.host.live = Some(live.clone());
            }
        }
        Some(out)
    }

    /// Plays or starts what a frame set off: a sound at the mob, a particle effect at `offset`
    /// (blocks from the feet, in the model's axes) that then follows the mob. `slot` names the
    /// held or worn item that set it off, so its effects and the mob's own stay apart.
    fn send_emit(
        &self,
        queue: &mut Vec<Emitted>,
        m: &Mob,
        live: &Live,
        e: Emit,
        offset: [f64; 3],
        slot: Option<&str>,
    ) {
        let pos = [m.position.x, m.position.y, m.position.z];
        let offset = if m.extra.first_person {
            first_person_offset(m, offset)
        } else {
            offset
        };
        let key = |locator: Option<String>| match slot {
            Some(slot) => Some(format!("{slot}:{}", locator.unwrap_or_default())),
            None => locator,
        };
        match e {
            Emit::Sound(event) => {
                let event = if event.contains(':') {
                    event.to_ascii_lowercase()
                } else {
                    format!("bedrock:{}", event.to_ascii_lowercase())
                };
                queue.push(Emitted::Sound { event, at: pos });
            }
            Emit::Hold {
                effect,
                locator,
                pre,
            } => {
                if !m.alive() {
                    return;
                }
                self.hold_particle(
                    &effect,
                    attach_point(pos, offset, m.body_yaw),
                    m.body_yaw,
                    m.is_baby(),
                    m.custom_name.clone(),
                    box_size(m),
                    Some(Follow {
                        mob: m.id,
                        locator: key(locator),
                        offset,
                    }),
                    Init {
                        script: pre,
                        seed: Default::default(),
                    },
                    // The mob's states are locked here: its live data comes from this frame.
                    Some(live.clone()),
                );
            }
            Emit::Particle {
                effect,
                locator,
                pre,
            } => {
                queue.push(Emitted::Particle {
                    effect,
                    at: attach_point(pos, offset, m.body_yaw),
                    yaw: m.body_yaw,
                    baby: m.is_baby(),
                    name: m.custom_name.clone(),
                    size: box_size(m),
                    follow: Some(Follow {
                        mob: m.id,
                        locator: key(locator),
                        offset,
                    }),
                    init: Init {
                        script: pre,
                        seed: Default::default(),
                    },
                });
            }
        }
    }

    /// The look of what `m` holds and wears that the pack dresses, one entry per slot.
    fn attachments(
        &self,
        m: &Mob,
        alpha: f32,
        dt: f32,
        from_camera: [f32; 3],
        owner: &horizon_bedrock::molang::Vars,
        holder: (&[DrawLayer], &horizon_bedrock::animation::Pose),
    ) -> Vec<Attached> {
        if self.bundle.attachables.is_empty() {
            return Vec::new();
        }
        let held = held_items(m);
        if super::bedrock_debug::settings().dumps(&m.kind.id) {
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            super::render::log_bedrock_once(format!("attach/{}/{secs}", m.kind.id), || {
                format!(
                    "bedrock-attach {}#{}: held {:?}; candidates {:?}",
                    m.kind.id,
                    m.id,
                    held,
                    held.iter()
                        .map(|(_, i)| attachables_for(&self.item_index, &i.to_ascii_lowercase()).cloned())
                        .collect::<Vec<_>>()
                )
            });
        }
        let mut out = Vec::new();
        let mut states = lock(&self.attached);
        states.retain(|(mob, slot), s| {
            (*mob != m.id || held.iter().any(|(h, _)| h == slot))
                && s.seen.elapsed().as_secs() < KEEP_SECONDS
        });
        let assets = Assets {
            animations: &self.bundle.library,
            render_controllers: &self.bundle.render_controllers,
            geometries: Some(&self.bundle.geometries),
        };
        let time = m.age as f32 + alpha;
        for (slot, item) in &held {
            let item_lc = item.to_ascii_lowercase();
            // Armour is dressed on its slot; in a hand it is an item like any other.
            if matches!(*slot, "main_hand" | "off_hand") && is_wearable(&item_lc) {
                continue;
            }
            let Some(candidates) = attachables_for(&self.item_index, &item_lc) else {
                continue;
            };
            let mut host = AttachHost {
                inner: MobHost {
                    m,
                    alpha,
                    dt,
                    from_camera,
                    entry_speed: None,
                },
                slot,
                owner,
            };
            let chosen = self.choose_attachable(candidates, &item_lc, &mut host);
            let Some(id) = chosen else { continue };
            let Some(def) = self.bundle.attachables.get(id) else {
                continue;
            };
            let mut warm = m.extra.first_person && !states.contains_key(&(m.id, slot));
            let st = states.entry((m.id, slot)).or_insert_with(|| AttState {
                id: id.clone(),
                instance: Instance::new(u64::from(m.id) * 8 + 3),
                time,
                seen: Instant::now(),
                bound_to: None,
            });
            warm |= m.extra.first_person && st.seen.elapsed().as_secs_f32() > 0.3;
            if st.id != *id {
                warm |= m.extra.first_person;
                *st = AttState {
                    id: id.clone(),
                    instance: Instance::new(u64::from(m.id) * 8 + 3),
                    time,
                    seen: Instant::now(),
                    bound_to: None,
                };
            }
            st.seen = Instant::now();
            st.time = time;
            // Where its bones are on the holder, for a pack that asks (a glow turned to the
            // camera from the bone it shines from: a torch's flame, a harness's lamps).
            {
                let target = st.bound_to.clone().unwrap_or_else(|| slot_bone(slot).to_owned());
                st.instance.place = holder.0.iter().find_map(|d| {
                    let g = self.bundle.geometries.get(&d.layer.geometry)?;
                    horizon_bedrock::entity::bone_place(g, holder.1, &target)
                });
                // Its bones turned relative to the entity take the hand's turn out (a lantern
                // hangs upright from the raised arm), in first person as in third: the
                // first-person view model is the holder's own skeleton.
                st.instance.root_rotation = holder.0.iter().find_map(|d| {
                    let g = self.bundle.geometries.get(&d.layer.geometry)?;
                    horizon_bedrock::entity::bone_world_rotation(g, holder.1, &target)
                });
            }
            // The pack aims its glows at the camera itself (`q.rotation_to_camera`, the eye
            // 1.62 above the feet in first person).
            st.instance.camera = None;
            if warm {
                let mut warming = AttachHost {
                    inner: MobHost {
                        m,
                        alpha,
                        dt: WARM_STEP,
                        from_camera,
                        entry_speed: None,
                    },
                    slot,
                    owner,
                };
                for _ in 0..WARM_STEPS {
                    st.instance.update(def, &assets, &mut warming, WARM_STEP);
                }
            }
            let mut frame = st.instance.update(def, &assets, &mut host, dt);
            // Where each effect starts: its locator in the attachable's model, carried to the
            // holder's bone the item is bound to (Bedrock draws the attachable as part of the
            // holder's skeleton; its y 24 sits on the bone); the bone itself without a locator.
            let place = st.instance.place;
            let emits: Vec<(Emit, [f64; 3])> = std::mem::take(&mut frame.emits)
                .into_iter()
                .map(|e| {
                    let locator = match &e {
                        Emit::Particle { locator, .. } | Emit::Hold { locator, .. } => locator.clone(),
                        Emit::Sound(_) => None,
                    };
                    let local = locator
                        .as_deref()
                        .and_then(|l| {
                            frame.layers.iter().find_map(|layer| {
                                self.bundle.geometries.get(&layer.geometry)?.locator(l)
                            })
                        })
                        .map_or([0.0, 24.0, 0.0], |(_, l)| l.offset);
                    let p = [local[0], local[1] - 24.0, local[2]];
                    let at = place.map_or([0.0, 18.0, 0.0], |t| {
                        std::array::from_fn(|r| t[r][0] * p[0] + t[r][1] * p[1] + t[r][2] * p[2] + t[r][3])
                    });
                    (e, at.map(|v| f64::from(v) / 16.0))
                })
                .collect();
            let layers: Vec<DrawLayer> = frame
                .layers
                .into_iter()
                .filter_map(|layer| {
                    let Some(built) = self.built(&layer.geometry) else {
                        if super::bedrock_debug::debug() {
                            super::render::log_bedrock_once(format!("{}/{}/{}", m.kind.id, slot, layer.geometry), || {
                                format!(
                                    "{}: {slot} layer {} ({}) dropped: the geometry is not in the pack",
                                    m.kind.id, layer.geometry, layer.texture
                                )
                            });
                        }
                        return None;
                    };
                    Some(DrawLayer {
                        built,
                        texture: atlas_name(&layer.texture),
                        fallback: horizon_bedrock::convert::java_texture(&layer.texture),
                        layer,
                    })
                })
                .collect();
            let mut binds = HashMap::new();
            let mut rng = horizon_bedrock::molang::Rng::new(1);
            for d in &layers {
                for (bone, program) in self.bindings_of(&d.layer.geometry).iter() {
                    let v = program.eval(&mut horizon_bedrock::molang::EvalCtx {
                        host: &mut host,
                        vars: &mut st.instance.vars,
                        rng: &mut rng,
                        this: Value::Null,
                    });
                    if let Value::Str(to) = v {
                        binds.insert(bone.clone(), to.to_ascii_lowercase());
                    }
                }
            }
            st.bound_to = binds.values().next().cloned();
            // The debug file's `bind_bones`: another bone of the holder than the pack's.
            {
                let debug = super::bedrock_debug::settings();
                for (bone, to) in binds.iter_mut() {
                    if let Some(other) = debug.bind_for(slot, bone) {
                        *to = other.to_ascii_lowercase();
                    }
                }
            }
            // What is held, with no binding of its own, sits in the hand.
            if binds.is_empty() && matches!(*slot, "main_hand" | "off_hand") {
                for d in &layers {
                    for bone in self
                        .bundle
                        .geometries
                        .get(&d.layer.geometry)
                        .into_iter()
                        .flat_map(|g| g.bones.iter().filter(|b| b.parent.is_none()))
                    {
                        binds.insert(bone.name.to_ascii_lowercase(), slot_bone(slot).to_owned());
                    }
                }
            }
            // The vanilla elytra opens in the game's own code (`ElytraAnimationState`: flight,
            // crouching, rest), not in an animation: the vanilla `leftwing`/`rightwing` follow
            // those angles, whatever the pack's animations leave them at (a pack that animates
            // only its own wing bones leaves them closed in flight; the vanilla crouch pose
            // barely opens them).
            let mut pose = frame.pose;
            if *slot == "torso" {
                let angles = m.extra.elytra.angles(alpha).map(f32::to_degrees);
                for (bone, side) in [("leftwing", 1.0_f32), ("rightwing", -1.0)] {
                    let has_bone = layers.iter().any(|d| {
                        self.bundle.geometries.get(&d.layer.geometry).is_some_and(|g| {
                            g.bones.iter().any(|b| b.name.eq_ignore_ascii_case(bone))
                        })
                    });
                    if has_bone {
                        pose.bones.entry(bone.to_owned()).or_default().rotation =
                            [angles[0], side * angles[1], side * angles[2]];
                    }
                }
            }
            if super::bedrock_debug::settings().dumps(&m.kind.id) {
                let secs = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs());
                super::render::log_bedrock_once(format!("attached/{}/{slot}/{secs}", m.kind.id), || {
                    format!(
                        "bedrock-attached {}#{} {slot}: attachable {id}, {} layers {:?}, binds {:?}",
                        m.kind.id,
                        m.id,
                        layers.len(),
                        layers.iter().enumerate().map(|(i, d)| (i, d.layer.geometry.as_str(), d.texture.as_str())).collect::<Vec<_>>(),
                        binds
                    )
                });
            }
            out.push(Attached {
                slot,
                item: item_lc.clone(),
                layers,
                pose,
                binds,
                emits,
            });
        }
        out
    }

    /// The first attachable of `candidates` whose condition for `item` holds.
    fn choose_attachable<'s>(
        &'s self,
        candidates: &'s [String],
        item_lc: &str,
        host: &mut AttachHost,
    ) -> Option<&'s String> {
        candidates.iter().find(|id| {
            let Some(def) = self.bundle.attachables.get(*id) else {
                return false;
            };
            if id.as_str() == item_lc {
                return true;
            }
            def.items.iter().any(|(i, cond)| {
                i.eq_ignore_ascii_case(item_lc)
                    && cond.as_ref().is_none_or(|c| {
                        let mut vars = horizon_bedrock::molang::Vars::default();
                        let mut rng = horizon_bedrock::molang::Rng::new(1);
                        c.eval(&mut horizon_bedrock::molang::EvalCtx {
                            host: &mut *host,
                            vars: &mut vars,
                            rng: &mut rng,
                            this: Value::Null,
                        })
                        .truthy()
                    })
            })
        })
    }

    /// An attachable's `parent_setup` runs on the holder: it sets the holder's variables (that
    /// the item is held, which arm poses to use) the holder's animations and the item read.
    fn parent_setups(
        &self,
        m: &Mob,
        alpha: f32,
        dt: f32,
        from_camera: [f32; 3],
        holder: &mut horizon_bedrock::molang::Vars,
    ) {
        if self.bundle.attachables.is_empty() {
            return;
        }
        // What the holder had before these scripts run, for `c.owning_entity -> v.x` (the
        // vanilla attachables set the holder's `use_attachable_animations_*` only when its
        // `has_detached_system` says it has the first-person rig for them).
        let owner = holder.clone();
        let mut rng = horizon_bedrock::molang::Rng::new(1);
        for (slot, item) in &held_items(m) {
            let item_lc = item.to_ascii_lowercase();
            if matches!(*slot, "main_hand" | "off_hand") && is_wearable(&item_lc) {
                continue;
            }
            let Some(candidates) = attachables_for(&self.item_index, &item_lc) else {
                continue;
            };
            let mut host = AttachHost {
                inner: MobHost {
                    m,
                    alpha,
                    dt,
                    from_camera,
                    entry_speed: None,
                },
                slot,
                owner: &owner,
            };
            let Some(id) = self.choose_attachable(candidates, &item_lc, &mut host) else {
                continue;
            };
            let Some(def) = self.bundle.attachables.get(id) else {
                continue;
            };
            for program in &def.parent_setup {
                program.eval(&mut horizon_bedrock::molang::EvalCtx {
                    host: &mut host,
                    vars: holder,
                    rng: &mut rng,
                    this: Value::Null,
                });
            }
        }
    }

    /// The bones of `geometry` that follow a bone of their holder, with the expression naming it.
    fn bindings_of(&self, geometry: &str) -> Arc<Vec<(String, horizon_bedrock::molang::Program)>> {
        lock(&self.bindings)
            .entry(geometry.to_owned())
            .or_insert_with(|| {
                Arc::new(
                    self.bundle
                        .geometries
                        .get(geometry)
                        .map(|g| {
                            g.bones
                                .iter()
                                .filter_map(|b| {
                                    let program =
                                        horizon_bedrock::molang::parse(b.binding.as_deref()?)
                                            .ok()?;
                                    Some((b.name.to_ascii_lowercase(), program))
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                )
            })
            .clone()
    }

    /// Forgets mobs not drawn for a while and returns what the frames set off since the last call.
    pub fn take_emitted(&self) -> Vec<Emitted> {
        let mut states = lock(&self.states);
        states.retain(|_, s| s.seen.elapsed().as_secs() < KEEP_SECONDS);
        // A looping effect an animation no longer asks for stops (a flame once the fire is out).
        for a in lock(&self.emitters).iter_mut() {
            if a.follow.is_some()
                && a.emitter.is_sustained()
                && a.requested.elapsed() > if a.held { HOLD_GRACE } else { LOOP_GRACE }
            {
                a.emitter.stop();
            }
        }
        // An effect attached to a mob that is gone goes with it.
        lock(&self.emitters).retain(|a| {
            a.follow
                .as_ref()
                .is_none_or(|f| states.contains_key(&f.mob))
        });
        drop(states);
        std::mem::take(&mut *lock(&self.queue))
    }

    /// Starts the effect `id` at `at`. Unknown effects are left out.
    pub fn spawn_particle(
        &self,
        id: &str,
        at: [f64; 3],
        yaw: f32,
        baby: bool,
        name: Option<String>,
        size: [f32; 2],
        follow: Option<Follow>,
        init: Init,
    ) {
        // Where the mob it follows was at the last frame, until the next one sets it again.
        let live = follow
            .as_ref()
            .and_then(|f| lock(&self.states).get(&f.mob).and_then(|s| s.live.clone()));
        self.start_particle(id, at, yaw, baby, name, size, follow, false, init, live);
    }

    /// Keeps `id` running for as long as the controller state that set it off lasts, starting
    /// it again if it ended meanwhile (a mob that left the view and came back).
    #[allow(clippy::too_many_arguments)]
    fn hold_particle(
        &self,
        id: &str,
        at: [f64; 3],
        yaw: f32,
        baby: bool,
        name: Option<String>,
        size: [f32; 2],
        follow: Option<Follow>,
        init: Init,
        live: Option<Live>,
    ) {
        self.start_particle(id, at, yaw, baby, name, size, follow, true, init, live);
    }

    #[allow(clippy::too_many_arguments)]
    fn start_particle(
        &self,
        id: &str,
        at: [f64; 3],
        yaw: f32,
        baby: bool,
        name: Option<String>,
        size: [f32; 2],
        follow: Option<Follow>,
        held: bool,
        init: Init,
        live: Option<Live>,
    ) {
        let Some(fx) = self.bundle.effects.get(&id.to_ascii_lowercase()) else {
            if super::bedrock_debug::debug() {
                log_first(&format!("particle {id} unknown"), 3);
            }
            return;
        };
        if super::bedrock_debug::debug() {
            log_first(
                &format!(
                    "particle {id} start: follow={} held={held} sustained={} texture={} material-lit={}",
                    follow.is_some(),
                    Emitter::new(fx.clone(), 1).is_sustained(),
                    fx.texture,
                    fx.lit
                ),
                4,
            );
            // What the effect reads of its mob when it starts (a splash reads how fast it fell);
            // a state that goes on only keeps it (held), it does not start it again.
            if !held
                && let Some(l) = &live
            {
                log_first(
                    &format!(
                        "particle {id} live: vertical_speed={:.1} delta_y={:.2}",
                        l.vertical_speed, l.delta[1]
                    ),
                    3,
                );
            }
        }
        // One effect at a time per mob and locator: an animation that sets it off every loop
        // does not pile them up.
        // The same effect on the same mob and locator, wherever the locator is now (a held
        // item's moves with the hand every frame): that one follows it.
        if let Some(f) = &follow
            && let Some(a) = lock(&self.emitters).iter_mut().find(|a| {
                !a.emitter.finished()
                    && (!held || a.emitter.emitting())
                    && a.follow
                        .as_ref()
                        .is_some_and(|g| g.mob == f.mob && g.locator == f.locator)
                    && a.script == init.script
                    && std::ptr::eq(a.emitter.effect(), &**fx)
            })
        {
            // Asked for again: the animation that sets it off is still running.
            a.requested = Instant::now();
            a.held |= held;
            a.follow = Some(f.clone());
            return;
        }
        let mut e = Emitter::new(
            fx.clone(),
            (at[0].to_bits() ^ at[2].to_bits().rotate_left(17)) | 1,
        );
        // A one-shot effect is not started again by a state that merely goes on.
        if held && !e.is_sustained() {
            return;
        }
        e.set_transform([at[0] as f32, at[1] as f32, at[2] as f32], yaw);
        e.set_init(init.script.as_deref(), init.seed);
        // A manual effect set off once (an animation's event, the game's own particle) makes one
        // particle; held by a state, it makes none, as in Bedrock.
        if !held {
            e.request_manual(1);
        }
        let mut emitters = lock(&self.emitters);
        // A runaway pack cannot fill memory.
        if emitters.len() < 512 {
            emitters.push(Active {
                emitter: e,
                host: Snapshot {
                    baby,
                    name,
                    age: 0.0,
                    size,
                    live,
                },
                follow,
                script: init.script,
                requested: Instant::now(),
                held,
            });
        }
    }

    /// Advances the running effects to now, handing back the sprites to draw and the effects and
    /// sounds they set off in turn.
    pub fn step_particles(&self) -> (Vec<Drawn>, Vec<Emitted>) {
        self.step_particles_on(None)
    }

    /// [`Self::step_particles`] with the blocks around: particles collide with them and know
    /// the block they are in (a rain drop's splash lands on the ground).
    pub fn step_particles_on(
        &self,
        ground: Option<&dyn Ground>,
    ) -> (Vec<Drawn>, Vec<Emitted>) {
        let now = Instant::now();
        let dt = {
            let mut clock = lock(&self.clock);
            let dt = clock
                .map_or(0.0, |t| now.duration_since(t).as_secs_f32())
                .min(0.1);
            *clock = Some(now);
            dt
        };
        let mut emitters = lock(&self.emitters);
        let mut drawn = Vec::new();
        let mut more = Vec::new();
        for a in emitters.iter_mut() {
            a.host.age += dt * 20.0;
            let mut out = Vec::new();
            let carrier = Carrier(a.host.size, ground);
            a.emitter.update(dt, &mut a.host, &carrier, &mut out);
            for s in out {
                match s {
                    Spawn::Effect { effect, at, seed } => more.push(Emitted::Particle {
                        effect,
                        at: [f64::from(at[0]), f64::from(at[1]), f64::from(at[2])],
                        yaw: 0.0,
                        baby: a.host.baby,
                        name: a.host.name.clone(),
                        size: a.host.size,
                        follow: None,
                        init: Init { script: None, seed },
                    }),
                    Spawn::Sound { event, at } => {
                        let event = if event.contains(':') {
                            event.to_ascii_lowercase()
                        } else {
                            format!("bedrock:{}", event.to_ascii_lowercase())
                        };
                        more.push(Emitted::Sound {
                            event,
                            at: [f64::from(at[0]), f64::from(at[1]), f64::from(at[2])],
                        });
                    }
                }
            }
            let sprites = a.emitter.sprites(&mut a.host);
            if !sprites.is_empty() {
                let fx = a.emitter.effect();
                drawn.push(Drawn {
                    texture: Arc::<str>::from(atlas_name(&fx.texture)),
                    sprites,
                    lit: fx.lit,
                    additive: fx.material.to_ascii_lowercase().contains("particles_add"),
                });
            }
        }
        if super::bedrock_debug::debug() {
            static LAST: std::sync::Mutex<Option<Instant>> = std::sync::Mutex::new(None);
            let mut last = lock(&LAST);
            if last.is_none_or(|t| t.elapsed().as_secs_f32() >= 2.0) {
                *last = Some(now);
                let mut per: HashMap<String, (usize, usize, usize)> = HashMap::new();
                for a in emitters.iter() {
                    let e = per.entry(a.emitter.effect().texture.clone()).or_default();
                    e.0 += 1;
                    e.1 += a.emitter.alive();
                }
                for d in &drawn {
                    per.entry(d.texture.to_string()).or_default().2 += d.sprites.len();
                }
                let mut rows: Vec<_> = per.into_iter().collect();
                rows.sort();
                let text: Vec<String> = rows
                    .iter()
                    .map(|(t, (e, a, s))| format!("{t}: emitters {e} alive {a} sprites {s}"))
                    .collect();
                log::info!("bedrock-debug: particles {} | {}", emitters.len(), text.join(" | "));
            }
        }
        emitters.retain(|a| !a.emitter.finished());
        (drawn, more)
    }

    pub fn particle_count(&self) -> usize {
        lock(&self.emitters).len()
    }
}

/// Applies a Bedrock pose over a game pose (`Poses`).
pub fn apply_pose(pose: &Pose, names: &[String], poses: &mut super::anim::Poses) {
    for name in names {
        let Some(b) = pose.get(name) else { continue };
        let p = poses.get(name);
        let signs = if super::render::in_first_person_view() {
            let s = &super::bedrock_debug::settings().fp_rot_signs;
            [0, 1, 2].map(|i| s.get(i).copied().unwrap_or(1.0))
        } else {
            [1.0; 3]
        };
        let r = geometry::rotation_to_java([
            (b.rotation[0] + b.rotation_entity[0]) * signs[0],
            (b.rotation[1] + b.rotation_entity[1]) * signs[1],
            (b.rotation[2] + b.rotation_entity[2]) * signs[2],
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
}

/// The part of an edge, from `0` at one end to `1` at the other, where the texture coordinate
/// (`a` at the first end, `b` at the second) is inside `0..=1`; none when no part is.
fn clip_span(a: f32, b: f32) -> Option<(f32, f32)> {
    if (b - a).abs() < 1e-6 {
        return (0.0..=1.0).contains(&a).then_some((0.0, 1.0));
    }
    let (p, q) = ((0.0 - a) / (b - a), (1.0 - a) / (b - a));
    let (lo, hi) = (p.min(q).max(0.0), p.max(q).min(1.0));
    (hi > lo).then_some((lo, hi))
}

/// The width and height directions of a sprite that does not face the camera squarely
/// (`facing_camera_mode`), `center` being where it is from the camera. None for one that does.
fn sprite_axes(s: &Sprite, center: glam::Vec3) -> Option<(glam::Vec3, glam::Vec3)> {
    use horizon_bedrock::particle::Facing;
    match s.facing {
        // Height along the direction, width across it and across the line of sight.
        Facing::LookAtDirection => {
            let up = glam::Vec3::from(s.velocity).try_normalize()?;
            let right = up.cross(-center).try_normalize()?;
            Some((right, up))
        }
        // Upright, turned about the vertical to the camera.
        Facing::LookAtY | Facing::RotateY => {
            let up = glam::Vec3::Y;
            let right = up.cross(-center).try_normalize()?;
            Some((right, up))
        }
        // One axis of the quad along the direction, the quad turned as close to up as it can
        // (`direction_x/y/z`), fixed in the world: `direction_x` lays its width along it and
        // stands up, `direction_z` faces along it and stands up, `direction_y` lays its height
        // along it and lies flat (a splash's ripple on the water).
        Facing::DirectionX | Facing::DirectionY | Facing::DirectionZ => {
            let d = glam::Vec3::from(s.velocity).try_normalize()?;
            let up = (glam::Vec3::Y - d * d.dot(glam::Vec3::Y))
                .try_normalize()
                .unwrap_or(glam::Vec3::X);
            Some(match s.facing {
                Facing::DirectionX => (d, up),
                Facing::DirectionY => (d.cross(up), d),
                _ => (up.cross(d), up),
            })
        }
        // Lying in a plane of the emitter (a rain drop's ring on the ground).
        Facing::EmitterXz => Some((glam::Vec3::X, glam::Vec3::Z)),
        Facing::EmitterXy => Some((glam::Vec3::X, glam::Vec3::Y)),
        Facing::EmitterYz => Some((glam::Vec3::Z, glam::Vec3::Y)),
        _ => None,
    }
}

/// Camera-facing quads for sprites, into `out`: the same vertices the game's own particles make.
#[allow(clippy::too_many_arguments)]
pub fn sprite_quads(
    sprites: &[Sprite],
    rect: [f32; 4],
    lit: bool,
    camera: glam::DVec3,
    yaw: f32,
    pitch: f32,
    light_at: &dyn Fn([i32; 3]) -> u8,
    out: &mut Vec<crate::mesher::Vertex>,
) {
    use crate::mesher::Vertex;
    let base = glam::Quat::from_rotation_y(std::f32::consts::PI - yaw.to_radians())
        * glam::Quat::from_rotation_x(-pitch.to_radians());
    let (du, dv) = (rect[2] - rect[0], rect[3] - rect[1]);
    for s in sprites {
        let p = glam::DVec3::new(
            f64::from(s.pos[0]),
            f64::from(s.pos[1]),
            f64::from(s.pos[2]),
        );
        let mut center = (p - camera).as_vec3();
        // A sprite lying on a surface (a ring on the ground, 0.01 above it) is drawn a hair
        // toward the camera so the surface does not win their depth.
        if matches!(
            s.facing,
            horizon_bedrock::particle::Facing::EmitterXz
                | horizon_bedrock::particle::Facing::EmitterXy
                | horizon_bedrock::particle::Facing::EmitterYz
        ) {
            center -= center.normalize_or_zero() * 0.02;
        }
        let rotation = if s.rotation != 0.0 {
            base * glam::Quat::from_rotation_z(s.rotation.to_radians())
        } else {
            base
        };
        let light = if lit {
            let packed = light_at([p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32]);
            (u32::from(packed >> 4) * 16) | (u32::from(packed & 15) * 16) << 8
        } else {
            super::render::FULL_BRIGHT
        };
        let axes = sprite_axes(s, center);
        let c = |v: f32| (v.clamp(0.0, 1.0) * 255.0) as u32;
        let color = c(s.color[3]) << 24 | c(s.color[2]) << 16 | c(s.color[1]) << 8 | c(s.color[0]);
        // The part of the quad whose texture coordinates fall inside the sprite: past its edge
        // the atlas holds another texture, where the particle's own would be clamped.
        let (Some((tu0, tu1)), Some((tv0, tv1))) =
            (clip_span(s.uv[0], s.uv[2]), clip_span(s.uv[3], s.uv[1]))
        else {
            continue;
        };
        let (fu0, fu1) = (
            s.uv[0] + (s.uv[2] - s.uv[0]) * tu0,
            s.uv[0] + (s.uv[2] - s.uv[0]) * tu1,
        );
        // `y` runs from the bottom (`uv[3]`) to the top (`uv[1]`).
        let (fv_bottom, fv_top) = (
            s.uv[3] + (s.uv[1] - s.uv[3]) * tv0,
            s.uv[3] + (s.uv[1] - s.uv[3]) * tv1,
        );
        let (u0, u1) = (rect[0] + du * fu0, rect[0] + du * fu1);
        let (v0, v1) = (rect[1] + dv * fv_top, rect[1] + dv * fv_bottom);
        let (x0, x1) = (-1.0 + 2.0 * tu0, -1.0 + 2.0 * tu1);
        let (y0, y1) = (-1.0 + 2.0 * tv0, -1.0 + 2.0 * tv1);
        for (n, uv) in [
            ((x1, y0), [u1, v1]),
            ((x1, y1), [u1, v0]),
            ((x0, y1), [u0, v0]),
            ((x0, y0), [u0, v1]),
        ] {
            let v = match axes {
                Some((right, up)) => center + right * (n.0 * s.size[0]) + up * (n.1 * s.size[1]),
                None => center + rotation * glam::Vec3::new(n.0 * s.size[0], n.1 * s.size[1], 0.0),
            };
            out.push(Vertex {
                normal: [0; 4],
                pos: v.to_array(),
                uv,
                layer: crate::gfx::TRANSLUCENT_PARTICLE_OVERLAY,
                color,
                light,
                glint_uv: [0.0, 0.0],
            });
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_mob_goes_into_water_as_fast_as_it_fell() {
        let mut w = super::WaterEntry::default();
        assert_eq!(w.look(1, false, -30.0), None);
        // The fall ends on the bottom of a shallow pool before the game finds the water.
        assert_eq!(w.look(2, false, -38.0), None);
        assert_eq!(w.look(3, true, -3.4), Some(-38.0));
        assert_eq!(w.look(3, true, -3.4), Some(-38.0), "every frame of that tick");
        assert_eq!(w.look(4, true, -2.0), None);
        // Slipping in slower than it moves on: the speed it has.
        assert_eq!(w.look(5, false, 0.0), None);
        assert_eq!(w.look(6, true, -4.0), Some(-4.0));
    }

    #[test]
    fn a_locator_ahead_and_on_the_right_lands_ahead_and_on_the_right() {
        // Facing south (+z, yaw 0): ahead is +z, the right side is west (-x).
        let ahead = attach_point([0.0; 3], [0.0, 1.0, -1.0], 0.0);
        assert!((ahead[2] - 1.0).abs() < 1e-9 && ahead[0].abs() < 1e-9 && ahead[1] == 1.0);
        let right = attach_point([0.0; 3], [-1.0, 0.0, 0.0], 0.0);
        assert!((right[0] + 1.0).abs() < 1e-9);
        // Facing west (-x, yaw 90): ahead is -x, the right side is north (-z).
        let ahead = attach_point([0.0; 3], [0.0, 0.0, -1.0], 90.0);
        assert!((ahead[0] + 1.0).abs() < 1e-9 && ahead[2].abs() < 1e-9);
        let right = attach_point([0.0; 3], [-1.0, 0.0, 0.0], 90.0);
        assert!((right[2] + 1.0).abs() < 1e-9);
        // The turn and mirror undo themselves.
        let p = [0.3, 1.2, -0.7];
        let back = model_to_world(model_to_world(p, 37.0), 37.0);
        assert!(p.iter().zip(back).all(|(a, b)| (a - b).abs() < 1e-9));
    }

    #[test]
    fn a_clock_shows_the_sun_at_noon_and_the_moon_at_midnight() {
        assert_eq!(clock_frame(6000), 0.0);
        assert_eq!(clock_frame(18000), 32.0);
        assert_eq!(clock_frame(30000), 0.0);
    }

    #[test]
    fn a_compass_points_ahead_when_the_target_is_in_front() {
        // Yaw 0 faces +z (south): a target due south is ahead, turn 0, frame 16.
        let ahead = super::compass_turn([0.0, 0.0], 0.0, Some([0.0, 10.0]));
        assert!(ahead.abs() < 1e-9 || (ahead - 1.0).abs() < 1e-9, "{ahead}");
        assert_eq!(super::compass_frame(ahead), 16.0);
        // Facing east (yaw -90), the same target is to the right: a quarter turn.
        let right = super::compass_turn([0.0, 0.0], -90.0, Some([0.0, 10.0]));
        assert!((right - 0.25).abs() < 1e-9, "{right}");
        assert_eq!(super::compass_frame(right), 24.0);
    }

    #[test]
    fn java_items_take_their_bedrock_names() {
        assert_eq!(super::bedrock_item_name("minecraft:magma_block"), "magma");
        assert_eq!(super::bedrock_item_name("minecraft:jack_o_lantern"), "lit_pumpkin");
        assert_eq!(super::bedrock_item_name("minecraft:snow_block"), "snow");
        assert_eq!(super::bedrock_item_name("minecraft:snow"), "snow_layer");
        assert_eq!(super::bedrock_item_name("minecraft:dirt"), "dirt");
        let index = HashMap::from([("minecraft:magma".to_owned(), vec!["minecraft:magma".to_owned()])]);
        assert!(super::attachables_for(&index, "minecraft:magma_block").is_some());
    }

    /// Every query the documentation lists has an answer somewhere: the host, the engine's
    /// bone tables, the animation clock, or the Molang builtins. One that reads as nothing is a
    /// gap in the engine, not in a pack.
    #[test]
    fn every_documented_query_has_an_answer() {
        let sources = [
            include_str!("bedrock.rs"),
            include_str!("../../../horizon-bedrock/src/entity.rs"),
            include_str!("../../../horizon-bedrock/src/animation.rs"),
            include_str!("../../../horizon-bedrock/src/molang/eval.rs"),
            include_str!("../../../horizon-bedrock/src/particle.rs"),
        ]
        .concat();
        let missing: Vec<&str> = horizon_bedrock::DOCUMENTED_QUERIES
            .lines()
            .filter(|q| !sources.contains(&format!("\"{q}\"")))
            .collect();
        assert!(missing.is_empty(), "no answer for: {missing:?}");
    }

    #[test]
    fn a_villager_profession_has_bedrocks_number() {
        use super::super::render::PROFESSIONS;
        let at = |name: &str| PROFESSIONS.iter().position(|p| *p == name).unwrap() as u8;
        assert_eq!(bedrock_profession(at("none")), 0);
        assert_eq!(bedrock_profession(at("farmer")), 1);
        assert_eq!(bedrock_profession(at("librarian")), 5);
        assert_eq!(bedrock_profession(at("armorer")), 8);
        assert_eq!(bedrock_profession(at("nitwit")), 14);
    }

    #[test]
    fn bedrock_calls_some_mobs_by_other_names() {
        assert_eq!(
            bedrock_id("minecraft:villager"),
            Some("minecraft:villager_v2")
        );
        assert_eq!(
            bedrock_id("minecraft:zombified_piglin"),
            Some("minecraft:zombie_pigman")
        );
        assert_eq!(bedrock_id("minecraft:zombie"), None);
        assert_eq!(bedrock_id("minecraft:snow_golem"), None);
    }

    #[test]
    fn an_attached_point_turns_with_the_mob() {
        let near = |a: [f64; 3], b: [f64; 3]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9);
        // Facing south: the model's back (+z) is north of the feet.
        assert!(near(
            attach_point([10.0, 64.0, -5.0], [1.0, 2.0, 3.0], 0.0),
            [11.0, 66.0, -8.0]
        ));
        // A quarter turn (facing west): model x and z swap into the world's z and x.
        assert!(near(
            attach_point([0.0, 0.0, 0.0], [1.0, 0.0, 3.0], 90.0),
            [3.0, 0.0, 1.0]
        ));
    }

    use super::*;
    use horizon_bedrock::particle::Facing;

    #[test]
    fn a_sprite_is_cut_where_its_texture_ends() {
        assert_eq!(clip_span(0.0, 1.0), Some((0.0, 1.0)));
        // Starts past the top edge: the first quarter of the quad has no texture.
        let (lo, hi) = clip_span(-0.5, 1.5).unwrap();
        assert!((lo - 0.25).abs() < 1e-6 && (hi - 0.75).abs() < 1e-6);
        assert_eq!(clip_span(1.5, 2.5), None);
        let (lo, hi) = clip_span(1.2, 0.2).unwrap();
        assert!((lo - 0.2).abs() < 1e-6 && (hi - 1.0).abs() < 1e-6);
    }

    #[test]
    fn names_are_exact() {
        assert!(name_matches(
            Some("Snowman"),
            &[Value::Str("snowman".into()), Value::Str("Snowman".into())]
        ));
        assert!(!name_matches(
            Some("snowman"),
            &[Value::Str("Snowman".into())]
        ));
        assert!(!name_matches(None, &[Value::Str("x".into())]));
    }

    #[test]
    fn shared_queries() {
        use horizon_bedrock::settings::{Choice, Def, Kind, install};
        let def = |name: &str, kind, default| Def {
            name: name.into(),
            label: String::new(),
            kind,
            options: vec![("a".into(), String::new()), ("b".into(), String::new())],
            default,
            range: None,
        };
        install(
            &[
                def("q:flag", Kind::Toggle, Choice::Flag(true)),
                def("q:mode", Kind::Dropdown, Choice::One("b".into())),
            ],
            &Default::default(),
        );
        let s = |t: &str| Value::Str(t.into());
        assert_eq!(
            common_query("is_pack_setting_enabled", &[s("q:flag")]),
            Value::Num(1.0)
        );
        assert_eq!(
            common_query("is_pack_setting_selected", &[s("q:mode"), s("a")]),
            Value::Num(0.0)
        );
        assert_eq!(
            common_query("is_pack_setting_selected", &[s("q:mode"), s("b")]),
            Value::Num(1.0)
        );
        assert_eq!(
            common_query(
                "in_range",
                &[Value::Num(5.0), Value::Num(1.0), Value::Num(9.0)]
            ),
            Value::Num(1.0)
        );
        assert_eq!(
            common_query(
                "in_range",
                &[Value::Num(10.0), Value::Num(1.0), Value::Num(9.0)]
            ),
            Value::Num(0.0)
        );
        assert_eq!(
            common_query(
                "graphics_mode_is_any",
                &[Value::Str("simple".into()), Value::Str("fancy".into())]
            ),
            Value::Num(1.0)
        );
        assert_eq!(common_query("nothing_like_it", &[]), Value::Null);
    }

    #[test]
    fn wraps() {
        assert_eq!(wrap_degrees(190.0), -170.0);
        assert_eq!(wrap_degrees(-190.0), 170.0);
        assert_eq!(wrap_degrees(720.0), 0.0);
    }

    #[test]
    fn a_held_item_modelled_at_the_origin_lands_on_the_hand() {
        use super::super::render::{bone_matrices, held_anchor, posed};
        let g = geometry::parse_file(
            &horizon_bedrock::jsonc::parse(
                r#"{"format_version":"1.12.0","minecraft:geometry":[{"description":{"identifier":"geometry.t","texture_width":16,"texture_height":16},
                "bones":[{"name":"item","pivot":[0,0,0],"cubes":[{"origin":[-1,0,-1],"size":[2,8,2],"uv":[0,0]}]},
                         {"name":"hand","pivot":[-6,15,1]}]}]}"#,
            )
            .unwrap(),
        )
        .unwrap()
        .remove(0);
        let b = bake(&g);
        let poses = super::super::anim::Poses::new(&b.part);
        let mut matrices = HashMap::new();
        bone_matrices(&b.part, &poses, &mut matrices);
        let (now, _) = matrices["hand"];
        let item = b.part.children.iter().find(|c| c.name == "item").unwrap().pose;
        // The item's origin, drawn the way `draw_attached` does: the holder's anchor, then the
        // item's own pose (in sixteenths, as every pose is).
        let at = (held_anchor(now, 1.0) * posed(glam::Mat4::IDENTITY, item)).transform_point3(glam::Vec3::ZERO);
        let hand = now.transform_point3(glam::Vec3::ZERO);
        assert!((at - hand).length() < 1e-4, "{at:?} vs {hand:?}");
        // And that is where the hand is: 15 sixteenths up, 6 to the side, not blocks away.
        assert!(hand.length() < 1.5, "{hand:?}");
    }

    /// `HORIZON_TEST_PACK=<pack dir> cargo test -p horizon-client-lib --lib -- sword_probe --ignored --nocapture`
    #[test]
    #[ignore = "needs a pack"]
    fn sword_probe() {
        use super::super::render::{bone_matrices, held_anchor, posed};
        let Ok(dir) = std::env::var("HORIZON_TEST_PACK") else { return };
        let pack = horizon_bedrock::pack::Pack::open(std::path::Path::new(&dir)).unwrap();
        let mut geos = HashMap::new();
        let mut builtin = horizon_bedrock::bundle::Bundle::default();
        builtin.add_builtins();
        geos.extend(builtin.geometries);
        for p in pack.paths().map(str::to_owned).collect::<Vec<_>>() {
            if p.starts_with("models/") && p.ends_with(".json") {
                if let Ok(list) = pack.read_json(&p).map_err(|_| ()).and_then(|v| geometry::parse_file(&v).map_err(|_| ())) {
                    for g in list {
                        geos.insert(g.id.to_ascii_lowercase(), g);
                    }
                }
            }
        }
        let player = bake(&geos["geometry.humanoid.custom"]);
        let sword = bake(&geos["geometry.oreville_ans.ccyhyr"]);
        let poses = super::super::anim::Poses::new(&player.part);
        let mut m = HashMap::new();
        bone_matrices(&player.part, &poses, &mut m);
        let (now, _) = m["rightitem"];
        let mut out = Vec::new();
        let item_poses = super::super::anim::Poses::new(&sword.part);
        let anchor = held_anchor(now, 1.0);
        let _ = posed;
        super::super::render::draw_attached(&sword.part, glam::Mat4::IDENTITY, glam::Mat4::IDENTITY, true, &|b| (b == "sword").then_some(anchor), &item_poses, [0.0, 0.0, 1.0, 1.0], 0, 0, 0, &mut out);
        let (mut lo, mut hi) = (glam::Vec3::splat(f32::MAX), glam::Vec3::splat(f32::MIN));
        for v in &out {
            let p = glam::Vec3::from_array(v.pos);
            lo = lo.min(p);
            hi = hi.max(p);
        }
        // The game's frame: y down from the 24 line; above the feet is 1.5 - y.
        eprintln!("sword x {:.2}..{:.2} height above feet {:.2}..{:.2} z {:.2}..{:.2}", lo.x, hi.x, 1.5 - hi.y, 1.5 - lo.y, lo.z, hi.z);
    }

    #[test]
    fn baking_lower_cases_parts_and_keeps_the_tree() {
        let g = geometry::parse_file(
            &horizon_bedrock::jsonc::parse(
                r#"{"format_version":"1.12.0","minecraft:geometry":[{"description":{"identifier":"geometry.t","texture_width":16,"texture_height":16},
                "bones":[{"name":"Body","pivot":[0,12,0],"cubes":[{"origin":[-4,6,-3],"size":[8,6,6],"uv":[0,0]}]},
                         {"name":"Head","parent":"Body","pivot":[0,12,0]}]}]}"#,
            )
            .unwrap(),
        )
        .unwrap()
        .remove(0);
        let b = bake(&g);
        assert_eq!(b.names, ["body", "head", ROOT]);
        assert_eq!(b.part.children[0].name, "body");
        assert_eq!(b.part.children[0].children[0].name, "head");
        assert_eq!(b.part.children[0].polygons.len(), 6);
    }

    #[test]
    fn a_bound_bone_follows_the_holders_bone() {
        use super::super::render::{bone_matrices, draw_attached};
        let quad = Polygon {
            normal: [0.0, 0.0, 1.0],
            vertices: [[0.0; 5]; 4],
            cull_back: false,
        };
        let part = |name: &str, pose: [f32; 9], polygons: Vec<Polygon>, children: Vec<Part>| Part {
            name: name.into(),
            pose,
            polygons,
            children,
        };
        let rest = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
        // The holder: an arm at the origin, with a hand 16 units along.
        let hand = part(
            "hand",
            [16.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            vec![],
            vec![],
        );
        let holder = part("arm", rest, vec![], vec![hand]);
        let mut poses = super::super::anim::Poses::new(&holder);
        // The animation lifts the arm by one block.
        poses.get("arm")[1] += 16.0;
        let mut matrices = HashMap::new();
        bone_matrices(&holder, &poses, &mut matrices);
        let (now, was) = matrices["hand"];
        assert!(
            ((now * was.inverse()).transform_point3(glam::Vec3::ZERO) - glam::Vec3::Y).length()
                < 1e-5
        );
        // The attachable: one bone with a quad at its origin, bound to the hand.
        let item = part("item", rest, vec![quad], vec![]);
        let item_poses = super::super::anim::Poses::new(&item);
        let base_of = |bone: &str| (bone == "item").then(|| now * was.inverse());
        let mut out = Vec::new();
        draw_attached(
            &item,
            glam::Mat4::IDENTITY,
            glam::Mat4::IDENTITY,
            true,
            &base_of,
            &item_poses,
            [0.0, 0.0, 1.0, 1.0],
            0,
            0,
            0,
            &mut out,
        );
        assert_eq!(out.len(), 4);
        assert!((out[0].pos[1] - 1.0).abs() < 1e-5, "{:?}", out[0].pos);
        // Unbound bones of a model that binds some are not drawn.
        let mut none = Vec::new();
        draw_attached(
            &item,
            glam::Mat4::IDENTITY,
            glam::Mat4::IDENTITY,
            true,
            &|_| None,
            &item_poses,
            [0.0, 0.0, 1.0, 1.0],
            0,
            0,
            0,
            &mut none,
        );
        assert!(none.is_empty());
    }

    #[test]
    fn a_packs_item_tags_read_as_the_kind_of_item() {
        assert!(item_has_tag("minecraft:iron_sword", "minecraft:is_sword"));
        assert!(item_has_tag("minecraft:iron_sword", "oreville:is_sword"));
        assert!(!item_has_tag("minecraft:iron_axe", "oreville:is_sword"));
        assert!(item_has_tag(
            "minecraft:golden_pickaxe",
            "minecraft:golden_tier"
        ));
        assert!(!item_has_tag(
            "minecraft:iron_pickaxe",
            "minecraft:golden_tier"
        ));
        assert!(item_has_tag(
            "minecraft:carrot_on_a_stick",
            "oreville:is_on_a_stick"
        ));
        assert!(item_has_tag(
            "minecraft:fishing_rod",
            "oreville:is_fishing_rod"
        ));
        assert!(item_has_tag("minecraft:bow", "oreville:is_bow"));
        assert!(!item_has_tag("minecraft:crossbow", "oreville:is_bow"));
        assert!(item_has_tag("minecraft:white_harness", "minecraft:harness"));
    }

    #[test]
    fn slots_name_their_bones() {
        assert_eq!(slot_bone("main_hand"), "rightitem");
        assert_eq!(slot_bone("off_hand"), "leftitem");
        assert_eq!(slot_of("slot.armor.head"), Some("head"));
        assert_eq!(slot_of("slot.nothing"), None);
    }

    #[test]
    fn pose_adds_to_the_rest_pose() {
        let mut library = horizon_bedrock::animation::Library::default();
        library.add_file(
            &horizon_bedrock::jsonc::parse(r#"{"animations":{"animation.t":{"loop":true,"animation_length":1,"bones":{"body":{"rotation":[90,0,0],"position":[1,2,3],"scale":2}}}}}"#).unwrap(),
        );
        let mut vars = horizon_bedrock::molang::Vars::default();
        let mut rng = horizon_bedrock::molang::Rng::new(1);
        let mut host = horizon_bedrock::molang::NoHost;
        let mut env = horizon_bedrock::animation::Env {
            host: &mut host,
            vars: &mut vars,
            rng: &mut rng,
        };
        let mut pose = Pose::default();
        library.animations["animation.t"].sample(
            &horizon_bedrock::animation::AnimState::default(),
            1.0,
            &mut env,
            &mut pose,
        );
        let part = Part {
            name: "body".into(),
            pose: [0.0, 12.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            polygons: Vec::new(),
            children: Vec::new(),
        };
        let mut poses = super::super::anim::Poses::new(&part);
        apply_pose(&pose, &["body".to_owned()], &mut poses);
        let p = poses.pose(&part);
        assert_eq!(&p[..3], &[1.0, 10.0, 3.0]);
        assert!((p[3] - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
        assert_eq!(&p[6..], &[2.0, 2.0, 2.0]);
    }

    #[test]
    fn sprites_become_camera_facing_quads() {
        let sprite = Sprite {
            pos: [1.0, 2.0, 3.0],
            size: [0.5, 0.5],
            rotation: 0.0,
            uv: [0.0, 0.0, 1.0, 1.0],
            color: [1.0, 0.0, 0.0, 0.5],
            facing: Facing::LookAtXyz,
            velocity: [0.0; 3],
        };
        let mut out = Vec::new();
        sprite_quads(
            &[sprite],
            [0.0, 0.0, 0.5, 0.5],
            false,
            glam::DVec3::new(1.0, 2.0, 0.0),
            0.0,
            0.0,
            &|_| 0,
            &mut out,
        );
        assert_eq!(out.len(), 4);
        assert_eq!(out[0].uv, [0.5, 0.5]);
        assert_eq!(out[0].light, super::super::render::FULL_BRIGHT);
        assert_eq!(out[0].color >> 24, 127);
        // Facing the camera: all four corners at the sprite's depth.
        assert!(out.iter().all(|v| (v.pos[2] - 3.0).abs() < 1e-4));
    }
}
