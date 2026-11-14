//! Bedrock packs' stencil, drawn with a real stencil buffer.
//!
//! A Bedrock material can test and write the stencil (`EnableStencilTest`, `StencilWrite`): a
//! clock's window writes a value without colour and its face shows only where that value is, an
//! armour's shine shows only inside a band. Horizon's own depth buffer (`Depth32Float`) has no
//! stencil, and none of its pipelines change for this: the pieces of a Bedrock model whose
//! material uses the stencil are handed here instead (`push`, from the mob renderer), and drawn
//! in a pass of their own with a depth-and-stencil buffer of this module:
//!
//! 1. the pass's depth starts as a copy of the scene's (the world's, or the first-person hand's),
//!    so the pieces hide behind what is nearer and the stencil starts cleared;
//! 2. the pieces draw in the order the model drew them, each with its material's stencil test
//!    and write (`State`), the entity shader and the scene's transforms.
//!
//! A piece that writes the stencil and also draws colour stays in the scene's own pass as well
//! (its colour, glow and transparency as before) and is drawn here again with no colour, for the
//! stencil it writes. What only tests the stencil, and the masks without colour, draw here only.
//!
//! The hooks in the renderer: `features` (the device's `DEPTH32FLOAT_STENCIL8`, where the
//! adapter has it), `Stencil::new`, `begin_frame`, `draw_world` after the world pass and
//! `draw_hand` after the first-person hand. The mob renderer asks `active` before handing pieces
//! over and emulates the stencil on the CPU otherwise (the inventory's doll, a shader pack).

use crate::mesher::Vertex;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// A stencil comparison, as a Bedrock material names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Compare {
    Always,
    Never,
    Less,
    LessEqual,
    Equal,
    NotEqual,
    Greater,
    GreaterEqual,
}

impl Compare {
    fn wgpu(self) -> wgpu::CompareFunction {
        use wgpu::CompareFunction as C;
        match self {
            Self::Always => C::Always,
            Self::Never => C::Never,
            Self::Less => C::Less,
            Self::LessEqual => C::LessEqual,
            Self::Equal => C::Equal,
            Self::NotEqual => C::NotEqual,
            Self::Greater => C::Greater,
            Self::GreaterEqual => C::GreaterEqual,
        }
    }
}

/// How a piece draws here: its material's colour, depth and stencil states.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct State {
    /// Draws colour (`DisableColorWrite` off).
    pub color: bool,
    /// Adds its colour to what is behind (`blendSrc One`/`SourceAlpha`, `blendDst One`).
    pub additive: bool,
    /// Writes depth (`DisableDepthWrite` off).
    pub depth_write: bool,
    /// Tests depth (`DisableDepthTest` off).
    pub depth_test: bool,
    /// Texels below the entity cutout (alpha 0.1) are dropped. Off for a material without
    /// `ALPHA_TEST`: it fills the stencil wherever the quad lies, whatever its texel (a clock's
    /// window samples a clear texel and still writes its reference).
    pub cutout: bool,
    /// The stencil test.
    pub compare: Compare,
    /// A passing pixel takes the reference (`StencilWrite`, `stencilPassOp Replace`).
    pub replace: bool,
    pub reference: u8,
    pub read_mask: u8,
    pub write_mask: u8,
}

/// Where a piece was drawn: among the world's entities, or in the first-person hand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    World,
    Hand,
}

#[derive(Clone, Copy, Debug)]
struct Batch {
    state: State,
    /// The first vertex, in its target's list.
    first: u32,
    vertices: u32,
}

#[derive(Default)]
struct Lists {
    vertices: [Vec<Vertex>; 2],
    batches: [Vec<Batch>; 2],
}

static PENDING: Mutex<Lists> = Mutex::new(Lists {
    vertices: [Vec::new(), Vec::new()],
    batches: [Vec::new(), Vec::new()],
});
/// A renderer with this module's pass exists, and the frame draws it (no shader pack).
static ACTIVE: AtomicBool = AtomicBool::new(false);

