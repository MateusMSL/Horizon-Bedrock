//! The game's memory over time, in its log: a thread samples the resident set and the
//! allocator's commit and writes a line whenever either moved by [`STEP`], so the log of a game
//! the system killed for its memory still shows how it got there; and [`phase`] guards that say
//! what one step of the game (a pack reload, a Bedrock conversion) cost and at what peak.
//!
//! Past [`WARN_SHARE`] of the memory the game may use (its cgroup's `memory.max` on Linux, else
//! the machine's RAM) a warning names who holds the heap, in a build that counts it
//! ([`crate::HEAP_CENSUS`]).
use std::{
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering::Relaxed},
    },
    time::{Duration, Instant},
};

const MB: u64 = 1 << 20;
/// How far the resident set or the heap moves before the timeline writes a line.
const STEP: u64 = 256 * MB;
/// How often the watcher samples.
const EVERY: Duration = Duration::from_millis(250);
/// The share of the allowed memory past which each new peak is a warning.
const WARN_SHARE: f64 = 0.75;
/// Phases measured at once at most (more are timed but not given a peak).
const SLOTS: usize = 16;

/// The highest resident set seen while each open phase runs (0: slot free).
static PEAKS: [AtomicU64; SLOTS] = [const { AtomicU64::new(0) }; SLOTS];
static STARTED: OnceLock<()> = OnceLock::new();

/// Bytes as `1.2 GB` or `340 MB`.
fn size(bytes: u64) -> String {
    if bytes >= 1 << 30 {
        format!("{:.1} GB", bytes as f64 / (1u64 << 30) as f64)
    } else {
        format!("{} MB", bytes / MB)
    }
}

/// A signed change as `+1.2 GB` or `-340 MB`.
fn change(from: u64, to: u64) -> String {
    if to >= from {
        format!("+{}", size(to - from))
    } else {
        format!("-{}", size(from - to))
    }
}

fn resident() -> u64 {
    crate::process_memory::resident_bytes().unwrap_or(0)
}

fn heap() -> u64 {
    crate::allocator_commit().0 as u64
}

/// What the game may hold before the system takes it away: its cgroup's limit (Linux), else
/// the machine's RAM; `None` when neither is known.
fn allowed() -> Option<u64> {
    cgroup_limit().or_else(machine_ram)
}

#[cfg(target_os = "linux")]
fn cgroup_limit() -> Option<u64> {
    let own = std::fs::read_to_string("/proc/self/cgroup").ok()?;
    let path = own.lines().find_map(|l| l.strip_prefix("0::"))?;
    let max = std::fs::read_to_string(format!("/sys/fs/cgroup{path}/memory.max")).ok()?;
    max.trim().parse().ok()
}

#[cfg(not(target_os = "linux"))]
fn cgroup_limit() -> Option<u64> {
    None
}

#[cfg(target_os = "linux")]
fn meminfo(key: &str) -> Option<u64> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let line = text.lines().find(|l| l.starts_with(key))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kib * 1024)
}

#[cfg(target_os = "linux")]
fn machine_ram() -> Option<u64> {
    meminfo("MemTotal:")
}

#[cfg(not(target_os = "linux"))]
fn machine_ram() -> Option<u64> {
    crate::process_memory::usage().map(|[_, _, ram]| ram)
}

/// The RAM the machine still has for anyone, where the system says.
#[cfg(target_os = "linux")]
fn available() -> Option<u64> {
    meminfo("MemAvailable:")
}

#[cfg(not(target_os = "linux"))]
fn available() -> Option<u64> {
    None
}

