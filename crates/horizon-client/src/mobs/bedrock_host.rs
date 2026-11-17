//! How a pack's queries are answered: the game's state of a mob (`MobHost`) and of what it holds
//! (`AttachHost`), and the items, slots and armour they read.

use super::*;

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
pub(super) struct WaterEntry {
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
    pub(super) fn look(&mut self, age: u32, in_water: bool, speed: f32) -> Option<f32> {
        if age != self.tick {
            if in_water && !self.in_water {
                let hit = if self.speed.abs() > speed.abs() {
                    self.speed
                } else {
                    speed
                };
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
pub(super) fn log_first(what: &str, times: u32) {
    static SEEN: std::sync::Mutex<Option<HashMap<String, u32>>> = std::sync::Mutex::new(None);
    let mut seen = lock(&SEEN);
    let n = seen
        .get_or_insert_with(HashMap::new)
        .entry(what.to_owned())
        .or_insert(0);
    if *n < times {
        *n += 1;
        log::info!("bedrock-debug: {what}");
    }
}

pub(super) static CAMERA_ROTATION: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// The camera's `yaw` and `pitch` in degrees, for `query.camera_rotation`.
pub fn set_camera_rotation(yaw: f32, pitch: f32) {
    let bits = (u64::from(yaw.to_bits()) << 32) | u64::from(pitch.to_bits());
    CAMERA_ROTATION.store(bits, std::sync::atomic::Ordering::Relaxed);
}

pub(super) fn camera_rotation() -> (f32, f32) {
    let bits = CAMERA_ROTATION.load(std::sync::atomic::Ordering::Relaxed);
    (
        f32::from_bits((bits >> 32) as u32),
        f32::from_bits(bits as u32),
    )
}

/// `query.rotation_to_camera`: the (pitch, yaw) in degrees that would face the camera from a
/// mob `from_camera` blocks away from it.
pub(super) fn rotation_to_camera(from_camera: [f32; 3]) -> (f32, f32) {
    let [x, y, z] = from_camera.map(|v| -v);
    let flat = x.hypot(z);
    if flat < 1e-4 && y.abs() < 1e-4 {
        return (0.0, 0.0);
    }
    ((-y.atan2(flat)).to_degrees(), (-x).atan2(z).to_degrees())
}

pub(super) fn is_villager(id: &str) -> bool {
    matches!(id, "minecraft:villager" | "minecraft:zombie_villager")
}

/// Bedrock's profession number for the Java profession at `index` of
/// [`super::super::render::PROFESSIONS`].
pub(super) fn bedrock_profession(index: u8) -> u32 {
    let java = super::super::render::PROFESSIONS
        [usize::from(index) % super::super::render::PROFESSIONS.len()];
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

pub(super) fn wrap_degrees(a: f32) -> f32 {
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
            "player_offhand_arm_height" => {
                Value::Num(super::super::render::first_person_arm_heights()[1])
            }
            "is_first_person" => Value::bool(self.m.extra.first_person),
            _ => Value::Null,
        }
    }
    fn variables(&mut self) -> Vec<(&'static str, Value)> {
        let m = self.m;
        // The swing's progress, 0 at rest to 1 at its end, as the Java mob keeps it.
        // At its end the swing is over (0), not carried on to 1 over one more tick: Actions &
        // Stuff's combo reads a swing still running 0.3 s after one began as the next click,
        // and a single click played the second hit after the first. A swing begun again before
        // the end starts from 0.
        let a = self.alpha;
        let (from, to) = (m.anim.attack_anim_o, m.anim.attack_anim);
        let mut swing = if to >= from {
            (from + (to - from) * a).clamp(0.0, 1.0)
        } else if to <= 0.0 {
            0.0
        } else {
            (to * a).clamp(0.0, 1.0)
        };
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
            (
                "is_paperdoll",
                Value::bool(super::super::cem_anim::is_in_gui()),
            ),
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
            (
                "player_arm_height",
                Value::Num(super::super::render::first_person_arm_heights()[0]),
            ),
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
            (
                "gliding_speed_value",
                Value::Num(if m.extra.fall_flying {
                    ((m.velocity.length_squared() as f32 / 0.2).powi(3)).max(1.0)
                } else {
                    1.0
                }),
            ),
            // Arms out in front: a zombie, a skeleton or a husk with something to attack.
            (
                "raise_arms",
                Value::bool(
                    m.ai.target.is_some() || {
                        let id: &str = &m.kind.id;
                        id.contains("zombie") || id.contains("husk") || id.contains("drowned")
                    },
                ),
            ),
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
        // A stand's Bedrock pose (`StandPose`: one of `BEDROCK_POSES`, else the default).
        if let Some(stand) = m.extra.stand_pose.as_deref() {
            let index = crate::entities::ArmorStand::bedrock_pose_index(&stand.pose).unwrap_or(0);
            vars.push(("armor_stand.pose_index", Value::Num(index as f32)));
        }
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
        if let Some(v) = super::super::bedrock_debug::query(name) {
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
            "is_in_ui" => Value::bool(super::super::cem_anim::is_in_gui()),
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
                    || (m.species() == super::super::Species::Blaze && super::super::nether::blaze_attacking(m)),
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
            "ground_speed"
                if m.kind.id == "minecraft:player"
                    && (m.extra.avatar_pose == crate::pose::Pose::Swimming
                        || m.extra.fall_flying) =>
            {
                let (dx, dy, dz) = (
                    m.position.x - m.previous.x,
                    m.position.y - m.previous.y,
                    m.position.z - m.previous.z,
                );
                Value::Num((dx * dx + dy * dy + dz * dz).sqrt() as f32 * 20.0)
            }
            "vertical_speed"
                if m.kind.id == "minecraft:player"
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
            "ride_body_x_rotation"
            | "ride_body_y_rotation"
            | "ride_head_x_rotation"
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
                Value::Num(if args.first().map_or(0.0, Value::num) as i32 == 0 {
                    pitch
                } else {
                    yaw
                })
            }
            "camera_rotation" => {
                let (yaw, pitch) = camera_rotation();
                Value::Num(if args.first().map_or(0.0, Value::num) as i32 == 0 {
                    pitch
                } else {
                    yaw
                })
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
                        Value::Str(s) => Some(horizon_bedrock::animation::lower(
                            s.trim_start_matches("minecraft:"),
                        )),
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
                    // An empty slot's name is "": `q.is_item_name_any('slot.armor.legs', 0, '')`
                    // is how a pack asks whether nothing is worn there (an armour stand shows its
                    // legs only under leggings or boots).
                    _ => Value::bool(match item.as_deref() {
                        Some(i) => {
                            let short = bedrock_item_name(i);
                            rest().any(|n| n == short)
                        }
                        None => rest().any(|n| n.is_empty()),
                    }),
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
            "is_powered" | "is_charged" => {
                Value::bool(if m.species() == super::super::Species::Blaze {
                    super::super::nether::blaze_charging(m)
                } else {
                    m.extra.charged
                })
            }
            // A camel sits by its own pose state (`Camel.isCamelSitting`), not a pet's `sitting`.
            "is_sitting" => Value::bool(
                m.extra.sitting
                    || (super::super::camel::is_camel(m.species())
                        && super::super::camel::sitting(m)),
            ),
            "sit_amount" => Value::Num(
                if m.extra.sitting
                    || (super::super::camel::is_camel(m.species())
                        && super::super::camel::sitting(m))
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
                m.extra.anger > 0
                    || (m.species() == super::super::Species::Enderman && m.extra.flag),
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
            "fuse_time" => Value::Num(if m.species() == super::super::Species::SulfurCube {
                // Seconds left on a lit fuse; 0 while the cube is not primed.
                super::super::sulfur_cube::fuse(m).max(0) as f32 / 20.0
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
            // The root actor: an entity that nothing holds is its own (Bedrock's
            // `query.is_owner_identifier_any` compares "the root actor identifier"). Actions &
            // Stuff's armour stand asks it for `minecraft:armor_stand` before it turns into a
            // "Prop" or "Item" stand, and every mob asks it for `minecraft:player`.
            "owner_identifier" => Value::Str({
                let java = m.kind.id.to_ascii_lowercase();
                bedrock_id(&java).map_or(java.clone(), str::to_owned).into()
            }),
            "is_owner_identifier_any" => Value::bool(holder_is_any(m, args)),
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
            "variant" if m.species() == super::super::Species::Pufferfish => {
                Value::Num(m.extra.size as f32)
            }
            // A shulker's variant is its shell colour: 0 to 15 the dyes, 16 the purple one it
            // has by default.
            "variant" if m.species() == super::super::Species::Shulker => {
                Value::Num(f32::from(m.extra.color))
            }
            "variant" => Value::Num(f32::from(m.extra.variant)),
            "swelling_dir" => Value::Num(match m.swell.cmp(&m.previous_swell) {
                std::cmp::Ordering::Greater => 1.0,
                std::cmp::Ordering::Less => -1.0,
                std::cmp::Ordering::Equal => 0.0,
            }),
            "is_on_screen" => Value::Num(1.0),
            "is_invisible" => Value::bool(super::super::trader::is_invisible(m)),
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
                (m.species() == super::super::Species::Sheep && m.extra.anim_ticks > 0)
                    || m.extra.using_item > 0,
            ),
            "is_roaring" => {
                Value::bool(m.species() == super::super::Species::Ravager && m.extra.anim_ticks > 0)
            }
            "is_stunned" => {
                Value::bool(m.species() == super::super::Species::Ravager && m.extra.counter > 0)
            }
            "hurt_direction" if m.extra.stand_pose.is_some() => Value::Num(
                m.extra
                    .stand_pose
                    .as_deref()
                    .map_or(0.0, |s| s.hurt_direction),
            ),
            "hurt_direction" | "head_roll_angle" | "sleep_rotation" | "roll_counter"
            | "sneeze_counter" | "unhappy_counter" | "shake_angle" | "shake_time" => {
                Value::Num(0.0)
            }
            "swim_amount" => Value::Num(f32::from(u8::from(m.fluids.in_water && !m.on_ground))),
            "lie_amount" => Value::Num(0.0),
            "standing_scale" => Value::Num(if m.species() == super::super::Species::PolarBear {
                super::super::polar_bear::standing_scale(m, self.alpha)
            } else {
                0.0
            }),
            // A vex flies at its target (`flag`): the pack's charge pose and attack trail.
            "is_charging" if m.species() == super::super::Species::Vex => Value::bool(m.extra.flag),
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
                    && super::super::riders::mount_kind(m.id).is_some_and(|k| {
                        args.iter()
                            .any(|a| matches!(a, Value::Str(s) if s.eq_ignore_ascii_case(&k)))
                    }),
            ),
            "is_sleeping" | "is_avoiding_mobs" | "is_shaking" => Value::Num(0.0),
            "time_stamp" => Value::Num(m.age as f32),
            "facing_target_to_range_attack" => {
                Value::bool(if m.species() == super::super::Species::Blaze {
                    super::super::nether::blaze_attacking(m)
                } else {
                    m.extra.using_item > 0
                })
            }
            "item_is_charged" => Value::bool(m.extra.flag2),
            "blocking" => Value::bool(m.extra.using_item > 0 && m.extra.off_hand.is_some()),
            // The properties vanilla's own entities carry, from the Java mob's state.
            "property" => match args.first() {
                Some(Value::Str(p)) if &**p == "minecraft:armadillo_state" => Value::Str(
                    match m.extra.armadillo.state {
                        super::super::armadillo::IDLE => "unrolled",
                        super::super::armadillo::UNROLLING => "rolled_up_unrolling",
                        _ => "rolled_up",
                    }
                    .into(),
                ),
                // The block a sulfur cube holds sets its archetype; `none` with nothing held (a
                // pack tests `!= 'none'` to tell a cube with a block from an empty one).
                Some(Value::Str(p)) if &**p == "minecraft:sulfur_cube_archetype" => Value::Str(
                    if m.species() != super::super::Species::SulfurCube || m.extra.body.is_none() {
                        "none"
                    } else {
                        super::super::sulfur_cube::archetype(m).map_or("regular", |a| a.name)
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
                Value::Num(if hi > lo {
                    ((d - lo) / (hi - lo)).clamp(0.0, 1.0)
                } else {
                    0.0
                })
            }
            "get_level_seed_based_fraction" => {
                Value::Num(((m.id.wrapping_mul(2654435761)) >> 8 & 0xFFFF) as f32 / 65536.0)
            }
            "average_frame_time" | "last_frame_time" | "minimum_frame_time"
            | "maximum_frame_time" => Value::Num(self.dt),
            "client_max_render_distance" => Value::Num(128.0),
            "client_memory_tier" | "server_memory_tier" => Value::Num(3.0),
            "last_input_mode_is_any" => Value::bool(args.iter().any(
                |a| matches!(a, Value::Str(s) if s.eq_ignore_ascii_case("keyboard_and_mouse")),
            )),
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
pub(super) fn bedrock_item_name(java: &str) -> std::borrow::Cow<'_, str> {
    let short = horizon_bedrock::animation::lower(java.trim_start_matches("minecraft:"));
    match &*short {
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
    .into()
}

/// The pack's attachables for a held Java item: by its own id, else by the id Bedrock gives it.
pub(super) fn attachables_for<'a>(
    index: &'a HashMap<String, Vec<String>>,
    item_lc: &str,
) -> Option<&'a Vec<String>> {
    index
        .get(item_lc)
        .or_else(|| index.get(&format!("minecraft:{}", bedrock_item_name(item_lc))))
}

pub(super) fn item_at(m: &Mob, slot: Option<&Value>) -> Option<Arc<str>> {
    // No slot named: the main hand (`query.is_item_equipped` and
    // `query.get_equipped_item_name` take an optional hand, the main one by default).
    let main = Value::Num(0.0);
    let slot = match slot.unwrap_or(&main) {
        Value::Str(s) => {
            let s = horizon_bedrock::animation::lower(s);
            slot_of(&s).or(match &*s {
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
pub(super) fn max_use_ticks(item: Option<&str>) -> f32 {
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
pub(super) fn armor_texture_index(m: &Mob, slot: usize) -> f32 {
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
pub(crate) fn armor_color(m: &Mob, slot: usize, channel: usize) -> f32 {
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
pub(super) fn item_has_tag(item: &str, tag: &str) -> bool {
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
pub(super) fn held_items(m: &Mob) -> Vec<(&'static str, Arc<str>)> {
    slot_items(m, true)
}

/// What each slot holds: as the pack's queries see it (`for_attachables` false: the sulfur
/// cube's block in its main hand) or as attachables dress it.
pub(super) fn slot_items(m: &Mob, for_attachables: bool) -> Vec<(&'static str, Arc<str>)> {
    let e = &m.extra;
    // A sulfur cube's block is the Java mob's body slot. Bedrock draws it as a block on the
    // cube's `rightitem` bone, never through an attachable (the generic held-block one would
    // shrink and tilt it by the hand's animation): the game draws it there, and the pack's
    // queries see it in the main hand (`item_at`).
    let cube = m.species() == super::super::Species::SulfurCube;
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
pub(super) const HISTORY_NAMES: [[&str; 2]; super::super::HISTORY] = [
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

/// A Java armor stand's pose over a pack's stand (`ArmorStandModel.setupAnim`): each part turns
/// by how far the stand's own pose is from the game's default one, over what the pack's
/// animations gave it. Bedrock's default pose holds the same numbers as Java's (`hlnmov`: left arm
/// -10, 0, -10), so a stand in the default pose is the pack's alone: its own poses for its modes
/// (Actions & Stuff's "Prop" and "Item" stands turn the arm that carries the item onto the ground;
/// the Java pose put in their place sank a tent into the ground). The body turns the waist, which
/// carries the torso's sticks.
/// Mobs whose head Bedrock's renderer draws apart from the body's scale (the mooshroom's,
/// which also carries the mushroom blocks): the head keeps only its own animated scale (and the
/// entity's), not what the animations scale the bones above it by. Actions & Stuff shrinks the
/// mooshroom to 0.01 (hiding those blocks) and scales `root` and `head` by 100 each; the snow
/// golem's same trick scales `root` alone, its head (under `body`) following it.
pub(super) fn head_drawn_apart(id: &str) -> bool {
    id == "minecraft:mooshroom"
}

/// `head_drawn_apart`: the head's scale divided by its ancestors' animated scales.
pub(super) fn head_apart(
    pose: &mut horizon_bedrock::animation::Pose,
    g: &horizon_bedrock::geometry::Geometry,
) {
    let parent_of = |n: &str| {
        g.bones
            .iter()
            .find(|b| b.name.eq_ignore_ascii_case(n))
            .and_then(|b| b.parent.clone())
    };
    let mut above = [1.0f32; 3];
    let mut at = parent_of("head");
    while let Some(p) = at {
        if let Some(b) = pose.get(&p) {
            for i in 0..3 {
                above[i] *= b.scale[i];
            }
        }
        at = parent_of(&p);
    }
    if let Some(h) = pose.bones.get_mut("head") {
        for i in 0..3 {
            if above[i].abs() > 1e-6 {
                h.scale[i] /= above[i];
            }
        }
    }
}

pub(crate) fn pose_stand(
    pose: &mut horizon_bedrock::animation::Pose,
    stand: &super::super::StandPose,
) {
    const DEFAULT: [[f32; 3]; 6] = crate::entities::ArmorStand::DEFAULT_POSE;
    const BONES: [&[&str]; 6] = [
        &["head"],
        &["waist"],
        &["leftarm"],
        &["rightarm"],
        &["leftleg"],
        &["rightleg"],
    ];
    // One of Bedrock's poses is the pack's to draw (`armor_stand.pose_index`): only a pose of
    // the game's own (a command's) turns the bones on top.
    let preset = crate::entities::ArmorStand::bedrock_pose_index(&stand.pose).is_some();
    for ((names, r), d) in BONES.iter().zip(stand.pose).zip(DEFAULT) {
        if preset || r == d {
            continue;
        }
        for name in *names {
            let b = pose.bones.entry((*name).to_owned()).or_default();
            for a in 0..3 {
                b.rotation[a] += r[a] - d[a];
            }
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
pub(super) fn is_wearable(item: &str) -> bool {
    let name = item.trim_start_matches("minecraft:");
    [
        "_helmet",
        "_chestplate",
        "_leggings",
        "_boots",
        "_head",
        "_skull",
    ]
    .iter()
    .any(|s| name.ends_with(s))
        || matches!(name, "elytra" | "carved_pumpkin" | "turtle_helmet")
}

/// The holder's bone an item in `slot` follows (`query.item_slot_to_bone_name`).
pub(crate) fn slot_bone(slot: &str) -> &'static str {
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
pub(super) fn slot_of(name: &str) -> Option<&'static str> {
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
pub(super) struct AttachHost<'a> {
    pub(super) inner: MobHost<'a>,
    pub(super) slot: &'static str,
    /// The holder's variables, for `context.owning_entity->variable.x`.
    pub(super) owner: &'a horizon_bedrock::molang::Vars,
}

/// Whether what `m` holds or wears in `slot` shows the enchantment glint
/// (`variable.is_enchanted` of an attachable).
pub(super) fn slot_enchanted(m: &Mob, slot: &str) -> bool {
    use super::super::gear;
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
            "is_owner_identifier_any" => super::super::bedrock_debug::query(name)
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
            // Bedrock eats and uses items from the main hand only, so a pack's use animation on a
            // held item asks only whether the holder uses one. Java uses either hand: the item in
            // the other hand is not the one in use.
            "is_using_item" | "is_eating"
                if matches!(self.slot, "main_hand" | "off_hand")
                    && (self.slot == "off_hand") != self.inner.m.extra.using_off_hand =>
            {
                Value::bool(false)
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
pub(super) fn holder_is_any(m: &Mob, args: &[Value]) -> bool {
    let java = m.kind.id.to_ascii_lowercase();
    let bedrock = bedrock_id(&java).unwrap_or(&java).to_owned();
    let mannequin = java == "minecraft:mannequin";
    args.iter().any(|a| {
        let Value::Str(s) = a else { return false };
        let want = s.trim().to_ascii_lowercase();
        let want = if want.contains(':') {
            want
        } else {
            format!("minecraft:{want}")
        };
        want == bedrock
            || want == java
            || (mannequin && matches!(want.as_str(), "minecraft:player" | "minecraft:armor_stand"))
    })
}
