//! Animations (`animations/*.json`) and animation controllers (`animation_controllers/*.json`).
//!
//! An [`Animation`] is keyframes on bones (rotation, position, scale), each component a number
//! or a Molang expression, plus timed sounds, particles and scripts. A [`Controller`] is a
//! state machine over animations whose transitions are Molang conditions. Both run against the
//! game through [`Env`]: the host answers `query.*`, the variables belong to the entity.
//!
//! Everything here works in Bedrock's frame (degrees, y up); [`crate::entity`] converts the
//! result to the game's part tree.

use crate::molang::{self, EvalCtx, Host, Program, Rng, Value, Vars};
use serde_json::Value as Json;
use std::collections::HashMap;

/// A number, or a Molang expression evaluated each frame.
#[derive(Clone, Debug, PartialEq)]
pub enum Scalar {
    Const(f32),
    Expr(Box<Program>),
}

impl Scalar {
    /// `v` is a JSON number or string; a string that is a bare number stays a constant.
    pub fn from_json(v: &Json) -> Option<Scalar> {
        match v {
            Json::Number(n) => Some(Scalar::Const(n.as_f64()? as f32)),
            Json::Bool(b) => Some(Scalar::Const(if *b { 1.0 } else { 0.0 })),
            Json::String(s) => Some(Scalar::compile(s)),
            _ => None,
        }
    }

    pub fn compile(src: &str) -> Scalar {
        match molang::parse(src) {
            Ok(p) => match p.constant() {
                Some(n) => Scalar::Const(n),
                None => Scalar::Expr(Box::new(p)),
            },
            Err(e) => {
                log::warn!(
                    "Horizon: Bedrock Molang does not parse ({e}): {}",
                    &src[..src.len().min(80)]
                );
                Scalar::Const(0.0)
            }
        }
    }

    pub fn eval(&self, env: &mut Env, scope: Scope) -> f32 {
        match self {
            Scalar::Const(n) => *n,
            Scalar::Expr(p) => env
                .run(p, scope, scope.this.map_or(Value::Null, Value::Num))
                .num(),
        }
    }
}

/// Compiles a Molang expression, or `None` (and a log line) when it does not parse.
pub fn compile(src: &str) -> Option<Program> {
    match molang::parse(src) {
        Ok(p) => Some(p),
        Err(e) => {
            log::warn!(
                "Horizon: Bedrock Molang does not parse ({e}): {}",
                &src[..src.len().min(80)]
            );
            None
        }
    }
}

/// How many `{` and `(` a line leaves open (a quoted string holds none).
pub fn open_depth(line: &str) -> i32 {
    let mut depth = 0;
    let mut quoted = false;
    for c in line.chars() {
        match c {
            '\'' => quoted = !quoted,
            '{' | '(' if !quoted => depth += 1,
            '}' | ')' if !quoted => depth -= 1,
            _ => {}
        }
    }
    depth
}

/// Compiles the lines of a script array. Each line is a program, except that a block the pack
/// opens on one line and closes on a later one (`"(v.a) ? {"`, `"  v.b = 1;"`, `"};"`, which the
/// vanilla entities write) is one program made of those lines.
pub fn compile_lines<'a>(lines: impl IntoIterator<Item = &'a str>) -> Vec<Program> {
    // A group that does not parse as a whole falls back to its lines one by one, so a stray `(`
    // costs its own line and not the ones after it.
    fn flush(group: &mut Vec<&str>, out: &mut Vec<Program>) {
        match group.as_slice() {
            [] => {}
            [one] => out.extend(compile(one)),
            many => match molang::parse(&many.join("\n")) {
                Ok(p) => out.push(p),
                Err(_) => out.extend(many.iter().filter_map(|l| compile(l))),
            },
        }
        group.clear();
    }
    let mut out = Vec::new();
    let mut group: Vec<&str> = Vec::new();
    let mut depth = 0;
    for line in lines {
        group.push(line);
        depth += open_depth(line);
        if depth <= 0 {
            flush(&mut group, &mut out);
            depth = 0;
        }
    }
    flush(&mut group, &mut out);
    out
}

/// What a running expression sees besides the entity: the time of the animation or state it is
/// part of.
#[derive(Clone, Copy, Debug, Default)]
pub struct Scope {
    /// `query.anim_time`.
    pub anim_time: f32,
    /// `query.state_time`.
    pub state_time: f32,
    pub any_finished: bool,
    pub all_finished: bool,
    /// `this`: the value the channel being evaluated has so far (what earlier animations gave it).
    pub this: Option<f32>,
}

struct Scoped<'a> {
    inner: &'a mut dyn Host,
    scope: Scope,
}

impl Host for Scoped<'_> {
    fn query(&mut self, name: &str, args: &[Value]) -> Value {
        match name {
            "anim_time" => Value::Num(self.scope.anim_time),
            "state_time" => Value::Num(self.scope.state_time),
            "any_animation_finished" => Value::bool(self.scope.any_finished),
            "all_animations_finished" => Value::bool(self.scope.all_finished),
            _ => self.inner.query(name, args),
        }
    }
    fn context(&mut self, name: &str) -> Value {
        self.inner.context(name)
    }
    fn array(&self, name: &str) -> Option<Vec<Value>> {
        self.inner.array(name)
    }
    fn entity_var(&mut self, entity: u64, var: molang::Sym) -> Value {
        self.inner.entity_var(entity, var)
    }
    fn entity_query(&mut self, entity: u64, name: &str, args: &[Value]) -> Value {
        self.inner.entity_query(entity, name, args)
    }
}

/// The entity an expression runs for: the game's answers, the entity's variables, a random
/// source.
pub struct Env<'a> {
    pub host: &'a mut dyn Host,
    pub vars: &'a mut Vars,
    pub rng: &'a mut Rng,
}

impl Env<'_> {
    pub fn run(&mut self, p: &Program, scope: Scope, this: Value) -> Value {
        let mut scoped = Scoped {
            inner: &mut *self.host,
            scope,
        };
        p.eval(&mut EvalCtx {
            host: &mut scoped,
            vars: &mut *self.vars,
            rng: &mut *self.rng,
            this,
        })
    }
}

type Vec3 = [Scalar; 3];