/// The heap's largest holders, in a build that counts them.
fn holders() -> String {
    if !crate::HEAP_CENSUS {
        return String::from("(build with --features alloc-census to see who holds the heap)");
    }
    crate::heap_census::census()
        .into_iter()
        .filter(|(_, bytes)| *bytes > 0)
        .take(12)
        .map(|(tag, bytes)| format!("{tag} {}", size(bytes as u64)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Starts the watcher thread once; later calls do nothing. `HORIZON_MEMORY_WATCH=0` keeps it
/// off.
pub fn start() {
    if std::env::var("HORIZON_MEMORY_WATCH").is_ok_and(|v| v.trim() == "0") {
        return;
    }
    STARTED.get_or_init(|| {
        let spawned = std::thread::Builder::new()
            .name("memory-watch".into())
            .spawn(watch);
        if let Err(e) = spawned {
            log::warn!("Horizon memory: the watcher did not start: {e}");
        }
    });
}

fn watch() {
    let mut allowed = allowed();
    let mut samples = 0u32;
    let started = Instant::now();
    let (mut logged_resident, mut logged_heap) = (resident(), heap());
    let mut logged_at = started;
    let mut peak = logged_resident;
    let mut warned_peak = 0;
    log::info!(
        "Horizon memory: watching (resident {}, heap {}, allowed {})",
        size(logged_resident),
        size(logged_heap),
        allowed.map_or_else(|| "unknown".into(), size),
    );
    loop {
        std::thread::sleep(EVERY);
        // The game may be moved under a limit after it started (an IDE's memory cap).
        samples = samples.wrapping_add(1);
        if samples.is_multiple_of(8) {
            let now = self::allowed();
            if now != allowed {
                log::info!(
                    "Horizon memory: the game may now use {}",
                    now.map_or_else(|| "unknown".into(), size)
                );
                allowed = now;
            }
        }
        let (now_resident, now_heap) = (resident(), heap());
        for slot in &PEAKS {
            // Only open slots (non-zero) rise.
            let mut seen = slot.load(Relaxed);
            while seen != 0 && now_resident > seen {
                match slot.compare_exchange_weak(seen, now_resident, Relaxed, Relaxed) {
                    Ok(_) => break,
                    Err(now) => seen = now,
                }
            }
        }
        peak = peak.max(now_resident);
        if now_resident.abs_diff(logged_resident) >= STEP || now_heap.abs_diff(logged_heap) >= STEP
        {
            log::info!(
                "Horizon memory: resident {} ({} in {:.1} s), heap {} ({}), peak {}{}",
                size(now_resident),
                change(logged_resident, now_resident),
                logged_at.elapsed().as_secs_f32(),
                size(now_heap),
                change(logged_heap, now_heap),
                size(peak),
                available().map_or_else(String::new, |a| format!(", RAM free {}", size(a))),
            );
            (logged_resident, logged_heap, logged_at) = (now_resident, now_heap, Instant::now());
        }
        // Each new peak past the warning share (in steps, so a slow climb warns a few times).
        if let Some(allowed) = allowed
            && now_resident as f64 >= allowed as f64 * WARN_SHARE
            && now_resident >= warned_peak + STEP
        {
            warned_peak = now_resident;
            log::warn!(
                "Horizon memory: {} of the {} the game may use; the heap: {}",
                size(now_resident),
                size(allowed),
                holders(),
            );
        }
    }
}

/// One step of the game measured: on drop, a log line with its time, the resident set and heap
/// before and after, and the highest resident set the watcher saw meanwhile. In a build that
/// counts the heap, what the step allocates is tagged with its name too.
pub struct Phase {
    name: &'static str,
    slot: Option<usize>,
    started: Instant,
    resident: u64,
    heap: u64,
    _tag: Option<crate::heap_census::Tag>,
}

/// Measure the step `name` until the guard drops.
pub fn phase(name: &'static str) -> Phase {
    let resident = resident();
    let slot = PEAKS.iter().position(|s| {
        s.compare_exchange(0, resident.max(1), Relaxed, Relaxed)
            .is_ok()
    });
    Phase {
        name,
        slot,
        started: Instant::now(),
        resident,
        heap: heap(),
        _tag: crate::heap_tag(name),
    }
}

impl Drop for Phase {
    fn drop(&mut self) {
        let (resident, heap) = (resident(), heap());
        let peak = self
            .slot
            .map_or(resident, |s| PEAKS[s].swap(0, Relaxed).max(resident));
        log::info!(
            "Horizon memory: {} took {:.2} s: resident {} -> {} ({}, peak {}), heap {}",
            self.name,
            self.started.elapsed().as_secs_f32(),
            size(self.resident),
            size(resident),
            change(self.resident, resident),
            size(peak),
            change(self.heap, heap),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_as_megabytes_then_gigabytes() {
        assert_eq!(size(340 * MB), "340 MB");
        assert_eq!(size(3 << 29), "1.5 GB");
        assert_eq!(change(2 << 30, 1 << 30), "-1.0 GB");
        assert_eq!(change(MB, 3 * MB), "+2 MB");
    }

    #[test]
    fn a_phase_sees_the_peak_the_watcher_records_and_frees_its_slot() {
        let measured = phase("test");
        let slot = measured.slot.expect("a free slot");
        // What the watcher does when the resident set rises above the phase's start.
        PEAKS[slot].fetch_max(u64::MAX / 2, Relaxed);
        assert_eq!(PEAKS[slot].load(Relaxed), u64::MAX / 2);
        drop(measured);
        assert_eq!(PEAKS[slot].load(Relaxed), 0, "the slot is free again");
    }
}
