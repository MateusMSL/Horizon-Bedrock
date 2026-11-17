//! The pieces of a Bedrock model whose material uses the stencil (`EnableStencilTest`), handed to
//! the renderer's stencil pass (`gfx::bedrock_stencil`), which draws them with a real stencil
//! buffer as Bedrock does: a clock's face only inside its window, an armour's shine only inside
//! its band, an outline only past the body.
//!
//! The mob renderer (`render_03.rs`) asks two things of each layer it draws:
//! - `emulated`: the flags its CPU stand-ins read (masks dropped, pieces cut to a mask's shape on
//!   the screen and pushed behind, an outline's hull turned inside out). With the pass on, they
//!   see no stencil and the piece draws whole;
//! - `hand_over`, once the piece is drawn into the scene's vertices: a piece that only tests the
//!   stencil, or a mask without colour, moves to the pass; a piece that writes it with colour
//!   stays in the scene and also goes to the pass without colour, for the value it writes.
//!
//! The pass is off in the inventory's doll (drawn to the screen, not among the world's entities)
//! and under a shader pack; the stand-ins draw then.

use crate::gfx::bedrock_stencil as pass;
use crate::mesher::Vertex;
use horizon_bedrock::material::{MaterialFlags, StencilFunc};

/// What becomes of a piece.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// Drawn in the scene only (no stencil, or the pass is off).
    Scene,
    /// Drawn in the scene, and in the pass without colour for the stencil it writes.
    SceneAndPass,
    /// Drawn in the pass only.
    Pass,
}

