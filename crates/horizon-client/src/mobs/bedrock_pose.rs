//! How often a Bedrock pack's mob is posed (`BedrockMobs::frame`). Away from the camera the pose
//! is worked out less often and the last one drawn in between: the mob still moves every frame,
//! its limbs at 30, 20 or 10 steps a second. A crowd of an add-on's mobs (hundreds of Molang
//! nodes each a frame) cost several milliseconds a frame at any distance. The player's own model
//! and the first-person view are posed every frame. Each mob steps at a phase of its own (by its
//! id), so a crowd's poses spread over the frames instead of all falling on one: that frame was
//! as slow as before, the others fast, and the game stuttered.

use super::Mob;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};

/// The pack's models drawn so far this frame (`drew_model`), and in the last one (`CROWD`).
static DRAWN: AtomicU32 = AtomicU32::new(0);
static CROWD: AtomicU32 = AtomicU32::new(0);
/// Up to this many models in view, the distances of `pose_step` are as written.
const CROWD_AS_WRITTEN: f32 = 24.0;

/// A frame's mobs are about to be drawn: the count of the last one's models becomes the crowd
/// `pose_step` reads.
///
/// With spans or a trace on (or `HORIZON_BEDROCK_PROFILE`), the Molang runs are timed, and the last
/// frame's are recorded: `molang`, all of Molang's time (the game's answers to the queries
/// included), and `molang_vm`, the bytecode's own.
pub fn begin_frame() {
    CROWD.store(DRAWN.swap(0, Relaxed), Relaxed);
    use horizon_bedrock::molang;
    use horizon_client_render::spans;
    let timed = molang::timing();
    molang::set_timing(spans::enabled() || spans::trace::on() || profiling());
    if timed {
        let runs = molang::take_run_stats();
        spans::record("molang", runs.total);
        spans::record("molang_vm", runs.exec());
    }
}

/// One of the pack's models was drawn this frame.
pub fn drew_model() {
    DRAWN.fetch_add(1, Relaxed);
}

/// `Instance::update` timed as `bd_update`: the poses worked out (Molang, animations,
/// controllers), apart from the frames `BedrockMobs::frame` hands back kept.
pub(super) fn update(
    instance: &mut horizon_bedrock::entity::Instance,
    def: &horizon_bedrock::entity::ClientEntity,
    assets: &horizon_bedrock::entity::Assets,
    host: &mut dyn horizon_bedrock::molang::Host,
    dt: f32,
) -> horizon_bedrock::entity::Frame {
    if !profiling() {
        return horizon_client_render::spans::time("bd_update", || {
            instance.update(def, assets, host, dt)
        });
    }
    let started = std::time::Instant::now();
    let frame =
        horizon_client_render::spans::time("bd_update", || instance.update(def, assets, host, dt));
    profile(&def.id, def.attachable, started.elapsed());
    frame
}

/// `HORIZON_BEDROCK_PROFILE=1`: the pose updates' time by entity (and attachable), logged every
/// five seconds, to find which of a crowd's kinds costs what in the real game, where the hosts'
/// answers take the pack down paths a benchmark does not.
fn profiling() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("HORIZON_BEDROCK_PROFILE").is_some())
}

fn profile(id: &str, attachable: bool, took: std::time::Duration) {
    type Table = (
        std::time::Instant,
        std::collections::HashMap<String, (u32, f64)>,
    );
    thread_local! {
        static TABLE: std::cell::RefCell<Option<Table>> = const { std::cell::RefCell::new(None) };
    }
    TABLE.with(|t| {
        let mut t = t.borrow_mut();
        let (since, by) = t.get_or_insert_with(|| (std::time::Instant::now(), Default::default()));
        let key = if attachable {
            format!("{id} (attachable)")
        } else {
            id.to_owned()
        };
        let e = by.entry(key).or_default();
        e.0 += 1;
        e.1 += took.as_secs_f64() * 1e6;
        let secs = since.elapsed().as_secs_f64();
        if secs < 5.0 {
            return;
        }
        let mut rows: Vec<_> = by.drain().collect();
        rows.sort_by(|a, b| b.1.1.total_cmp(&a.1.1));
        let line: Vec<String> = rows
            .iter()
            .take(12)
            .map(|(k, (n, us))| {
                format!(
                    "{k}: {:.0} us/s, {:.0}/s, {:.1} us each",
                    us / secs,
                    f64::from(*n) / secs,
                    us / f64::from(*n)
                )
            })
            .collect();
        log::info!("bedrock-profile: {}", line.join(" | "));
        *since = std::time::Instant::now();
    });
}

