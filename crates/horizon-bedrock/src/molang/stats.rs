//! What Molang costs: how long a pack's expressions took to parse and compile, and, while
//! [`set_timing`] is on, how long the runs took and how much of that the game's answers were.

use super::ast::Sym;
use super::eval::{Host, Value};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::time::{Duration, Instant};

static PARSE_NS: AtomicU64 = AtomicU64::new(0);
static COMPILE_NS: AtomicU64 = AtomicU64::new(0);
static PROGRAMS: AtomicU64 = AtomicU64::new(0);
static COMPILED: AtomicU64 = AtomicU64::new(0);

static TIMING: AtomicBool = AtomicBool::new(false);
static RUNS: AtomicU64 = AtomicU64::new(0);
static RUN_NS: AtomicU64 = AtomicU64::new(0);
static HOST_NS: AtomicU64 = AtomicU64::new(0);

thread_local! {
    /// Whether this thread is inside a timed run: a run the game's answer starts is part of it.
    static INSIDE: Cell<bool> = const { Cell::new(false) };
}

/// Every expression parsed so far ([`super::parse`]), on every thread.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CompileStats {
    /// Expressions parsed.
    pub programs: u64,
    /// Of those, the ones compiled to bytecode; the rest run on the tree walker.
    pub compiled: u64,
    /// Time spent reading the text into trees.
    pub parse: Duration,
    /// Time spent compiling the trees to bytecode.
    pub compile: Duration,
}

impl CompileStats {
    /// What was parsed between `self` (taken before) and `after`.
    pub fn since(self, after: CompileStats) -> CompileStats {
        CompileStats {
            programs: after.programs - self.programs,
            compiled: after.compiled - self.compiled,
            parse: after.parse.saturating_sub(self.parse),
            compile: after.compile.saturating_sub(self.compile),
        }
    }
}

pub fn compile_stats() -> CompileStats {
    CompileStats {
        programs: PROGRAMS.load(Relaxed),
        compiled: COMPILED.load(Relaxed),
        parse: Duration::from_nanos(PARSE_NS.load(Relaxed)),
        compile: Duration::from_nanos(COMPILE_NS.load(Relaxed)),
    }
}

pub(super) fn parsed(took: Duration) {
    PARSE_NS.fetch_add(took.as_nanos() as u64, Relaxed);
}

pub(super) fn compiled(ok: bool, took: Duration) {
    PROGRAMS.fetch_add(1, Relaxed);
    COMPILED.fetch_add(u64::from(ok), Relaxed);
    COMPILE_NS.fetch_add(took.as_nanos() as u64, Relaxed);
}

/// The runs since the last [`take_run_stats`], on every thread.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RunStats {
    pub runs: u64,
    /// Time inside the runs, the game's answers included: all of Molang's time.
    pub total: Duration,
    /// Of that, the time the game took to answer (`query.*`, `context.*`, arrays, other
    /// entities): what remains is the bytecode's (or the walker's) own.
    pub host: Duration,
}

impl RunStats {
    /// The runs' own time, without the game's answers.
    pub fn exec(&self) -> Duration {
        self.total.saturating_sub(self.host)
    }
}

/// Times every run from now on (two clock reads a run and two per answer of the game), or stops.
pub fn set_timing(on: bool) {
    TIMING.store(on, Relaxed);
}

pub fn timing() -> bool {
    TIMING.load(Relaxed)
}

/// The runs since the last call, and starts counting again.
pub fn take_run_stats() -> RunStats {
    RunStats {
        runs: RUNS.swap(0, Relaxed),
        total: Duration::from_nanos(RUN_NS.swap(0, Relaxed)),
        host: Duration::from_nanos(HOST_NS.swap(0, Relaxed)),
    }
}

/// Runs `f` timed when timing is on and this thread is not already in a timed run; `f` gets the
/// host to use (the timed one).
pub(super) fn timed<R>(host: &mut dyn Host, f: impl FnOnce(&mut dyn Host) -> R) -> R {
    if !timing() || INSIDE.with(Cell::get) {
        return f(host);
    }
    INSIDE.with(|i| i.set(true));
    let started = Instant::now();
    let mut timed = Timed {
        inner: host,
        ns: Cell::new(0),
    };
    let r = f(&mut timed);
    RUN_NS.fetch_add(started.elapsed().as_nanos() as u64, Relaxed);
    HOST_NS.fetch_add(timed.ns.get(), Relaxed);
    RUNS.fetch_add(1, Relaxed);
    INSIDE.with(|i| i.set(false));
    r
}

/// A host that adds up the time its answers take.
struct Timed<'a> {
    inner: &'a mut dyn Host,
    ns: Cell<u64>,
}

impl Timed<'_> {
    fn time<R>(&self, started: Instant, r: R) -> R {
        self.ns
            .set(self.ns.get() + started.elapsed().as_nanos() as u64);
        r
    }
}

impl Host for Timed<'_> {
    fn query(&mut self, name: &str, args: &[Value]) -> Value {
        let t = Instant::now();
        let r = self.inner.query(name, args);
        self.time(t, r)
    }
    fn pose_changed(&mut self, pose: &crate::animation::Pose) {
        let t = Instant::now();
        self.inner.pose_changed(pose);
        self.time(t, ())
    }
    fn variables(&mut self) -> Vec<(&'static str, Value)> {
        let t = Instant::now();
        let r = self.inner.variables();
        self.time(t, r)
    }
    fn context(&mut self, name: &str) -> Value {
        let t = Instant::now();
        let r = self.inner.context(name);
        self.time(t, r)
    }
    fn array(&self, name: &str) -> Option<Vec<Value>> {
        let t = Instant::now();
        let r = self.inner.array(name);
        self.time(t, r)
    }
    fn array_at(&self, name: &str, index: f32) -> Option<Value> {
        let t = Instant::now();
        let r = self.inner.array_at(name, index);
        self.time(t, r)
    }
    fn entity_var(&mut self, entity: u64, var: Sym) -> Value {
        let t = Instant::now();
        let r = self.inner.entity_var(entity, var);
        self.time(t, r)
    }
    fn entity_query(&mut self, entity: u64, name: &str, args: &[Value]) -> Value {
        let t = Instant::now();
        let r = self.inner.entity_query(entity, name, args);
        self.time(t, r)
    }
}