/// The most vertices a target keeps for a frame: past it (a frame never drawn), the oldest
/// frame's pieces are dropped.
const MAX_VERTICES: usize = 1 << 20;

/// The `Vertex::layer` bits that route a quad among the scene's pipelines
/// (`entity_batch::OIT_ENTITY` and the bedrock material bits above it): here the pipeline is the
/// piece's own.
const ROUTING_BITS: u32 = 0xF << 24;

/// Whether pieces that use the stencil go to this module's pass (`push`) this frame.
pub fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// Hands a piece of a Bedrock model, already drawn into scene vertices (quads), to the stencil
/// pass of `target`, with its material's `state`.
pub fn push(target: Target, state: State, vertices: &[Vertex]) {
    if vertices.len() < 4 || !active() {
        return;
    }
    let t = target as usize;
    let mut lists = PENDING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if lists.vertices[t].len() + vertices.len() > MAX_VERTICES {
        lists.vertices[t].clear();
        lists.batches[t].clear();
    }
    let first = lists.vertices[t].len() as u32;
    let count = (vertices.len() / 4 * 4) as u32;
    lists.vertices[t].extend(vertices[..count as usize].iter().map(|v| {
        let mut v = *v;
        v.layer &= !ROUTING_BITS;
        v
    }));
    match lists.batches[t].last_mut() {
        Some(last) if last.state == state && last.first + last.vertices == first => {
            last.vertices += count;
        }
        _ => lists.batches[t].push(Batch {
            state,
            first,
            vertices: count,
        }),
    }
}

/// The device features the pass prefers: a 32-bit float depth with the stencil, so the copy of
/// the scene's depth is exact (else `Depth24PlusStencil8`).
pub fn features(adapter: &wgpu::Adapter) -> wgpu::Features {
    adapter.features() & wgpu::Features::DEPTH32FLOAT_STENCIL8
}

const COPY_SHADER: &str = r"
@group(0) @binding(0) var scene_depth: texture_depth_2d;

@vertex
fn vs_copy(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}

struct Copied {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};

@fragment
fn fs_copy(@builtin(position) at: vec4<f32>) -> Copied {
    var out: Copied;
    out.color = vec4<f32>(0.0);
    out.depth = textureLoad(scene_depth, vec2<i32>(at.xy), 0);
    return out;
}
";

/// The renderer's half: the pass, its pipelines and buffers.
pub struct Stencil {
    device: wgpu::Device,
    entity_shader: wgpu::ShaderModule,
    entity_layout: wgpu::PipelineLayout,
    bias: wgpu::DepthBiasState,
    /// `entity.wgsl`'s `HDR_WRITE_LIGHT`, as the scene's entity pipelines have it.
    hdr_write_light: f64,
    format: wgpu::TextureFormat,
    pipelines: HashMap<(wgpu::TextureFormat, State), wgpu::RenderPipeline>,
    copy_shader: wgpu::ShaderModule,
    copy_layout: wgpu::BindGroupLayout,
    copy_pipeline_layout: wgpu::PipelineLayout,
    copies: HashMap<wgpu::TextureFormat, wgpu::RenderPipeline>,
    /// Depth-and-stencil targets by size (the world's may be scaled, the hand's is the window's).
    targets: Vec<(wgpu::TextureView, [u32; 2])>,
    buffer: Option<(wgpu::Buffer, u64)>,
    /// This frame's pieces (`begin_frame`), the hand's after the world's in `buffer`.
    frame: Lists,
    /// The pass starts from the scene's depth (else from an empty one).
    copy_depth: bool,
    /// When each target last logged what it drew (`log`).
    logged: [Option<std::time::Instant>; 2],
}