/// Whether `m`, posed at `then` (ticks) and drawn now at `now`, `from_camera` (blocks) away,
/// keeps that pose.
pub(super) fn keeps(m: &Mob, then: f32, now: f32, from_camera: [f32; 3]) -> bool {
    !m.extra.first_person
        && !m.extra.local_player
        && now >= then
        && same_pose_step(then, now, pose_step(from_camera, CROWD.load(Relaxed)), m.id)
}

/// How long (ticks) a mob `from_camera` (blocks) keeps its pose before it is worked out again,
/// with `crowd` of the pack's models in view: every frame within 16 blocks, then 30, 20 and 10
/// times a second from 16, 32 and 64. A larger crowd than [`CROWD_AS_WRITTEN`] draws those
/// distances in by the square root of how much larger: a copper golem farm's 90 golems in view
/// (8 ms of a frame, most of them within 16 blocks) step from 8 blocks.
fn pose_step(from_camera: [f32; 3], crowd: u32) -> f32 {
    let d2: f32 = from_camera.iter().map(|v| v * v).sum();
    let d2 = d2 * (crowd as f32 / CROWD_AS_WRITTEN).max(1.0);
    if d2 < 16.0 * 16.0 {
        0.0
    } else if d2 < 32.0 * 32.0 {
        20.0 / 30.0
    } else if d2 < 64.0 * 64.0 {
        1.0
    } else {
        2.0
    }
}

/// Whether `then` and `now` (ticks) fall in the same step of `step` ticks for the mob `id`, its
/// steps shifted by a fraction of one by its id; a step of 0 never holds.
fn same_pose_step(then: f32, now: f32, step: f32, id: u32) -> bool {
    if step <= 0.0 {
        return false;
    }
    // Sixteen phases, by the id's low bits mixed so neighbouring ids spread.
    let phase = (id.wrapping_mul(0x9E37_79B9) >> 28) as f32 / 16.0 * step;
    ((then + phase) / step).floor() == ((now + phase) / step).floor()
}

#[cfg(test)]
mod tests {
    /// A far crowd's poses are worked out once a step each, spread over the frames rather than
    /// all in one.
    #[test]
    fn a_far_crowd_poses_once_a_step_each_spread_over_the_frames() {
        let step = super::pose_step([0.0, 0.0, 40.0], 0);
        assert!(step > 0.0, "40 blocks away is not posed every frame");
        assert_eq!(super::pose_step([0.0, 0.0, 4.0], 0), 0.0, "near is");
        // 64 mobs at 600 frames a second (20 ticks a second) for one second.
        let dt = 20.0 / 600.0;
        let mut last = vec![0.0f32; 64];
        let mut posed = vec![0u32; 64];
        let mut busiest = 0;
        for frame in 1..=600 {
            let now = frame as f32 * dt;
            let mut this_frame = 0;
            for (id, then) in last.iter_mut().enumerate() {
                if !super::same_pose_step(*then, now, step, id as u32) {
                    *then = now;
                    posed[id] += 1;
                    this_frame += 1;
                }
            }
            busiest = busiest.max(this_frame);
        }
        // 20 steps a second at 40 blocks: each mob about 20 times, never every frame.
        assert!(posed.iter().all(|&n| (19..=21).contains(&n)), "{posed:?}");
        assert!(busiest <= 16, "{busiest} of 64 mobs posed in one frame");
        assert!(
            !super::same_pose_step(1.0, 1.0, 0.0, 7),
            "a step of 0 always poses"
        );
    }