#[cfg(test)]
thread_local! {
    /// A test's stand-in for the renderer's pass being on, on its own thread.
    static TEST_ON: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether pieces go to the stencil pass now.
pub fn on() -> bool {
    #[cfg(test)]
    let active = pass::active() || TEST_ON.get();
    #[cfg(not(test))]
    let active = pass::active();
    active && !super::cem_anim::is_in_gui() && !super::bedrock_debug::settings().cpu_stencil
}

/// Routes pieces as if the renderer's pass were on (`on`), on this thread only.
#[cfg(test)]
pub fn set_on_for_test(on: bool) {
    TEST_ON.set(on);
}

pub fn route(f: &MaterialFlags) -> Route {
    if !f.stencil.matters() || !on() {
        Route::Scene
    } else if f.no_color || f.stencil.func != StencilFunc::Always {
        Route::Pass
    } else {
        Route::SceneAndPass
    }
}

/// The flags the CPU stand-ins read: with the pass drawing the stencil, none of it.
pub fn emulated(mut f: MaterialFlags) -> MaterialFlags {
    if route(&f) != Route::Scene {
        f.stencil_write = 0;
        f.stencil_not_equal = 0;
        f.stencil_equal = 0;
        f.stencil_mask = 0;
        f.no_color = false;
    }
    f
}

fn state(f: &MaterialFlags, color: bool) -> pass::State {
    use pass::Compare as C;
    let s = f.stencil;
    pass::State {
        color,
        additive: f.additive,
        blended: f.blend || f.additive,
        // As the scene draws them: an additive or `DisableDepthWrite` piece hides nothing.
        depth_write: !f.no_depth_write
            && !f.additive
            && !(f.no_color && super::bedrock_debug::settings().stencil_masks_no_depth),
        depth_test: !f.no_depth_test
            && !(f.stencil.func != StencilFunc::Always
                && super::bedrock_debug::settings().stencil_tests_no_depth),
        // Bedrock drops clear texels only under `ALPHA_TEST`; a blended or added piece is
        // clear there anyway.
        cutout: f.alpha_test || f.blend || f.additive || color,
        compare: match s.func {
            StencilFunc::Always => C::Always,
            StencilFunc::Never => C::Never,
            StencilFunc::Less => C::Less,
            StencilFunc::LessEqual => C::LessEqual,
            StencilFunc::Equal => C::Equal,
            StencilFunc::NotEqual => C::NotEqual,
            StencilFunc::Greater => C::Greater,
            StencilFunc::GreaterEqual => C::GreaterEqual,
        },
        replace: s.replace,
        reference: s.reference,
        read_mask: s.read_mask,
        write_mask: s.write_mask,
    }
}

/// A piece handed to the pass: where, how, and its vertices as they went.
pub type Handed = (pass::Target, pass::State, Vec<Vertex>);

thread_local! {
    /// The pieces handed to the pass while `record` runs.
    static RECORDING: std::cell::RefCell<Option<Vec<Handed>>> =
        const { std::cell::RefCell::new(None) };
}

/// Runs `f` and gives back, beside its result, the pieces it handed to the pass: what
/// `render_bedrock` keeps with a mob's vertices to hand over again (`replay`) when it draws
/// them again. A `record` running around this one keeps them too: they are part of its drawing.
pub fn record<R>(f: impl FnOnce() -> R) -> (R, Vec<Handed>) {
    let outer = RECORDING.with(|r| r.borrow_mut().replace(Vec::new()));
    let result = f();
    let handed = RECORDING
        .with(|r| std::mem::replace(&mut *r.borrow_mut(), outer))
        .unwrap_or_default();
    RECORDING.with(|r| {
        if let Some(outer) = r.borrow_mut().as_mut() {
            outer.extend(handed.iter().cloned());
        }
    });
    (result, handed)
}

/// A piece posed for the pass (`hand_over_posed`): where, how, and its vertices' place in the
/// stencil jobs' output and their count.
pub type PosedHanded = (pass::Target, pass::State, u32, u32);

thread_local! {
    /// The posed pieces handed to the pass while `record_posed` runs.
    static POSED: std::cell::RefCell<Option<Vec<PosedHanded>>> =
        const { std::cell::RefCell::new(None) };
}

/// [`record`] for the pieces posed on the GPU: `render_bedrock` keeps them with a mob's posed
/// draws and hands them over again (`replay_posed`) when it draws the mob from them.
pub fn record_posed<R>(f: impl FnOnce() -> R) -> (R, Vec<PosedHanded>) {
    let outer = POSED.with(|r| r.borrow_mut().replace(Vec::new()));
    let result = f();
    let handed = POSED
        .with(|r| std::mem::replace(&mut *r.borrow_mut(), outer))
        .unwrap_or_default();
    POSED.with(|r| {
        if let Some(outer) = r.borrow_mut().as_mut() {
            outer.extend(handed.iter().copied());
        }
    });
    (result, handed)
}

/// Hands `pieces` to the pass again, their vertices now from `at` in the stencil jobs' output
/// where they were from `was`.
pub fn replay_posed(pieces: &[PosedHanded], was: u32, at: u32) {
    for (target, state, first, count) in pieces {
        push_posed(*target, *state, at + (first - was), *count);
    }
}

fn push_posed(target: pass::Target, state: pass::State, first: u32, count: u32) {
    POSED.with(|r| {
        if let Some(pieces) = r.borrow_mut().as_mut() {
            pieces.push((target, state, first, count));
        }
    });
    pass::push_posed(target, state, first, count);
}

/// Hands `pieces` to the pass again, each vertex through `map` (moved to where the mob is now).
pub fn replay(pieces: &[Handed], map: impl Fn(&Vertex) -> Vertex) {
    for (target, state, vertices) in pieces {
        let moved: Vec<Vertex> = vertices.iter().map(&map).collect();
        push(*target, *state, &moved);
    }
}

#[cfg(test)]
thread_local! {
    static PUSHED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Tests: the vertices handed to the pass from the CPU so far.
#[cfg(test)]
pub fn pushed_for_test() -> usize {
    PUSHED.with(|p| p.get())
}

/// `pass::push`, kept for `record` when it runs.
fn push(target: pass::Target, state: pass::State, vertices: &[Vertex]) {
    #[cfg(test)]
    PUSHED.with(|p| p.set(p.get() + vertices.len()));
    RECORDING.with(|r| {
        if let Some(pieces) = r.borrow_mut().as_mut() {
            pieces.push((target, state, vertices.to_vec()));
        }
    });
    pass::push(target, state, vertices);
}

/// [`hand_over`] for a piece posed on the GPU: the jobs `skin` pushed since job `first` (the
/// scene's posed output stood at `vertices` before them) go to the pass as the route says, copied
/// or moved to its posed output. The world's only: the first-person view draws on the CPU.
pub fn hand_over_posed(
    f: &MaterialFlags,
    skin: &mut super::skin::Skin,
    first: usize,
    vertices: u32,
) {
    match route(f) {
        Route::Scene => {}
        Route::SceneAndPass => {
            let (at, count) = skin.stencil_copy_since(first);
            push_posed(pass::Target::World, state(f, false), at, count);
        }
        Route::Pass => {
            let (at, count) = skin.stencil_move_since(first, vertices);
            push_posed(pass::Target::World, state(f, !f.no_color), at, count);
        }
    }
}

/// The piece just drawn into `out[start..]` with material `f`: handed to the pass as its route
/// says, and taken out of the scene's vertices when the pass alone draws it.
pub fn hand_over(f: &MaterialFlags, out: &mut Vec<Vertex>, start: usize) {
    let route = route(f);
    if f.stencil.matters() && super::bedrock_debug::settings().dump_draw {
        let id = format!(
            "{:?}/{:?}/{}",
            route,
            f.stencil,
            super::render::in_first_person_view()
        );
        super::render::log_bedrock_once(format!("stencil-route/{id}"), || {
            format!(
                "bedrock-stencil: piece with {:?} goes {:?} (pass on {}, first person {}, {} vertices)",
                f.stencil,
                route,
                on(),
                super::render::in_first_person_view(),
                out.len().saturating_sub(start)
            )
        });
    }
    if route == Route::Scene || out.len() <= start {
        return;
    }
    let target = if super::render::in_first_person_view() {
        pass::Target::Hand
    } else {
        pass::Target::World
    };
    match route {
        Route::Scene => {}
        Route::SceneAndPass => {
            let mut s = state(f, false);
            // The first-person hand: the scene already drew this piece, and its copy here does
            // not land on the hand's stored depth exactly (the enchanted 2D sword's outline
            // covered the whole blade: its body never wrote the reference it is kept off). The
            // value is written over the piece whole; nothing is nearer than the hand.
            if target == pass::Target::Hand {
                s.depth_test = false;
            }
            push(target, s, &out[start..])
        }
        Route::Pass => {
            push(target, state(f, !f.no_color), &out[start..]);
            out.truncate(start);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piece(reference: u8) -> (pass::State, Vec<Vertex>) {
        let state = pass::State {
            color: false,
            additive: false,
            blended: false,
            depth_write: true,
            depth_test: true,
            cutout: true,
            compare: pass::Compare::Always,
            replace: true,
            reference,
            read_mask: 255,
            write_mask: 255,
        };
        let v = Vertex {
            pos: [f32::from(reference); 3],
            uv: [0.0; 2],
            layer: 0,
            color: 0,
            light: 0,
            normal: [0; 4],
            glint_uv: [0.0; 2],
        };
        (state, vec![v; 4])
    }

    /// What a mob's drawing hands to the pass is kept for it, a drawing inside another kept for
    /// both (it is part of the outer one), and nothing is kept outside `record`.
    #[test]
    fn a_drawing_keeps_the_pieces_it_handed_over() {
        let (a, va) = piece(1);
        let (b, vb) = piece(2);
        push(pass::Target::World, a, &va);
        let ((), outer) = record(|| {
            push(pass::Target::World, a, &va);
            let ((), inner) = record(|| push(pass::Target::World, b, &vb));
            assert_eq!(inner.len(), 1);
            assert_eq!(inner[0].1.reference, 2);
        });
        let references: Vec<u8> = outer.iter().map(|p| p.1.reference).collect();
        assert_eq!(
            references,
            [1, 2],
            "the inner drawing's piece is the outer one's too"
        );
        assert_eq!(outer[0].2.len(), 4);
        assert_eq!(outer[0].2[0].pos, [1.0; 3]);
        let ((), none) = record(|| ());
        assert!(none.is_empty());
    }
}
