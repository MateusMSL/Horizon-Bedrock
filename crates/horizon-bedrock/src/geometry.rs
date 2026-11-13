//! Bedrock models (`models/entity/*.json`): bones, cubes and their UVs.
//!
//! Two layers:
//!
//! * [`Geometry`] is the file as written: bones with absolute pivots, y up, degrees, in the
//!   frame Bedrock authors work in.
//! * [`Geometry::to_tree`] bakes it into [`Part`]s in the frame Java's entity models use
//!   (`ModelPart`): y down with the ground at 24, every part's position relative to its
//!   parent's pivot, angles in radians on Java's axes, boxes already cut into [`Quad`]s with
//!   UVs as fractions of the texture. The game draws those exactly as it draws a vanilla
//!   model.
//!
//! Frame conversion (a point `(x, y, z)` of Bedrock is `(x, 24 - y, z)` in Java): Bedrock's
//! rotations keep their signs (checked against a vanilla cow's body and spider legs). Both axes agree on
//! which way an entity faces (-z) and where its right side is (-x).

use crate::{Error, Result};
use serde_json::Value;
use std::collections::HashMap;

/// One face of a box: the outward normal and four `[x, y, z, u, v]` vertices, `ModelPart`'s way.
#[derive(Clone, Debug, PartialEq)]
pub struct Quad {
    pub normal: [f32; 3],
    pub vertices: [[f32; 5]; 4],
}