impl Stencil {
    pub fn new(
        device: &wgpu::Device,
        entity_shader: &wgpu::ShaderModule,
        entity_layout: &wgpu::PipelineLayout,
        bias: wgpu::DepthBiasState,
        hdr_write_light: f64,
    ) -> Self {
        let format = if device.features().contains(wgpu::Features::DEPTH32FLOAT_STENCIL8) {
            wgpu::TextureFormat::Depth32FloatStencil8
        } else {
            wgpu::TextureFormat::Depth24PlusStencil8
        };
        let copy_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Bedrock stencil depth copy"),
            source: wgpu::ShaderSource::Wgsl(COPY_SHADER.into()),
        });
        let copy_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Bedrock stencil depth copy"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let copy_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Bedrock stencil depth copy"),
            bind_group_layouts: &[Some(&copy_layout)],
            ..Default::default()
        });
        ACTIVE.store(true, Ordering::Relaxed);
        Self {
            device: device.clone(),
            entity_shader: entity_shader.clone(),
            entity_layout: entity_layout.clone(),
            bias,
            hdr_write_light,
            format,
            pipelines: HashMap::new(),
            copy_shader,
            copy_layout,
            copy_pipeline_layout,
            copies: HashMap::new(),
            targets: Vec::new(),
            buffer: None,
            frame: Lists::default(),
            copy_depth: true,
            logged: [None; 2],
        }
    }

    /// Whether the mob renderer hands pieces over (`active`): off while a shader pack draws the
    /// scene, whose passes this one does not follow.
    pub fn set_active(&self, on: bool) {
        ACTIVE.store(on, Ordering::Relaxed);
    }

    /// Takes the pieces handed over since the last frame and uploads them.
    pub fn begin_frame(&mut self, queue: &wgpu::Queue) {
        {
            let mut pending = PENDING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            std::mem::swap(&mut self.frame, &mut *pending);
            for t in 0..2 {
                pending.vertices[t].clear();
                pending.batches[t].clear();
            }
        }
        for t in 0..2 {
            Self::prune(&mut self.frame, t);
        }
        let total = self.frame.vertices[0].len() + self.frame.vertices[1].len();
        if total == 0 {
            return;
        }
        let size = (total * std::mem::size_of::<Vertex>()) as u64;
        if self.buffer.as_ref().is_none_or(|(_, cap)| *cap < size) {
            let cap = size.next_power_of_two();
            self.buffer = Some((
                self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Bedrock stencil vertices"),
                    size: cap,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                cap,
            ));
        }
        let Some((buffer, _)) = &self.buffer else {
            return;
        };
        queue.write_buffer(buffer, 0, bytemuck::cast_slice(&self.frame.vertices[0]));
        if !self.frame.vertices[1].is_empty() {
            queue.write_buffer(
                buffer,
                (self.frame.vertices[0].len() * std::mem::size_of::<Vertex>()) as u64,
                bytemuck::cast_slice(&self.frame.vertices[1]),
            );
        }
    }

    /// Keeps what can change a pixel: nothing when no piece of the target tests the stencil
    /// (its pass, the world's depth kept for it and the hand drawn apart all cost a frame for
    /// nothing), and of the pieces that only write it, those whose value a test looks for.
    fn prune(frame: &mut Lists, t: usize) {
        let tests: Vec<(u8, u8)> = frame.batches[t]
            .iter()
            .filter(|b| b.state.compare != Compare::Always)
            .map(|b| (b.state.reference & b.state.read_mask, b.state.read_mask))
            .collect();
        let read = |b: &Batch| {
            b.state.compare != Compare::Always
                || b.state.color
                || (b.state.replace
                    && tests
                        .iter()
                        .any(|(r, mask)| b.state.reference & mask == *r))
        };
        if tests.is_empty() {
            frame.batches[t].clear();
        } else if !frame.batches[t].iter().all(read) {
            frame.batches[t].retain(read);
        }
        if frame.batches[t].is_empty() {
            frame.vertices[t].clear();
            return;
        }
        // The kept pieces' vertices, packed.
        let mut vertices = Vec::with_capacity(frame.vertices[t].len());
        for b in &mut frame.batches[t] {
            let from = b.first as usize;
            b.first = vertices.len() as u32;
            vertices.extend_from_slice(&frame.vertices[t][from..from + b.vertices as usize]);
        }
        frame.vertices[t] = vertices;
    }

    pub fn has_world(&self) -> bool {
        !self.frame.batches[Target::World as usize].is_empty()
    }

    pub fn has_hand(&self) -> bool {
        !self.frame.batches[Target::Hand as usize].is_empty()
    }

    /// The world's pieces, over the world drawn into `color` with `depth`, transformed as the
    /// world's entities (`group`, the entity bindings).
    pub fn draw_world(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        group: &wgpu::BindGroup,
        indices: &wgpu::Buffer,
    ) {
        self.draw(Target::World, encoder, color, depth, group, indices);
    }

    /// The first-person hand's pieces, over the hand drawn into `color` with `depth` (stored by
    /// the hand's pass), transformed as the hand (`group`, the hand's bindings).
    pub fn draw_hand(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        group: &wgpu::BindGroup,
        indices: &wgpu::Buffer,
    ) {
        self.draw(Target::Hand, encoder, color, depth, group, indices);
    }

    /// The hand's pieces when nothing else of the hand drew (no arm, no other item): from an
    /// empty depth, as the hand's own pass starts.
    pub fn draw_hand_alone(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        group: &wgpu::BindGroup,
        indices: &wgpu::Buffer,
    ) {
        self.copy_depth = false;
        self.draw(Target::Hand, encoder, color, depth, group, indices);
        self.copy_depth = true;
    }

    fn draw(
        &mut self,
        target: Target,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        group: &wgpu::BindGroup,
        indices: &wgpu::Buffer,
    ) {
        let t = target as usize;
        if self.frame.batches[t].is_empty() || self.buffer.is_none() {
            return;
        }
        let color_texture = color.texture();
        let depth_texture = depth.texture();
        let size = [color_texture.width(), color_texture.height()];
        // The scene's depth is read to start from it; the two must cover the same pixels.
        let skipped = if size != [depth_texture.width(), depth_texture.height()] {
            Some("the scene's depth is another size")
        } else if !depth_texture.usage().contains(wgpu::TextureUsages::TEXTURE_BINDING) {
            Some("the scene's depth cannot be read")
        } else if depth_texture.format() != wgpu::TextureFormat::Depth32Float {
            Some("the scene's depth is not Depth32Float")
        } else {
            None
        };
        self.log(target, skipped, size, depth_texture.size());
        if skipped.is_some() {
            return;
        }
        let format = color_texture.format();
        self.ensure_target(size);
        let states: Vec<State> = self.frame.batches[t].iter().map(|b| b.state).collect();
        for state in states {
            self.ensure_pipeline(format, state);
        }
        self.ensure_copy(format);
        let copy_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bedrock stencil depth copy"),
            layout: &self.copy_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(depth),
            }],
        });
        let (Some((ds, _)), Some((buffer, _))) =
            (self.targets.iter().find(|(_, s)| *s == size), &self.buffer)
        else {
            return;
        };
        let attachments = [Some(wgpu::RenderPassColorAttachment {
            view: color,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })];
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(match target {
                Target::World => "Bedrock stencil (world)",
                Target::Hand => "Bedrock stencil (hand)",
            }),
            color_attachments: &attachments,
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: ds,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0),
                    store: wgpu::StoreOp::Discard,
                }),
            }),
            ..Default::default()
        });
        // 1. The scene's depth.
        if self.copy_depth {
            pass.set_pipeline(&self.copies[&format]);
            pass.set_bind_group(0, &copy_group, &[]);
            pass.draw(0..3, 0..1);
        }
        // 2. The pieces, in the order the model drew them.
        let base = if target == Target::Hand {
            self.frame.vertices[0].len() as i32
        } else {
            0
        };
        pass.set_bind_group(0, group, &[]);
        pass.set_vertex_buffer(0, buffer.slice(..));
        pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
        for batch in &self.frame.batches[t] {
            pass.set_pipeline(&self.pipelines[&(format, batch.state)]);
            pass.set_stencil_reference(u32::from(batch.state.reference));
            pass.draw_indexed(
                0..batch.vertices / 4 * 6,
                base + batch.first as i32,
                0..1,
            );
        }
    }

    /// Once every two seconds a target draws: what it drew, or why it did not.
    fn log(
        &mut self,
        target: Target,
        skipped: Option<&str>,
        size: [u32; 2],
        depth: wgpu::Extent3d,
    ) {
        let t = target as usize;
        let now = std::time::Instant::now();
        if self.logged[t].is_some_and(|at| now.duration_since(at).as_secs_f32() < 2.0) {
            return;
        }
        self.logged[t] = Some(now);
        let batches = &self.frame.batches[t];
        let states: Vec<String> = batches
            .iter()
            .map(|b| {
                let s = b.state;
                format!(
                    "{}{:?} ref {} mask {:#x}{} {}q",
                    if s.color { "" } else { "nocolor " },
                    s.compare,
                    s.reference,
                    s.read_mask,
                    if s.replace { " replace" } else { "" },
                    b.vertices / 4
                )
            })
            .collect();
        log::info!(
            "bedrock-stencil {:?}: {} batch(es) [{}] into {}x{} (depth {}x{}, {:?}){}",
            target,
            batches.len(),
            states.join(", "),
            size[0],
            size[1],
            depth.width,
            depth.height,
            self.format,
            skipped.map_or(String::new(), |why| format!(": skipped, {why}")),
        );
    }

    fn ensure_target(&mut self, size: [u32; 2]) {
        if self.targets.iter().any(|(_, s)| *s == size) {
            return;
        }
        if self.targets.len() >= 2 {
            self.targets.remove(0);
        }
        let view = self
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Bedrock stencil depth"),
                size: wgpu::Extent3d {
                    width: size[0],
                    height: size[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&Default::default());
        self.targets.push((view, size));
    }

    fn ensure_copy(&mut self, format: wgpu::TextureFormat) {
        if self.copies.contains_key(&format) {
            return;
        }
        let pipeline = self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Bedrock stencil depth copy"),
            layout: Some(&self.copy_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &self.copy_shader,
                entry_point: Some("vs_copy"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: self.format,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &self.copy_shader,
                entry_point: Some("fs_copy"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::empty(),
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        self.copies.insert(format, pipeline);
    }

    fn ensure_pipeline(&mut self, format: wgpu::TextureFormat, state: State) {
        if self.pipelines.contains_key(&(format, state)) {
            return;
        }
        let attributes = wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x2,2=>Uint32,3=>Unorm8x4,4=>Uint8x4,5=>Snorm8x4,6=>Float32x2];
        let buffers = [Some(wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &attributes,
        })];
        let face = wgpu::StencilFaceState {
            compare: state.compare.wgpu(),
            fail_op: wgpu::StencilOperation::Keep,
            depth_fail_op: wgpu::StencilOperation::Keep,
            pass_op: if state.replace {
                wgpu::StencilOperation::Replace
            } else {
                wgpu::StencilOperation::Keep
            },
        };
        let blend = if state.additive {
            wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent::OVER,
            }
        } else {
            wgpu::BlendState::ALPHA_BLENDING
        };
        let constants = [("HDR_WRITE_LIGHT", self.hdr_write_light)];
        let pipeline = self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Bedrock stencil piece"),
            layout: Some(&self.entity_layout),
            vertex: wgpu::VertexState {
                module: &self.entity_shader,
                entry_point: Some("vs_entity"),
                compilation_options: Default::default(),
                buffers: &buffers,
            },
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: self.format,
                depth_write_enabled: Some(state.depth_write),
                depth_compare: Some(if state.depth_test {
                    wgpu::CompareFunction::GreaterEqual
                } else {
                    wgpu::CompareFunction::Always
                }),
                stencil: wgpu::StencilState {
                    front: face,
                    back: face,
                    read_mask: u32::from(state.read_mask),
                    write_mask: if state.replace {
                        u32::from(state.write_mask)
                    } else {
                        0
                    },
                },
                bias: replay_bias(self.bias, state),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &self.entity_shader,
                entry_point: Some(if state.cutout { "fs_entity" } else { "fs_entity_uncut" }),
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &constants,
                    ..Default::default()
                },
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(blend),
                    write_mask: if state.color {
                        wgpu::ColorWrites::ALL
                    } else {
                        wgpu::ColorWrites::empty()
                    },
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        self.pipelines.insert((format, state), pipeline);
    }
}

