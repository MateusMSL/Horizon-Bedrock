//! Mob models posed on the GPU (`gfx::entity_skin`): the store of models
//! uploaded once, and the walk that turns one `render::draw` into the parts'
//! matrices and jobs the shader expands into the very vertices `draw` would
//! push, in its order.
//!
//! A model takes this path only when every polygon faces an axis: the
//! normal matrix times an axis is one of its columns, exactly, so the shader's
//! packed normals are the CPU's to the bit. Positions and UVs are the same
//! sums in the same order (`Mat4::transform_point3`).

use super::{ModelNormals, Part, anim::Poses};
use crate::fast_hash::FxHashMap;
use crate::gfx::entity_skin::{SkinFrame, SkinJob, SkinPart, SkinVertex};
use horizon_assets::item_bake::BakedItemModel;
use std::sync::Arc;

/// A stored model: where its vertices start, and per part (pre-order) its
/// own vertices and the end of its subtree, for the walk.
struct Stored {
    first: u32,
    parts: Vec<StoredPart>,
    /// The model's bounds in its parts' own space, per part: min and max of
    /// its own vertices (none: no polygons).
    bounds: Vec<Option<([f32; 3], [f32; 3])>>,
    /// The model's render type culls back faces (the bat, the baby turtle).
    cull: bool,
}

#[derive(Clone, Copy)]
struct StoredPart {
    /// The part's first vertex, from the model's.
    first: u32,
    count: u32,
    /// One past the last part of its subtree.
    end: u32,
}

/// The models on the GPU, by the `Arc` they are drawn from. An entry holds
/// its `Arc`, so the address is never reused while stored; models no one
/// else holds any more are dropped by `begin`.
#[derive(Default)]
pub struct SkinModels {
    stored: FxHashMap<usize, (Arc<Part>, Option<Stored>)>,
    rotations: Rotations,
    rotation_cache_disabled: bool,
    /// Item models, the same way (`item_render::ItemRenderState::submit_skinned`).
    items: FxHashMap<usize, (Arc<BakedItemModel>, Option<StoredItem>)>,
    vertices: u32,
    /// Whether the layers improved transparency would draw in its stages are
    /// posed here too (`gfx::Renderer::skins_tagged`; set each frame): the eyes
    /// render type's (spiders', endermen's...) as [`SkinJob::FLAT`] draws, and
    /// `entityTranslucent` layers (a horse's markings, a slime's outer cube).
    pub tagged: bool,
    /// Restarts of the store (`epoch`).
    epoch: u64,
}

/// A stored item model: its quads' vertices in order, by runs of quads that
/// share a sprite and a tint, and its bounds.
pub struct StoredItem {
    pub runs: Vec<ItemRun>,
    pub bounds: (glam::Vec3, glam::Vec3),
}

/// Consecutive quads of an item model with one sprite and tint index.
pub struct ItemRun {
    /// The run's first vertex in the store, and its vertex count.
    pub first: u32,
    pub count: u32,
    /// Its first quad in the model.
    pub quad: usize,
    pub tint_index: i32,
}

/// Models whose store would grow past this restart it (a pack reload's
/// models pile up otherwise).
const STORE_LIMIT: u32 = 4 << 20;

/// Local rotations repeat across bones, entities and frames. Exact input bits
/// keep signed zero and animation precision; collisions only replace an entry.
struct Rotations([Option<Rotation>; 64]);

#[derive(Clone, Copy)]
struct Rotation {
    key: [u32; 3],
    matrix: glam::Mat4,
    normal: glam::Mat3,
}

impl Default for Rotations {
    fn default() -> Self {
        Self([None; 64])
    }
}

impl Rotations {
    #[inline]
    fn get(&mut self, p: [f32; 9]) -> &Rotation {
        use std::hash::Hasher;
        let key = [p[3].to_bits(), p[4].to_bits(), p[5].to_bits()];
        // Positive zero produces these exact matrices on both original paths.
        // Keep it outside the table so a stream of animated misses cannot evict it.
        if key == [0; 3] {
            return &Rotation {
                key: [0; 3],
                matrix: glam::Mat4::IDENTITY,
                normal: glam::Mat3::IDENTITY,
            };
        }
        let mut hash = crate::fast_hash::FxHasher::default();
        // Hash each angle separately: packing two into a u64 makes opposite
        // signs collide in common mirrored limb poses.
        for angle in key {
            hash.write_u32(angle);
        }
        let index = hash.finish() as usize & (self.0.len() - 1);
        let slot = &mut self.0[index];
        if slot.as_ref().is_none_or(|rotation| rotation.key != key) {
            *slot = Some(Rotation {
                key,
                matrix: glam::Mat4::from_quat(glam::Quat::from_euler(
                    glam::EulerRot::ZYX,
                    p[5],
                    p[4],
                    p[3],
                )),
                normal: ModelNormals::rotation(p[3], p[4], p[5]),
            });
        }
        slot.as_ref().unwrap()
    }
}

impl SkinModels {
    /// Select the original per-part rotation arithmetic when reuse is disabled.
    /// The caller can change this between frames without rebuilding the model store.
    pub fn set_rotation_cache_enabled(&mut self, enabled: bool) {
        self.rotation_cache_disabled = !enabled;
    }

    /// Starts a frame: drops the models nothing else holds (their vertices
    /// stay in the store until it restarts), restarts the store when it grew
    /// too large or the renderer's (`stored`, its size) is out of step.
    /// Counts the store's restarts: a job kept from an earlier frame names its model's vertices
    /// by where they were stored, which only holds within one epoch.
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn begin(&mut self, frame: &mut SkinFrame, stored: u32) {
        if self.vertices > STORE_LIMIT || stored + frame.new_model.len() as u32 != self.vertices {
            self.restart(frame);
        }
        self.stored
            .retain(|_, (model, _)| Arc::strong_count(model) > 1);
        self.items
            .retain(|_, (model, _)| Arc::strong_count(model) > 1);
        frame.clear_draws();
    }

    pub fn restart(&mut self, frame: &mut SkinFrame) {
        self.epoch += 1;
        self.stored.clear();
        self.items.clear();
        self.vertices = 0;
        frame.new_model.clear();
        frame.base = 0;
        frame.reset = true;
    }

    fn entry(
        &mut self,
        model: &Arc<Part>,
        frame: &mut SkinFrame,
    ) -> Option<(&Stored, &mut Rotations)> {
        let key = Arc::as_ptr(model) as usize;
        let vertices = &mut self.vertices;
        let (_, stored) = self.stored.entry(key).or_insert_with(|| {
            // The renderer appends `new_model` to its store in order, so a
            // model's first vertex is the store's size when it is added.
            let before = frame.new_model.len();
            let stored = flatten(model, *vertices, &mut frame.new_model);
            *vertices += (frame.new_model.len() - before) as u32;
            (model.clone(), stored)
        });
        Some((stored.as_ref()?, &mut self.rotations))
    }

    /// An item model's runs, stored on first sight; `None` when it has no
    /// quads, or one that glows (`light_emission`) or is forced translucent.
    pub fn item(
        &mut self,
        model: &Arc<BakedItemModel>,
        frame: &mut SkinFrame,
    ) -> Option<&StoredItem> {
        let key = Arc::as_ptr(model) as usize;
        let vertices = &mut self.vertices;
        let (_, stored) = self.items.entry(key).or_insert_with(|| {
            let before = frame.new_model.len();
            let stored = flatten_item(model, *vertices, &mut frame.new_model);
            *vertices += (frame.new_model.len() - before) as u32;
            (model.clone(), stored)
        });
        stored.as_ref()
    }
}

