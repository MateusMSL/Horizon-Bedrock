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
#[path = "bedrock_host.rs"]
mod host;
#[path = "bedrock_pose.rs"]
mod pose;
use horizon_bedrock::{
    animation::Pose,
    bundle::Bundle,
    convert::atlas_name,
    entity::{Assets, Emit, Instance, Layer},
    geometry::{self, Geometry},
    molang::{Host, Value},
    particle::{Emitter, Spawn, Sprite, World},
};
use host::*;
pub use host::{MobHost, set_camera_rotation};
pub(super) use host::{armor_color, pose_stand, slot_bone};
pub use pose::{begin_frame, drew_model};
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
    let tilt =
        level * glam::DQuat::from_rotation_x(-f64::from(pitch).to_radians()) * level.inverse();
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
///
/// The vanilla behaviour packs' `minecraft:scale`: the cat at 0.8 (0.4 a kitten, the half every
/// baby gets on top), the wither skeleton at 1.2.
fn builtin_scale(id: &str, baby: bool) -> f32 {
    match id {
        "minecraft:rabbit" if baby => 0.4,
        "minecraft:rabbit" => 0.6,
        "minecraft:cat" => 0.8,
        "minecraft:wither_skeleton" => 1.2,
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
    let mut cells =
        Vec::with_capacity(((2 * side + 1) * (2 * side + 1) * (below + above + 1)) as usize);
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

static STANDS_HAVE_ARMS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether armour stands take items in their hands whatever their `ShowArms`: Bedrock's stands
/// always have arms, and a pack that dresses the stand builds on that (Actions & Stuff shows the
/// item in a stand's hand in its "Item" mode). On while the loaded pack dresses the armour stand;
/// a world played without it keeps the game's rule.
pub fn armor_stands_have_arms() -> bool {
    STANDS_HAVE_ARMS.load(std::sync::atomic::Ordering::Relaxed)
}

/// No pack dresses the armour stand any more ([`armor_stands_have_arms`]).
pub fn clear_pack_rules() {
    horizon_client_render::pvp::PACK_GLINT.store(0, std::sync::atomic::Ordering::Relaxed);
    STANDS_HAVE_ARMS.store(false, std::sync::atomic::Ordering::Relaxed);
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
        let full = if t.contains(':') {
            t.to_owned()
        } else {
            format!("minecraft:{t}")
        };
        tags.get(full.as_str())
            .is_some_and(|set| set.contains(name))
    };
    let air = matches!(
        name,
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
    );
    let tag = tag.to_ascii_lowercase();
    match tag.trim_start_matches("minecraft:") {
        "water" => matches!(
            name,
            "minecraft:water"
                | "minecraft:bubble_column"
                | "minecraft:kelp"
                | "minecraft:kelp_plant"
                | "minecraft:seagrass"
                | "minecraft:tall_seagrass"
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
        "snow" => matches!(
            name,
            "minecraft:snow" | "minecraft:snow_block" | "minecraft:powder_snow"
        ),
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
        !std::env::var("HORIZON_BEDROCK_PLAYERS").is_ok_and(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "0" | "off" | "false"
            )
        })
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
    *COMPASS_TARGET
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = target;
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
            let target = *COMPASS_TARGET
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
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
pub const ENGINE_BUILD: &str = "v278-use-hand";

/// `HORIZON_BEDROCK_DUMP=skeleton,creeper`: the kinds (by part of the id) whose state is logged
/// once a second while within twelve blocks of the camera.
fn dump_wanted(kind: &str) -> bool {
    super::bedrock_debug::settings().dumps(kind)
}

/// One line of what a mob's pack is doing: what the game answers, what runs, the bones moved
/// and what is drawn.
fn dump_line(
    m: &Mob,
    host: &mut MobHost,
    frame: &horizon_bedrock::entity::Frame,
    running: &str,
) -> String {
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
        m.extra.main_hand, m.extra.body, m.extra.using_item, m.hurt_time, m.death_time
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
    /// The resource root the pack's converted textures are in.
    root: std::path::PathBuf,
    /// A glint texture's colour (`glint_colour`), by atlas name.
    glints: Mutex<HashMap<String, u32>>,
    /// A geometry's bone bindings, parsed once.
    bindings: Mutex<HashMap<String, Arc<Vec<(String, horizon_bedrock::molang::Program)>>>>,
}

/// Bedrock's glint colour, which the `GLINT` shader multiplies the glint texture by (the
/// vanilla purple, 0.5 / 0.25 / 0.8).
const GLINT_TINT: [f64; 3] = [0.5, 0.25, 0.8];

fn read_glint_colour(path: &Path) -> Option<u32> {
    let mut decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).ok()?));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let channels = info.color_type.samples();
    if channels < 3 {
        return None;
    }
    let mut sum = [0.0f64; 3];
    let mut weight = 0.0f64;
    for px in buf[..info.buffer_size()].chunks_exact(channels) {
        let a = if channels == 4 {
            f64::from(px[3]) / 255.0
        } else {
            1.0
        };
        // The `GLINT` shader multiplies its texture by the glint colour: Actions & Stuff's
        // glint textures are stored divided by it (its magenta one reads olive green, its
        // white one green until multiplied).
        let rgb = [0, 1, 2].map(|i| f64::from(px[i]) / 255.0 * GLINT_TINT[i]);
        let w = rgb.iter().copied().fold(0.0, f64::max) * a;
        for i in 0..3 {
            sum[i] += rgb[i] * w;
        }
        weight += w;
    }
    if weight <= 0.0 {
        return None;
    }
    let mean = sum.map(|c| c / weight);
    let top = mean.iter().copied().fold(0.0, f64::max);
    if top <= 0.0 {
        return None;
    }
    let byte = |c: f64| ((c / top).clamp(0.0, 1.0) * 255.0).round() as u32;
    Some(byte(mean[0]) | byte(mean[1]) << 8 | byte(mean[2]) << 16 | 0xff << 24)
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
        let (lo, hi) = vertices.iter().fold((f32::MAX, f32::MIN), |(lo, hi), v| {
            (lo.min(v[i]), hi.max(v[i]))
        });
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
        // Logged with its time and memory: a pack's models once took 15 GB here.
        let _memory = horizon_threads::memory_watch::phase("bedrock-mobs");
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
            root: root.to_path_buf(),
            ..Default::default()
        }
    }

    /// The colour of glint texture `texture` (atlas name), packed as PvP & HUD's glint colour
    /// (red in the low byte): its texels weighed by their brightness, the brightest channel
    /// raised to full, so the game's shimmer keeps its pattern and takes the colour. 0 when the
    /// file cannot be read.
    pub fn glint_path(&self, texture: &str) -> std::path::PathBuf {
        self.root
            .join(format!("assets/minecraft/textures/{texture}.png"))
    }

    pub fn glint_colour(&self, texture: &str) -> u32 {
        if let Some(c) = lock(&self.glints).get(texture) {
            return *c;
        }
        let path = self.glint_path(texture);
        let colour = read_glint_colour(&path).unwrap_or(0);
        lock(&self.glints).insert(texture.to_owned(), colour);
        colour
    }

    pub fn is_empty(&self) -> bool {
        self.bundle.is_empty()
    }

    /// Whether an attachable of the pack dresses `item` held in a hand (worn pieces are dressed
    /// on their slot, not in the hand).
    pub fn dresses_held(&self, item: &str) -> bool {
        let item_lc = item.to_ascii_lowercase();
        !is_wearable(&item_lc) && attachables_for(&self.item_index, &item_lc).is_some()
    }

    /// Hands the game's own particles that this pack draws itself (its
    /// `minecraft:rain_splash_particle`) to it: once for the pack in use, every frame cheap.
    pub fn take_game_particles(&self) {
        STANDS_HAVE_ARMS.store(
            self.entity_def("minecraft:armor_stand").is_some(),
            std::sync::atomic::Ordering::Relaxed,
        );
        let key = std::ptr::from_ref(self) as usize;
        if let Some(ids) = horizon_client_render::bedrock_particles::install_for(key, |id| {
            self.bundle
                .effects
                .get(id)
                .is_some_and(|fx| fx.one_per_request())
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

    /// The pack's `scale` script for `m`, as of its last frame (`None` before the first or when
    /// no pack dresses it).
    pub fn pack_scale(&self, m: &Mob) -> Option<f32> {
        lock(&self.states)
            .get(&m.id)
            .and_then(|s| s.frame.as_ref())
            .map(|f| f.scale)
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
        if !gui && m.extra.first_person && since > 0.3 {
            // Back in first person (the camera went round): the view model starts over, as one
            // newly put in view, and settles (`warm`). Its old state, frozen where the camera
            // left it, kept a pack's arm hidden until the item was put back in the hand.
            *st = MobState {
                instance: Instance::new(u64::from(m.id) + 1),
                time,
                frame: None,
                seen: Instant::now(),
                dumped: f32::NEG_INFINITY,
                live: None,
                water: WaterEntry::default(),
            };
            warm = true;
        }
        st.seen = Instant::now();
        if !gui
            && st.time == time
            && let Some(f) = &st.frame
        {
            return Some(f.clone());
        }
        // Away from the camera the pose is kept for a few frames (`pose::keeps`).
        if !gui
            && let Some(f) = &st.frame
            && pose::keeps(m, st.time, time, from_camera)
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
        let entry_speed = if gui {
            None
        } else {
            st.water.look(m.age, m.fluids.in_water, vertical_speed(m))
        };
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
        let mut frame = pose::update(&mut st.instance, def, &assets, &mut host, dt);
        if let Some(stand) = &m.extra.stand_pose {
            pose_stand(&mut frame.pose, stand);
        }
        if head_drawn_apart(&m.kind.id)
            && let Some(g) = frame
                .layers
                .first()
                .and_then(|l| self.bundle.geometries.get(&l.geometry.to_ascii_lowercase()))
        {
            head_apart(&mut frame.pose, g);
        }
        st.time = time;
        if !gui && avatar && m.extra.fall_flying {
            // The same for a glide: a pack that follows the look with `root` itself (the
            // gliding animation turns it by `-target_x_rotation`) turns the body a second time
            // over the game's pitch, and a climb straight up lies it back down.
            let turned = frame
                .pose
                .get("root")
                .is_some_and(|b| (b.rotation[0] + b.rotation_entity[0]).abs() >= 5.0);
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
            let turned = frame
                .pose
                .get("root")
                .is_some_and(|b| (b.rotation[0] + b.rotation_entity[0]).abs() >= 15.0);
            if turned {
                set_pack_lies_down(m.id, true);
            }
        }
        if !gui && super::camel::is_camel(m.species()) {
            let moved = |n: &str| frame.pose.get(n).map_or(0.0, |b| b.position[1]);
            super::camel::set_model_drop(m.id, (moved("root") + moved("body")) / 16.0);
        }
        if !gui
            && (m.extra.passenger
                || m.rider.is_some()
                || super::bedrock_debug::is_traced_mount(m.id))
        {
            super::bedrock_debug::trace_pose(m, alpha, time, dt, &frame);
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
                log::info!(
                    "bedrock-dump-vars {}#{} {}",
                    m.kind.id,
                    m.id,
                    vars.join(" ")
                );
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
                let skin =
                    skinned.then(|| m.skin.clone().unwrap_or_else(|| m.kind.texture.to_string()));
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
                    + if m.kind.id == "minecraft:player" {
                        1.62
                    } else {
                        0.0
                    },
                lerp(m.previous.z, m.position.z),
            ],
            delta: [
                m.position.x - m.previous.x,
                m.position.y - m.previous.y,
                m.position.z - m.previous.z,
            ],
            body_yaw: wrap_degrees(
                m.previous_body_yaw + (m.body_yaw - m.previous_body_yaw) * alpha,
            ),
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
                        layers
                            .iter()
                            .find_map(|d| self.bundle.geometries.get(&d.layer.geometry)?.locator(l))
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
        let mut attached = self.attachments(
            m,
            alpha,
            dt,
            from_camera,
            &st.instance.vars,
            (&layers, &frame.pose),
        );
        for a in &mut attached {
            for (e, offset) in std::mem::take(&mut a.emits) {
                self.send_emit(&mut queue, m, &live, e, offset, Some(a.slot));
            }
        }
        let dressed = |slot: &str| {
            attached
                .iter()
                .any(|a| a.slot == slot && !a.layers.is_empty())
        };
        held.main_replaced = dressed("main_hand");
        held.off_replaced = dressed("off_hand");
        if held.hidden {
            attached.retain(|a| !matches!(a.slot, "main_hand" | "off_hand"));
        }
        // An attachable's animations that name bones of its holder its own model lacks (the arm
        // a held item raises) do not pose the holder: Bedrock poses it by the holder's own
        // animations alone. Actions & Stuff plays the sword's swing (`ccqkmf`) on the player and
        // on the sword; added together the arm turned twice as far (v191 added them). The debug
        // file's `attachables_drive_holder: true` adds them again, to compare.
        let mut holder_pose = frame.pose;
        let drives = super::bedrock_debug::settings()
            .attachables_drive_holder
            .unwrap_or(false);
        for a in attached.iter().filter(|_| drives) {
            for (bone, drive) in &a.pose.bones {
                let own = a
                    .layers
                    .iter()
                    .any(|d| d.built.names.iter().any(|n| n == bone));
                let holder_has = layers
                    .iter()
                    .any(|d| d.built.names.iter().any(|n| n == bone));
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
                        .map(
                            |(_, i)| attachables_for(&self.item_index, &i.to_ascii_lowercase())
                                .cloned()
                        )
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
            let left = m.extra.first_person && st.seen.elapsed().as_secs_f32() > 0.3;
            if st.id != *id || left {
                // Another attachable, or the view model is back in view: it starts over.
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
                let target = st
                    .bound_to
                    .clone()
                    .unwrap_or_else(|| slot_bone(slot).to_owned());
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
            let mut frame = pose::update(&mut st.instance, def, &assets, &mut host, dt);
            // Where each effect starts: its locator in the attachable's model, carried to the
            // holder's bone the item is bound to (Bedrock draws the attachable as part of the
            // holder's skeleton; its y 24 sits on the bone); the bone itself without a locator.
            let place = st.instance.place;
            let emits: Vec<(Emit, [f64; 3])> = std::mem::take(&mut frame.emits)
                .into_iter()
                .map(|e| {
                    let locator = match &e {
                        Emit::Particle { locator, .. } | Emit::Hold { locator, .. } => {
                            locator.clone()
                        }
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
                        std::array::from_fn(|r| {
                            t[r][0] * p[0] + t[r][1] * p[1] + t[r][2] * p[2] + t[r][3]
                        })
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
            // The elytra's wings are the pack's (and vanilla's `animation.elytra.*`, built in):
            // Bedrock opens them by animation, on `left_wing`/`right_wing` of `geometry.elytra`,
            // the same bones the pack's own glint and outline models carry.
            let pose = frame.pose;
            if super::bedrock_debug::settings().dumps(&m.kind.id) {
                let secs = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs());
                super::render::log_bedrock_once(
                    format!("attached/{}/{slot}/{secs}", m.kind.id),
                    || {
                        format!(
                            "bedrock-attached {}#{} {slot}: attachable {id}, {} layers {:?}, binds {:?}",
                            m.kind.id,
                            m.id,
                            layers.len(),
                            layers
                                .iter()
                                .enumerate()
                                .map(|(i, d)| (i, d.layer.geometry.as_str(), d.texture.as_str()))
                                .collect::<Vec<_>>(),
                            binds
                        )
                    },
                );
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
            if !held && let Some(l) = &live {
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
    pub fn step_particles_on(&self, ground: Option<&dyn Ground>) -> (Vec<Drawn>, Vec<Emitted>) {
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
                log::info!(
                    "bedrock-debug: particles {} | {}",
                    emitters.len(),
                    text.join(" | ")
                );
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
            let settings = super::bedrock_debug::settings();
            let s = &settings.fp_rot_signs;
            let own = settings.fp_signs_for(name);
            [0, 1, 2].map(|i| s.get(i).copied().unwrap_or(1.0) * own[i])
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
        assert_eq!(
            w.look(3, true, -3.4),
            Some(-38.0),
            "every frame of that tick"
        );
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
        assert_eq!(
            super::bedrock_item_name("minecraft:jack_o_lantern"),
            "lit_pumpkin"
        );
        assert_eq!(super::bedrock_item_name("minecraft:snow_block"), "snow");
        assert_eq!(super::bedrock_item_name("minecraft:snow"), "snow_layer");
        assert_eq!(super::bedrock_item_name("minecraft:dirt"), "dirt");
        let index = HashMap::from([(
            "minecraft:magma".to_owned(),
            vec!["minecraft:magma".to_owned()],
        )]);
        assert!(super::attachables_for(&index, "minecraft:magma_block").is_some());
    }

    /// Every query the documentation lists has an answer somewhere: the host, the engine's
    /// bone tables, the animation clock, or the Molang builtins. One that reads as nothing is a
    /// gap in the engine, not in a pack.
    #[test]
    fn every_documented_query_has_an_answer() {
        let sources = [
            include_str!("bedrock.rs"),
            include_str!("bedrock_host.rs"),
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
        let item = b
            .part
            .children
            .iter()
            .find(|c| c.name == "item")
            .unwrap()
            .pose;
        // The item's origin, drawn the way `draw_attached` does: the holder's anchor, then the
        // item's own pose (in sixteenths, as every pose is).
        let at = (held_anchor(now, 1.0) * posed(glam::Mat4::IDENTITY, item))
            .transform_point3(glam::Vec3::ZERO);
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
        let Ok(dir) = std::env::var("HORIZON_TEST_PACK") else {
            return;
        };
        let pack = horizon_bedrock::pack::Pack::open(std::path::Path::new(&dir)).unwrap();
        let mut geos = HashMap::new();
        let mut builtin = horizon_bedrock::bundle::Bundle::default();
        builtin.add_builtins();
        geos.extend(builtin.geometries);
        for p in pack.paths().map(str::to_owned).collect::<Vec<_>>() {
            if p.starts_with("models/")
                && p.ends_with(".json")
                && let Ok(list) = pack
                    .read_json(&p)
                    .map_err(|_| ())
                    .and_then(|v| geometry::parse_file(&v).map_err(|_| ()))
            {
                for g in list {
                    geos.insert(g.id.to_ascii_lowercase(), g);
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
        super::super::render::draw_attached(
            &sword.part,
            glam::Mat4::IDENTITY,
            glam::Mat4::IDENTITY,
            true,
            &|b| (b == "sword").then_some(anchor),
            &item_poses,
            [0.0, 0.0, 1.0, 1.0],
            0,
            0,
            0,
            &mut out,
        );
        let (mut lo, mut hi) = (glam::Vec3::splat(f32::MAX), glam::Vec3::splat(f32::MIN));
        for v in &out {
            let p = glam::Vec3::from_array(v.pos);
            lo = lo.min(p);
            hi = hi.max(p);
        }
        // The game's frame: y down from the 24 line; above the feet is 1.5 - y.
        eprintln!(
            "sword x {:.2}..{:.2} height above feet {:.2}..{:.2} z {:.2}..{:.2}",
            lo.x,
            hi.x,
            1.5 - hi.y,
            1.5 - lo.y,
            lo.z,
            hi.z
        );
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