/// The depth bias of a piece drawn with `state`. A piece the scene already drew, drawn again
/// here only for the stencil it writes (no colour), meets its own depth in the scene's copy: the
/// two depths are the same surface computed twice, and where they come out a few ulps apart the
/// write fails, leaving the piece unmarked at some angles of the camera (an armour then covered
/// by the outline that should show only past its edge). Pulled a little nearer, with the slope,
/// it always reaches its own surface.
fn replay_bias(bias: wgpu::DepthBiasState, state: State) -> wgpu::DepthBiasState {
    if state.color || !state.replace || !state.depth_test {
        return bias;
    }
    wgpu::DepthBiasState {
        constant: bias.constant.saturating_mul(4).max(512),
        slope_scale: bias.slope_scale.max(1.0),
        clamp: bias.clamp,
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_stencil_only_replay_is_pulled_nearer_than_its_own_surface() {
        let base = wgpu::DepthBiasState { constant: 128, slope_scale: 0.0, clamp: 0.0 };
        let mut state = State {
            color: false,
            additive: false,
            depth_write: true,
            depth_test: true,
            cutout: true,
            compare: Compare::Always,
            replace: true,
            reference: 2,
            read_mask: 255,
            write_mask: 255,
        };
        let b = replay_bias(base, state);
        assert!(b.constant > base.constant && b.slope_scale > 0.0);
        // What draws colour, or only tests, keeps the scene's bias.
        state.color = true;
        assert_eq!(replay_bias(base, state), base);
        state.color = false;
        state.replace = false;
        assert_eq!(replay_bias(base, state), base);
    }
    use super::*;

    fn quad(layer: u32) -> [Vertex; 4] {
        [Vertex {
            pos: [0.0; 3],
            uv: [0.0; 2],
            layer,
            color: 0,
            light: 0,
            normal: [0; 4],
            glint_uv: [0.0; 2],
        }; 4]
    }

    #[test]
    fn pieces_of_one_state_join_and_lose_their_routing_bits() {
        ACTIVE.store(true, Ordering::Relaxed);
        let state = State {
            color: false,
            additive: false,
            depth_write: true,
            depth_test: true,
            cutout: true,
            compare: Compare::Always,
            replace: true,
            reference: 3,
            read_mask: 255,
            write_mask: 255,
        };
        push(Target::Hand, state, &quad(5 | 1 << 24));
        push(Target::Hand, state, &quad(5));
        push(Target::Hand, State { color: true, compare: Compare::Equal, replace: false, ..state }, &quad(5));
        let lists = PENDING.lock().unwrap();
        let hand = Target::Hand as usize;
        assert_eq!(lists.batches[hand].len(), 2);
        assert_eq!(lists.batches[hand][0].vertices, 8);
        assert!(lists.vertices[hand].iter().all(|v| v.layer == 5));
    }
}
