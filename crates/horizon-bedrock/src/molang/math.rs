//! `math.*`. Angles are degrees, as in Molang.

use std::f32::consts::PI;

/// A small deterministic generator for `math.random` and the dice. Entities that must look the
/// same on every client seed it from something they share.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    fn next_u64(&mut self) -> u64 {
        // xorshift64*
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// In `[0, 1)`.
    pub fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EaseKind {
    Sine,
    /// Power curves: 2 quad, 3 cubic, 4 quart, 5 quint.
    Pow(u8),
    Expo,
    Circ,
    Back,
    Elastic,
    Bounce,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EaseMode {
    In,
    Out,
    InOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathFn {
    Abs,
    Acos,
    Asin,
    Atan,
    Atan2,
    Ceil,
    Clamp,
    CopySign,
    Cos,
    DieRoll,
    DieRollInteger,
    Exp,
    Floor,
    HermiteBlend,
    InverseLerp,
    Lerp,
    LerpRotate,
    Ln,
    Max,
    Min,
    MinAngle,
    Mod,
    Pi,
    Pow,
    Random,
    RandomInteger,
    Round,
    Sign,
    Sin,
    Sqrt,
    Trunc,
    Ease(EaseKind, EaseMode),
}

impl MathFn {
    /// The function called `name` (lower case, without `math.`).
    pub fn from_name(name: &str) -> Option<MathFn> {
        use MathFn::*;
        Some(match name {
            "abs" => Abs,
            "acos" => Acos,
            "asin" => Asin,
            "atan" => Atan,
            "atan2" => Atan2,
            "ceil" => Ceil,
            "clamp" => Clamp,
            "copy_sign" => CopySign,
            "cos" => Cos,
            "die_roll" => DieRoll,
            "die_roll_integer" => DieRollInteger,
            "exp" => Exp,
            "floor" => Floor,
            "hermite_blend" => HermiteBlend,
            "inverse_lerp" => InverseLerp,
            "lerp" => Lerp,
            "lerprotate" => LerpRotate,
            "ln" => Ln,
            "max" => Max,
            "min" => Min,
            "min_angle" => MinAngle,
            "mod" => Mod,
            "pi" => Pi,
            "pow" => Pow,
            "random" => Random,
            "random_integer" => RandomInteger,
            "round" => Round,
            "sign" => Sign,
            "sin" => Sin,
            "sqrt" => Sqrt,
            "trunc" => Trunc,
            _ => return Self::ease_from_name(name),
        })
    }

    fn ease_from_name(name: &str) -> Option<MathFn> {
        let rest = name.strip_prefix("ease_")?;
        let (mode, kind) = if let Some(k) = rest.strip_prefix("in_out_") {
            (EaseMode::InOut, k)
        } else if let Some(k) = rest.strip_prefix("in_") {
            (EaseMode::In, k)
        } else {
            (EaseMode::Out, rest.strip_prefix("out_")?)
        };
        let kind = match kind {
            "sine" => EaseKind::Sine,
            "quad" => EaseKind::Pow(2),
            "cubic" => EaseKind::Pow(3),
            "quart" => EaseKind::Pow(4),
            "quint" => EaseKind::Pow(5),
            "expo" => EaseKind::Expo,
            "circ" => EaseKind::Circ,
            "back" => EaseKind::Back,
            "elastic" => EaseKind::Elastic,
            "bounce" => EaseKind::Bounce,
            _ => return None,
        };
        Some(MathFn::Ease(kind, mode))
    }

    /// Calls the function; missing arguments are 0, and a result that is not a number (the
    /// arcsine of 2, the root of -1) is 0 like a division by zero: a NaN stored in a variable
    /// would otherwise stay there and unpose every bone that reads it.
    pub fn call(self, args: &[f32], rng: &mut Rng) -> f32 {
        let r = self.call_raw(args, rng);
        if r.is_nan() { 0.0 } else { r }
    }

    fn call_raw(self, args: &[f32], rng: &mut Rng) -> f32 {
        use MathFn::*;
        let a = |i: usize| args.get(i).copied().unwrap_or(0.0);
        let rad = PI / 180.0;
        match self {
            Abs => a(0).abs(),
            Acos => a(0).acos() / rad,
            Asin => a(0).asin() / rad,
            Atan => a(0).atan() / rad,
            Atan2 => a(0).atan2(a(1)) / rad,
            Ceil => a(0).ceil(),
            Clamp => {
                let (lo, hi) = (a(1), a(2));
                if lo > hi { lo } else { a(0).clamp(lo, hi) }
            }
            CopySign => a(0).abs().copysign(a(1)),
            Cos => (a(0) * rad).cos(),
            DieRoll => (0..a(0).max(0.0) as u32)
                .map(|_| a(1) + rng.unit() * (a(2) - a(1)))
                .sum(),
            DieRollInteger => (0..a(0).max(0.0) as u32)
                .map(|_| random_integer(rng, a(1), a(2)))
                .sum(),
            Exp => a(0).exp(),
            Floor => a(0).floor(),
            HermiteBlend => {
                let t = a(0);
                3.0 * t * t - 2.0 * t * t * t
            }
            InverseLerp => {
                if a(1) == a(0) {
                    0.0
                } else {
                    (a(2) - a(0)) / (a(1) - a(0))
                }
            }
            Lerp => a(0) + (a(1) - a(0)) * a(2),
            LerpRotate => {
                let mut d = (a(1) - a(0)) % 360.0;
                if d > 180.0 {
                    d -= 360.0;
                } else if d < -180.0 {
                    d += 360.0;
                }
                a(0) + d * a(2)
            }
            Ln => a(0).ln(),
            Max => a(0).max(a(1)),
            Min => a(0).min(a(1)),
            MinAngle => {
                let mut x = a(0) % 360.0;
                if x >= 180.0 {
                    x -= 360.0;
                } else if x < -180.0 {
                    x += 360.0;
                }
                x
            }
            Mod => {
                if a(1) == 0.0 {
                    0.0
                } else {
                    a(0) % a(1)
                }
            }
            Pi => PI,
            Pow => a(0).powf(a(1)),
            Random => a(0) + rng.unit() * (a(1) - a(0)),
            RandomInteger => random_integer(rng, a(0), a(1)),
            Round => a(0).round(),
            Sign => {
                let x = a(0);
                if x > 0.0 {
                    1.0
                } else if x < 0.0 {
                    -1.0
                } else {
                    0.0
                }
            }
            Sin => (a(0) * rad).sin(),
            Sqrt => a(0).sqrt(),
            Trunc => a(0).trunc(),
            Ease(kind, mode) => {
                let t = ease(kind, mode, a(2));
                a(0) + (a(1) - a(0)) * t
            }
        }
        .pipe_finite()
    }
}

trait PipeFinite {
    fn pipe_finite(self) -> f32;
}
impl PipeFinite for f32 {
    /// Molang turns errors into 0: NaN and infinities too.
    fn pipe_finite(self) -> f32 {
        if self.is_finite() { self } else { 0.0 }
    }
}

fn random_integer(rng: &mut Rng, lo: f32, hi: f32) -> f32 {
    let (lo, hi) = (lo.floor(), hi.floor());
    let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
    (lo + (rng.unit() * (hi - lo + 1.0)).floor()).min(hi)
}

fn bounce_out(t: f32) -> f32 {
    let (n1, d1) = (7.5625, 2.75);
    if t < 1.0 / d1 {
        n1 * t * t
    } else if t < 2.0 / d1 {
        let t = t - 1.5 / d1;
        n1 * t * t + 0.75
    } else if t < 2.5 / d1 {
        let t = t - 2.25 / d1;
        n1 * t * t + 0.9375
    } else {
        let t = t - 2.625 / d1;
        n1 * t * t + 0.984375
    }
}

/// The easing curve of `kind` and `mode` at `t` in `[0, 1]` (outside, clamped).
pub fn ease(kind: EaseKind, mode: EaseMode, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t == 0.0 || t == 1.0 {
        return t;
    }
    let c1 = 1.70158f32;
    let c2 = c1 * 1.525;
    let c3 = c1 + 1.0;
    let c4 = 2.0 * PI / 3.0;
    let c5 = 2.0 * PI / 4.5;
    use EaseMode::*;
    match (kind, mode) {
        (EaseKind::Sine, In) => 1.0 - (t * PI / 2.0).cos(),
        (EaseKind::Sine, Out) => (t * PI / 2.0).sin(),
        (EaseKind::Sine, InOut) => -((PI * t).cos() - 1.0) / 2.0,
        (EaseKind::Pow(n), In) => t.powi(n as i32),
        (EaseKind::Pow(n), Out) => 1.0 - (1.0 - t).powi(n as i32),
        (EaseKind::Pow(n), InOut) => {
            if t < 0.5 {
                2f32.powi(n as i32 - 1) * t.powi(n as i32)
            } else {
                1.0 - (-2.0 * t + 2.0).powi(n as i32) / 2.0
            }
        }
        (EaseKind::Expo, In) => 2f32.powf(10.0 * t - 10.0),
        (EaseKind::Expo, Out) => 1.0 - 2f32.powf(-10.0 * t),
        (EaseKind::Expo, InOut) => {
            if t < 0.5 {
                2f32.powf(20.0 * t - 10.0) / 2.0
            } else {
                (2.0 - 2f32.powf(-20.0 * t + 10.0)) / 2.0
            }
        }
        (EaseKind::Circ, In) => 1.0 - (1.0 - t * t).sqrt(),
        (EaseKind::Circ, Out) => (1.0 - (t - 1.0) * (t - 1.0)).sqrt(),
        (EaseKind::Circ, InOut) => {
            if t < 0.5 {
                (1.0 - (1.0 - (2.0 * t).powi(2)).sqrt()) / 2.0
            } else {
                ((1.0 - (-2.0 * t + 2.0).powi(2)).sqrt() + 1.0) / 2.0
            }
        }
        (EaseKind::Back, In) => c3 * t * t * t - c1 * t * t,
        (EaseKind::Back, Out) => 1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2),
        (EaseKind::Back, InOut) => {
            if t < 0.5 {
                ((2.0 * t).powi(2) * ((c2 + 1.0) * 2.0 * t - c2)) / 2.0
            } else {
                ((2.0 * t - 2.0).powi(2) * ((c2 + 1.0) * (t * 2.0 - 2.0) + c2) + 2.0) / 2.0
            }
        }
        (EaseKind::Elastic, In) => -(2f32.powf(10.0 * t - 10.0)) * ((10.0 * t - 10.75) * c4).sin(),
        (EaseKind::Elastic, Out) => 2f32.powf(-10.0 * t) * ((10.0 * t - 0.75) * c4).sin() + 1.0,
        (EaseKind::Elastic, InOut) => {
            if t < 0.5 {
                -(2f32.powf(20.0 * t - 10.0) * ((20.0 * t - 11.125) * c5).sin()) / 2.0
            } else {
                (2f32.powf(-20.0 * t + 10.0) * ((20.0 * t - 11.125) * c5).sin()) / 2.0 + 1.0
            }
        }
        (EaseKind::Bounce, Out) => bounce_out(t),
        (EaseKind::Bounce, In) => 1.0 - bounce_out(1.0 - t),
        (EaseKind::Bounce, InOut) => {
            if t < 0.5 {
                (1.0 - bounce_out(1.0 - 2.0 * t)) / 2.0
            } else {
                (1.0 + bounce_out(2.0 * t - 1.0)) / 2.0
            }
        }
    }
}
