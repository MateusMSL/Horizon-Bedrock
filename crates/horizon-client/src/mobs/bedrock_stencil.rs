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

/// Whether pieces go to the stencil pass now.
pub fn on() -> bool {
    pass::active() && !super::cem_anim::is_in_gui() && !super::bedrock_debug::settings().cpu_stencil
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

/// The piece just drawn into `out[start..]` with material `f`: handed to the pass as its route
/// says, and taken out of the scene's vertices when the pass alone draws it.
pub fn hand_over(f: &MaterialFlags, out: &mut Vec<Vertex>, start: usize) {
    let route = route(f);
    if f.stencil.matters() && super::bedrock_debug::settings().dump_draw {
        let id = format!("{:?}/{:?}/{}", route, f.stencil, super::render::in_first_person_view());
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
        Route::SceneAndPass => pass::push(target, state(f, false), &out[start..]),
        Route::Pass => {
            pass::push(target, state(f, !f.no_color), &out[start..]);
            out.truncate(start);
        }
    }
}