    /// A small crowd keeps the distances as written; a large one steps nearer mobs too, and
    /// the nearest still every frame.
    #[test]
    fn a_large_crowd_steps_nearer_mobs() {
        let at = |d: f32, crowd| super::pose_step([d, 0.0, 0.0], crowd);
        assert_eq!(at(12.0, 10), 0.0);
        assert_eq!(at(12.0, 24), 0.0);
        assert!(at(12.0, 96) > 0.0, "a farm's golem 12 blocks off");
        assert!(at(12.0, 96) < at(40.0, 96), "nearer still steps more often");
        assert_eq!(at(4.0, 96), 0.0, "next to the camera every frame");
        assert_eq!(at(40.0, 24), at(40.0, 0), "no crowd below the threshold");
    }

    /// The time one update of a pack's mob takes with the game's own host (`MobHost`), which a
    /// benchmark's host cannot stand in for: `HORIZON_TEST_BEDROCK_PACK` (a converted pack),
    /// `HORIZON_POSE_KIND` (default the copper golem). A measure, not a check: run it by name
    /// with `--ignored --nocapture`, and profile it.
    #[test]
    #[ignore]
    fn time_a_pack_mob_posed_with_the_game_host() {
        let Some(pack) = std::env::var_os("HORIZON_TEST_BEDROCK_PACK") else {
            eprintln!("no HORIZON_TEST_BEDROCK_PACK: skipped");
            return;
        };
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let root = std::env::var_os("HORIZON_TEST_RESOURCES")
            .map_or_else(|| repo.join(".local/horizon-client/26.3"), Into::into);
        let types = crate::mobs::MobTypes::load(&root).unwrap();
        let bedrock = super::super::BedrockMobs::load(std::path::Path::new(&pack));
        let kind = std::env::var("HORIZON_POSE_KIND")
            .unwrap_or_else(|_| "minecraft:copper_golem".to_owned());
        // `first_person`: the player's first-person view model (posed every frame); the player
        // has no mob type of its own here, so it borrows the zombie's under the player's id.
        let first_person = kind == "first_person";
        let kind = if first_person {
            let mut player = types.types["minecraft:zombie"].clone();
            player.id = "minecraft:player".into();
            player
        } else {
            types.types[&kind].clone()
        };
        let kind = std::sync::Arc::new(kind);
        let rounds: u32 = std::env::var("HORIZON_POSE_ROUNDS")
            .ok()
            .and_then(|n| n.parse().ok())
            .unwrap_or(3000);
        let at = horizon_core::geometry::Vec3::new(0.5, 64.0, 0.5);
        let mut m = super::Mob::new(7, kind, at, 30.0);
        m.limb_speed = 0.6;
        m.extra.first_person = first_person;
        m.extra.local_player = first_person;
        // `HORIZON_POSE_HANDS=main,off`: what it holds (item ids).
        if let Ok(hands) = std::env::var("HORIZON_POSE_HANDS") {
            let mut hands = hands.split(',').map(str::trim).filter(|h| !h.is_empty());
            m.extra.main_hand = hands.next().map(Into::into);
            m.extra.off_hand = hands.next().map(Into::into);
        }
        let started = std::time::Instant::now();
        let mut updates = 0u32;
        // What the poses were, to compare a change that only makes posing faster.
        let mut sum = std::collections::hash_map::DefaultHasher::new();
        for i in 0..rounds {
            // Three frames a tick: each one a new time, so each is posed.
            if i % 3 == 0 {
                m.age += 1;
                m.limb_position += 0.37;
            }
            let alpha = (i % 3) as f32 / 3.0;
            if let Some(f) = bedrock.frame(&m, alpha, [0.0, 0.0, 3.0]) {
                updates += 1;
                if i % 97 == 0 {
                    use std::hash::Hash;
                    let mut bones: Vec<_> = f.pose.bones.iter().collect();
                    bones.sort_by(|a, b| a.0.cmp(b.0));
                    format!("{bones:?}").hash(&mut sum);
                }
            }
        }
        let us = started.elapsed().as_secs_f64() * 1e6 / f64::from(updates.max(1));
        use std::hash::Hasher;
        eprintln!(
            "{}: {updates} updates, {us:.1} us each, poses {:016x}",
            m.kind.id,
            sum.finish()
        );
    }
}