fn vec3(v: &Json) -> Option<Vec3> {
    match v {
        Json::Array(a) if a.len() >= 3 => Some([
            Scalar::from_json(&a[0])?,
            Scalar::from_json(&a[1])?,
            Scalar::from_json(&a[2])?,
        ]),
        // A lone number or expression: every component (scale's shorthand).
        Json::Number(_) | Json::String(_) => {
            let s = Scalar::from_json(v)?;
            Some([s.clone(), s.clone(), s])
        }
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interp {
    Linear,
    CatmullRom,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Keyframe {
    pub time: f32,
    /// The value approaching the keyframe, and leaving it.
    pub pre: Vec3,
    pub post: Vec3,
    pub interp: Interp,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Channel {
    Fixed(Vec3),
    Keys(Vec<Keyframe>),
    /// A rotation given as single-axis turns in the order they are made
    /// (`[{"z": 120}, {"x": 200}, ...]`, the vanilla first-person arm): the axis (0 x, 1 y,
    /// 2 z) and the degrees.
    Sequence(Vec<(usize, Scalar)>),
}

/// `[{"x": a}, {"z": b}, ...]`: single-axis turns, in order.
fn sequence(v: &Json) -> Option<Vec<(usize, Scalar)>> {
    let a = v.as_array()?;
    if a.is_empty() || !a.iter().all(Json::is_object) {
        return None;
    }
    a.iter()
        .map(|o| {
            let (k, v) = o.as_object()?.iter().next()?;
            let axis = match k.as_str() {
                "x" => 0,
                "y" => 1,
                "z" => 2,
                _ => return None,
            };
            Some((axis, Scalar::from_json(v)?))
        })
        .collect()
}

impl Channel {
    fn parse(v: &Json) -> Option<Channel> {
        match v {
            Json::Object(o) => {
                let mut keys = Vec::new();
                for (t, kf) in o {
                    let Ok(time) = t.trim().parse::<f32>() else {
                        continue;
                    };
                    let (pre, post, interp) = match kf {
                        Json::Object(k) => {
                            let post = vec3(k.get("post").or_else(|| k.get("vector"))?)?;
                            let pre = k.get("pre").and_then(vec3).unwrap_or_else(|| post.clone());
                            let interp = match k.get("lerp_mode").and_then(Json::as_str) {
                                Some("catmullrom") => Interp::CatmullRom,
                                _ => Interp::Linear,
                            };
                            (pre, post, interp)
                        }
                        other => {
                            let v = vec3(other)?;
                            (v.clone(), v, Interp::Linear)
                        }
                    };
                    keys.push(Keyframe {
                        time,
                        pre,
                        post,
                        interp,
                    });
                }
                if keys.is_empty() {
                    return None;
                }
                keys.sort_by(|a, b| a.time.total_cmp(&b.time));
                Some(Channel::Keys(keys))
            }
            other => vec3(other)
                .map(Channel::Fixed)
                .or_else(|| sequence(other).map(Channel::Sequence)),
        }
    }

    fn last_time(&self) -> f32 {
        match self {
            Channel::Fixed(_) | Channel::Sequence(_) => 0.0,
            Channel::Keys(k) => k.last().map_or(0.0, |k| k.time),
        }
    }

    fn eval3(v: &Vec3, env: &mut Env, scope: Scope, this: [f32; 3]) -> [f32; 3] {
        std::array::from_fn(|i| {
            v[i].eval(
                env,
                Scope {
                    this: Some(this[i]),
                    ..scope
                },
            )
        })
    }

    pub fn sample(&self, t: f32, env: &mut Env, scope: Scope, this: [f32; 3]) -> [f32; 3] {
        let keys = match self {
            Channel::Fixed(v) => return Self::eval3(v, env, scope, this),
            Channel::Keys(k) => k,
            Channel::Sequence(turns) => {
                // Each turn made after the ones before it, about the model's own axes: the
                // later ones on the left.
                let mut m = crate::entity::rotation_matrix([0.0; 3]);
                for (axis, deg) in turns {
                    let mut e = [0.0; 3];
                    e[*axis] = deg.eval(env, scope);
                    m = crate::entity::mat_mul(&crate::entity::rotation_matrix(e), &m);
                }
                return crate::entity::rotation_degrees(&m);
            }
        };
        let first = &keys[0];
        if t <= first.time {
            return Self::eval3(&first.pre, env, scope, this);
        }
        let last = &keys[keys.len() - 1];
        if t >= last.time {
            return Self::eval3(&last.post, env, scope, this);
        }
        let i = keys.partition_point(|k| k.time <= t) - 1;
        let (a, b) = (&keys[i], &keys[i + 1]);
        let f = (t - a.time) / (b.time - a.time);
        let (p1, p2) = (
            Self::eval3(&a.post, env, scope, this),
            Self::eval3(&b.pre, env, scope, this),
        );
        if a.interp == Interp::CatmullRom || b.interp == Interp::CatmullRom {
            let p0 = if i > 0 {
                Self::eval3(&keys[i - 1].post, env, scope, this)
            } else {
                p1
            };
            let p3 = if i + 2 < keys.len() {
                Self::eval3(&keys[i + 2].pre, env, scope, this)
            } else {
                p2
            };
            return std::array::from_fn(|c| catmull_rom(p0[c], p1[c], p2[c], p3[c], f));
        }
        std::array::from_fn(|c| p1[c] + (p2[c] - p1[c]) * f)
    }
}

fn catmull_rom(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let (t2, t3) = (t * t, t * t * t);
    0.5 * ((2.0 * p1)
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
}

#[derive(Clone, Debug, PartialEq)]
pub struct BoneAnim {
    /// Lower case.
    pub bone: String,
    pub rotation: Option<Channel>,
    pub position: Option<Channel>,
    pub scale: Option<Channel>,
    /// `relative_to.rotation: "entity"`: the rotation is the entity's, not added to the parent's.
    pub rotation_relative_to_entity: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Loop {
    No,
    Yes,
    HoldOnLastFrame,
}

/// Something an animation sets off at a time.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Sound {
        effect: String,
    },
    Particle {
        effect: String,
        locator: Option<String>,
        /// `pre_effect_script`: Molang the new emitter runs once as it starts.
        pre: Option<String>,
    },
    /// A controller state's effect, repeated every frame the state lasts: the effect keeps
    /// running while the state does and stops after it.
    Hold {
        effect: String,
        locator: Option<String>,
        pre: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Animation {
    /// `animation.name`, lower case.
    pub id: String,
    pub looping: Loop,
    /// Seconds; 0 when the file gives none and nothing is keyed.
    pub length: f32,
    pub override_previous: bool,
    pub anim_time_update: Option<Program>,
    pub blend_weight: Option<Program>,
    /// Seconds to wait before it first plays (`start_delay`).
    pub start_delay: Option<Program>,
    /// Seconds to wait between loops (`loop_delay`).
    pub loop_delay: Option<Program>,
    pub bones: Vec<BoneAnim>,
    /// Scripts run when their time passes.
    pub timeline: Vec<(f32, Vec<Program>)>,
    pub events: Vec<(f32, Event)>,
}

fn timed(v: Option<&Json>) -> Vec<(f32, &Json)> {
    let mut out: Vec<(f32, &Json)> = v
        .and_then(Json::as_object)
        .map(|o| {
            o.iter()
                .filter_map(|(k, v)| Some((k.trim().parse::<f32>().ok()?, v)))
                .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}

fn one_or_many(v: &Json) -> Vec<&Json> {
    match v {
        Json::Array(a) => a.iter().collect(),
        other => vec![other],
    }
}

impl Animation {
    pub fn parse(id: &str, v: &Json) -> Animation {
        let mut bones = Vec::new();
        if let Some(o) = v.get("bones").and_then(Json::as_object) {
            for (name, b) in o {
                let entity = b
                    .get("relative_to")
                    .and_then(|r| r.get("rotation"))
                    .and_then(Json::as_str)
                    == Some("entity");
                bones.push(BoneAnim {
                    bone: name.to_ascii_lowercase(),
                    rotation: b.get("rotation").and_then(Channel::parse),
                    position: b.get("position").and_then(Channel::parse),
                    scale: b.get("scale").and_then(Channel::parse),
                    rotation_relative_to_entity: entity,
                });
            }
        }
        let mut timeline = Vec::new();
        for (t, scripts) in timed(v.get("timeline")) {
            let progs: Vec<Program> = one_or_many(scripts)
                .into_iter()
                .filter_map(|s| s.as_str())
                .filter_map(compile)
                .collect();
            if !progs.is_empty() {
                timeline.push((t, progs));
            }
        }
        let mut events = Vec::new();
        for (t, list) in timed(v.get("sound_effects")) {
            for e in one_or_many(list) {
                if let Some(effect) = e.get("effect").and_then(Json::as_str) {
                    events.push((
                        t,
                        Event::Sound {
                            effect: effect.to_owned(),
                        },
                    ));
                }
            }
        }
        for (t, list) in timed(v.get("particle_effects")) {
            for e in one_or_many(list) {
                if let Some(effect) = e.get("effect").and_then(Json::as_str) {
                    let locator = e
                        .get("locator")
                        .and_then(Json::as_str)
                        .map(str::to_ascii_lowercase);
                    let pre = e
                        .get("pre_effect_script")
                        .and_then(Json::as_str)
                        .map(str::to_owned);
                    events.push((
                        t,
                        Event::Particle {
                            effect: effect.to_owned(),
                            locator,
                            pre,
                        },
                    ));
                }
            }
        }
        events.sort_by(|a, b| a.0.total_cmp(&b.0));
        let keyed = bones
            .iter()
            .flat_map(|b| [&b.rotation, &b.position, &b.scale])
            .flatten()
            .map(Channel::last_time)
            .fold(0.0f32, f32::max)
            .max(events.last().map_or(0.0, |e| e.0))
            .max(timeline.last().map_or(0.0, |e| e.0));
        Animation {
            id: id.to_ascii_lowercase(),
            looping: match v.get("loop") {
                Some(Json::Bool(true)) => Loop::Yes,
                Some(Json::String(s)) if s == "hold_on_last_frame" => Loop::HoldOnLastFrame,
                _ => Loop::No,
            },
            length: v
                .get("animation_length")
                .and_then(Json::as_f64)
                .map_or(keyed, |l| l as f32),
            override_previous: v
                .get("override_previous_animation")
                .and_then(Json::as_bool)
                .unwrap_or(false),
            anim_time_update: v
                .get("anim_time_update")
                .and_then(Json::as_str)
                .and_then(compile),
            blend_weight: v.get("blend_weight").and_then(|w| match w {
                Json::String(s) => compile(s),
                Json::Number(n) => compile(&n.to_string()),
                _ => None,
            }),
            start_delay: v.get("start_delay").and_then(number_or_expression),
            loop_delay: v.get("loop_delay").and_then(number_or_expression),
            bones,
            timeline,
            events,
        }
    }
}

fn number_or_expression(v: &Json) -> Option<Program> {
    match v {
        Json::String(s) => compile(s),
        Json::Number(n) => compile(&n.to_string()),
        _ => None,
    }
}

/// How far into an animation an entity is.
#[derive(Clone, Debug, Default)]
pub struct AnimState {
    pub time: f32,
    pub finished: bool,
    started: bool,
    /// Seconds left before it first plays, and whether `start_delay` was read.
    delay: f32,
    delay_known: bool,
    /// Seconds left of the wait between loops.
    pause: f32,
}

/// One bone's accumulated animation, in Bedrock's frame.
#[derive(Clone, Debug, PartialEq)]
pub struct BonePose {
    /// Degrees, added to the bone's rest rotation.
    pub rotation: [f32; 3],
    /// Degrees applied in the entity's frame instead of the parent's, added to `rotation` (a glow
    /// the engine turns to face the camera).
    pub rotation_entity: [f32; 3],
    /// An animation turns the bone relative to the entity (`relative_to.rotation: "entity"`),
    /// even by nothing: its whole `rotation` counts in the entity's frame, whatever its
    /// parent's turn.
    pub entity_frame: bool,
    pub position: [f32; 3],
    pub scale: [f32; 3],
}

impl Default for BonePose {
    fn default() -> Self {
        BonePose {
            rotation: [0.0; 3],
            rotation_entity: [0.0; 3],
            entity_frame: false,
            position: [0.0; 3],
            scale: [1.0; 3],
        }
    }
}

/// Every animated bone, by lower-case name.
#[derive(Clone, Debug, Default)]
pub struct Pose {
    pub bones: HashMap<String, BonePose>,
    /// Where each bone with a parent sits from that parent's pivot, in the model's own axes
    /// (pixels): what `this` starts from in a bone's `position` channel.
    pub rest: std::sync::Arc<HashMap<String, [f32; 3]>>,
}

impl Pose {
    pub fn get(&self, bone: &str) -> Option<&BonePose> {
        self.bones.get(&bone.to_ascii_lowercase())
    }
}

impl Animation {
    /// Moves the animation on by `dt` seconds and appends what it sets off.
    pub fn advance(&self, st: &mut AnimState, env: &mut Env, dt: f32, out: &mut Vec<Event>) {
        let old = st.time;
        let scope = Scope {
            anim_time: old,
            ..Scope::default()
        };
        if !st.delay_known {
            st.delay_known = true;
            if let Some(p) = &self.start_delay {
                st.delay = env.run(p, scope, Value::Null).num().max(0.0);
            }
        }
        if st.delay > 0.0 {
            st.delay -= dt;
            return;
        }
        if st.pause > 0.0 {
            st.pause -= dt;
            return;
        }
        let mut new = match &self.anim_time_update {
            Some(p) => env.run(p, scope, Value::Null).num(),
            None => old + dt,
        };
        if !new.is_finite() {
            new = old;
        }
        let first = !st.started;
        st.started = true;
        let (wrapped, time) = match self.looping {
            Loop::Yes if self.length > 0.0 && new >= self.length => (true, new % self.length),
            Loop::HoldOnLastFrame if self.length > 0.0 => (false, new.min(self.length)),
            _ => (false, new),
        };
        // A one-shot with no length ends at once; a loop has finished each time it comes round,
        // which is how a controller waits for one blink of a looping animation.
        st.finished = if self.looping == Loop::Yes {
            self.length > 0.0 && new >= self.length
        } else {
            new >= self.length
        };
        if wrapped && let Some(p) = &self.loop_delay {
            st.pause = env.run(p, scope, Value::Null).num().max(0.0);
        }
        // Timed things in (old, new]; the very first frame includes time 0.
        let from = if first { -1.0 } else { old };
        let fires = |t: f32| -> bool {
            if wrapped {
                (t > from && t <= self.length) || t <= time
            } else {
                t > from && t <= time
            }
        };
        let scope = Scope {
            anim_time: time,
            ..Scope::default()
        };
        for (t, progs) in &self.timeline {
            if fires(*t) {
                for p in progs {
                    env.run(p, scope, Value::Null);
                }
            }
        }
        for (t, e) in &self.events {
            if fires(*t) {
                out.push(e.clone());
            }
        }
        st.time = time;
    }

    /// Adds the animation, at `weight`, to `pose`.
    pub fn sample(&self, st: &AnimState, weight: f32, env: &mut Env, pose: &mut Pose) {
        if (st.finished && self.looping == Loop::No) || st.delay > 0.0 {
            return;
        }
        let scope = Scope {
            anim_time: st.time,
            ..Scope::default()
        };
        let weight = match &self.blend_weight {
            Some(p) => weight * env.run(p, scope, Value::Null).num(),
            None => weight,
        };
        if weight == 0.0 {
            return;
        }
        {
            for b in &self.bones {
                let entry = pose.bones.entry(b.bone.clone()).or_default();
                if self.override_previous {
                    *entry = BonePose::default();
                }
                if let Some(c) = &b.rotation {
                    // Bedrock keeps one rotation per bone: a channel `relative_to` the entity adds
                    // to what the others gave it (`this`, which the packs take out with
                    // `-this + ...`) and makes the whole of it count in the entity's frame.
                    entry.entity_frame |= b.rotation_relative_to_entity;
                    let v = c.sample(st.time, env, scope, entry.rotation);
                    for a in 0..3 {
                        entry.rotation[a] += v[a] * weight;
                    }
                }
                if let Some(c) = &b.position {
                    let rest = pose.rest.get(&b.bone.to_ascii_lowercase());
                    let here = std::array::from_fn(|a| {
                        entry.position[a] + rest.map_or(0.0, |r| r[a])
                    });
                    let v = c.sample(st.time, env, scope, here);
                    for a in 0..3 {
                        entry.position[a] += v[a] * weight;
                    }
                }
                if let Some(c) = &b.scale {
                    let v = c.sample(st.time, env, scope, entry.scale);
                    for a in 0..3 {
                        entry.scale[a] *= 1.0 + (v[a] - 1.0) * weight;
                    }
                }
            }
        }
        env.host.pose_changed(pose);
    }
}

// ---------------------------------------------------------------------------------------------
// Controllers

/// An animation a controller state plays: its short name and the Molang weight it plays at.
#[derive(Clone, Debug, PartialEq)]
pub struct StateAnim {
    pub name: String,
    pub weight: Option<Program>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub name: String,
    pub animations: Vec<StateAnim>,
    pub transitions: Vec<(String, Program)>,
    /// Seconds to blend in from the previous state; 0 switches at once.
    pub blend_transition: f32,
    /// Blend rotations by the shorter way round (`blend_via_shortest_path`).
    pub blend_via_shortest_path: bool,
    pub on_entry: Vec<Program>,
    pub on_exit: Vec<Program>,
    pub events: Vec<Event>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Controller {
    /// `controller.animation.name`, lower case.
    pub id: String,
    pub initial_state: String,
    pub states: Vec<State>,
}

fn programs(v: Option<&Json>) -> Vec<Program> {
    v.and_then(Json::as_array)
        .map(|a| compile_lines(a.iter().filter_map(Json::as_str)))
        .unwrap_or_default()
}

impl Controller {
    pub fn parse(id: &str, v: &Json) -> Controller {
        let mut states = Vec::new();
        if let Some(o) = v.get("states").and_then(Json::as_object) {
            for (name, s) in o {
                let animations = s
                    .get("animations")
                    .and_then(Json::as_array)
                    .map(|a| {
                        a.iter()
                            .flat_map(|e| match e {
                                Json::String(n) => vec![StateAnim {
                                    name: n.to_ascii_lowercase(),
                                    weight: None,
                                }],
                                Json::Object(o) => o
                                    .iter()
                                    .map(|(n, w)| StateAnim {
                                        name: n.to_ascii_lowercase(),
                                        weight: match w {
                                            Json::String(s) => compile(s),
                                            Json::Number(n) => compile(&n.to_string()),
                                            _ => None,
                                        },
                                    })
                                    .collect(),
                                _ => Vec::new(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let transitions = s
                    .get("transitions")
                    .and_then(Json::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(Json::as_object)
                            .flat_map(|o| o.iter())
                            .filter_map(|(to, cond)| {
                                let src = match cond {
                                    Json::String(s) => s.clone(),
                                    Json::Number(n) => n.to_string(),
                                    Json::Bool(b) => (if *b { "1" } else { "0" }).to_owned(),
                                    _ => return None,
                                };
                                Some((to.clone(), compile(&src)?))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let blend_transition = match s.get("blend_transition") {
                    Some(Json::Number(n)) => n.as_f64().unwrap_or(0.0) as f32,
                    Some(Json::Object(o)) => o
                        .keys()
                        .filter_map(|k| k.trim().parse::<f32>().ok())
                        .fold(0.0, f32::max),
                    _ => 0.0,
                };
                let mut events = Vec::new();
                for e in s.get("sound_effects").map(one_or_many).unwrap_or_default() {
                    if let Some(effect) = e.get("effect").and_then(Json::as_str) {
                        events.push(Event::Sound {
                            effect: effect.to_owned(),
                        });
                    }
                }
                for e in s
                    .get("particle_effects")
                    .map(one_or_many)
                    .unwrap_or_default()
                {
                    if let Some(effect) = e.get("effect").and_then(Json::as_str) {
                        let locator = e
                            .get("locator")
                            .and_then(Json::as_str)
                            .map(str::to_ascii_lowercase);
                        let pre = e
                            .get("pre_effect_script")
                            .and_then(Json::as_str)
                            .map(str::to_owned);
                        events.push(Event::Particle {
                            effect: effect.to_owned(),
                            locator,
                            pre,
                        });
                    }
                }
                states.push(State {
                    name: name.clone(),
                    animations,
                    transitions,
                    blend_transition,
                    blend_via_shortest_path: s
                        .get("blend_via_shortest_path")
                        .and_then(Json::as_bool)
                        .unwrap_or(false),
                    on_entry: programs(s.get("on_entry")),
                    on_exit: programs(s.get("on_exit")),
                    events,
                });
            }
        }
        let initial_state = v
            .get("initial_state")
            .and_then(Json::as_str)
            .unwrap_or("default")
            .to_owned();
        Controller {
            id: id.to_ascii_lowercase(),
            initial_state,
            states,
        }
    }

    fn state_index(&self, name: &str) -> Option<usize> {
        self.states.iter().position(|s| s.name == name)
    }
}

/// Every animation and controller of a pack.
#[derive(Debug, Default)]
pub struct Library {
    pub animations: HashMap<String, Animation>,
    pub controllers: HashMap<String, Controller>,
}

impl Library {
    /// Adds the animations and controllers in an animation or controller file.
    pub fn add_file(&mut self, root: &Json) {
        if let Some(o) = root.get("animations").and_then(Json::as_object) {
            for (id, a) in o {
                self.animations
                    .insert(id.to_ascii_lowercase(), Animation::parse(id, a));
            }
        }
        if let Some(o) = root.get("animation_controllers").and_then(Json::as_object) {
            for (id, c) in o {
                self.controllers
                    .insert(id.to_ascii_lowercase(), Controller::parse(id, c));
            }
        }
    }
}

/// The running state of one animation or controller of one entity.
#[derive(Clone, Debug)]
pub enum Node {
    Anim(AnimState),
    Ctrl(Box<CtrlState>),
}

#[derive(Clone, Debug)]
pub struct CtrlState {
    /// The controller's full name, to find its definition again.
    full: String,
    state: usize,
    time: f32,
    /// The state being blended out, with its children, and how far the blend is.
    previous: Option<(usize, Children)>,
    blend: f32,
    children: Children,
    entered: bool,
}

pub type Children = HashMap<String, Node>;

/// The running nodes as text, for a log: `name@seconds` for an animation (`!` once it has
/// finished), `name[state]` for a controller followed by what its state plays.
pub fn describe(
    nodes: &Children,
    lib: &Library,
    names: &HashMap<String, String>,
    out: &mut Vec<String>,
) {
    let mut keys: Vec<&String> = nodes.keys().collect();
    keys.sort();
    for k in keys {
        match &nodes[k] {
            Node::Anim(a) => out.push(format!(
                "{k}@{:.2}{}",
                a.time,
                if a.finished { "!" } else { "" }
            )),
            Node::Ctrl(c) => {
                let state = names
                    .get(k)
                    .and_then(|n| lib.controllers.get(&n.to_ascii_lowercase()))
                    .and_then(|ctrl| ctrl.states.get(c.state))
                    .map_or("?", |s| s.name.as_str());
                let mut inner = Vec::new();
                describe(&c.children, lib, names, &mut inner);
                out.push(format!(
                    "{k}[{state} {:.2}s]{{{}}}",
                    c.time,
                    inner.join(" ")
                ));
            }
        }
    }
}

/// What running a node needs.
pub struct Run<'a, 'b> {
    pub lib: &'a Library,
    /// The entity's `animations` map: short name to `animation.x` or `controller.animation.x`.
    pub names: &'a HashMap<String, String>,
    /// The entity's `animation_controllers` section, a name space of its own: what `animate`
    /// names first, while a controller's states name the `animations` first (`move` is both the
    /// animation and the controller that plays it in the vanilla entities).
    pub controllers: &'a HashMap<String, String>,
    pub env: &'a mut Env<'b>,
    pub pose: &'a mut Pose,
    pub events: &'a mut Vec<Event>,
    pub dt: f32,
}

impl Run<'_, '_> {
    /// Advances and applies the animation or controller called `name` (a short name) at `weight`.
    pub fn play(&mut self, nodes: &mut Children, name: &str, weight: f32) {
        thread_local! {
            static NESTING: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
        }
        /// Controllers that play controllers go this deep; past it a pack has a loop (a state
        /// that plays the controller it belongs to), which is cut rather than followed.
        const MAX_NESTING: u32 = 24;
        let (first, second) = if NESTING.get() == 0 {
            (self.controllers, self.names)
        } else {
            (self.names, self.controllers)
        };
        let Some(full) = first.get(name).or_else(|| second.get(name)) else {
            return;
        };
        let full = full.to_ascii_lowercase();
        if NESTING.get() >= MAX_NESTING {
            log::warn!("Horizon: Bedrock animation `{name}` ({full}) plays itself: cut");
            return;
        }
        NESTING.set(NESTING.get() + 1);
        self.play_node(nodes, name, &full, weight);
        NESTING.set(NESTING.get() - 1);
    }

    fn play_node(&mut self, nodes: &mut Children, name: &str, full: &str, weight: f32) {
        let full = full.to_owned();
        if let Some(anim) = self.lib.animations.get(&full) {
            let node = nodes
                .entry(name.to_owned())
                .or_insert_with(|| Node::Anim(AnimState::default()));
            if let Node::Anim(st) = node {
                anim.advance(st, self.env, self.dt, self.events);
                anim.sample(st, weight, self.env, self.pose);
            }
        } else if let Some(ctrl) = self.lib.controllers.get(&full) {
            let node = nodes.entry(name.to_owned()).or_insert_with(|| {
                let state = ctrl.state_index(&ctrl.initial_state).unwrap_or(0);
                Node::Ctrl(Box::new(CtrlState {
                    full: full.to_owned(),
                    state,
                    time: 0.0,
                    previous: None,
                    blend: 1.0,
                    children: HashMap::new(),
                    entered: false,
                }))
            });
            if let Node::Ctrl(st) = node {
                self.controller(ctrl, st, weight);
            }
        }
    }

    fn finished(&self, children: &Children, state: &State) -> (bool, bool) {
        let mut any = false;
        let mut all = !state.animations.is_empty();
        for a in &state.animations {
            let known = self.names.get(&a.name).is_some_and(|n| {
                let n = n.to_ascii_lowercase();
                self.lib.animations.contains_key(&n) || self.lib.controllers.contains_key(&n)
            });
            let done = match children.get(&a.name) {
                Some(Node::Anim(s)) => s.finished,
                Some(Node::Ctrl(_)) => false,
                // An animation the entity does not have plays nothing, which is over; one it
                // has but is not playing (weight 0) is not.
                None => !known,
            };
            any |= done;
            all &= done;
        }
        (any, all)
    }

    fn controller(&mut self, ctrl: &Controller, st: &mut CtrlState, weight: f32) {
        let Some(state) = ctrl.states.get(st.state) else {
            return;
        };
        if !st.entered {
            st.entered = true;
            let scope = Scope::default();
            for p in &state.on_entry {
                self.env.run(p, scope, Value::Null);
            }
            self.events.extend(state.events.iter().cloned());
        }
        // Transitions: the first whose condition holds.
        let (any, all) = self.finished(&st.children, state);
        let scope = Scope {
            state_time: st.time,
            any_finished: any,
            all_finished: all,
            ..Scope::default()
        };
        let mut target = None;
        // The conditions of one state share their `temp.*` variables: a pack sets one in the
        // first and tests it in the next (`t.c = q.x ? 'a' : 'b'; ...` then `t.c == 'b'`).
        crate::molang::shared_temps(|| {
            for (to, cond) in &state.transitions {
                if self.env.run(cond, scope, Value::Null).truthy()
                    && let Some(i) = ctrl.state_index(to)
                {
                    target = Some(i);
                    break;
                }
            }
        });
        if let Some(next) = target {
            for p in &state.on_exit {
                self.env.run(p, scope, Value::Null);
            }
            let old_children = std::mem::take(&mut st.children);
            let blend_time = ctrl.states[next].blend_transition;
            st.previous = (blend_time > 0.0).then_some((st.state, old_children));
            st.blend = if blend_time > 0.0 { 0.0 } else { 1.0 };
            st.state = next;
            st.time = 0.0;
            let scope = Scope::default();
            for p in &ctrl.states[next].on_entry {
                self.env.run(p, scope, Value::Null);
            }
            self.events.extend(ctrl.states[next].events.iter().cloned());
        }
        let state = &ctrl.states[st.state];
        for e in &state.events {
            if let Event::Particle { effect, locator, pre } = e {
                self.events.push(Event::Hold {
                    effect: effect.clone(),
                    locator: locator.clone(),
                    pre: pre.clone(),
                });
            }
        }
        st.time += self.dt;
        let blend_time = state.blend_transition;
        if st.previous.is_some() {
            st.blend = if blend_time > 0.0 {
                (st.blend + self.dt / blend_time).min(1.0)
            } else {
                1.0
            };
        }
        let scope = Scope {
            state_time: st.time,
            ..Scope::default()
        };
        if state.blend_via_shortest_path && st.blend < 1.0 && st.previous.is_some() {
            self.blend_shortest(ctrl, st, scope, weight);
            return;
        }
        // The previous state fades out.
        if let Some((prev, mut children)) = st.previous.take() {
            if st.blend < 1.0 {
                let prev_state = &ctrl.states[prev];
                self.play_state(prev_state, &mut children, (1.0 - st.blend) * weight, scope);
                st.previous = Some((prev, children));
            }
        }
        let w = if st.previous.is_some() {
            st.blend * weight
        } else {
            weight
        };
        let mut children = std::mem::take(&mut st.children);
        self.play_state(state, &mut children, w, scope);
        st.children = children;
    }

    /// Blends the previous state into the current one with each rotation turning the shorter way
    /// round, instead of adding the two weighted poses.
    fn blend_shortest(&mut self, ctrl: &Controller, st: &mut CtrlState, scope: Scope, weight: f32) {
        let outer = std::mem::take(&mut *self.pose);
        let mut from = Pose {
            rest: outer.rest.clone(),
            ..Pose::default()
        };
        if let Some((prev, mut children)) = st.previous.take() {
            std::mem::swap(&mut *self.pose, &mut from);
            self.play_state(&ctrl.states[prev], &mut children, 1.0, scope);
            std::mem::swap(&mut *self.pose, &mut from);
            st.previous = Some((prev, children));
        }
        let mut to = Pose {
            rest: outer.rest.clone(),
            ..Pose::default()
        };
        let mut children = std::mem::take(&mut st.children);
        std::mem::swap(&mut *self.pose, &mut to);
        self.play_state(&ctrl.states[st.state], &mut children, 1.0, scope);
        std::mem::swap(&mut *self.pose, &mut to);
        st.children = children;
        *self.pose = outer;
        let b = st.blend.clamp(0.0, 1.0);
        let turn = |a: f32, z: f32| {
            let d = (z - a + 180.0).rem_euclid(360.0) - 180.0;
            a + d * b
        };
        let mut names: Vec<&String> = from.bones.keys().chain(to.bones.keys()).collect();
        names.sort();
        names.dedup();
        for name in names {
            let a = from.bones.get(name).cloned().unwrap_or_default();
            let z = to.bones.get(name).cloned().unwrap_or_default();
            let entry = self.pose.bones.entry(name.clone()).or_default();
            entry.entity_frame |= a.entity_frame || z.entity_frame;
            for i in 0..3 {
                entry.rotation[i] += turn(a.rotation[i], z.rotation[i]) * weight;
                entry.rotation_entity[i] +=
                    turn(a.rotation_entity[i], z.rotation_entity[i]) * weight;
                entry.position[i] += (a.position[i] + (z.position[i] - a.position[i]) * b) * weight;
                let scale = a.scale[i] + (z.scale[i] - a.scale[i]) * b;
                entry.scale[i] *= 1.0 + (scale - 1.0) * weight;
            }
        }
    }

    /// A controller that stops being played starts over at its initial state, and that state's
    /// `on_entry` runs: the flags the controller set while it ran (`v.busy = 1`) are the ones
    /// the pack clears there, and nothing else would while the controller is off.
    fn reset(&mut self, node: Node) {
        let Node::Ctrl(st) = node else { return };
        for (_, child) in st.children {
            self.reset(child);
        }
        let Some(ctrl) = self.lib.controllers.get(&st.full) else {
            return;
        };
        let initial = ctrl.state_index(&ctrl.initial_state).unwrap_or(0);
        if st.entered
            && st.state != initial
            && let Some(state) = ctrl.states.get(initial)
        {
            for p in &state.on_entry {
                self.env.run(p, Scope::default(), Value::Null);
            }
        }
    }

    fn play_state(&mut self, state: &State, children: &mut Children, weight: f32, scope: Scope) {
        let mut touched = Vec::with_capacity(state.animations.len());
        for a in &state.animations {
            let w = match &a.weight {
                Some(p) => self.env.run(p, scope, Value::Null).num(),
                None => 1.0,
            };
            touched.push(a.name.as_str());
            if w != 0.0 {
                self.play(children, &a.name, w * weight);
            } else if matches!(children.get(&a.name), Some(Node::Ctrl(_)))
                && let Some(node) = children.remove(&a.name)
            {
                self.reset(node);
            }
        }
        // Animations the state stopped playing start over when they come back.
        children.retain(|name, _| touched.contains(&name.as_str()));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_block_over_several_lines_is_one_program() {
        let lines = [
            "v.a = 1;",
            "(v.a) ? {",
            "  t.b = 2;",
            "  v.c = t.b + 1;",
            "};",
            "v.d = '{(' ;",
        ];
        assert_eq!(compile_lines(lines).len(), 3);
        // A line that does not parse is lost on its own, not with the ones around it.
        assert_eq!(compile_lines(["v.a = 1;", "v.b = (;", "v.c = 3;"]).len(), 2);
    }

    use super::*;
    use crate::jsonc;

    #[derive(Default)]
    struct H {
        q: HashMap<String, f32>,
    }
    impl Host for H {
        fn query(&mut self, name: &str, _args: &[Value]) -> Value {
            self.q.get(name).map_or(Value::Null, |n| Value::Num(*n))
        }
    }

    fn lib(text: &str) -> Library {
        let mut l = Library::default();
        l.add_file(&jsonc::parse(text).unwrap());
        l
    }

    struct Rig {
        host: H,
        vars: Vars,
        rng: Rng,
        pose: Pose,
        events: Vec<Event>,
    }

    impl Rig {
        fn new() -> Rig {
            Rig {
                host: H::default(),
                vars: Vars::default(),
                rng: Rng::new(1),
                pose: Pose::default(),
                events: Vec::new(),
            }
        }
        fn frame(
            &mut self,
            lib: &Library,
            names: &HashMap<String, String>,
            nodes: &mut Children,
            entry: &str,
            dt: f32,
        ) {
            self.pose = Pose::default();
            let mut env = Env {
                host: &mut self.host,
                vars: &mut self.vars,
                rng: &mut self.rng,
            };
            let mut run = Run {
                lib,
                names,
                controllers: &HashMap::new(),
                env: &mut env,
                pose: &mut self.pose,
                events: &mut self.events,
                dt,
            };
            run.play(nodes, entry, 1.0);
        }
    }

    fn names(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect()
    }

    const WALK: &str = r#"{"format_version":"1.8.0","animations":{
      "animation.t.walk":{"loop":true,"animation_length":2.0,"bones":{
        "Head":{"rotation":{"0.0":[0,0,0],"1.0":[10,20,30],"2.0":[0,0,0]},"position":[0,"q.speed",0],"scale":2}}},
      "animation.t.once":{"animation_length":1.0,"sound_effects":{"0.5":{"effect":"ding"}},
        "particle_effects":{"0.0":{"effect":"puff","locator":"Mouth"}},"bones":{"head":{"rotation":[5,0,0]}}}
    }}"#;

    #[test]
    fn keyframes_lerp_and_loop() {
        let l = lib(WALK);
        let n = names(&[("walk", "animation.t.walk")]);
        let mut nodes = Children::new();
        let mut rig = Rig::new();
        rig.host.q.insert("speed".into(), 3.0);
        rig.frame(&l, &n, &mut nodes, "walk", 0.0);
        let p = rig.pose.get("head").unwrap();
        assert_eq!(p.rotation, [0.0; 3]);
        assert_eq!(p.position, [0.0, 3.0, 0.0]);
        assert_eq!(p.scale, [2.0; 3]);
        rig.frame(&l, &n, &mut nodes, "walk", 0.5);
        assert_eq!(rig.pose.get("HEAD").unwrap().rotation, [5.0, 10.0, 15.0]);
        rig.frame(&l, &n, &mut nodes, "walk", 0.5);
        assert_eq!(rig.pose.get("head").unwrap().rotation, [10.0, 20.0, 30.0]);
        // 2.5 s: wrapped to 0.5.
        rig.frame(&l, &n, &mut nodes, "walk", 1.5);
        assert_eq!(rig.pose.get("head").unwrap().rotation, [5.0, 10.0, 15.0]);
    }

    #[test]
    fn one_shot_animation_ends_and_fires_events_once() {
        let l = lib(WALK);
        let n = names(&[("once", "animation.t.once")]);
        let mut nodes = Children::new();
        let mut rig = Rig::new();
        rig.frame(&l, &n, &mut nodes, "once", 0.0);
        assert_eq!(
            rig.events,
            [Event::Particle {
                effect: "puff".into(),
                locator: Some("mouth".into()),
                pre: None
            }]
        );
        rig.events.clear();
        rig.frame(&l, &n, &mut nodes, "once", 0.4);
        assert!(rig.events.is_empty());
        rig.frame(&l, &n, &mut nodes, "once", 0.2);
        assert_eq!(
            rig.events,
            [Event::Sound {
                effect: "ding".into()
            }]
        );
        rig.events.clear();
        assert_eq!(rig.pose.get("head").unwrap().rotation, [5.0, 0.0, 0.0]);
        rig.frame(&l, &n, &mut nodes, "once", 0.5);
        assert!(rig.events.is_empty());
        // Over: no contribution.
        rig.frame(&l, &n, &mut nodes, "once", 0.5);
        assert!(
            rig.pose.get("head").is_none() || rig.pose.get("head").unwrap().rotation == [0.0; 3]
        );
    }

    #[test]
    fn looping_animation_fires_each_lap() {
        let l = lib(
            r#"{"animations":{"animation.t.l":{"loop":true,"animation_length":1.0,"sound_effects":{"0.25":{"effect":"tick"}}}}}"#,
        );
        let n = names(&[("l", "animation.t.l")]);
        let mut nodes = Children::new();
        let mut rig = Rig::new();
        for _ in 0..30 {
            rig.frame(&l, &n, &mut nodes, "l", 0.1);
        }
        let ticks = rig.events.len();
        assert!((2..=3).contains(&ticks), "{ticks}");
    }

    #[test]
    fn catmull_rom_passes_through_its_keys() {
        let l = lib(
            r#"{"animations":{"animation.t.c":{"animation_length":3.0,"bones":{"a":{"position":{
          "0":{"post":[0,0,0],"lerp_mode":"catmullrom"},"1":{"post":[10,0,0],"lerp_mode":"catmullrom"},
          "2":{"post":[0,0,0],"lerp_mode":"catmullrom"},"3":[0,0,0]}}}}}}"#,
        );
        let n = names(&[("c", "animation.t.c")]);
        let mut nodes = Children::new();
        let mut rig = Rig::new();
        rig.frame(&l, &n, &mut nodes, "c", 0.0);
        rig.frame(&l, &n, &mut nodes, "c", 1.0);
        assert!((rig.pose.get("a").unwrap().position[0] - 10.0).abs() < 1e-4);
        rig.frame(&l, &n, &mut nodes, "c", 0.5);
        let mid = rig.pose.get("a").unwrap().position[0];
        assert!(mid > 5.0, "a spline overshoots the straight line: {mid}");
    }

    #[test]
    fn weights_override_and_scale_blend() {
        let l = lib(r#"{"animations":{
          "animation.t.a":{"loop":true,"animation_length":1,"bones":{"b":{"rotation":[10,0,0],"scale":3}}},
          "animation.t.b":{"loop":true,"animation_length":1,"override_previous_animation":true,"bones":{"b":{"rotation":[4,0,0]}}}}}"#);
        let mut rig = Rig::new();
        let mut env = Env {
            host: &mut rig.host,
            vars: &mut rig.vars,
            rng: &mut rig.rng,
        };
        let a = &l.animations["animation.t.a"];
        let b = &l.animations["animation.t.b"];
        let st = AnimState::default();
        let mut pose = Pose::default();
        a.sample(&st, 0.5, &mut env, &mut pose);
        assert_eq!(pose.get("b").unwrap().rotation, [5.0, 0.0, 0.0]);
        assert_eq!(pose.get("b").unwrap().scale, [2.0; 3]);
        b.sample(&st, 1.0, &mut env, &mut pose);
        assert_eq!(pose.get("b").unwrap().rotation, [4.0, 0.0, 0.0]);
        assert_eq!(pose.get("b").unwrap().scale, [1.0; 3]);
    }

    #[test]
    fn anim_time_update_drives_the_clock() {
        let l = lib(
            r#"{"animations":{"animation.t.u":{"loop":true,"animation_length":10,"anim_time_update":"q.speed * 2","bones":{"b":{"position":["q.anim_time",0,0]}}}}}"#,
        );
        let n = names(&[("u", "animation.t.u")]);
        let mut nodes = Children::new();
        let mut rig = Rig::new();
        rig.host.q.insert("speed".into(), 3.0);
        rig.frame(&l, &n, &mut nodes, "u", 0.1);
        assert_eq!(rig.pose.get("b").unwrap().position[0], 6.0);
    }

    #[test]
    fn start_and_loop_delays_hold_an_animation_back() {
        let l = lib(r#"{"animations":{
          "animation.t.d":{"loop":true,"animation_length":1,"start_delay":"0.5","loop_delay":1,
            "bones":{"b":{"position":["q.anim_time * 10",0,0]}}}}}"#);
        let n = names(&[("d", "animation.t.d")]);
        let mut nodes = Children::new();
        let mut rig = Rig::new();
        // Nothing plays through the start delay.
        rig.frame(&l, &n, &mut nodes, "d", 0.3);
        assert!(rig.pose.get("b").is_none());
        rig.frame(&l, &n, &mut nodes, "d", 0.3);
        assert!(rig.pose.get("b").is_none() || rig.pose.get("b").unwrap().position[0] == 0.0);
        // Then it runs, and waits a second after each loop.
        rig.frame(&l, &n, &mut nodes, "d", 0.4);
        let early = rig.pose.get("b").unwrap().position[0];
        assert!(early > 0.0, "{early}");
        for _ in 0..3 {
            rig.frame(&l, &n, &mut nodes, "d", 0.4);
        }
        let held = rig.pose.get("b").unwrap().position[0];
        rig.frame(&l, &n, &mut nodes, "d", 0.4);
        assert_eq!(rig.pose.get("b").unwrap().position[0], held);
    }

    const TURN: &str = r#"{"format_version":"1.10.0","animations":{
      "animation.t.a":{"loop":true,"animation_length":1,"bones":{"b":{"rotation":[0,0,350]}}},
      "animation.t.z":{"loop":true,"animation_length":1,"bones":{"b":{"rotation":[0,0,10]}}}
    },"animation_controllers":{"controller.animation.t.c":{"initial_state":"a","states":{
      "a":{"animations":["a"],"transitions":[{"z":"q.speed > 1"}]},
      "z":{"animations":["z"],"blend_transition":1.0,"blend_via_shortest_path":true,"transitions":[]}}}}}"#;

    #[test]
    fn shortest_path_blend_turns_through_zero() {
        let l = lib(TURN);
        let n = names(&[
            ("c", "controller.animation.t.c"),
            ("a", "animation.t.a"),
            ("z", "animation.t.z"),
        ]);
        let mut nodes = Children::new();
        let mut rig = Rig::new();
        rig.frame(&l, &n, &mut nodes, "c", 0.1);
        rig.host.q.insert("speed".into(), 2.0);
        rig.frame(&l, &n, &mut nodes, "c", 0.5);
        let z = rig.pose.get("b").unwrap().rotation[2];
        assert!(z.abs() < 1e-3 || (z - 360.0).abs() < 1e-3, "{z}");
    }

    const CTRL: &str = r#"{"format_version":"1.10.0","animations":{
      "animation.t.idle":{"loop":true,"animation_length":1,"bones":{"b":{"position":[1,0,0]}}},
      "animation.t.run":{"loop":true,"animation_length":1,"bones":{"b":{"position":[0,10,0]}}},
      "animation.t.punch":{"animation_length":0.5,"bones":{"b":{"position":[0,0,7]}}}
    },"animation_controllers":{"controller.animation.t.c":{"initial_state":"idle","states":{
      "idle":{"animations":["idle"],"transitions":[{"run":"q.speed > 1"}]},
      "run":{"animations":["run"],"blend_transition":1.0,"transitions":[{"punch":"v.hit"},{"idle":"q.speed <= 1"}],"on_exit":["v.left = v.left + 1"]},
      "punch":{"animations":["punch"],"transitions":[{"idle":"q.all_animations_finished"}]}}}}}"#;

    fn ctrl_names() -> HashMap<String, String> {
        names(&[
            ("c", "controller.animation.t.c"),
            ("idle", "animation.t.idle"),
            ("run", "animation.t.run"),
            ("punch", "animation.t.punch"),
        ])
    }

    #[test]
    fn transition_conditions_of_a_state_share_temps() {
        let l = lib(r#"{"format_version":"1.10.0","animations":{
          "animation.t.a":{"loop":true,"animation_length":1,"bones":{"b":{"position":[1,0,0]}}},
          "animation.t.b":{"loop":true,"animation_length":1,"bones":{"b":{"position":[0,2,0]}}}
        },"animation_controllers":{"controller.animation.t.c":{"initial_state":"default","states":{
          "default":{"transitions":[
            {"a":"t.k=q.x?'a':'b';return t.k=='a'?!v.s:v.s=='a';"},
            {"b":"t.k=='b'?!v.s:v.s=='b'"}]},
          "a":{"animations":["a"]},
          "b":{"animations":["b"]}}}}}"#);
        let n = names(&[
            ("c", "controller.animation.t.c"),
            ("a", "animation.t.a"),
            ("b", "animation.t.b"),
        ]);
        let mut nodes = Children::new();
        let mut rig = Rig::new();
        // `q.x` is 0: the second condition reads the first one's `t.k`.
        rig.frame(&l, &n, &mut nodes, "c", 0.1);
        rig.frame(&l, &n, &mut nodes, "c", 0.1);
        assert_eq!(rig.pose.get("b").unwrap().position, [0.0, 2.0, 0.0]);
    }

    #[test]
    fn controller_switches_blends_and_finishes() {
        let l = lib(CTRL);
        let n = ctrl_names();
        let mut nodes = Children::new();
        let mut rig = Rig::new();
        rig.frame(&l, &n, &mut nodes, "c", 0.1);
        assert_eq!(rig.pose.get("b").unwrap().position, [1.0, 0.0, 0.0]);
        // Faster than 1: moves to run, blending over a second.
        rig.host.q.insert("speed".into(), 2.0);
        rig.frame(&l, &n, &mut nodes, "c", 0.5);
        let p = rig.pose.get("b").unwrap().position;
        assert!(
            (p[0] - 0.5).abs() < 1e-4 && (p[1] - 5.0).abs() < 1e-4,
            "{p:?}"
        );
        rig.frame(&l, &n, &mut nodes, "c", 0.6);
        assert_eq!(rig.pose.get("b").unwrap().position, [0.0, 10.0, 0.0]);
        // run -> punch on a variable; the exit script ran.
        rig.vars.insert(molang::intern("hit"), Value::Num(1.0));
        rig.frame(&l, &n, &mut nodes, "c", 0.1);
        assert_eq!(rig.vars[&molang::intern("left")], Value::Num(1.0));
        rig.vars.insert(molang::intern("hit"), Value::Num(0.0));
        rig.host.q.insert("speed".into(), 2.0);
        rig.frame(&l, &n, &mut nodes, "c", 0.2);
        assert_eq!(rig.pose.get("b").unwrap().position, [0.0, 0.0, 7.0]);
        // The one-shot ends and the controller returns to idle by itself.
        rig.host.q.insert("speed".into(), 0.0);
        rig.frame(&l, &n, &mut nodes, "c", 0.4);
        rig.frame(&l, &n, &mut nodes, "c", 0.4);
        rig.frame(&l, &n, &mut nodes, "c", 0.1);
        assert_eq!(rig.pose.get("b").unwrap().position, [1.0, 0.0, 0.0]);
    }

    #[test]
    fn a_controller_the_parent_stops_playing_starts_over_and_clears_its_flags() {
        let l = lib(
            r#"{"format_version":"1.10.0","animation_controllers":{
              "controller.animation.t.parent":{"initial_state":"default","states":{
                "default":{"animations":[{"child":"!q.stop"}]}}},
              "controller.animation.t.child":{"initial_state":"a","states":{
                "a":{"on_entry":["v.busy=0;"],"transitions":[{"b":"q.go"}]},
                "b":{"on_entry":["v.busy=1;"]}}}}}"#,
        );
        let n = names(&[
            ("parent", "controller.animation.t.parent"),
            ("child", "controller.animation.t.child"),
        ]);
        let mut nodes = Children::new();
        let mut rig = Rig::new();
        let busy = molang::intern("busy");
        rig.frame(&l, &n, &mut nodes, "parent", 0.1);
        rig.host.q.insert("go".into(), 1.0);
        rig.frame(&l, &n, &mut nodes, "parent", 0.1);
        assert_eq!(rig.vars[&busy], Value::Num(1.0));
        rig.host.q.insert("stop".into(), 1.0);
        rig.frame(&l, &n, &mut nodes, "parent", 0.1);
        assert_eq!(rig.vars[&busy], Value::Num(0.0));
    }

    #[test]
    fn unknown_names_do_nothing() {
        let l = lib(WALK);
        let mut nodes = Children::new();
        let mut rig = Rig::new();
        rig.frame(&l, &names(&[("x", "animation.nope")]), &mut nodes, "x", 0.1);
        rig.frame(&l, &names(&[]), &mut nodes, "y", 0.1);
        assert!(rig.pose.bones.is_empty());
    }

    #[test]
    fn a_looping_blink_ends_each_cycle_and_the_controller_moves_on() {
        let l = lib(
            r#"{"animations":{"animation.t.blink":{"loop":true,"animation_length":0.2,"bones":{"b":{"position":[1,0,0]}}}},
               "animation_controllers":{"controller.animation.t.c":{"initial_state":"wait","states":{
                 "wait":{"transitions":[{"blink":"q.state_time>0.5"}]},
                 "blink":{"animations":["blink"],"transitions":[{"wait":"q.any_animation_finished"}]}}}}}"#,
        );
        let n = names(&[
            ("c", "controller.animation.t.c"),
            ("blink", "animation.t.blink"),
        ]);
        let mut nodes = Children::new();
        let mut rig = Rig::new();
        let mut shown = Vec::new();
        for _ in 0..40 {
            rig.frame(&l, &n, &mut nodes, "c", 0.1);
            shown.push(rig.pose.get("b").is_some());
        }
        let on = shown.iter().filter(|b| **b).count();
        assert!(on > 0 && on < 40, "{shown:?}");
    }

    #[test]
    fn a_rotation_sequence_turns_like_its_euler_angles() {
        let one = Channel::parse(&crate::jsonc::parse(r#"[{"x": 90}]"#).unwrap()).unwrap();
        let mut vars = crate::molang::Vars::default();
        let mut rng = crate::molang::Rng::new(1);
        let mut host = crate::molang::NoHost;
        let mut env = Env {
            host: &mut host,
            vars: &mut vars,
            rng: &mut rng,
        };
        let r = one.sample(0.0, &mut env, Scope::default(), [0.0; 3]);
        assert!((r[0] - 90.0).abs() < 1e-3 && r[1].abs() < 1e-3 && r[2].abs() < 1e-3, "{r:?}");
        // Turns about one axis add up.
        let two = Channel::parse(&crate::jsonc::parse(r#"[{"z": 30}, {"z": 15}]"#).unwrap()).unwrap();
        let r = two.sample(0.0, &mut env, Scope::default(), [0.0; 3]);
        assert!((r[2] - 45.0).abs() < 1e-3, "{r:?}");
    }
}