/// The model's quads as `item_render::submit_quads` reads them, appended to
/// `out` (part 0, the layer's pose); `None` when a quad glows or is forced
/// translucent (another material).
fn flatten_item(
    model: &BakedItemModel,
    first: u32,
    out: &mut Vec<SkinVertex>,
) -> Option<StoredItem> {
    use horizon_assets::model::Direction as D;
    let other =
        |q: &horizon_assets::bake::ModelQuad| q.light_emission != 0 || q.material.force_translucent;
    if model.quads.is_empty() || model.quads.iter().any(other) {
        return None;
    }
    let mut runs: Vec<ItemRun> = Vec::new();
    let mut lo = glam::Vec3::INFINITY;
    let mut hi = glam::Vec3::NEG_INFINITY;
    for (i, q) in model.quads.iter().enumerate() {
        let vertex = first + 4 * i as u32;
        match runs.last_mut() {
            Some(run)
                if model.quads[run.quad].material.sprite == q.material.sprite
                    && run.tint_index == q.tint_index =>
            {
                run.count += 4;
            }
            _ => runs.push(ItemRun {
                first: vertex,
                count: 4,
                quad: i,
                tint_index: q.tint_index,
            }),
        }
        let axis = match q.facing {
            D::East => 0,
            D::Up => 1,
            D::South => 2,
            D::West => 3,
            D::Down => 4,
            D::North => 5,
        };
        for (pos, uv) in q.positions.iter().zip(q.uv) {
            lo = lo.min(glam::Vec3::from_array(*pos));
            hi = hi.max(glam::Vec3::from_array(*pos));
            out.push(SkinVertex {
                pos: *pos,
                uv,
                axis,
                part: 0,
            });
        }
    }
    Some(StoredItem {
        runs,
        bounds: (lo, hi),
    })
}

/// The model's vertices in pre-order (a part's polygons, then its children),
/// appended to `out`; `None` when a polygon does not face an axis.
fn flatten(model: &Part, first: u32, out: &mut Vec<SkinVertex>) -> Option<Stored> {
    let start = out.len();
    let mut parts = Vec::new();
    let mut bounds = Vec::new();
    fn walk(
        part: &Part,
        start: usize,
        out: &mut Vec<SkinVertex>,
        parts: &mut Vec<StoredPart>,
        bounds: &mut Vec<Option<([f32; 3], [f32; 3])>>,
    ) -> bool {
        let index = parts.len();
        let own = (out.len() - start) as u32;
        parts.push(StoredPart {
            first: own,
            count: 0,
            end: 0,
        });
        bounds.push(None);
        let mut lo = [f32::INFINITY; 3];
        let mut hi = [f32::NEG_INFINITY; 3];
        for poly in &part.polygons {
            let Some(axis) = axis(poly.normal) else {
                return false;
            };
            for v in poly.vertices {
                let pos = (glam::Vec3::new(v[0], v[1], v[2]) / 16.0).to_array();
                for a in 0..3 {
                    lo[a] = lo[a].min(pos[a]);
                    hi[a] = hi[a].max(pos[a]);
                }
                out.push(SkinVertex {
                    pos,
                    uv: [v[3], v[4]],
                    axis,
                    part: index as u32,
                });
            }
        }
        parts[index].count = (out.len() - start) as u32 - own;
        if !part.polygons.is_empty() {
            bounds[index] = Some((lo, hi));
        }
        for child in &part.children {
            if !walk(child, start, out, parts, bounds) {
                return false;
            }
        }
        parts[index].end = parts.len() as u32;
        true
    }
    if !walk(model, start, out, &mut parts, &mut bounds) {
        out.truncate(start);
        return None;
    }
    Some(Stored {
        first,
        parts,
        bounds,
        cull: has_cull(model),
    })
}

/// Whether the model's polygons cull back faces (all or none, set per model).
fn has_cull(part: &Part) -> bool {
    part.polygons.iter().any(|p| p.cull_back) || part.children.iter().any(has_cull)
}

/// The part's normal matrix as the shader reads it (an axis normal is one of its columns): the
/// columns normalized when the scale is not uniform, as `Polygon::vertex_normal` normalizes the
/// product. A part flattened on one axis (a Bedrock pack scales a bone to 0 that way) has an
/// infinite column, and the CPU's product with any axis meets `0 × ∞`: a NaN it normalizes to
/// no normal at all, which every column here takes too.
fn normal_matrix(normals: ModelNormals) -> glam::Mat3 {
    if normals.trusted {
        normals.matrix
    } else if !normals.matrix.is_finite() {
        glam::Mat3::ZERO
    } else {
        glam::Mat3::from_cols(
            normals.matrix.x_axis.normalize_or_zero(),
            normals.matrix.y_axis.normalize_or_zero(),
            normals.matrix.z_axis.normalize_or_zero(),
        )
    }
}

/// 0..2 for +x, +y, +z, 3..5 for -x, -y, -z; `None` for any other normal.
fn axis(n: [f32; 3]) -> Option<u32> {
    let mut found = None;
    for (a, &c) in n.iter().enumerate() {
        if c == 1.0 || c == -1.0 {
            if found.is_some() {
                return None;
            }
            found = Some(a as u32 + if c < 0.0 { 3 } else { 0 });
        } else if c != 0.0 {
            return None;
        }
    }
    found
}

/// One draw's parameters (`render::draw_scrolled`'s).
#[derive(Clone, Copy)]
pub struct Paint {
    pub uv: [f32; 4],
    pub scroll: f32,
    pub color: u32,
    pub light: u32,
    pub overlay: u32,
    /// How far (blocks) the vertices move toward the camera (`SkinJob::bias`): 0 but for a
    /// Bedrock pack's later layers.
    pub bias: f32,
}

/// Where a frame's posed draws go, and the bounds of the current mob's.
pub struct Skin<'a> {
    pub models: &'a mut SkinModels,
    pub frame: &'a mut SkinFrame,
    /// Camera-relative bounds of what this mob drew here (for culling).
    pub bounds: Option<(glam::Vec3, glam::Vec3)>,
    /// The last walk, for a draw that repeats it.
    last: Option<Walked>,
}

/// One walk of [`Skin::draw`]: what it drew, and the jobs it pushed.
struct Walked {
    model: usize,
    base: glam::Mat4,
    poses: u64,
    /// `Poses::shape_version`: what placed the parts.
    shape: u64,
    /// Where its part matrices start in the frame.
    first_part: u32,
    jobs: std::ops::Range<usize>,
}

impl<'a> Skin<'a> {
    /// One entity's draws into `frame`.
    pub fn new(models: &'a mut SkinModels, frame: &'a mut SkinFrame) -> Self {
        Self {
            models,
            frame,
            bounds: None,
            last: None,
        }
    }

    /// Grows the bounds of what this entity drew here by `lo..hi` posed by `m`.
    pub fn grow_bounds(&mut self, m: glam::Mat4, (lo, hi): (glam::Vec3, glam::Vec3)) {
        let center = m.transform_point3((lo + hi) * 0.5);
        let half = (hi - lo) * 0.5;
        let r = glam::Mat3::from_mat4(m);
        let extent = r.x_axis.abs() * half.x + r.y_axis.abs() * half.y + r.z_axis.abs() * half.z;
        let (a, b) = (center - extent, center + extent);
        self.bounds = Some(match self.bounds {
            Some((l, h)) => (l.min(a), h.max(b)),
            None => (a, b),
        });
    }

    /// Whether this model can be posed here (stored on first sight).
    pub fn supports(&mut self, model: &Arc<Part>) -> bool {
        self.models.entry(model, self.frame).is_some()
    }

    /// The draws since job `first` also go to the Bedrock stencil pass (a piece it draws besides
    /// the scene): where they land in its posed output, and how many vertices.
    pub fn stencil_copy_since(&mut self, first: usize) -> (u32, u32) {
        self.frame.stencil_copy_since(first)
    }

    /// The models' store epoch (`SkinModels::epoch`).
    pub fn store_epoch(&self) -> u64 {
        self.models.epoch()
    }

    /// Parts or jobs were added by hand (draws kept from an earlier frame): no later draw
    /// repeats the last walk.
    pub fn forget_last(&mut self) {
        self.last = None;
    }

    /// The draws since job `first` go to the Bedrock stencil pass instead of the scene (a piece
    /// only it draws); `vertices` is where the scene's posed output stood before them.
    pub fn stencil_move_since(&mut self, first: usize, vertices: u32) -> (u32, u32) {
        // The jobs a repeated walk would copy are gone from the scene.
        self.last = None;
        self.frame.stencil_move_since(first, vertices)
    }