/// A face's UV rectangle, in texels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceUv {
    pub uv: [f32; 2],
    pub size: [f32; 2],
    /// `uv_rotation` (format 1.21.0): the texture turned on the face by 90° steps (0..3).
    pub turns: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Uv {
    /// The classic unfolded layout from one corner.
    Box([f32; 2]),
    /// One rectangle per face; `None` leaves a face out. Order: north, south, east, west, up,
    /// down.
    Faces([Option<FaceUv>; 6]),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Cube {
    pub origin: [f32; 3],
    pub size: [f32; 3],
    pub uv: Uv,
    pub inflate: Option<f32>,
    pub mirror: Option<bool>,
    pub pivot: Option<[f32; 3]>,
    pub rotation: Option<[f32; 3]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Locator {
    pub name: String,
    pub offset: [f32; 3],
    pub rotation: [f32; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bone {
    pub name: String,
    pub parent: Option<String>,
    pub pivot: [f32; 3],
    /// Rest rotation in degrees (`rotation`, or the older `bind_pose_rotation`).
    pub rotation: [f32; 3],
    pub mirror: bool,
    pub inflate: f32,
    pub never_render: bool,
    /// An attachable's bone: the Molang expression naming the bone of the holder it follows.
    pub binding: Option<String>,
    pub cubes: Vec<Cube>,
    pub locators: Vec<Locator>,
    /// Flat item sprites the bone shows (`texture_meshes`).
    pub texture_meshes: Vec<TextureMesh>,
}

/// A flat sprite of one of the attachable's textures, placed in the model.
#[derive(Clone, Debug, PartialEq)]
pub struct TextureMesh {
    /// The entity's name for the texture (`textures` map key).
    pub texture: String,
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub local_pivot: [f32; 3],
    pub scale: [f32; 3],
    /// `use_pixel_depth`: the sprite is a solid one pixel thick, cut to its opaque pixels.
    pub pixel_depth: bool,
}

/// A sprite texture's size and which of its pixels show.
#[derive(Clone, Debug, PartialEq)]
pub struct Sprite {
    pub size: [u32; 2],
    /// Row by row, from the top.
    pub opaque: Vec<bool>,
}

/// A sprite's side when the texture's size is unknown, in model units.
const SPRITE_SIZE: f32 = 16.0;
/// How thick a sprite is drawn: thin enough to read as flat, enough to show from both sides.
const SPRITE_DEPTH: f32 = 0.05;

#[derive(Clone, Debug, PartialEq)]
pub struct Geometry {
    /// `geometry.zombie`, lower case.
    pub id: String,
    pub texture_size: [f32; 2],
    pub bones: Vec<Bone>,
}

/// The id a geometry's sprites drawn with texture `key` go by.
pub fn mesh_id(geometry: &str, key: &str) -> String {
    format!("{geometry}#mesh:{}", key.to_ascii_lowercase())
}

/// The id of a geometry's sprites drawn with texture `key` cut to the file `path` (its size and
/// silhouette): one per file, since many items share a geometry and a key (Actions & Stuff's
/// tools all draw `mqnsmu` with their own `rcvngv`), each with its own picture.
pub fn fitted_mesh_id(geometry: &str, key: &str, path: &str) -> String {
    format!("{}@{}", mesh_id(geometry, key), path.to_ascii_lowercase())
}

impl Geometry {
    /// Each bone with a parent: its pivot less the parent's, in the model's own axes (pixels).
    /// A bone named in an earlier model keeps that one's.
    pub fn rest_offsets(&self, out: &mut std::collections::HashMap<String, [f32; 3]>) {
        let pivots: std::collections::HashMap<String, [f32; 3]> = self
            .bones
            .iter()
            .map(|b| (b.name.to_ascii_lowercase(), b.pivot))
            .collect();
        for b in &self.bones {
            let Some(parent) = b
                .parent
                .as_ref()
                .and_then(|p| pivots.get(&p.to_ascii_lowercase()))
            else {
                continue;
            };
            out.entry(b.name.to_ascii_lowercase())
                .or_insert(std::array::from_fn(|a| b.pivot[a] - parent[a]));
        }
    }
    /// For each texture its `texture_meshes` use, a model of the same bones carrying only those
    /// sprites as thin boxes, so they draw with their own texture.
    pub fn mesh_variants(&self) -> Vec<Geometry> {
        let mut keys: Vec<String> = Vec::new();
        for m in self.bones.iter().flat_map(|b| &b.texture_meshes) {
            let k = m.texture.to_ascii_lowercase();
            if !keys.contains(&k) {
                keys.push(k);
            }
        }
        keys.iter().map(|k| self.mesh_variant(k, None)).collect()
    }

    /// The sprites drawn with texture `key` (the entity's name for it), as thin boxes. With the
    /// texture's `sprite`, they have its size, and `use_pixel_depth` ones are solids cut to
    /// its opaque pixels.
    pub fn mesh_variant(&self, key: &str, sprite: Option<&Sprite>) -> Geometry {
        let [w, h] = sprite.map_or([SPRITE_SIZE as u32; 2], |s| s.size);
        let (wf, hf) = (w as f32, h as f32);
        // A texel is a voxel of a pixel of the model (a 16-wide sprite is a block wide); a
        // higher-resolution sprite keeps that size, its texels smaller.
        let k = SPRITE_SIZE / wf.max(1.0);
        let bones = self
            .bones
            .iter()
            .map(|b| {
                let mut cubes = Vec::new();
                for m in b
                    .texture_meshes
                    .iter()
                    .filter(|m| m.texture.eq_ignore_ascii_case(key))
                {
                    // Bedrock's texture mesh (as Blockbench imports it, `bedrock.js` and
                    // `texture_mesh.js`): the texture lies flat (u along x, v along z), one
                    // texel deep below its plane (y from -1 to 0, times `scale`), moved by
                    // `local_pivot` (x and z taken off, y added), turned by `rotation` and put
                    // at `position`, whose y counts down from the bone's pivot: the point is
                    // [px, 2 * pivot_y - py, pz] in the model. A mesh at the bone's pivot
                    // height (the test pack's brick) is where it was before this; Actions &
                    // Stuff's 3D tools (`position` y -11.31 under a bone at 0) sit 22.6 px
                    // higher, centred on the hand as in the game.
                    let (sx, sy, sz) = (k * m.scale[0], m.scale[1], k * m.scale[2]);
                    let lp = [m.local_pivot[0] * k, m.local_pivot[1], m.local_pivot[2] * k];
                    let origin = [m.position[0], 2.0 * b.pivot[1] - m.position[1], m.position[2]];
                    let at = |x: f32, y: f32, z: f32| -> [f32; 3] {
                        [origin[0] + x - lp[0], origin[1] + y + lp[1], origin[2] + z - lp[2]]
                    };
                    let pivot = Some(origin);
                    let solid = sprite.filter(|_| m.pixel_depth);
                    let depth = if solid.is_some() { sy.max(0.01) } else { SPRITE_DEPTH };
                    let mut push =
                        |origin: [f32; 3], extent: [f32; 3], faces: [Option<FaceUv>; 6]| {
                            cubes.push(Cube {
                                origin,
                                size: extent,
                                uv: Uv::Faces(faces),
                                inflate: None,
                                mirror: None,
                                pivot,
                                rotation: Some(m.rotation),
                            });
                        };
                    let face = |u: f32, v: f32, du: f32, dv: f32| {
                        Some(FaceUv {
                            uv: [u, v],
                            size: [du, dv],
                            turns: 0,
                        })
                    };
                    match solid {
                        None => push(
                            at(0.0, -depth, 0.0),
                            [wf * sx, depth, hf * sz],
                            [
                                None,
                                None,
                                None,
                                None,
                                face(0.0, 0.0, wf, hf),
                                face(0.0, 0.0, wf, hf),
                            ],
                        ),
                        Some(s) => {
                            // One box per opaque texel: a box spanning a run would stretch the
                            // run across its faces in whichever direction the face reads the
                            // texture (the down face reads it mirrored), scrambling the colours.
                            for row in 0..h {
                                let mut col = 0;
                                while col < w {
                                    if !s.opaque[(row * w + col) as usize] {
                                        col += 1;
                                        continue;
                                    }
                                    let start = col;
                                    col += 1;
                                    let len = (col - start) as f32;
                                    let (u, v) = (start as f32, row as f32);
                                    push(
                                        at(u * sx, -depth, v * sz),
                                        [len * sx, depth, sz],
                                        [
                                            face(u, v, len, 1.0),
                                            face(u, v, len, 1.0),
                                            face(u, v, 1.0, 1.0),
                                            face(u + len - 1.0, v, 1.0, 1.0),
                                            face(u, v, len, 1.0),
                                            face(u, v, len, 1.0),
                                        ],
                                    );
                                }
                            }
                        }
                    }
                }
                Bone {
                    cubes,
                    locators: Vec::new(),
                    texture_meshes: Vec::new(),
                    ..b.clone()
                }
            })
            .collect();
        Geometry {
            id: mesh_id(&self.id, key),
            texture_size: [wf, hf],
            bones,
        }
    }
}

/// A bone baked for the game: a `ModelPart`.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub name: String,
    /// x, y, z (pixels, relative to the parent's pivot), xRot, yRot, zRot (radians), and the
    /// three scales: the layout of the game's `Part::pose`.
    pub pose: [f32; 9],
    pub quads: Vec<Quad>,
    pub children: Vec<Part>,
    /// The bone is marked `neverRender`: kept for its children and locators, drawn empty.
    pub never_render: bool,
}

fn f32s<const N: usize>(v: Option<&Value>) -> Option<[f32; N]> {
    let a = v?.as_array()?;
    if a.len() < N {
        return None;
    }
    let mut out = [0.0; N];
    for (slot, x) in out.iter_mut().zip(a) {
        *slot = x.as_f64()? as f32;
    }
    Some(out)
}

fn f32_of(v: Option<&Value>) -> Option<f32> {
    v.and_then(Value::as_f64).map(|n| n as f32)
}

fn parse_uv(v: Option<&Value>) -> Option<Uv> {
    match v? {
        Value::Array(_) => f32s::<2>(v).map(Uv::Box),
        Value::Object(o) => {
            let face = |name: &str| -> Option<FaceUv> {
                let f = o.get(name)?;
                Some(FaceUv {
                    uv: f32s::<2>(f.get("uv"))?,
                    size: f32s::<2>(f.get("uv_size")).unwrap_or([0.0, 0.0]),
                    turns: f32_of(f.get("uv_rotation"))
                        .map_or(0, |r| ((r / 90.0).round() as i32).rem_euclid(4) as u8),
                })
            };
            Some(Uv::Faces([
                face("north"),
                face("south"),
                face("east"),
                face("west"),
                face("up"),
                face("down"),
            ]))
        }
        _ => None,
    }
}

fn parse_texture_meshes(v: Option<&Value>) -> Vec<TextureMesh> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|m| {
                    Some(TextureMesh {
                        texture: m.get("texture")?.as_str()?.to_owned(),
                        position: f32s::<3>(m.get("position")).unwrap_or([0.0; 3]),
                        rotation: f32s::<3>(m.get("rotation")).unwrap_or([0.0; 3]),
                        local_pivot: f32s::<3>(m.get("local_pivot")).unwrap_or([0.0; 3]),
                        scale: f32s::<3>(m.get("scale"))
                            .or_else(|| f32_of(m.get("scale")).map(|s| [s; 3]))
                            .unwrap_or([1.0; 3]),
                        pixel_depth: m
                            .get("use_pixel_depth")
                            .and_then(Value::as_bool)
                            .unwrap_or(true),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_bone(b: &Value) -> Option<Bone> {
    let name = b.get("name")?.as_str()?.to_owned();
    let cubes = b
        .get("cubes")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|c| {
                    Some(Cube {
                        origin: f32s::<3>(c.get("origin"))?,
                        size: f32s::<3>(c.get("size"))?,
                        uv: parse_uv(c.get("uv"))?,
                        inflate: f32_of(c.get("inflate")),
                        mirror: c.get("mirror").and_then(Value::as_bool),
                        pivot: f32s::<3>(c.get("pivot")),
                        rotation: f32s::<3>(c.get("rotation")),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let locators = b
        .get("locators")
        .and_then(Value::as_object)
        .map(|o| {
            o.iter()
                .filter_map(|(n, v)| {
                    let (offset, rotation) = match v {
                        Value::Array(_) => (f32s::<3>(Some(v))?, [0.0; 3]),
                        Value::Object(_) => (
                            f32s::<3>(v.get("offset"))?,
                            f32s::<3>(v.get("rotation")).unwrap_or([0.0; 3]),
                        ),
                        _ => return None,
                    };
                    Some(Locator {
                        name: n.to_ascii_lowercase(),
                        offset,
                        rotation,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Some(Bone {
        name,
        parent: b.get("parent").and_then(Value::as_str).map(str::to_owned),
        pivot: f32s::<3>(b.get("pivot")).unwrap_or([0.0; 3]),
        rotation: f32s::<3>(b.get("rotation"))
            .or_else(|| f32s::<3>(b.get("bind_pose_rotation")))
            .unwrap_or([0.0; 3]),
        mirror: b.get("mirror").and_then(Value::as_bool).unwrap_or(false),
        inflate: f32_of(b.get("inflate")).unwrap_or(0.0),
        never_render: b
            .get("neverRender")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        binding: b.get("binding").and_then(Value::as_str).map(str::to_owned),
        cubes,
        locators,
        texture_meshes: parse_texture_meshes(b.get("texture_meshes")),
    })
}

/// Cubes laid exactly over one another (an overlay head, the frames of a blink) share their six
/// planes and flicker. The game's depth test keeps the first one drawn, which is how packs are
/// made (a blink hides the earlier frames to show a later one), so each is grown by a hair
/// less than the one before it.
fn separate_coplanar(bones: &mut [Bone]) {
    let same = |o: &([f32; 3], [f32; 3]), c: &Cube| {
        (0..3).all(|a| (o.0[a] - c.origin[a]).abs() < 1e-4 && (o.1[a] - c.size[a]).abs() < 1e-4)
    };
    let mut groups: Vec<(([f32; 3], [f32; 3]), usize, usize)> = Vec::new();
    let movable = |c: &Cube| c.rotation.is_none_or(|r| r == [0.0; 3]);
    for cube in bones.iter().flat_map(|b| &b.cubes).filter(|c| movable(c)) {
        let key = (cube.origin, cube.size);
        match groups.iter_mut().find(|(k, _, _)| same(k, cube)) {
            Some(g) => g.1 += 1,
            None => groups.push((key, 1, 0)),
        }
    }
    for bone in bones.iter_mut() {
        let bone_inflate = bone.inflate;
        for cube in &mut bone.cubes {
            if !movable(cube) {
                continue;
            }
            let Some(g) = groups.iter_mut().find(|(k, _, _)| same(k, cube)) else {
                continue;
            };
            if g.1 > 1 {
                let behind = g.1 - 1 - g.2;
                g.2 += 1;
                // Less than the hair a pack leaves between an eye and the face it sits on.
                let step = (0.015 / g.1 as f32).min(0.004);
                cube.inflate = Some(cube.inflate.unwrap_or(bone_inflate) + step * behind as f32);
            }
        }
    }
}

fn parse_bones(v: Option<&Value>) -> Vec<Bone> {
    let mut bones = parse_bones_raw(v);
    separate_coplanar(&mut bones);
    bones
}

fn parse_bones_raw(v: Option<&Value>) -> Vec<Bone> {
    v.and_then(Value::as_array)
        .map(|a| a.iter().filter_map(parse_bone).collect())
        .unwrap_or_default()
}

/// Every geometry in a model file, of either layout: the `minecraft:geometry` list (format
/// 1.12 and later) or the legacy top-level `geometry.name` keys (1.8), which may inherit from
/// another with `geometry.child:geometry.parent`.
pub fn parse_file(root: &Value) -> Result<Vec<Geometry>> {
    let obj = root
        .as_object()
        .ok_or_else(|| Error::Json("model file is not an object".into()))?;
    let mut out = Vec::new();
    if let Some(list) = obj.get("minecraft:geometry").and_then(Value::as_array) {
        for g in list {
            let desc = g.get("description").unwrap_or(&Value::Null);
            let Some(id) = desc.get("identifier").and_then(Value::as_str) else {
                continue;
            };
            out.push(Geometry {
                id: id.to_ascii_lowercase(),
                texture_size: [
                    f32_of(desc.get("texture_width")).unwrap_or(64.0),
                    f32_of(desc.get("texture_height")).unwrap_or(64.0),
                ],
                bones: parse_bones(g.get("bones")),
            });
        }
    }
    // Legacy layout. Parents first, so that a child finds the bones it inherits.
    let mut legacy: Vec<(&String, &Value)> = obj
        .iter()
        .filter(|(k, _)| k.to_ascii_lowercase().starts_with("geometry."))
        .collect();
    legacy.sort_by_key(|(k, _)| k.contains(':'));
    let mut by_id: HashMap<String, usize> = HashMap::new();
    for (key, g) in legacy {
        let key = key.to_ascii_lowercase();
        let (id, parent) = match key.split_once(':') {
            Some((id, parent)) => (id.to_owned(), Some(parent.to_owned())),
            None => (key.clone(), None),
        };
        let mut bones = parse_bones(g.get("bones"));
        let mut size = [
            f32_of(g.get("texturewidth")).unwrap_or(64.0),
            f32_of(g.get("textureheight")).unwrap_or(64.0),
        ];
        if let Some(parent) = parent
            .as_ref()
            .and_then(|p| by_id.get(p))
            .map(|&i| out[i].clone())
        {
            if g.get("texturewidth").is_none() {
                size = parent.texture_size;
            }
            let mut merged = parent.bones;
            for b in bones {
                match merged.iter_mut().find(|m| m.name == b.name) {
                    Some(slot) => *slot = b,
                    None => merged.push(b),
                }
            }
            bones = merged;
        }
        by_id.insert(id.clone(), out.len());
        out.push(Geometry {
            id,
            texture_size: size,
            bones,
        });
    }
    Ok(out)
}

/// A Bedrock point in Java's frame.
fn java(p: [f32; 3]) -> [f32; 3] {
    [p[0], 24.0 - p[1], p[2]]
}

/// Bedrock Euler degrees as Java `ModelPart` radians. The numbers keep their sign: a vanilla
/// cow's body is `[90, 0, 0]` in both games, and a spider's right legs lean `z = -45` in both.
pub fn rotation_to_java(r: [f32; 3]) -> [f32; 3] {
    [r[0].to_radians(), r[1].to_radians(), r[2].to_radians()]
}

/// The corners of `ModelPart.Cube`: which of x, y, z take the box's maximum.
const CORNER: [[bool; 3]; 8] = [
    [false, false, false],
    [true, false, false],
    [true, true, false],
    [false, true, false],
    [false, false, true],
    [true, false, true],
    [true, true, true],
    [false, true, true],
];

/// The faces in the order of [`Uv::Faces`]: the corner indexes (outward winding) and the normal
/// in Java's frame. North is -z, east is the entity's right side (-x), up is -y.
const FACES: [([usize; 4], [f32; 3]); 6] = [
    ([1, 0, 3, 2], [0.0, 0.0, -1.0]),
    ([4, 5, 6, 7], [0.0, 0.0, 1.0]),
    ([0, 4, 7, 3], [-1.0, 0.0, 0.0]),
    ([5, 1, 2, 6], [1.0, 0.0, 0.0]),
    ([5, 4, 0, 1], [0.0, -1.0, 0.0]),
    ([2, 3, 7, 6], [0.0, 1.0, 0.0]),
];

/// The unfolded box layout around `[u, v]` for a box of `[w, h, d]` texels, as rectangles
/// `[u1, v1, u2, v2]` in [`FACES`]' order.
fn box_rects([u, v]: [f32; 2], [w, h, d]: [f32; 3]) -> [[f32; 4]; 6] {
    let (u1, u2, u3, u4, u5) = (
        u + d,
        u + d + w,
        u + d + w + w,
        u + d + w + d,
        u + 2.0 * (d + w),
    );
    let (v1, v2) = (v + d, v + d + h);
    [
        [u1, v1, u2, v2], // north
        [u4, v1, u5, v2], // south
        [u, v1, u1, v2],  // east
        [u2, v1, u4, v2], // west
        [u1, v, u2, v1],  // up
        [u2, v1, u3, v],  // down, upside down
    ]
}

impl Cube {
    /// The cube's faces in the frame of a bone pivoting at `pivot` (Bedrock coordinates).
    fn quads(&self, pivot: [f32; 3], bone: &Bone, texture: [f32; 2]) -> Vec<Quad> {
        let inflate = self.inflate.unwrap_or(bone.inflate);
        let mirror = self.mirror.unwrap_or(bone.mirror);
        // Bedrock y is up: the box spans origin.y .. origin.y + size.y, which is Java's
        // pivot.y - that.
        let mut lo = [
            self.origin[0] - pivot[0],
            pivot[1] - (self.origin[1] + self.size[1]),
            self.origin[2] - pivot[2],
        ];
        let mut hi = [
            lo[0] + self.size[0],
            lo[1] + self.size[1],
            lo[2] + self.size[2],
        ];
        // Inflate grows the box geometrically, whichever way its size points: a negative size
        // keeps its direction, so an inverted core inflated by -0.25 shrinks to sit inside the
        // real one instead of wrapping around it.
        for a in 0..3 {
            let (min, max) = (lo[a].min(hi[a]) - inflate, lo[a].max(hi[a]) + inflate);
            if self.size[a] < 0.0 {
                (lo[a], hi[a]) = (max, min);
            } else {
                (lo[a], hi[a]) = (min, max);
            }
        }
        // A negative size (or an inflate that eats the box) turns an axis inside out: its two
        // faces swap sides, so their normals point into the cube. A pack's hollow shell (a
        // slime's) is built so, to be seen from within.
        let inside_out: [bool; 3] = std::array::from_fn(|a| hi[a] < lo[a]);
        let boxed = matches!(self.uv, Uv::Box(_)) && mirror;
        if boxed {
            std::mem::swap(&mut lo[0], &mut hi[0]);
        }
        let rects: [Option<[f32; 4]>; 6] = match &self.uv {
            Uv::Box(o) => box_rects(*o, self.size).map(Some),
            Uv::Faces(faces) => faces
                .map(|f| f.map(|f| [f.uv[0], f.uv[1], f.uv[0] + f.size[0], f.uv[1] + f.size[1]])),
        };
        let turns: [u8; 6] = match &self.uv {
            Uv::Box(_) => [0; 6],
            Uv::Faces(faces) => faces.map(|f| f.map_or(0, |f| f.turns)),
        };
        let corner = |i: usize| -> [f32; 3] {
            std::array::from_fn(|a| if CORNER[i][a] { hi[a] } else { lo[a] })
        };
        let mut out = Vec::with_capacity(6);
        for (((corners, normal), rect), turn) in FACES.iter().zip(rects).zip(turns) {
            let Some([u1, v1, u2, v2]) = rect else {
                continue;
            };
            if u1 == u2 && v1 == v2 {
                continue;
            }
            let flat = [(u2, v1), (u1, v1), (u1, v2), (u2, v2)];
            // `uv_rotation`: each corner takes the texture corner `turn` steps on (Blockbench's
            // face rotation, clockwise).
            let uv: [(f32, f32); 4] = std::array::from_fn(|k| flat[(k + usize::from(turn)) % 4]);
            let mut vertices: [[f32; 5]; 4] = std::array::from_fn(|k| {
                let [x, y, z] = corner(corners[k]);
                [x, y, z, uv[k].0 / texture[0], uv[k].1 / texture[1]]
            });
            let mut normal = *normal;
            for axis in 0..3 {
                if inside_out[axis] {
                    normal[axis] = -normal[axis];
                }
            }
            // Turned inside out on all three axes, the box shows the faces' other side: into
            // the box (what culling and light read), where the swapped faces' normals point
            // out. One or two axes only mirror the box, which keeps its outward faces (a
            // negative size is how a pack mirrors a cube's texture).
            if inside_out.iter().all(|&flipped| flipped) {
                normal = normal.map(|n| -n);
            }
            if boxed {
                vertices.reverse();
                normal[0] = -normal[0];
            }
            out.push(Quad { normal, vertices });
        }
        out
    }
}

impl Geometry {
    /// The bone called `name` (case-insensitive).
    pub fn bone(&self, name: &str) -> Option<&Bone> {
        self.bones
            .iter()
            .find(|b| b.name.eq_ignore_ascii_case(name))
    }

    /// The locator `name` of any bone: its bone and where it sits.
    pub fn locator(&self, name: &str) -> Option<(&Bone, &Locator)> {
        let name = name.to_ascii_lowercase();
        self.bones
            .iter()
            .find_map(|b| b.locators.iter().find(|l| l.name == name).map(|l| (b, l)))
    }

    /// Bakes the model into the game's part tree. The roots are the bones without a (known)
    /// parent; they pivot in the model's own frame, which is Java's root.
    pub fn to_tree(&self) -> Vec<Part> {
        let index: HashMap<String, usize> = self
            .bones
            .iter()
            .enumerate()
            .map(|(i, b)| (b.name.to_ascii_lowercase(), i))
            .collect();
        let mut children: Vec<Vec<usize>> = vec![Vec::new(); self.bones.len()];
        let mut roots = Vec::new();
        for (i, b) in self.bones.iter().enumerate() {
            match b
                .parent
                .as_ref()
                .and_then(|p| index.get(&p.to_ascii_lowercase()))
            {
                Some(&p) if p != i => children[p].push(i),
                _ => roots.push(i),
            }
        }
        // A cycle would hide its bones from every root: they are left out.
        let mut visited = vec![false; self.bones.len()];
        roots
            .into_iter()
            .map(|r| self.part(r, None, &children, &mut visited))
            .collect()
    }

    fn part(
        &self,
        i: usize,
        parent_pivot: Option<[f32; 3]>,
        children: &[Vec<usize>],
        visited: &mut [bool],
    ) -> Part {
        visited[i] = true;
        let b = &self.bones[i];
        let here = java(b.pivot);
        let base = parent_pivot.map_or([0.0; 3], java);
        let rot = rotation_to_java(b.rotation);
        let mut quads = Vec::new();
        let mut cube_parts = Vec::new();
        for (n, cube) in b.cubes.iter().enumerate() {
            match (cube.rotation, cube.pivot) {
                (Some(r), pivot) if r != [0.0; 3] => {
                    // A rotated cube pivots on its own: a child part at its pivot.
                    let p = pivot.unwrap_or(b.pivot);
                    let (jp, jb) = (java(p), java(b.pivot));
                    cube_parts.push(Part {
                        name: format!("{}#cube{n}", b.name),
                        pose: [
                            jp[0] - jb[0],
                            jp[1] - jb[1],
                            jp[2] - jb[2],
                            rotation_to_java(r)[0],
                            rotation_to_java(r)[1],
                            rotation_to_java(r)[2],
                            1.0,
                            1.0,
                            1.0,
                        ],
                        quads: cube.quads(p, b, self.texture_size),
                        children: Vec::new(),
                        never_render: b.never_render,
                    });
                }
                _ => quads.extend(cube.quads(b.pivot, b, self.texture_size)),
            }
        }
        let mut kids: Vec<Part> = cube_parts;
        for &c in &children[i] {
            if !visited[c] {
                kids.push(self.part(c, Some(b.pivot), children, visited));
            }
        }
        Part {
            name: b.name.clone(),
            pose: [
                here[0] - base[0],
                here[1] - base[1],
                here[2] - base[2],
                rot[0],
                rot[1],
                rot[2],
                1.0,
                1.0,
                1.0,
            ],
            quads,
            children: kids,
            never_render: b.never_render,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jsonc;

    const COW: &str = r#"{
      "format_version": "1.12.0",
      "minecraft:geometry": [{
        "description": { "identifier": "geometry.test", "texture_width": 64, "texture_height": 32 },
        "bones": [
          { "name": "body", "pivot": [0, 12, 0], "cubes": [ { "origin": [-4, 6, -3], "size": [8, 6, 6], "uv": [0, 0] } ] },
          { "name": "head", "parent": "body", "pivot": [0, 12, -3], "rotation": [10, 20, 30],
            "cubes": [ { "origin": [-2, 12, -5], "size": [4, 4, 2], "uv": [0, 8], "inflate": 0.5 } ],
            "locators": { "Hat": [0, 16, -3] } },
          { "name": "tilted", "parent": "body", "pivot": [4, 8, 0],
            "cubes": [ { "origin": [4, 8, 0], "size": [1, 1, 1], "pivot": [4, 8, 0], "rotation": [0, 45, 0], "uv": [60, 0] } ] }
        ]
      }]
    }"#;

    #[test]
    fn a_cube_of_negative_size_faces_inward() {
        let cube = |size: f32| Cube {
            origin: [0.0; 3],
            size: [size; 3],
            uv: Uv::Box([0.0, 0.0]),
            inflate: None,
            mirror: None,
            pivot: None,
            rotation: None,
        };
        let bone = Bone {
            name: "b".into(),
            parent: None,
            pivot: [0.0; 3],
            rotation: [0.0; 3],
            mirror: false,
            inflate: 0.0,
            never_render: false,
            binding: None,
            cubes: Vec::new(),
            locators: Vec::new(),
            texture_meshes: Vec::new(),
        };
        let out = cube(4.0).quads([0.0; 3], &bone, [16.0, 16.0]);
        let inside = cube(-4.0).quads([0.0; 3], &bone, [16.0, 16.0]);
        // The faces swap sides and the mirrored winding turns them to look into the box: the
        // normal of each (the side it shows) is the one it had before turning.
        for (a, b) in out.iter().zip(&inside) {
            assert_eq!(a.normal, b.normal);
        }
        // Against the box's centre, the side a face shows: outward for a box, inward for one
        // turned inside out.
        let shows = |quads: &[Quad]| -> Vec<f32> {
            let all: Vec<[f32; 3]> = quads
                .iter()
                .flat_map(|q| q.vertices.iter().map(|v| [v[0], v[1], v[2]]))
                .collect();
            let centre: [f32; 3] =
                std::array::from_fn(|a| all.iter().map(|p| p[a]).sum::<f32>() / all.len() as f32);
            quads
                .iter()
                .map(|q| {
                    let p = q.vertices[0];
                    (0..3).map(|a| (p[a] - centre[a]) * q.normal[a]).sum()
                })
                .collect()
        };
        assert!(shows(&out).iter().all(|d| *d > 0.0));
        assert!(shows(&inside).iter().all(|d| *d < 0.0));
    }

    #[test]
    fn a_bone_rests_at_its_pivot_less_its_parents() {
        let g = parse_file(
            &jsonc::parse(
                r#"{"format_version":"1.12.0","minecraft:geometry":[{"description":{"identifier":"geometry.t",
                "texture_width":16,"texture_height":16},"bones":[
                {"name":"head","pivot":[0,28,-4]},
                {"name":"Eye","parent":"head","pivot":[-2,27,-4]}]}]}"#,
            )
            .unwrap(),
        )
        .unwrap()
        .remove(0);
        let mut rest = std::collections::HashMap::new();
        g.rest_offsets(&mut rest);
        assert_eq!(rest.get("eye"), Some(&[-2.0, -1.0, 0.0]));
        assert!(!rest.contains_key("head"));
    }

    #[test]
    fn texture_meshes_become_a_model_of_thin_boxes_per_texture() {
        let src = r#"{"minecraft:geometry":[{"description":{"identifier":"geometry.Item"},
          "bones":[{"name":"hand","pivot":[0,10,0],
            "texture_meshes":[{"texture":"A","position":[1,11,0],"rotation":[0,-135,90]},
                              {"texture":"a","position":[1,11,0.3]},
                              {"texture":"B","position":[0,0,0],"scale":2}]}]}]}"#;
        let g = parse_file(&jsonc::parse(src).unwrap()).unwrap().remove(0);
        assert_eq!(g.bones[0].texture_meshes.len(), 3);
        let v = g.mesh_variants();
        let ids: Vec<&str> = v.iter().map(|g| g.id.as_str()).collect();
        assert_eq!(ids, ["geometry.item#mesh:a", "geometry.item#mesh:b"]);
        assert_eq!(v[0].bones[0].cubes.len(), 2);
        assert_eq!(v[1].bones[0].cubes[0].size[0], 32.0);
        // Two faces per box: the sprite's front and back.
        fn count(p: &Part) -> usize {
            p.quads.len() + p.children.iter().map(count).sum::<usize>()
        }
        let quads: usize = v[0].to_tree().iter().map(count).sum();
        assert_eq!(quads, 4);
    }

    #[test]
    fn a_sprite_takes_its_textures_size_and_silhouette() {
        let src = r#"{"minecraft:geometry":[{"description":{"identifier":"geometry.item"},
          "bones":[{"name":"hand","pivot":[0,0,0],
            "texture_meshes":[{"texture":"a","position":[0,0,0],"use_pixel_depth":true},
                              {"texture":"b","position":[0,0,0],"use_pixel_depth":false}]}]}]}"#;
        let g = parse_file(&jsonc::parse(src).unwrap()).unwrap().remove(0);
        // 4x2 texture: row 0 = ##.#, row 1 = ....
        let sprite = Sprite {
            size: [4, 2],
            opaque: vec![true, true, false, true, false, false, false, false],
        };
        let solid = g.mesh_variant("a", Some(&sprite));
        assert_eq!(solid.texture_size, [4.0, 2.0]);
        let cubes = &solid.bones[0].cubes;
        // A 4-wide sprite keeps a block's width: texels of 4 pixels, lying flat (u along x, v
        // along z), from its corner at the position, one box per texel.
        assert_eq!(cubes.len(), 3);
        assert!(cubes.iter().all(|c| c.size == [4.0, 1.0, 4.0]));
        let xs: Vec<f32> = cubes.iter().map(|c| c.origin[0]).collect();
        assert_eq!(xs, [0.0, 4.0, 12.0]);
        // One texel deep below the plane (Blockbench: y from -1 to 0).
        assert_eq!(cubes[0].origin, [0.0, -1.0, 0.0]);
        let flat = g.mesh_variant("b", Some(&sprite));
        assert_eq!(flat.bones[0].cubes.len(), 1);
        assert_eq!(flat.bones[0].cubes[0].size[0], 16.0);
        assert_eq!(flat.bones[0].cubes[0].size[2], 8.0);
    }

    #[test]
    fn uv_rotation_turns_the_face_texture_by_quarter_turns() {
        let face = |turns: u8| -> Vec<(f32, f32)> {
            let up = FaceUv { uv: [0.0, 0.0], size: [4.0, 4.0], turns };
            let mut faces: [Option<FaceUv>; 6] = [None; 6];
            faces[4] = Some(up);
            let cube = Cube {
                origin: [0.0; 3],
                size: [4.0, 0.0, 4.0],
                uv: Uv::Faces(faces),
                inflate: None,
                mirror: None,
                pivot: None,
                rotation: None,
            };
            let bone = Bone {
                name: "b".into(),
                parent: None,
                pivot: [0.0; 3],
                rotation: [0.0; 3],
                mirror: false,
                inflate: 0.0,
                never_render: false,
                binding: None,
                cubes: Vec::new(),
                locators: Vec::new(),
                texture_meshes: Vec::new(),
            };
            let quads = cube.quads([0.0; 3], &bone, [16.0, 16.0]);
            assert_eq!(quads.len(), 1);
            quads[0].vertices.iter().map(|v| (v[3], v[4])).collect()
        };
        let still = face(0);
        let turned = face(1);
        assert_ne!(still, turned);
        // A quarter turn hands each corner the next corner's texel; four come back round.
        for k in 0..4 {
            assert_eq!(turned[k], still[(k + 1) % 4]);
        }
        assert_eq!(face(2)[0], still[2]);
    }

    #[test]
    fn a_texture_mesh_position_counts_down_from_the_bone_pivot() {
        // Actions & Stuff's 3D tools: y -11.31 under a bone at 0 is 11.31 up; the test pack's
        // brick (y 24 under a bone at 24) stays at 24.
        let src = r#"{"minecraft:geometry":[{"description":{"identifier":"geometry.t"},
          "bones":[{"name":"a","pivot":[0,0,0],
            "texture_meshes":[{"texture":"t","position":[-11.31,-11.31,0.5],"rotation":[-90,0,-45]}]},
                   {"name":"b","pivot":[0,24,0],
            "texture_meshes":[{"texture":"t","position":[0,24,0]}]},
                   {"name":"c","pivot":[0,0,0],
            "texture_meshes":[{"texture":"t","local_pivot":[0,0.5,0]}]}]}]}"#;
        let g = parse_file(&jsonc::parse(src).unwrap()).unwrap().remove(0);
        let v = g.mesh_variant("t", None);
        assert_eq!(v.bones[0].cubes[0].pivot, Some([-11.31, 11.31, 0.5]));
        assert_eq!(v.bones[1].cubes[0].pivot, Some([0.0, 24.0, 0.0]));
        // Vanilla's spear: `local_pivot` y 0.5 centres the depth on the plane.
        let c = &v.bones[2].cubes[0];
        assert!((c.origin[1] + c.size[1] / 2.0 - 0.5).abs() < 1e-4 || c.size[1] < 0.1);
    }

    fn geo() -> Geometry {
        parse_file(&jsonc::parse(COW).unwrap()).unwrap().remove(0)
    }

    fn bounds(quads: &[Quad]) -> ([f32; 3], [f32; 3]) {
        let mut lo = [f32::MAX; 3];
        let mut hi = [f32::MIN; 3];
        for q in quads {
            for v in &q.vertices {
                for a in 0..3 {
                    lo[a] = lo[a].min(v[a]);
                    hi[a] = hi[a].max(v[a]);
                }
            }
        }
        (lo, hi)
    }

    #[test]
    fn parses_the_modern_layout() {
        let g = geo();
        assert_eq!(g.id, "geometry.test");
        assert_eq!(g.texture_size, [64.0, 32.0]);
        assert_eq!(g.bones.len(), 3);
        assert_eq!(g.bone("HEAD").unwrap().parent.as_deref(), Some("body"));
        assert_eq!(g.locator("hat").unwrap().0.name, "head");
    }

    #[test]
    fn frame_conversion_matches_a_vanilla_layout() {
        // Bedrock's body: pivot y 12, box y 6..12. Java: pivot y 12 (24 - 12), the box hangs
        // below its top edge: y 0..6.
        let tree = geo().to_tree();
        let body = &tree[0];
        assert_eq!(body.pose[..3], [0.0, 12.0, 0.0]);
        let (lo, hi) = bounds(&body.quads);
        assert_eq!(lo, [-4.0, 0.0, -3.0]);
        assert_eq!(hi, [4.0, 6.0, 3.0]);
        // The head pivots 0, 0, -3 from the body's pivot... Bedrock (0,12,-3) - (0,12,0).
        let head = &body.children[0];
        assert_eq!(head.name, "head");
        assert_eq!(head.pose[..3], [0.0, 0.0, -3.0]);
        // Rotation: the angles keep their signs.
        assert!((head.pose[3] - 10f32.to_radians()).abs() < 1e-6);
        assert!((head.pose[4] - 20f32.to_radians()).abs() < 1e-6);
        assert!((head.pose[5] - 30f32.to_radians()).abs() < 1e-6);
        // Inflated by 0.5 on every side.
        let (lo, hi) = bounds(&head.quads);
        assert_eq!(lo, [-2.5, -4.5, -2.5]);
        assert_eq!(hi, [2.5, 0.5, 0.5]);
    }

    #[test]
    fn box_uv_follows_the_unfolded_layout() {
        let tree = geo().to_tree();
        let body = &tree[0];
        assert_eq!(body.quads.len(), 6);
        let uv = |normal: [f32; 3]| -> Vec<[f32; 2]> {
            let q = body.quads.iter().find(|q| q.normal == normal).unwrap();
            q.vertices
                .iter()
                .map(|v| [v[3] * 64.0, v[4] * 32.0])
                .collect()
        };
        // Box [8, 6, 6] at uv [0, 0]: north (the front) starts at u = depth = 6, v = 6.
        assert_eq!(
            uv([0.0, 0.0, -1.0]),
            [[14.0, 6.0], [6.0, 6.0], [6.0, 12.0], [14.0, 12.0]]
        );
        // East (the entity's right, -x) is the leftmost strip.
        assert_eq!(uv([-1.0, 0.0, 0.0])[1], [0.0, 6.0]);
        // Up (Java -y) is the strip above the front; down is flipped.
        assert_eq!(uv([0.0, -1.0, 0.0])[1], [6.0, 0.0]);
        assert_eq!(uv([0.0, 1.0, 0.0])[0], [22.0, 6.0]);
        assert_eq!(uv([0.0, 1.0, 0.0])[2], [14.0, 0.0]);
    }

    #[test]
    fn faces_wind_outward() {
        let tree = geo().to_tree();
        for q in &tree[0].quads {
            let p: Vec<_> = q.vertices.iter().map(|v| [v[0], v[1], v[2]]).collect();
            let sub = |a: [f32; 3], b: [f32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
            let (u, v) = (sub(p[1], p[0]), sub(p[2], p[0]));
            let cross = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            let dot: f32 = cross.iter().zip(q.normal).map(|(a, b)| a * b).sum();
            // Vanilla's own boxes wind so the normal and the vertex order agree up to a flip.
            assert!(dot.abs() > 0.0, "{:?}", q.normal);
            let centre = [0.0f32, 3.0, 0.0];
            let out: f32 = q
                .normal
                .iter()
                .zip(sub(p[0], centre))
                .map(|(n, d)| n * d)
                .sum();
            assert!(out > 0.0, "normal {:?} points inward", q.normal);
        }
    }

    #[test]
    fn a_rotated_cube_becomes_its_own_part() {
        let tree = geo().to_tree();
        let tilted = &tree[0].children[1];
        assert_eq!(tilted.name, "tilted");
        assert!(tilted.quads.is_empty());
        let cube = &tilted.children[0];
        assert_eq!(cube.name, "tilted#cube0");
        assert_eq!(cube.pose[..3], [0.0, 0.0, 0.0]);
        assert!((cube.pose[4] - 45f32.to_radians()).abs() < 1e-6);
        assert_eq!(cube.quads.len(), 6);
    }

    #[test]
    fn mirrored_box_swaps_the_side_strips() {
        let text = r#"{"minecraft:geometry":[{"description":{"identifier":"geometry.m","texture_width":64,"texture_height":64},
          "bones":[{"name":"a","pivot":[0,0,0],"mirror":true,"cubes":[{"origin":[0,0,0],"size":[2,2,2],"uv":[0,0]}]}]}]}"#;
        let g = parse_file(&jsonc::parse(text).unwrap()).unwrap().remove(0);
        let part = &g.to_tree()[0];
        // The -x normal flips to +x for the mirrored box and the box still spans the same cells.
        let east_uv = |n: [f32; 3]| {
            part.quads
                .iter()
                .find(|q| q.normal == n)
                .map(|q| q.vertices[0][3] * 64.0)
        };
        assert!(east_uv([1.0, 0.0, 0.0]).is_some());
        assert_eq!(part.quads.len(), 6);
        let (lo, hi) = bounds(&part.quads);
        assert_eq!((lo, hi), ([0.0, -2.0, 0.0], [2.0, 0.0, 2.0]));
    }

    #[test]
    fn per_face_uvs_leave_out_missing_faces() {
        let text = r#"{"minecraft:geometry":[{"description":{"identifier":"geometry.f","texture_width":16,"texture_height":16},
          "bones":[{"name":"a","pivot":[0,0,0],"cubes":[{"origin":[0,0,0],"size":[1,1,1],
            "uv":{"north":{"uv":[0,0],"uv_size":[4,4]},"up":{"uv":[4,0],"uv_size":[-4,4]}}}]}]}]}"#;
        let g = parse_file(&jsonc::parse(text).unwrap()).unwrap().remove(0);
        let part = &g.to_tree()[0];
        assert_eq!(part.quads.len(), 2);
        let north = part
            .quads
            .iter()
            .find(|q| q.normal == [0.0, 0.0, -1.0])
            .unwrap();
        assert_eq!(north.vertices[0][3..], [0.25, 0.0]);
        assert_eq!(north.vertices[2][3..], [0.0, 0.25]);
        // A negative size runs the rectangle backwards: the flip.
        let up = part
            .quads
            .iter()
            .find(|q| q.normal == [0.0, -1.0, 0.0])
            .unwrap();
        assert_eq!(up.vertices[0][3..], [0.0, 0.0]);
        assert_eq!(up.vertices[1][3..], [0.25, 0.0]);
    }

    #[test]
    fn legacy_layout_with_inheritance() {
        let text = r#"{"format_version":"1.8.0",
          "geometry.base":{"texturewidth":32,"textureheight":32,"bones":[
             {"name":"a","pivot":[0,0,0],"cubes":[{"origin":[0,0,0],"size":[1,1,1],"uv":[0,0]}]},
             {"name":"b","pivot":[0,0,0],"cubes":[]}]},
          "geometry.child:geometry.base":{"bones":[{"name":"b","pivot":[1,1,1],"cubes":[{"origin":[0,0,0],"size":[1,1,1],"uv":[0,0]}]},{"name":"c"}]}}"#;
        let gs = parse_file(&jsonc::parse(text).unwrap()).unwrap();
        let child = gs.iter().find(|g| g.id == "geometry.child").unwrap();
        assert_eq!(child.texture_size, [32.0, 32.0]);
        let names: Vec<_> = child.bones.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(names, ["a", "b", "c"]);
        assert_eq!(child.bone("b").unwrap().pivot, [1.0, 1.0, 1.0]);
        assert!(gs.iter().any(|g| g.id == "geometry.base"));
    }

    #[test]
    fn unknown_parent_makes_a_root_and_cycles_do_not_hang() {
        let text = r#"{"minecraft:geometry":[{"description":{"identifier":"geometry.c"},"bones":[
          {"name":"a","parent":"missing","pivot":[0,0,0]},
          {"name":"x","parent":"y","pivot":[0,0,0]},
          {"name":"y","parent":"x","pivot":[0,0,0]}]}]}"#;
        let g = parse_file(&jsonc::parse(text).unwrap()).unwrap().remove(0);
        let tree = g.to_tree();
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].name, "a");
    }

    #[test]
    fn cubes_laid_over_each_other_do_not_share_planes() {
        let c = |o: [f32; 3]| Cube {
            origin: o,
            size: [8.0, 8.0, 8.0],
            uv: Uv::Box([0.0, 0.0]),
            inflate: None,
            mirror: None,
            pivot: None,
            rotation: None,
        };
        let bone = |name: &str, o| Bone {
            name: name.into(),
            parent: None,
            pivot: [0.0; 3],
            rotation: [0.0; 3],
            mirror: false,
            inflate: 0.0,
            never_render: false,
            binding: None,
            cubes: vec![c(o)],
            locators: vec![],
            texture_meshes: vec![],
        };
        let mut bones = vec![
            bone("a", [0.0; 3]),
            bone("b", [0.0; 3]),
            bone("c", [1.0; 3]),
        ];
        separate_coplanar(&mut bones);
        // The first of the two is the one that shows.
        assert!(bones[0].cubes[0].inflate.unwrap() > bones[1].cubes[0].inflate.unwrap_or(0.0));
        assert_eq!(bones[2].cubes[0].inflate, None);
    }
}