    /// `render::draw_scrolled` on the GPU: false (nothing done) when the
    /// model cannot take this path.
    pub fn draw(
        &mut self,
        model: &Arc<Part>,
        base: glam::Mat4,
        poses: &Poses,
        paint: Paint,
    ) -> bool {
        let key = Arc::as_ptr(model) as usize;
        // A draw that repeats the last walk (a villager's type, profession and
        // level over its body, a spider's eyes): the same model, matrix and
        // poses give the same part matrices and parts left out, so its jobs
        // are the last ones' over the same parts, with this paint.
        if let Some(last) = &self.last
            && last.model == key
            && last.base == base
            && last.poses == poses.version()
        {
            for i in last.jobs.clone() {
                let job = self.frame.jobs[i];
                self.frame.jobs.push(SkinJob {
                    dst: self.frame.vertices,
                    ..paint.job(job.first, job.count, job.part, job.cull)
                });
                self.frame.vertices += job.count;
            }
            return true;
        }
        // The same parts placed alike, other parts skipping their cubes (a Bedrock model's
        // material groups, each drawing its own bones): the last walk's part matrices, and jobs
        // for the parts this draw shows.
        if let Some(last) = &self.last
            && last.model == key
            && last.base == base
            && last.shape == poses.shape_version()
        {
            let first_part = last.first_part;
            let Some((stored, _)) = self.models.entry(model, self.frame) else {
                return false;
            };
            let jobs = self.frame.jobs.len();
            let mut walk = Rejob {
                stored,
                poses,
                parts: &self.frame.parts[first_part as usize..],
                jobs: &mut self.frame.jobs,
                vertices: &mut self.frame.vertices,
                bounds: &mut self.bounds,
                first_part,
                paint,
                index: 0,
                since: jobs,
            };
            walk.part(model);
            self.last = Some(Walked {
                model: key,
                base,
                poses: poses.version(),
                shape: poses.shape_version(),
                first_part,
                jobs: jobs..self.frame.jobs.len(),
            });
            return true;
        }
        let cache_enabled = !self.models.rotation_cache_disabled;
        let Some((stored, rotations)) = self.models.entry(model, self.frame) else {
            return false;
        };
        let jobs = self.frame.jobs.len();
        let first_part = self.frame.parts.len() as u32;
        self.frame.parts.resize(
            self.frame.parts.len() + stored.parts.len(),
            bytemuck::Zeroable::zeroed(),
        );
        if cache_enabled {
            let mut walk = CachedWalk {
                rotations,
                stored,
                poses,
                parts: &mut self.frame.parts[first_part as usize..],
                jobs: &mut self.frame.jobs,
                vertices: &mut self.frame.vertices,
                bounds: &mut self.bounds,
                first_part,
                paint,
                index: 0,
            };
            walk.part(model, base, ModelNormals::new(base));
        } else {
            let mut walk = Walk {
                stored,
                poses,
                parts: &mut self.frame.parts[first_part as usize..],
                jobs: &mut self.frame.jobs,
                vertices: &mut self.frame.vertices,
                bounds: &mut self.bounds,
                first_part,
                paint,
                index: 0,
            };
            walk.part(model, base, ModelNormals::new(base));
        }
        // The walk's first job never extends an earlier draw's: those have
        // other parts.
        self.last = Some(Walked {
            model: key,
            base,
            poses: poses.version(),
            shape: poses.shape_version(),
            first_part,
            jobs: jobs..self.frame.jobs.len(),
        });
        true
    }
}

/// [`Walk`] over part matrices a walk already wrote (`Skin::draw` with only the skipped parts
/// changed): the jobs and bounds of the parts this draw shows, with the same parts left out.
struct Rejob<'a> {
    stored: &'a Stored,
    poses: &'a Poses,
    /// The walked model's part matrices, from its first.
    parts: &'a [SkinPart],
    jobs: &'a mut Vec<SkinJob>,
    vertices: &'a mut u32,
    bounds: &'a mut Option<(glam::Vec3, glam::Vec3)>,
    first_part: u32,
    paint: Paint,
    index: u32,
    /// This draw's first job: earlier ones are another draw's, painted apart.
    since: usize,
}

impl Rejob<'_> {
    fn part(&mut self, part: &Part) {
        let index = self.index;
        let stored = self.stored.parts[index as usize];
        let p = self.poses.pose(part);
        if self.poses.is_hidden(part) || p[6] == 0.0 && p[7] == 0.0 && p[8] == 0.0 {
            self.index = stored.end;
            return;
        }
        if !self.poses.skips(part) && stored.count > 0 {
            let m = glam::Mat4::from_cols_array(&self.parts[index as usize].matrix);
            Walk::emit_into(
                self.stored,
                self.jobs,
                self.since,
                self.vertices,
                self.first_part,
                self.paint,
                stored,
            );
            if let Some((lo, hi)) = self.stored.bounds[index as usize] {
                grow_bounds(self.bounds, m, lo, hi);
            }
        }
        self.index += 1;
        for child in &part.children {
            self.part(child);
        }
    }
}

/// `bounds` grown by a part's own box `lo..hi` under its matrix `m`.
fn grow_bounds(
    bounds: &mut Option<(glam::Vec3, glam::Vec3)>,
    m: glam::Mat4,
    lo: [f32; 3],
    hi: [f32; 3],
) {
    let (lo, hi) = (glam::Vec3::from_array(lo), glam::Vec3::from_array(hi));
    let center = m.transform_point3((lo + hi) * 0.5);
    let half = (hi - lo) * 0.5;
    let r = glam::Mat3::from_mat4(m);
    let extent = r.x_axis.abs() * half.x + r.y_axis.abs() * half.y + r.z_axis.abs() * half.z;
    let (a, b) = (center - extent, center + extent);
    *bounds = Some(match *bounds {
        Some((l, h)) => (l.min(a), h.max(b)),
        None => (a, b),
    });
}

impl Paint {
    /// A job drawing `count` stored vertices from `first`, posed by the parts
    /// from `part`, in this paint (`dst` left 0).
    fn job(self, first: u32, count: u32, part: u32, cull: u32) -> SkinJob {
        SkinJob {
            first,
            count,
            part,
            dst: 0,
            rect: self.uv,
            scroll: self.scroll,
            color: self.color,
            light: self.light,
            overlay: self.overlay,
            glint: u32::from(self.overlay == super::render::ARMOR_GLINT),
            mode: SkinJob::MODEL,
            cull,
            spare: 0,
            bias: self.bias,
            reserved: [0; 3],
        }
    }
}

struct Walk<'a> {
    stored: &'a Stored,
    poses: &'a Poses,
    parts: &'a mut [SkinPart],
    jobs: &'a mut Vec<SkinJob>,
    vertices: &'a mut u32,
    bounds: &'a mut Option<(glam::Vec3, glam::Vec3)>,
    first_part: u32,
    paint: Paint,
    /// The pre-order index of the part being walked.
    index: u32,
}

impl Walk<'_> {
    /// `render::draw_normals` without the vertices: the part's matrices, and
    /// its polygons as (part of) a job.
    fn part(&mut self, part: &Part, base: glam::Mat4, normals: ModelNormals) {
        let index = self.index;
        let stored = self.stored.parts[index as usize];
        let p = self.poses.pose(part);
        let m = super::render::posed(base, p);
        if self.poses.is_hidden(part) || p[6] == 0.0 && p[7] == 0.0 && p[8] == 0.0 {
            // The whole subtree is left out.
            self.index = stored.end;
            return;
        }
        let normals = normals.part(p);
        let n = normal_matrix(normals);
        self.parts[index as usize] = SkinPart {
            matrix: m.to_cols_array(),
            normal: n.to_cols_array(),
        };
        if !self.poses.skips(part) && stored.count > 0 {
            self.emit(stored);
            if let Some((lo, hi)) = self.stored.bounds[index as usize] {
                let (lo, hi) = (glam::Vec3::from_array(lo), glam::Vec3::from_array(hi));
                let center = m.transform_point3((lo + hi) * 0.5);
                let half = (hi - lo) * 0.5;
                let r = glam::Mat3::from_mat4(m);
                let extent =
                    r.x_axis.abs() * half.x + r.y_axis.abs() * half.y + r.z_axis.abs() * half.z;
                let (a, b) = (center - extent, center + extent);
                *self.bounds = Some(match *self.bounds {
                    Some((l, h)) => (l.min(a), h.max(b)),
                    None => (a, b),
                });
            }
        }
        self.index += 1;
        for child in &part.children {
            self.part(child, m, normals);
        }
    }

    /// The part's vertices: extends the last job when they follow its own.
    fn emit(&mut self, stored: StoredPart) {
        Self::emit_into(
            self.stored,
            self.jobs,
            0,
            self.vertices,
            self.first_part,
            self.paint,
            stored,
        );
    }

    /// [`Walk::emit`] for any walk over `model`'s parts from `first_part`, extending only the
    /// jobs from `since` on (this draw's: a rewalk shares an earlier draw's parts, painted apart).
    fn emit_into(
        model: &Stored,
        jobs: &mut Vec<SkinJob>,
        since: usize,
        vertices: &mut u32,
        first_part: u32,
        paint: Paint,
        stored: StoredPart,
    ) {
        let first = model.first + stored.first;
        if jobs.len() > since
            && let Some(job) = jobs.last_mut()
            && job.first + job.count == first
            && job.part == first_part
            && job.dst + job.count == *vertices
        {
            job.count += stored.count;
        } else {
            jobs.push(SkinJob {
                dst: *vertices,
                ..paint.job(first, stored.count, first_part, u32::from(model.cull))
            });
        }
        *vertices += stored.count;
    }
}

struct CachedWalk<'a> {
    stored: &'a Stored,
    rotations: &'a mut Rotations,
    poses: &'a Poses,
    parts: &'a mut [SkinPart],
    jobs: &'a mut Vec<SkinJob>,
    vertices: &'a mut u32,
    bounds: &'a mut Option<(glam::Vec3, glam::Vec3)>,
    first_part: u32,
    paint: Paint,
    /// The pre-order index of the part being walked.
    index: u32,
}

impl CachedWalk<'_> {
    /// `render::draw_normals` without the vertices: the part's matrices, and
    /// its polygons as (part of) a job.
    fn part(&mut self, part: &Part, base: glam::Mat4, normals: ModelNormals) {
        let index = self.index;
        let stored = self.stored.parts[index as usize];
        let p = self.poses.pose(part);
        if self.poses.is_hidden(part) || p[6] == 0.0 && p[7] == 0.0 && p[8] == 0.0 {
            // The whole subtree is left out.
            self.index = stored.end;
            return;
        }
        let rotation = self.rotations.get(p);
        let m = super::render::posed_rotated(base, p, rotation.matrix);
        let normals = normals.part_rotated(p, rotation.normal);
        let n = normal_matrix(normals);
        self.parts[index as usize] = SkinPart {
            matrix: m.to_cols_array(),
            normal: n.to_cols_array(),
        };
        if !self.poses.skips(part) && stored.count > 0 {
            self.emit(stored);
            if let Some((lo, hi)) = self.stored.bounds[index as usize] {
                let (lo, hi) = (glam::Vec3::from_array(lo), glam::Vec3::from_array(hi));
                let center = m.transform_point3((lo + hi) * 0.5);
                let half = (hi - lo) * 0.5;
                let r = glam::Mat3::from_mat4(m);
                let extent =
                    r.x_axis.abs() * half.x + r.y_axis.abs() * half.y + r.z_axis.abs() * half.z;
                let (a, b) = (center - extent, center + extent);
                *self.bounds = Some(match *self.bounds {
                    Some((l, h)) => (l.min(a), h.max(b)),
                    None => (a, b),
                });
            }
        }
        self.index += 1;
        for child in &part.children {
            self.part(child, m, normals);
        }
    }

    /// The part's vertices: extends the last job when they follow its own.
    fn emit(&mut self, stored: StoredPart) {
        let first = self.stored.first + stored.first;
        let paint = self.paint;
        if let Some(job) = self.jobs.last_mut()
            && job.first + job.count == first
            && job.part == self.first_part
            && job.dst + job.count == *self.vertices
        {
            job.count += stored.count;
        } else {
            self.jobs.push(SkinJob {
                dst: *self.vertices,
                ..paint.job(
                    first,
                    stored.count,
                    self.first_part,
                    u32::from(self.stored.cull),
                )
            });
        }
        *self.vertices += stored.count;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gfx::entity_skin::expand;
    use crate::mobs::{Mob, MobData, MobTypes, render};

    /// Draws of one model that only skip other parts (a Bedrock model's material groups) reuse
    /// the first draw's part matrices, and draw what full walks would: the same vertices, no
    /// more parts posed.
    #[test]
    fn a_draw_skipping_other_parts_reuses_the_part_matrices() {
        let quad = serde_json::json!([[
            [0, 0, 0, 0, 0],
            [0, 16, 0, 0, 1],
            [0, 16, 16, 1, 1],
            [0, 0, 16, 1, 0]
        ]]);
        let model = Arc::new(Part::parse(
            "root",
            &serde_json::json!({
                "pose": [0, 0, 0, 0, 0, 0, 1, 1, 1],
                "children": {
                    "arm": {"pose": [4, 2, 0, 0, 0, 0, 1, 1, 1], "polygons": quad, "normals": [[1, 0, 0]]},
                    "leg": {"pose": [-4, 0, 0, 0, 0, 0, 1, 1, 1], "polygons": quad, "normals": [[1, 0, 0]]}
                }
            }),
        ));
        let paint = |color| Paint {
            uv: [0.0, 0.0, 1.0, 1.0],
            scroll: 0.0,
            color,
            light: 0x70a0,
            overlay: 0,
            bias: 0.0,
        };
        let base = glam::Mat4::from_translation(glam::Vec3::new(1.0, 2.0, -3.0))
            * glam::Mat4::from_rotation_y(0.7);
        let mut poses = Poses::new(&model);
        poses.get("arm")[3] = 0.6;
        poses.get("leg")[4] = -0.4;
        // The groups as the CPU draws them, each from a fresh pose.
        let mut cpu = Vec::new();
        for (skip, color) in [("leg", 0xff00_00ff), ("arm", 0xff00_ff00)] {
            let mut own = Poses::default();
            own.copy_from(&poses);
            own.skip_draw(skip);
            render::draw(
                &model,
                base,
                &own,
                [0.0, 0.0, 1.0, 1.0],
                color,
                0x70a0,
                0,
                &mut cpu,
            );
        }
        let mut models = SkinModels::default();
        let mut frame = SkinFrame::default();
        models.begin(&mut frame, 0);
        let mut skin = Skin::new(&mut models, &mut frame);
        poses.skip_draw("leg");
        assert!(skin.draw(&model, base, &poses, paint(0xff00_00ff)));
        let parts = skin.frame.parts.len();
        poses.clear_skips();
        poses.skip_draw("arm");
        assert!(skin.draw(&model, base, &poses, paint(0xff00_ff00)));
        assert_eq!(
            skin.frame.parts.len(),
            parts,
            "the second group posed no parts"
        );
        let store = frame.new_model.clone();
        let gpu = expand(&store, &frame);
        assert_eq!(gpu.len(), cpu.len());
        for (i, (a, b)) in gpu.iter().zip(&cpu).enumerate() {
            assert_eq!(bytemuck::bytes_of(a), bytemuck::bytes_of(b), "vertex {i}");
        }
    }

    #[test]
    fn rotation_cache_can_be_bypassed_between_frames() {
        let model = Arc::new(Part::parse(
            "root",
            &serde_json::json!({
                "pose": [0, 0, 0, 0, 0, 0, 1, 1, 1],
                "polygons": [[[0, 0, 0, 0, 0], [0, 16, 0, 0, 1],
                              [0, 16, 16, 1, 1], [0, 0, 16, 1, 0]]],
                "normals": [[1, 0, 0]]
            }),
        ));
        let mut models = SkinModels::default();
        let mut frame = SkinFrame::default();
        let mut store = Vec::new();
        let paint = Paint {
            uv: [0.125, 0.25, 0.75, 0.875],
            scroll: 0.0,
            color: 0xff80_c0ff,
            light: 0x70a0,
            overlay: 0,
            bias: 0.0,
        };
        for (i, enabled) in [false, true, false, true].into_iter().enumerate() {
            let mut poses = Poses::new(&model);
            poses.get("root")[3..6].copy_from_slice(&[0.3 + i as f32 * 0.1, -0.7, 1.2]);
            let base = glam::Mat4::from_scale(glam::Vec3::new(-0.75, 1.25, 0.5));
            let mut expected = Vec::new();
            render::draw(
                &model,
                base,
                &poses,
                paint.uv,
                paint.color,
                paint.light,
                paint.overlay,
                &mut expected,
            );
            let before = models
                .rotations
                .0
                .map(|entry| entry.map(|rotation| rotation.key));
            models.set_rotation_cache_enabled(enabled);
            models.begin(&mut frame, store.len() as u32);
            assert!(Skin::new(&mut models, &mut frame).draw(&model, base, &poses, paint));
            let after = models
                .rotations
                .0
                .map(|entry| entry.map(|rotation| rotation.key));
            if enabled {
                assert_ne!(before, after, "enabled cache must record the new rotation");
            } else {
                assert_eq!(
                    before, after,
                    "bypass must not access or populate the cache"
                );
            }
            store.extend_from_slice(&frame.new_model);
            frame.handed();
            assert_eq!(
                bytemuck::cast_slice::<_, u8>(&expand(&store, &frame)),
                bytemuck::cast_slice::<_, u8>(&expected),
            );
        }
    }

    #[test]
    fn reused_rotations_keep_scaled_and_animated_vertices_bit_exact() {
        let leaf = |name: &str| {
            Part::parse(
                name,
                &serde_json::json!({
                    "pose": [0, 0, 0, 0, 0, 0, 1, 1, 1],
                    "polygons": [[[0, 0, 0, 0, 0], [0, 16, 0, 0, 1],
                                  [0, 16, 16, 1, 1], [0, 0, 16, 1, 0]]],
                    "normals": [[1, 0, 0]]
                }),
            )
        };
        let mut root = leaf("root");
        let mut child = leaf("child");
        child.children.push(leaf("tip"));
        root.children.push(child);
        let model = Arc::new(root);
        let mut models = SkinModels::default();
        let mut frame = SkinFrame::default();
        let mut store = Vec::new();
        let angles = [0.0, -0.0, 0.7, f32::from_bits(0.7f32.to_bits() + 1), -1.2];
        // More distinct rotations than the cache holds, interspersed with hits.
        // Translation, parent transforms and scale keep changing under a reused
        // rotation, including mirrored and nonuniform normal stacks.
        for i in 0..512 {
            let mut poses = Poses::new(&model);
            for (j, name) in ["root", "child", "tip"].into_iter().enumerate() {
                let pose = poses.get(name);
                pose[0] = i as f32 * 0.03125;
                pose[1] = j as f32 * -2.25;
                pose[3] = angles[(i + j) % angles.len()];
                pose[4] = if i % 3 == 0 { i as f32 * 0.013 } else { 0.0 };
                pose[5] = angles[(i / 2 + j) % angles.len()];
                pose[6..9].copy_from_slice(match (i + j) % 4 {
                    0 => &[1.0, 1.0, 1.0],
                    1 => &[-1.0, 1.0, -1.0],
                    2 => &[0.75, 1.25, 0.5],
                    _ => &[0.0, 0.0, 0.0],
                });
            }
            if i % 17 == 0 {
                poses.visible("child", false);
            }
            if i % 19 == 0 {
                poses.skip_draw("root");
            }
            let base = glam::Mat4::from_translation(glam::Vec3::new(i as f32 * 0.1, -3.25, 7.0))
                * glam::Mat4::from_rotation_y(i as f32 * 0.017);
            let paint = Paint {
                uv: [0.125, 0.25, 0.75, 0.875],
                scroll: 0.0,
                color: 0xff80_c0ff,
                light: 0x70a0,
                overlay: 0,
                bias: 0.0,
            };
            let mut expected = Vec::new();
            render::draw(
                &model,
                base,
                &poses,
                paint.uv,
                paint.color,
                paint.light,
                paint.overlay,
                &mut expected,
            );
            models.begin(&mut frame, store.len() as u32);
            assert!(Skin::new(&mut models, &mut frame).draw(&model, base, &poses, paint));
            store.extend_from_slice(&frame.new_model);
            frame.handed();
            let actual = expand(&store, &frame);
            assert_eq!(
                bytemuck::cast_slice::<_, u8>(&actual),
                bytemuck::cast_slice::<_, u8>(&expected),
                "draw {i}"
            );
        }
    }

    #[test]
    fn restart_clears_models_and_marks_frame_reset() {
        let mut models = SkinModels::default();
        let mut frame = SkinFrame::default();
        frame.jobs.push(Default::default());
        frame.parts.push(SkinPart {
            matrix: [0.0; 16],
            normal: [0.0; 9],
        });
        frame.vertices = 4;
        frame.outline_jobs.push(Default::default());
        frame.outline_vertices = 4;
        assert!(frame.pending());

        models.restart(&mut frame);
        frame.clear_draws();

        assert_eq!(models.vertices, 0);
        assert_eq!(frame.base, 0);
        assert_eq!(frame.vertices, 0);
        assert_eq!(frame.outline_vertices, 0);
        assert!(frame.jobs.is_empty());
        assert!(frame.parts.is_empty());
        assert!(frame.outline_jobs.is_empty());
        assert!(frame.reset);
    }

    /// Every vanilla mob, at a few ages and turns: the GPU path's vertices
    /// (its jobs expanded as `entity_skin.wgsl` does) are the CPU path's, to
    /// the bit and in the same order, for each mob it takes; the mobs it
    /// leaves to the CPU draw exactly as before. Twice: with the eyes render
    /// type's layers left to the CPU (improved transparency on), then posed
    /// too with a horse's markings and a slime's outer cube (`SkinModels::tagged`).
    /// Needs the prepared resources.
    #[test]
    fn posed_on_the_gpu_every_mob_draws_what_the_cpu_draws() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.local/horizon-client/26.3");
        if !root.join("entity-models.json").exists() {
            eprintln!("no prepared resources: skipped");
            return;
        }
        let types = MobTypes::load(&root).unwrap();
        let data = MobData {
            states: Vec::new(),
            placements: Default::default(),
            heightmaps: Default::default(),
            animations: MobData::load_animations(&root),
            biome_tags: Default::default(),
            entity_tags: Default::default(),
            villager_hats: Default::default(),
            cem: Default::default(),
            bedrock: Default::default(),
            random_textures: Default::default(),
            emissive: Default::default(),
            blinks: Default::default(),
            pois: Default::default(),
            species_placements: Default::default(),
        };
        // Every texture its own rect, so a swapped texture shows.
        let atlas = |name: &str| {
            let h = name
                .bytes()
                .fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b)));
            let u = (h % 61) as f32 / 64.0;
            let v = (h / 61 % 61) as f32 / 64.0;
            Some([u, v, u + 1.0 / 64.0, v + 1.0 / 32.0])
        };
        let camera = glam::DVec3::new(0.3, 62.0, -4.0);
        let mut models = SkinModels::default();
        let mut frame = SkinFrame::default();
        let mut store: Vec<crate::gfx::entity_skin::SkinVertex> = Vec::new();
        let (mut posed, mut kept) = (0, 0);
        let mut ids: Vec<_> = types.types.keys().cloned().collect();
        ids.sort();
        // The mobs posed at least once, without and with the tagged layers.
        let mut taken = [std::collections::BTreeSet::new(), Default::default()];
        for eyes in [false, true] {
            for (n, id) in ids.iter().enumerate() {
                let kind = Arc::new(types.types[id].clone());
                for (age, yaw) in [(0u32, 0.0f32), (37, 123.0), (412, -71.5)] {
                    let at = horizon_core::geometry::Vec3::new(1.5 + n as f64 % 5.0, 64.0, 3.25);
                    let mut m = Mob::new(n as u32 + 1, kind.clone(), at, yaw);
                    m.previous = horizon_core::geometry::Vec3::new(at.x - 0.1, at.y, at.z + 0.05);
                    m.age = age;
                    m.limb_speed = 0.6;
                    m.limb_position = age as f32 * 0.37;
                    // A farmer: its type, profession and level layers repeat
                    // the body's draw (`Skin::draw` reuses its walk).
                    if matches!(
                        id.as_str(),
                        "minecraft:villager" | "minecraft:zombie_villager"
                    ) {
                        m.extra.variant2 = 5;
                        m.extra.variant3 = 3;
                    }
                    // White field markings (`entityTranslucent`).
                    if id == "minecraft:horse" {
                        m.extra.variant2 = 2;
                    }
                    let mut cpu = Vec::new();
                    render::render_with(&m, 0.4, camera, &atlas, 0x50a0, &data, &[], &mut cpu);
                    models.begin(&mut frame, store.len() as u32);
                    models.tagged = eyes;
                    let mut gpu = Vec::new();
                    let mut skin = Skin::new(&mut models, &mut frame);
                    render::render_skinned(
                        &m, 0.4, camera, &atlas, 0x50a0, &data, &mut gpu, &mut skin,
                    );
                    let bounds = skin.bounds;
                    if frame.reset {
                        store.clear();
                    }
                    assert_eq!(store.len() as u32, frame.base);
                    store.extend_from_slice(&frame.new_model);
                    frame.handed();
                    if frame.jobs.is_empty() {
                        kept += 1;
                        assert_eq!(
                            bytemuck::cast_slice::<_, u8>(&gpu),
                            bytemuck::cast_slice::<_, u8>(&cpu),
                            "{id} age {age}: the CPU path changed"
                        );
                        continue;
                    }
                    posed += 1;
                    taken[usize::from(eyes)].insert(id.as_str());
                    assert!(gpu.is_empty(), "{id}: a posed mob left vertices on the CPU");
                    let expanded = expand(&store, &frame);
                    assert_eq!(expanded.len(), cpu.len(), "{id} age {age}: vertex count");
                    for (i, (a, b)) in expanded.iter().zip(&cpu).enumerate() {
                        assert_eq!(
                            bytemuck::bytes_of(a),
                            bytemuck::bytes_of(b),
                            "{id} age {age} vertex {i}: {a:?} != {b:?}"
                        );
                    }
                    // The culling bounds hold every vertex.
                    let (lo, hi) = bounds.expect("a posed mob has bounds");
                    for v in &cpu {
                        let p = glam::Vec3::from_array(v.pos);
                        assert!(
                            p.cmpge(lo - 1e-4).all() && p.cmple(hi + 1e-4).all(),
                            "{id}: {p} outside {lo}..{hi}"
                        );
                    }
                }
            }
        }
        eprintln!("posed on the GPU: {posed} mob draws; kept on the CPU: {kept}");
        assert!(posed > 2 * 3 * 40, "only {posed} draws took the GPU path");
        let added: Vec<_> = taken[1].difference(&taken[0]).copied().collect();
        eprintln!("posed only with the tagged layers: {added:?}");
        for id in [
            "minecraft:spider",
            "minecraft:cave_spider",
            "minecraft:enderman",
            "minecraft:phantom",
            "minecraft:copper_golem",
            "minecraft:horse",
            "minecraft:slime",
        ] {
            assert!(added.contains(&id), "{id} not posed with its tagged layers");
        }
        assert!(taken[0].is_subset(&taken[1]));
    }

    /// A Bedrock pack's mobs (docs/bedrock-gpu-pose.md, step 1): what the GPU poses of them (its
    /// jobs expanded as `entity_skin.wgsl` does) together with what stays on the CPU is, to the
    /// bit, what the CPU path draws of them. The two paths order the vertices apart (the posed
    /// ones come first in the entity buffer), so they are compared as sets. Needs the prepared
    /// resources (`HORIZON_TEST_RESOURCES`, else `.local/horizon-client/26.3`) and a converted
    /// Bedrock pack (`HORIZON_TEST_BEDROCK_PACK`, a folder of `.horizon-cache/bedrock/`).
    #[test]
    fn a_bedrock_packs_mobs_posed_on_the_gpu_draw_what_the_cpu_draws() {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let root = std::env::var_os("HORIZON_TEST_RESOURCES")
            .map_or_else(|| repo.join(".local/horizon-client/26.3"), Into::into);
        let Some(pack) =
            std::env::var_os("HORIZON_TEST_BEDROCK_PACK").map(std::path::PathBuf::from)
        else {
            eprintln!("no HORIZON_TEST_BEDROCK_PACK: skipped");
            return;
        };
        if !root.join("entity-models.json").exists() {
            eprintln!("no prepared resources: skipped");
            return;
        }
        let types = MobTypes::load(&root).unwrap();
        let bedrock = Arc::new(crate::mobs::bedrock::BedrockMobs::load(&pack));
        let data = MobData {
            states: Vec::new(),
            placements: Default::default(),
            heightmaps: Default::default(),
            animations: MobData::load_animations(&root),
            biome_tags: Default::default(),
            entity_tags: Default::default(),
            villager_hats: Default::default(),
            cem: Default::default(),
            bedrock: std::sync::RwLock::new(bedrock.clone()),
            random_textures: Default::default(),
            emissive: Default::default(),
            blinks: Default::default(),
            pois: Default::default(),
            species_placements: Default::default(),
        };
        let atlas = |name: &str| {
            let h = name
                .bytes()
                .fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b)));
            let u = (h % 61) as f32 / 64.0;
            let v = (h / 61 % 61) as f32 / 64.0;
            Some([u, v, u + 1.0 / 64.0, v + 1.0 / 32.0])
        };
        let camera = glam::DVec3::new(0.3, 62.0, -4.0);
        let mut models = SkinModels::default();
        let mut frame = SkinFrame::default();
        let mut store: Vec<crate::gfx::entity_skin::SkinVertex> = Vec::new();
        let mut ids: Vec<_> = types
            .types
            .keys()
            .filter(|id| bedrock.has_entity(id) && !id.contains("player"))
            .cloned()
            .collect();
        ids.sort();
        let mut posed = std::collections::BTreeSet::new();
        let mut stenciled = std::collections::BTreeSet::new();
        // As a set: vertices in any order, each as its bytes.
        let set = |vertices: &mut dyn Iterator<Item = crate::mesher::Vertex>| {
            let mut all: Vec<Vec<u8>> = vertices.map(|v| bytemuck::bytes_of(&v).to_vec()).collect();
            all.sort_unstable();
            all
        };
        // Without and with the stencil pass (its pieces posed for it, `hand_over_posed`), and
        // with improved transparency on and off (off, the glow of an emissive layer is posed
        // too: `SkinModels::tagged`).
        for (pass_on, tagged) in [(false, false), (true, false), (true, true)] {
            crate::mobs::bedrock_stencil::set_on_for_test(pass_on);
            models.tagged = tagged;
            for (n, id) in ids.iter().enumerate() {
                let kind = Arc::new(types.types[id].clone());
                for (age, yaw) in [(0u32, 0.0f32), (37, 123.0), (412, -71.5)] {
                    let at = horizon_core::geometry::Vec3::new(1.5 + n as f64 % 5.0, 64.0, 3.25);
                    let mut m = Mob::new(n as u32 + 1, kind.clone(), at, yaw);
                    m.previous = horizon_core::geometry::Vec3::new(at.x - 0.1, at.y, at.z + 0.05);
                    m.age = age;
                    m.limb_speed = 0.6;
                    m.limb_position = age as f32 * 0.37;
                    // The GPU path first: a mob it poses keeps no vertices for the CPU path.
                    models.begin(&mut frame, store.len() as u32);
                    let mut gpu = Vec::new();
                    let ((), gpu_pieces) = crate::mobs::bedrock_stencil::record(|| {
                        let mut skin = Skin::new(&mut models, &mut frame);
                        render::render_skinned(
                            &m, 0.4, camera, &atlas, 0x50a0, &data, &mut gpu, &mut skin,
                        );
                    });
                    if frame.reset {
                        store.clear();
                    }
                    assert_eq!(store.len() as u32, frame.base);
                    store.extend_from_slice(&frame.new_model);
                    frame.handed();
                    if !frame.jobs.is_empty() {
                        posed.insert(id.clone());
                    }
                    if !frame.stencil_jobs.is_empty() {
                        stenciled.insert(id.clone());
                    }
                    let what = format!("{id} age {age} pass {pass_on} tagged {tagged}");
                    let scene = set(&mut expand(&store, &frame)
                        .into_iter()
                        .chain(gpu.iter().copied()));
                    let pieces = set(&mut crate::gfx::entity_skin::expand_stencil(&store, &frame)
                        .into_iter()
                        .chain(gpu_pieces.into_iter().flat_map(|(_, _, v)| v)));
                    // Drawn again in the same pose and place, before the CPU path draws it (its
                    // drawing is kept in place of this one): from what the first drawing kept
                    // (`KeptPose`), its part matrices and jobs replayed without a walk.
                    models.begin(&mut frame, store.len() as u32);
                    let mut again = Vec::new();
                    let ((), again_pieces) = crate::mobs::bedrock_stencil::record(|| {
                        let mut skin = Skin::new(&mut models, &mut frame);
                        render::render_skinned(
                            &m, 0.4, camera, &atlas, 0x50a0, &data, &mut again, &mut skin,
                        );
                    });
                    if frame.reset {
                        store.clear();
                    }
                    store.extend_from_slice(&frame.new_model);
                    frame.handed();
                    let scene_again = set(&mut expand(&store, &frame)
                        .into_iter()
                        .chain(again.iter().copied()));
                    assert!(
                        scene_again == scene,
                        "{what}: drawn again from what it kept"
                    );
                    let pieces_again =
                        set(&mut crate::gfx::entity_skin::expand_stencil(&store, &frame)
                            .into_iter()
                            .chain(again_pieces.into_iter().flat_map(|(_, _, v)| v)));
                    assert!(pieces_again == pieces, "{what}: kept stencil pieces");
                    let mut cpu = Vec::new();
                    let ((), cpu_pieces) = crate::mobs::bedrock_stencil::record(|| {
                        render::render_with(&m, 0.4, camera, &atlas, 0x50a0, &data, &[], &mut cpu);
                    });
                    let want = set(&mut cpu.iter().copied());
                    assert_eq!(scene.len(), want.len(), "{what}: scene vertex count");
                    if scene != want {
                        let a: Vec<_> = scene
                            .iter()
                            .filter(|b| !want.contains(b))
                            .take(4)
                            .map(|b| *bytemuck::from_bytes::<crate::mesher::Vertex>(b))
                            .collect();
                        let b: Vec<_> = want
                            .iter()
                            .filter(|b| !scene.contains(b))
                            .take(4)
                            .map(|b| *bytemuck::from_bytes::<crate::mesher::Vertex>(b))
                            .collect();
                        panic!("{what}\nGPU {a:?}\nCPU {b:?}");
                    }
                    let want = set(&mut cpu_pieces.into_iter().flat_map(|(_, _, v)| v));
                    assert_eq!(pieces.len(), want.len(), "{what}: stencil vertex count");
                    assert!(
                        pieces == want,
                        "{what}: the paths hand the stencil pass other pieces"
                    );
                }
            }
        }
        crate::mobs::bedrock_stencil::set_on_for_test(false);
        eprintln!("posing for the stencil pass on the GPU: {stenciled:?}");
        assert!(
            !stenciled.is_empty(),
            "no pack mob posed pieces for the stencil pass"
        );
        eprintln!(
            "posed on the GPU: {} of {} pack mobs: {posed:?}",
            posed.len(),
            ids.len()
        );
        assert!(!posed.is_empty(), "no pack mob took the GPU path");
    }

    /// A Bedrock pack's first-person view model posed on the GPU (into the hand's room,
    /// `SkinFrame::hand_jobs`) draws what the CPU draws, holding a few items.
    #[test]
    fn a_bedrock_packs_first_person_view_posed_on_the_gpu_draws_what_the_cpu_draws() {
        let Some(pack) =
            std::env::var_os("HORIZON_TEST_BEDROCK_PACK").map(std::path::PathBuf::from)
        else {
            eprintln!("no HORIZON_TEST_BEDROCK_PACK: skipped");
            return;
        };
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let root = std::env::var_os("HORIZON_TEST_RESOURCES")
            .map_or_else(|| repo.join(".local/horizon-client/26.3"), Into::into);
        if !root.join("entity-models.json").exists() {
            eprintln!("no prepared resources: skipped");
            return;
        }
        let types = MobTypes::load(&root).unwrap();
        let bedrock = Arc::new(crate::mobs::bedrock::BedrockMobs::load(&pack));
        let data = MobData {
            states: Vec::new(),
            placements: Default::default(),
            heightmaps: Default::default(),
            animations: MobData::load_animations(&root),
            biome_tags: Default::default(),
            entity_tags: Default::default(),
            villager_hats: Default::default(),
            cem: Default::default(),
            bedrock: std::sync::RwLock::new(bedrock.clone()),
            random_textures: Default::default(),
            emissive: Default::default(),
            blinks: Default::default(),
            pois: Default::default(),
            species_placements: Default::default(),
        };
        let atlas = |name: &str| {
            let h = name
                .bytes()
                .fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b)));
            let u = (h % 61) as f32 / 64.0;
            let v = (h / 61 % 61) as f32 / 64.0;
            Some([u, v, u + 1.0 / 64.0, v + 1.0 / 32.0])
        };
        // The player, as the game draws it (its own type is made outside this crate).
        let mut kind = types.types["minecraft:zombie"].clone();
        kind.id = "minecraft:player".into();
        let kind = Arc::new(kind);
        let set = |vertices: &mut dyn Iterator<Item = crate::mesher::Vertex>| {
            let mut all: Vec<Vec<u8>> = vertices.map(|v| bytemuck::bytes_of(&v).to_vec()).collect();
            all.sort_unstable();
            all
        };
        let hand = glam::Mat4::from_translation(glam::Vec3::new(0.3, -0.4, -0.7));
        let mut models = SkinModels::default();
        let mut frame = SkinFrame::default();
        let mut store: Vec<crate::gfx::entity_skin::SkinVertex> = Vec::new();
        let mut posed = 0;
        for tagged in [false, true] {
            for (n, items) in [
                (None, None),
                (Some("minecraft:netherite_sword"), None),
                (Some("minecraft:iron_pickaxe"), Some("minecraft:shield")),
                (Some("minecraft:bow"), Some("minecraft:golden_carrot")),
            ]
            .into_iter()
            .enumerate()
            {
                let mut m = Mob::new(500 + n as u32, kind.clone(), Default::default(), 30.0);
                m.age = 40 + n as u32 * 7;
                let draw = |skin: Option<&mut Skin>, out: &mut Vec<crate::mesher::Vertex>| {
                    render::first_person_item(
                        &m,
                        items.0,
                        items.1,
                        0.4,
                        hand,
                        [30.0, 10.0, 1.62],
                        &atlas,
                        0x50a0,
                        &data,
                        out,
                        skin,
                    )
                };
                let mut cpu = Vec::new();
                draw(None, &mut cpu);
                models.begin(&mut frame, store.len() as u32);
                models.tagged = tagged;
                let mut gpu = Vec::new();
                draw(Some(&mut Skin::new(&mut models, &mut frame)), &mut gpu);
                if frame.reset {
                    store.clear();
                }
                store.extend_from_slice(&frame.new_model);
                frame.handed();
                let room =
                    frame.hand_first as usize..(frame.hand_first + frame.hand_vertices) as usize;
                if !frame.hand_jobs.is_empty() {
                    posed += 1;
                }
                let what = format!("items {items:?} tagged {tagged}");
                assert!(
                    frame.jobs.is_empty(),
                    "{what}: posed outside the hand's room"
                );
                assert!(
                    room.end <= gpu.len(),
                    "{what}: no room for the posed vertices"
                );
                let mut drawn = gpu[..room.start].to_vec();
                drawn.extend_from_slice(&gpu[room.end..]);
                drawn.extend(crate::gfx::entity_skin::expand_hand(&store, &frame));
                assert_eq!(drawn.len(), cpu.len(), "{what}: vertex count");
                assert!(
                    set(&mut drawn.into_iter()) == set(&mut cpu.into_iter()),
                    "{what}: the paths draw other vertices"
                );
                frame.clear_draws();
            }
        }
        assert!(posed > 0, "the first-person view never took the GPU path");
    }

    /// A measure, not a check: for the pack mobs `HORIZON_CPU_KINDS` names (comma separated,
    /// `first_person` for the player's view model), what the GPU path poses and what stays on
    /// the CPU and why, in vertices (`render_03::cpu_why`).
    #[test]
    #[ignore]
    fn what_a_pack_mob_leaves_on_the_cpu() {
        let Some(pack) =
            std::env::var_os("HORIZON_TEST_BEDROCK_PACK").map(std::path::PathBuf::from)
        else {
            return;
        };
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let root = std::env::var_os("HORIZON_TEST_RESOURCES")
            .map_or_else(|| repo.join(".local/horizon-client/26.3"), Into::into);
        let types = MobTypes::load(&root).unwrap();
        let bedrock = Arc::new(crate::mobs::bedrock::BedrockMobs::load(&pack));
        let data = MobData {
            states: Vec::new(),
            placements: Default::default(),
            heightmaps: Default::default(),
            animations: MobData::load_animations(&root),
            biome_tags: Default::default(),
            entity_tags: Default::default(),
            villager_hats: Default::default(),
            cem: Default::default(),
            bedrock: std::sync::RwLock::new(bedrock.clone()),
            random_textures: Default::default(),
            emissive: Default::default(),
            blinks: Default::default(),
            pois: Default::default(),
            species_placements: Default::default(),
        };
        let atlas = |_: &str| Some([0.0, 0.0, 1.0 / 64.0, 1.0 / 32.0]);
        let camera = glam::DVec3::new(0.3, 62.0, -4.0);
        crate::mobs::bedrock_stencil::set_on_for_test(true);
        let kinds = std::env::var("HORIZON_CPU_KINDS")
            .unwrap_or_else(|_| "minecraft:copper_golem".to_owned());
        for id in kinds.split(',') {
            let kind = Arc::new(types.types[id].clone());
            let at = horizon_core::geometry::Vec3::new(0.5, 64.0, 3.25);
            let mut m = Mob::new(9, kind, at, 30.0);
            if let Ok(item) = std::env::var("HORIZON_CPU_HOLD") {
                m.extra.main_hand = Some(item.as_str().into());
            }
            crate::mobs::render::cpu_why::TALLY.with(|t| t.borrow_mut().clear());
            let (mut gpu, mut gpu_stencil, mut cpu, mut cpu_stencil) = (0usize, 0usize, 0, 0);
            for frame_no in 0..30u32 {
                m.age = 100 + frame_no;
                // `HORIZON_CPU_TAGGED`: as with improved transparency off.
                let mut models = SkinModels {
                    tagged: std::env::var_os("HORIZON_CPU_TAGGED").is_some(),
                    ..Default::default()
                };
                let mut frame = SkinFrame::default();
                models.begin(&mut frame, 0);
                let mut out = Vec::new();
                let ((), pieces) = crate::mobs::bedrock_stencil::record(|| {
                    let mut skin = Skin::new(&mut models, &mut frame);
                    render::render_skinned(
                        &m, 0.4, camera, &atlas, 0x50a0, &data, &mut out, &mut skin,
                    );
                });
                gpu += frame.vertices as usize;
                gpu_stencil += frame.stencil_vertices as usize;
                cpu += out.len();
                cpu_stencil += pieces.iter().map(|(_, _, v)| v.len()).sum::<usize>();
            }
            eprintln!(
                "{id}: per frame GPU {} (+{} stencil), CPU {} (+{} stencil)",
                gpu / 30,
                gpu_stencil / 30,
                cpu / 30,
                cpu_stencil / 30
            );
            crate::mobs::render::cpu_why::TALLY.with(|t| {
                for (why, n) in t.borrow().iter() {
                    eprintln!("    {why}: {} vertices a frame", n / 30);
                }
            });
        }
        crate::mobs::bedrock_stencil::set_on_for_test(false);
    }

    /// `BatModel` and `BabyTurtleModel` draw with `entityCutoutCull`: their
    /// zero-depth ears, wings, feet and legs put two quads in one place, and
    /// only culling tells them apart. Their vertices ask for the shaded cull
    /// encoding, other mobs keep -127, on both paths (the whole-mob test above
    /// holds them bit-equal).
    #[test]
    fn only_culled_render_types_ask_for_culled_back_faces() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.local/horizon-client/26.3");
        if !root.join("entity-models.json").exists() {
            eprintln!("no prepared resources: skipped");
            return;
        }
        let types = MobTypes::load(&root).unwrap();
        let turtle = &types.types["minecraft:turtle"];
        let baby_turtle = turtle.layer("turtle_baby#main").expect("baby turtle model");
        let cases: [(&str, &std::sync::Arc<crate::mobs::Part>, i8, u32); 5] = [
            (
                "minecraft:bat",
                &types.types["minecraft:bat"].model,
                crate::mesher::SHADED_CULL_W,
                1,
            ),
            (
                "minecraft:turtle_baby",
                baby_turtle,
                crate::mesher::SHADED_CULL_W,
                1,
            ),
            ("minecraft:turtle", &turtle.model, -127, 0),
            (
                "minecraft:cow",
                &types.types["minecraft:cow"].model,
                -127,
                0,
            ),
            (
                "minecraft:cat",
                &types.types["minecraft:cat"].model,
                -127,
                0,
            ),
        ];
        for (id, model, w, _) in cases {
            let mut part = vec![&**model];
            let mut seen = 0;
            while let Some(p) = part.pop() {
                for poly in &p.polygons {
                    let n = poly.vertex_normal(ModelNormals::new(glam::Mat4::IDENTITY));
                    assert_eq!(n[3], w, "{id}");
                    seen += 1;
                }
                part.extend(&p.children);
            }
            assert!(seen > 0, "{id} has polygons");
        }
        // The GPU path flags the job, the CPU path the polygons.
        let mut models = SkinModels::default();
        let mut frame = SkinFrame::default();
        for (id, model, _, cull) in cases {
            models.begin(&mut frame, 0);
            let mut skin = Skin::new(&mut models, &mut frame);
            assert!(skin.draw(
                model,
                glam::Mat4::IDENTITY,
                &Poses::default(),
                Paint {
                    uv: [0.0, 0.0, 1.0, 1.0],
                    scroll: 0.0,
                    color: u32::MAX,
                    light: 0,
                    overlay: 0,
                    bias: 0.0,
                },
            ));
            assert!(frame.jobs.iter().all(|j| j.cull == cull), "{id}");
            frame.clear_draws();
        }
    }

    /// The armor stand's models (`ArmorStandRenderer`: the stand, the small
    /// one, their armour layers) posed like a stand: turned arms and head, a
    /// hidden part (scale 0, as a stand without arms or base plate), the
    /// armour glint on one layer. The GPU path draws what the CPU path draws,
    /// to the bit. Needs the prepared resources.
    #[test]
    fn armor_stand_models_pose_on_the_gpu_as_on_the_cpu() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.local/horizon-client/26.3");
        if !root.join("entity-models.json").exists() {
            eprintln!("no prepared resources: skipped");
            return;
        }
        let document = crate::mobs::EntityModelDocument::load(&root).unwrap();
        let base = glam::Mat4::from_translation(glam::Vec3::new(2.5, -1.0, 6.25))
            * glam::Mat4::from_rotation_y(2.1)
            * glam::Mat4::from_scale(glam::Vec3::new(-1.0, -1.0, 1.0))
            * glam::Mat4::from_translation(glam::Vec3::new(0.0, -1.501, 0.0));
        let (mut models, mut frame) = (SkinModels::default(), SkinFrame::default());
        let mut store = Vec::new();
        let mut drawn = 0;
        for model in ["armor_stand", "armor_stand_small"] {
            for slot in ["main", "helmet", "chestplate", "leggings", "boots"] {
                let key = format!("minecraft:{model}#{slot}");
                let part = Arc::new(document.model(&key).unwrap_or_else(|| panic!("no {key}")));
                let mut poses = Poses::new(&part);
                for (name, turn) in [("head", [0.3, -0.7, 0.1]), ("left_arm", [-1.2, 0.4, 0.25])] {
                    if poses.has(name) {
                        poses.get(name)[3..6].copy_from_slice(&turn);
                    }
                }
                for hidden in ["base_plate", "right_arm"] {
                    if poses.has(hidden) {
                        poses.get(hidden)[6..9].copy_from_slice(&[0.0; 3]);
                    }
                }
                let overlay = if slot == "chestplate" {
                    render::ARMOR_GLINT
                } else {
                    0
                };
                let uv = [0.125, 0.25, 0.1875, 0.28125];
                let mut cpu = Vec::new();
                render::draw(
                    &part,
                    base,
                    &poses,
                    uv,
                    0xff80_c0ff,
                    0x70a0,
                    overlay,
                    &mut cpu,
                );
                models.begin(&mut frame, store.len() as u32);
                let mut gpu = Vec::new();
                let mut skin = Skin::new(&mut models, &mut frame);
                let paint = (uv, 0.0, 0xff80_c0ff, 0x70a0, overlay);
                render::put(
                    &mut Some(&mut skin),
                    &part,
                    base,
                    &poses,
                    paint.0,
                    paint.1,
                    paint.2,
                    paint.3,
                    paint.4,
                    &mut gpu,
                );
                if frame.reset {
                    store.clear();
                }
                store.extend_from_slice(&frame.new_model);
                frame.handed();
                assert!(gpu.is_empty(), "{key} stayed on the CPU");
                let expanded = expand(&store, &frame);
                assert_eq!(expanded.len(), cpu.len(), "{key}: vertex count");
                assert!(!cpu.is_empty(), "{key} drew nothing");
                for (i, (a, b)) in expanded.iter().zip(&cpu).enumerate() {
                    assert_eq!(
                        bytemuck::bytes_of(a),
                        bytemuck::bytes_of(b),
                        "{key} vertex {i}: {a:?} != {b:?}"
                    );
                }
                drawn += 1;
            }
        }
        assert_eq!(drawn, 10);
    }
}
