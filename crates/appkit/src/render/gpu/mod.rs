//! The GPU canvas: the same drawing as [`Canvas`](super::canvas::Canvas) on wgpu (Vulkan, with
//! GLES as the fallback), for the Wayland shell. A view paints through [`Painter`](super::Painter)
//! and does not know which one it draws on.
//!
//! The formulas are the CPU canvas's, in `shader.wgsl`:
//!
//! - paths are filled by stencil-and-cover on a 4× multisampled target (non-zero and even-odd),
//!   strokes are expanded by kurbo as on the CPU;
//! - the brushes (solid, linear, radial and sweep gradients, images) are evaluated per pixel in
//!   the paint's own space; opacity groups, additive groups and blurs are layers composited back;
//! - a rectangular clip is a scissor, any other clip a coverage mask rendered by the same path
//!   code and multiplied in the fragment stage;
//! - shadows are the analytic blurred rounded rectangle; a backdrop is copied, reduced for a large
//!   σ, blurred by a Gaussian and drawn back through the outline with the saturation and
//!   brightness matrix and Ossido's refraction (`backdrop.rs`);
//! - glyphs are filled as outlines (skrifa), as on the CPU.
//!
//! [`GpuContext::new`] is `None` without an adapter: the caller then keeps the CPU canvas
//! (which also makes the snapshots). The frame texture is retained between frames: only the
//! clipped region is repainted, and [`GpuCanvas::damage`] says which one changed.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use bytemuck::{Pod, Zeroable};
use vello_cpu::kurbo::{self, Affine, BezPath, PathEl};
use vello_cpu::peniko::FontData;
use vello_cpu::{Glyph, Pixmap};
use wgpu::util::DeviceExt;

use crate::color::Color;
use crate::geom::{Rect, Transform};
use crate::render::Painter;
use crate::render::backdrop::{self, Backdrop};
use crate::render::canvas::{Brush, StrokeStyle};
use crate::render::image::Image;
use crate::render::path::Path;

/// Samples per pixel of the targets.
const SAMPLES: u32 = 4;
/// The format of the colour targets (premultiplied RGBA8, like the CPU canvas's pixmap).
const COLOR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// The format of the clip masks.
const MASK: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;

/// The uniform of a draw (`Paint` in the shader).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PaintU {
    inv_a: [f32; 4],
    inv_b: [f32; 4],
    color: [f32; 4],
    g0: [f32; 4],
    g1: [f32; 4],
    g2: [f32; 4],
    g3: [f32; 4],
    g4: [f32; 4],
    g5: [f32; 4],
    m0: [f32; 4],
    m1: [f32; 4],
    m2: [f32; 4],
    fwd_a: [f32; 4],
    fwd_b: [f32; 4],
    h0: [f32; 4],
    h1: [f32; 4],
    stops_t: [[f32; 4]; 2],
    stops_c: [[f32; 4]; 8],
}

/// Which pipeline a draw uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Mode {
    /// Writes the winding of a path into the stencil (non-zero).
    StencilNonZero,
    /// Writes the parity of a path into the stencil.
    StencilEvenOdd,
    /// Paints where the stencil is set and clears it.
    Cover,
    /// Paints a shape directly.
    Simple,
    /// A full-screen filter pass on a single-sample target (`blur` or `copy`).
    Filter { blur: bool },
}

/// How a draw combines with the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Blend {
    Over,
    Add,
    /// Removes the coverage from the target (the complement of a clip).
    Erase,
    Replace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    mode: Mode,
    blend: Blend,
    format: wgpu::TextureFormat,
}

/// The device, the queue and what is shared by every canvas.
pub struct GpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter: String,
    shader: wgpu::ShaderModule,
    bgl: wgpu::BindGroupLayout,
    layout: wgpu::PipelineLayout,
    sampler: wgpu::Sampler,
    nearest: wgpu::Sampler,
    white: wgpu::TextureView,
    pipelines: RefCell<HashMap<Key, wgpu::RenderPipeline>>,
    images: RefCell<HashMap<u64, wgpu::TextureView>>,
}

impl std::fmt::Debug for GpuContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GpuContext({})", self.adapter)
    }
}

impl GpuContext {
    /// A context on the first adapter (Vulkan, then GLES); `None` without one.
    pub fn new() -> Option<Rc<GpuContext>> {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = wgpu::Backends::VULKAN | wgpu::Backends::GL;
        let instance = wgpu::Instance::new(desc);
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
            ..Default::default()
        }))
        .ok()?;
        let info = adapter.get_info();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("appkit"),
            ..Default::default()
        }))
        .ok()?;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("appkit canvas"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty,
            count: None,
        };
        let tex = wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        };
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("appkit paint"),
            entries: &[
                entry(
                    0,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: None,
                    },
                ),
                entry(1, tex),
                entry(2, tex),
                entry(
                    3,
                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                ),
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("appkit"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let nearest = device.create_sampler(&wgpu::SamplerDescriptor::default());
        let white_tex = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("white"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: COLOR,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &[255, 255, 255, 255],
        );
        let white = white_tex.create_view(&wgpu::TextureViewDescriptor::default());
        Some(Rc::new(GpuContext {
            device,
            queue,
            adapter: format!("{} ({:?})", info.name, info.backend),
            shader,
            bgl,
            layout,
            sampler,
            nearest,
            white,
            pipelines: RefCell::new(HashMap::new()),
            images: RefCell::new(HashMap::new()),
        }))
    }

    /// The adapter's name and backend.
    pub fn adapter(&self) -> &str {
        &self.adapter
    }

    fn pipeline(&self, key: Key) -> wgpu::RenderPipeline {
        if let Some(p) = self.pipelines.borrow().get(&key) {
            return p.clone();
        }
        let filter = matches!(key.mode, Mode::Filter { .. });
        let stencil_mode = matches!(key.mode, Mode::StencilNonZero | Mode::StencilEvenOdd);
        let blend = match key.blend {
            Blend::Over => Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
            }),
            Blend::Add => Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
            }),
            Blend::Erase => Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::Zero,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::Zero,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
            }),
            Blend::Replace => None,
        };
        let face = |op| wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::Always,
            fail_op: wgpu::StencilOperation::Keep,
            depth_fail_op: wgpu::StencilOperation::Keep,
            pass_op: op,
        };
        let stencil = match key.mode {
            Mode::StencilNonZero => wgpu::StencilState {
                front: face(wgpu::StencilOperation::IncrementWrap),
                back: face(wgpu::StencilOperation::DecrementWrap),
                read_mask: 0xff,
                write_mask: 0xff,
            },
            Mode::StencilEvenOdd => wgpu::StencilState {
                front: face(wgpu::StencilOperation::Invert),
                back: face(wgpu::StencilOperation::Invert),
                read_mask: 0xff,
                write_mask: 0xff,
            },
            Mode::Cover => {
                let f = wgpu::StencilFaceState {
                    compare: wgpu::CompareFunction::NotEqual,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Keep,
                    pass_op: wgpu::StencilOperation::Zero,
                };
                wgpu::StencilState {
                    front: f,
                    back: f,
                    read_mask: 0xff,
                    write_mask: 0xff,
                }
            }
            _ => wgpu::StencilState::default(),
        };
        let (vs, fs) = match key.mode {
            Mode::Filter { blur: true } => ("vs", "fs_blur"),
            Mode::Filter { blur: false } => ("vs", "fs_copy"),
            Mode::StencilNonZero | Mode::StencilEvenOdd => ("vs_stencil", "fs_stencil"),
            _ => ("vs", "fs"),
        };
        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("appkit"),
                layout: Some(&self.layout),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: 8,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x2],
                    })],
                },
                primitive: wgpu::PrimitiveState {
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: (!filter).then_some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Stencil8,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::Always),
                    stencil,
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: if filter { 1 } else { SAMPLES },
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: key.format,
                        blend,
                        write_mask: if stencil_mode {
                            wgpu::ColorWrites::empty()
                        } else {
                            wgpu::ColorWrites::ALL
                        },
                    })],
                }),
                multiview_mask: None,
                cache: None,
            });
        self.pipelines.borrow_mut().insert(key, pipeline.clone());
        pipeline
    }

    /// The texture of `image`, uploaded once.
    fn image_view(&self, image: &Image) -> Option<wgpu::TextureView> {
        let id = image.id();
        if let Some(v) = self.images.borrow().get(&id) {
            return Some(v.clone());
        }
        let (w, h) = (
            u32::from(image.pixmap().width()),
            u32::from(image.pixmap().height()),
        );
        if w == 0 || h == 0 {
            return None;
        }
        let tex = self.device.create_texture_with_data(
            &self.queue,
            &wgpu::TextureDescriptor {
                label: Some("image"),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: COLOR,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            image.pixmap().data_as_u8_slice(),
        );
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        self.images.borrow_mut().insert(id, view.clone());
        Some(view)
    }
}

/// A multisampled target and the texture it resolves into.
struct Target {
    msaa: wgpu::TextureView,
    stencil: wgpu::TextureView,
    color: wgpu::Texture,
    view: wgpu::TextureView,
    format: wgpu::TextureFormat,
    /// The device pixels it covers: x, y, width, height.
    region: [u32; 4],
}

impl Target {
    fn new(ctx: &GpuContext, region: [u32; 4], format: wgpu::TextureFormat) -> Target {
        let (w, h) = (region[2].max(1), region[3].max(1));
        let size = wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        };
        let make = |format, samples, usage| {
            ctx.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("appkit target"),
                size,
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let color = make(
            format,
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
        );
        let msaa = make(format, SAMPLES, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let stencil = make(
            wgpu::TextureFormat::Stencil8,
            SAMPLES,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let v = |t: &wgpu::Texture| t.create_view(&wgpu::TextureViewDescriptor::default());
        Target {
            msaa: v(&msaa),
            stencil: v(&stencil),
            view: v(&color),
            color,
            format,
            region,
        }
    }
}

/// What a layer does when it is closed.
#[derive(Debug, Clone, Copy)]
enum LayerKind {
    Opacity(f32),
    Additive,
    Blur(f32),
}

/// The target a draw goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Where {
    Main,
    Layer(usize),
    Scratch,
}

/// A clip by a path (device pixels): rendered into a mask when something is drawn under it,
/// for the region of the draw only.
struct MaskEntry {
    path: Path,
    invert: bool,
    target: Option<Target>,
}

/// One recorded draw of the open pass.
struct Cmd {
    pipeline: wgpu::RenderPipeline,
    uniform: u32,
    vertex: u64,
    count: u32,
    image: Option<wgpu::TextureView>,
    mask: Option<wgpu::TextureView>,
    scissor: [u32; 4],
}

/// The stride of the uniforms in their buffer (a multiple of the offset alignment).
const UNIFORM_STRIDE: usize = 512;

struct Layer {
    kind: LayerKind,
    target: Target,
    scissor: [u32; 4],
    masks: usize,
    /// What was drawn into it (device pixels).
    bounds: Option<Rect>,
}

#[derive(Clone, Copy)]
struct Saved {
    transform: Affine,
    scissor: [u32; 4],
    masks: usize,
    layers: usize,
}

/// A drawing context on the GPU over a retained frame.
pub struct GpuCanvas {
    ctx: Rc<GpuContext>,
    width: u32,
    height: u32,
    main: Target,
    layers: Vec<Layer>,
    masks: Vec<MaskEntry>,
    /// Targets of closed layers and masks, reused.
    free: Vec<Target>,
    transform: Affine,
    stack: Vec<Saved>,
    /// The clip rectangle in device pixels: x0, y0, x1, y1.
    scissor: [u32; 4],
    encoder: Option<wgpu::CommandEncoder>,
    /// Masks are rendered in an encoder submitted before the frame's: they depend on nothing
    /// drawn, and the frame's pass stays open meanwhile.
    mask_encoder: Option<wgpu::CommandEncoder>,
    /// Mask targets that closed this frame: reused only after the submit.
    retired: Vec<Target>,
    /// Filter passes go to the mask encoder (a blurred shadow does not depend on the frame).
    aux: bool,
    pending: Vec<Cmd>,
    uniforms: Vec<u8>,
    vertices: Vec<u8>,
    /// Where the open pass draws.
    pass_on: Option<Where>,
    /// A mask being rendered: draws go here instead of the frame or a layer.
    scratch: Option<Target>,
    /// The main target has been cleared since it was made.
    started: bool,
    damage: Option<Rect>,
}

impl std::fmt::Debug for GpuCanvas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GpuCanvas({}×{})", self.width, self.height)
    }
}

fn identity_paint(w: u32, h: u32) -> PaintU {
    let mut p = PaintU::zeroed();
    p.inv_a = [1.0, 0.0, 0.0, 1.0];
    p.inv_b = [0.0, 0.0, w as f32, h as f32];
    p
}

fn premul(c: Color, opacity: f32) -> [f32; 4] {
    let a = f32::from(c.a) / 255.0 * opacity;
    [
        f32::from(c.r) / 255.0 * a,
        f32::from(c.g) / 255.0 * a,
        f32::from(c.b) / 255.0 * a,
        a,
    ]
}

/// The flattened contours of `bez` (device pixels).
fn contours(bez: &BezPath, to_device: Affine) -> Vec<Vec<[f32; 2]>> {
    let mut out: Vec<Vec<[f32; 2]>> = Vec::new();
    let mut cur: Vec<[f32; 2]> = Vec::new();
    let dev = to_device * bez.clone();
    kurbo::flatten(dev.elements().iter().copied(), 0.1, |el| match el {
        PathEl::MoveTo(p) => {
            if cur.len() >= 3 {
                out.push(std::mem::take(&mut cur));
            } else {
                cur.clear();
            }
            cur.push([p.x as f32, p.y as f32]);
        }
        PathEl::LineTo(p) => cur.push([p.x as f32, p.y as f32]),
        PathEl::ClosePath => {
            if cur.len() >= 3 {
                out.push(std::mem::take(&mut cur));
            } else {
                cur.clear();
            }
        }
        _ => {}
    });
    if cur.len() >= 3 {
        out.push(cur);
    }
    out
}

impl GpuCanvas {
    /// A transparent canvas of `width` × `height` pixels.
    pub fn new(ctx: &Rc<GpuContext>, width: u32, height: u32) -> GpuCanvas {
        let w = width.clamp(1, 16384);
        let h = height.clamp(1, 16384);
        GpuCanvas {
            ctx: ctx.clone(),
            width: w,
            height: h,
            main: Target::new(ctx, [0, 0, w, h], COLOR),
            layers: Vec::new(),
            masks: Vec::new(),
            free: Vec::new(),
            transform: Affine::IDENTITY,
            stack: Vec::new(),
            scissor: [0, 0, w, h],
            encoder: None,
            mask_encoder: None,
            retired: Vec::new(),
            aux: false,
            pending: Vec::new(),
            uniforms: Vec::new(),
            vertices: Vec::new(),
            pass_on: None,
            scratch: None,
            started: false,
            damage: None,
        }
    }

    /// The region painted since the last [`GpuCanvas::take_damage`], in device pixels.
    pub fn damage(&self) -> Option<Rect> {
        self.damage
    }

    /// The damage, reset: what a `wl_surface.damage_buffer` call should cover.
    pub fn take_damage(&mut self) -> Option<Rect> {
        self.damage.take()
    }

    /// Starts a frame: the retained texture keeps what was painted before, so a frame that
    /// clips to a region repaints only that region. `clear` makes the whole frame transparent.
    pub fn begin(&mut self, clear: bool) {
        self.restore_to(0);
        self.end_pass();
        if clear {
            self.started = false;
        }
        self.scissor = [0, 0, self.width, self.height];
    }

    fn encoder(&mut self) -> &mut wgpu::CommandEncoder {
        self.encoder.get_or_insert_with(|| {
            self.ctx
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("appkit frame"),
                })
        })
    }

    /// Encodes the open pass: one buffer of vertices and one of uniforms for all its draws.
    fn end_pass(&mut self) {
        let Some(want) = self.pass_on.take() else {
            return;
        };
        let cmds = std::mem::take(&mut self.pending);
        let uniforms = std::mem::take(&mut self.uniforms);
        let vertices = std::mem::take(&mut self.vertices);
        let is_main = want == Where::Main;
        if is_main && self.started && cmds.is_empty() {
            return;
        }
        let ctx = self.ctx.clone();
        let (ub, vb) = if cmds.is_empty() {
            (None, None)
        } else {
            (
                Some(
                    ctx.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: None,
                            contents: &uniforms,
                            usage: wgpu::BufferUsages::UNIFORM,
                        }),
                ),
                Some(
                    ctx.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: None,
                            contents: &vertices,
                            usage: wgpu::BufferUsages::VERTEX,
                        }),
                ),
            )
        };
        let use_mask = want == Where::Scratch;
        let Some(mut encoder) = (if use_mask {
            self.mask_encoder.take().or_else(|| {
                Some(
                    ctx.device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("appkit masks"),
                        }),
                )
            })
        } else {
            self.encoder();
            self.encoder.take()
        }) else {
            return;
        };
        {
            let t = self.target_of(want);
            let load = if is_main && self.started {
                wgpu::LoadOp::Load
            } else {
                wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("appkit"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &t.msaa,
                    depth_slice: None,
                    resolve_target: Some(&t.view),
                    ops: wgpu::Operations {
                        load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &t.stencil,
                    depth_ops: None,
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0),
                        store: wgpu::StoreOp::Discard,
                    }),
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if let (Some(ub), Some(vb)) = (&ub, &vb) {
                type Views = (Option<wgpu::TextureView>, Option<wgpu::TextureView>);
                let mut binds: Vec<(Views, wgpu::BindGroup)> = Vec::new();
                let mut last: Option<usize> = None;
                for c in &cmds {
                    let key: Views = (c.image.clone(), c.mask.clone());
                    let idx = match binds.iter().position(|(k, _)| *k == key) {
                        Some(i) => i,
                        None => {
                            let bind = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
                                label: None,
                                layout: &ctx.bgl,
                                entries: &[
                                    wgpu::BindGroupEntry {
                                        binding: 0,
                                        resource: wgpu::BindingResource::Buffer(
                                            wgpu::BufferBinding {
                                                buffer: ub,
                                                offset: 0,
                                                size: std::num::NonZeroU64::new(
                                                    std::mem::size_of::<PaintU>() as u64,
                                                ),
                                            },
                                        ),
                                    },
                                    wgpu::BindGroupEntry {
                                        binding: 1,
                                        resource: wgpu::BindingResource::TextureView(
                                            c.image.as_ref().unwrap_or(&ctx.white),
                                        ),
                                    },
                                    wgpu::BindGroupEntry {
                                        binding: 2,
                                        resource: wgpu::BindingResource::TextureView(
                                            c.mask.as_ref().unwrap_or(&ctx.white),
                                        ),
                                    },
                                    wgpu::BindGroupEntry {
                                        binding: 3,
                                        resource: wgpu::BindingResource::Sampler(&ctx.sampler),
                                    },
                                ],
                            });
                            binds.push((key, bind));
                            binds.len() - 1
                        }
                    };
                    let _ = last.replace(idx);
                    if let Some((_, bind)) = binds.get(idx) {
                        pass.set_pipeline(&c.pipeline);
                        pass.set_bind_group(0, bind, &[c.uniform]);
                        pass.set_vertex_buffer(
                            0,
                            vb.slice(c.vertex..c.vertex + u64::from(c.count) * 8),
                        );
                        pass.set_scissor_rect(
                            c.scissor[0],
                            c.scissor[1],
                            c.scissor[2],
                            c.scissor[3],
                        );
                        pass.draw(0..c.count, 0..1);
                    }
                }
            }
        }
        if use_mask {
            self.mask_encoder = Some(encoder);
        } else {
            self.encoder = Some(encoder);
        }
        if is_main {
            self.started = true;
        }
    }

    /// Submits what was recorded.
    pub fn submit(&mut self) {
        self.end_pass();
        let mut buffers = Vec::new();
        if let Some(e) = self.mask_encoder.take() {
            buffers.push(e.finish());
        }
        if let Some(e) = self.encoder.take() {
            buffers.push(e.finish());
        }
        if !buffers.is_empty() {
            self.ctx.queue.submit(buffers);
        }
        let retired = std::mem::take(&mut self.retired);
        self.free.extend(retired);
    }

    fn current(&self) -> Where {
        if self.scratch.is_some() {
            Where::Scratch
        } else if self.layers.is_empty() {
            Where::Main
        } else {
            Where::Layer(self.layers.len() - 1)
        }
    }

    fn target_of(&self, w: Where) -> &Target {
        match w {
            Where::Main => &self.main,
            Where::Layer(i) => self.layers.get(i).map_or(&self.main, |l| &l.target),
            Where::Scratch => self.scratch.as_ref().unwrap_or(&self.main),
        }
    }

    /// Starts recording the pass on the current target, closing the one on another.
    fn ensure_pass(&mut self) {
        let want = self.current();
        if self.pass_on == Some(want) {
            return;
        }
        self.end_pass();
        self.pass_on = Some(want);
    }

    /// The device matrix.
    fn device_transform(&self) -> Affine {
        self.transform
    }

    fn inverse_paint(&self) -> ([f32; 4], [f32; 2]) {
        let inv = self.transform.inverse();
        let c = if inv.determinant().is_finite() {
            inv.as_coeffs()
        } else {
            [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]
        };
        (
            [c[0] as f32, c[1] as f32, c[2] as f32, c[3] as f32],
            [c[4] as f32, c[5] as f32],
        )
    }

    fn uniform_for(&self, brush: &Brush, opacity: f32) -> PaintU {
        let (w, h) = self.target_size();
        let mut p = identity_paint(w, h);
        let (a, e) = self.inverse_paint();
        p.inv_a = a;
        p.inv_b[0] = e[0];
        p.inv_b[1] = e[1];
        p.g1[2] = if self.masks.is_empty() { 0.0 } else { 1.0 };
        let op = opacity.clamp(0.0, 1.0);
        let stops = |s: &[(f32, Color)], p: &mut PaintU| {
            let n = s.len().min(8);
            p.g1[1] = n as f32;
            for (i, (t, c)) in s.iter().take(n).enumerate() {
                if let Some(slot) = p.stops_t.get_mut(i / 4).and_then(|r| r.get_mut(i % 4)) {
                    *slot = *t;
                }
                if let Some(slot) = p.stops_c.get_mut(i) {
                    *slot = [
                        f32::from(c.r) / 255.0,
                        f32::from(c.g) / 255.0,
                        f32::from(c.b) / 255.0,
                        f32::from(c.a) / 255.0,
                    ];
                }
            }
            p.color[0] = op;
        };
        match brush {
            Brush::Solid(c) => {
                p.color = premul(*c, op);
            }
            Brush::Linear { from, to, stops: s } => {
                p.g1[0] = 1.0;
                p.g0 = [from.x, from.y, to.x, to.y];
                stops(s, &mut p);
            }
            Brush::Radial {
                center,
                radius,
                stops: s,
            } => {
                p.g1[0] = 2.0;
                p.g0 = [center.x, center.y, *radius, 0.0];
                stops(s, &mut p);
            }
            Brush::Sweep {
                center,
                start,
                stops: s,
                ..
            } => {
                p.g1[0] = 3.0;
                p.g0 = [center.x, center.y, start.to_radians(), 0.0];
                stops(s, &mut p);
            }
        }
        p
    }

    fn target_size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// The clip rectangle as x, y, width, height (at least 1 × 1).
    fn scissor_region(&self) -> [u32; 4] {
        let [x0, y0, x1, y1] = self.scissor;
        [
            x0,
            y0,
            x1.saturating_sub(x0).max(1),
            y1.saturating_sub(y0).max(1),
        ]
    }

    fn scissor_rect(&self) -> Option<[u32; 4]> {
        let [x0, y0, x1, y1] = self.scissor;
        (x1 > x0 && y1 > y0).then_some([x0, y0, x1 - x0, y1 - y0])
    }

    /// Draws vertices with a pipeline and a paint.
    fn draw(
        &mut self,
        verts: &[[f32; 2]],
        mode: Mode,
        blend: Blend,
        paint: &PaintU,
        image: Option<&wgpu::TextureView>,
    ) {
        if verts.len() < 3 {
            return;
        }
        let Some(sc) = self.scissor_rect() else {
            return;
        };
        if self.scratch.is_none() && !self.masks.is_empty() {
            let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for v in verts {
                x0 = x0.min(v[0]);
                y0 = y0.min(v[1]);
                x1 = x1.max(v[0]);
                y1 = y1.max(v[1]);
            }
            let want = Rect::new(x0, y0, x1 - x0, y1 - y0).intersect(self.scissor_as_rect());
            if want.is_empty() {
                return;
            }
            self.ensure_masks(want);
        }
        self.ensure_pass();
        let ctx = self.ctx.clone();
        let format = self.target_of(self.current()).format;
        let pipeline = ctx.pipeline(Key {
            mode,
            blend,
            format,
        });
        let mask = self
            .masks
            .last()
            .and_then(|m| m.target.as_ref())
            .map(|t| t.view.clone());
        let t = self.target_of(self.current());
        let region = t.region;
        let mut paint = *paint;
        paint.inv_b[2] = region[2] as f32;
        paint.inv_b[3] = region[3] as f32;
        paint.h1 = [region[0] as f32, region[1] as f32, 0.0, 0.0];
        if let Some(m) = self.masks.last().and_then(|m| m.target.as_ref()) {
            paint.h0 = [
                m.region[0] as f32,
                m.region[1] as f32,
                m.region[2] as f32,
                m.region[3] as f32,
            ];
        }
        let sc = [
            sc[0].saturating_sub(region[0]),
            sc[1].saturating_sub(region[1]),
            sc[2],
            sc[3],
        ];
        let uniform = self.uniforms.len();
        self.uniforms.extend_from_slice(bytemuck::bytes_of(&paint));
        self.uniforms.resize(uniform + UNIFORM_STRIDE, 0);
        let vertex = self.vertices.len() as u64;
        self.vertices.extend_from_slice(bytemuck::cast_slice(verts));
        self.pending.push(Cmd {
            pipeline,
            uniform: uniform as u32,
            vertex,
            count: verts.len() as u32,
            image: image.cloned(),
            mask,
            scissor: sc,
        });
        let _ = &ctx.nearest;
    }
}

fn quad(r: Rect) -> [[f32; 2]; 6] {
    let (x0, y0, x1, y1) = (r.x, r.y, r.right(), r.bottom());
    [[x0, y0], [x1, y0], [x0, y1], [x1, y0], [x1, y1], [x0, y1]]
}

fn quad_of(corners: [[f32; 2]; 4]) -> [[f32; 2]; 6] {
    let [a, b, c, d] = corners;
    [a, b, d, b, c, d]
}

impl GpuCanvas {
    fn mark(&mut self, r: Rect) {
        let r = r.intersect(self.scissor_as_rect());
        if r.is_empty() {
            return;
        }
        if let Where::Layer(i) = self.current() {
            if let Some(l) = self.layers.get_mut(i) {
                l.bounds = Some(l.bounds.map_or(r, |b| b.union(r)));
            }
            return;
        }
        self.damage = Some(match self.damage {
            Some(d) => d.union(r),
            None => r,
        });
    }

    fn scissor_as_rect(&self) -> Rect {
        let [x0, y0, x1, y1] = self.scissor;
        Rect::new(x0 as f32, y0 as f32, (x1 - x0) as f32, (y1 - y0) as f32)
    }

    fn intersect_scissor(&mut self, r: Rect) {
        let [x0, y0, x1, y1] = self.scissor;
        let nx0 = (r.x.round().max(0.0) as u32).max(x0);
        let ny0 = (r.y.round().max(0.0) as u32).max(y0);
        let nx1 = (r.right().round().max(0.0) as u32).min(x1);
        let ny1 = (r.bottom().round().max(0.0) as u32).min(y1);
        self.scissor = [nx0, ny0, nx1.max(nx0), ny1.max(ny0)];
    }

    /// Fills the contours of `bez` (current coordinates) with `paint`, by stencil and cover.
    fn cover_fill(
        &mut self,
        bez: &BezPath,
        even_odd: bool,
        paint: &PaintU,
        image: Option<&wgpu::TextureView>,
        blend: Blend,
    ) {
        let polys = contours(bez, self.device_transform());
        if polys.is_empty() {
            return;
        }
        let mut tris: Vec<[f32; 2]> = Vec::new();
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for poly in &polys {
            let Some(first) = poly.first() else { continue };
            for p in poly {
                if !(p[0].is_finite() && p[1].is_finite()) {
                    return;
                }
                x0 = x0.min(p[0]);
                y0 = y0.min(p[1]);
                x1 = x1.max(p[0]);
                y1 = y1.max(p[1]);
            }
            for w in poly.windows(2) {
                if let [a, b] = w {
                    tris.extend([*first, *a, *b]);
                }
            }
        }
        let cover = Rect::new(x0, y0, x1 - x0, y1 - y0).intersect(self.scissor_as_rect());
        if tris.len() < 3 || cover.is_empty() {
            return;
        }
        self.mark(cover);
        let (w, h) = self.target_size();
        let sp = identity_paint(w, h);
        let mode = if even_odd {
            Mode::StencilEvenOdd
        } else {
            Mode::StencilNonZero
        };
        self.draw(&tris, mode, Blend::Over, &sp, None);
        // The cover is grown a pixel: the multisampled stencil decides the edge.
        self.draw(
            &quad(cover.inflate(1.0, 1.0)),
            Mode::Cover,
            blend,
            paint,
            image,
        );
    }

    fn close_layer(&mut self) {
        self.end_pass();
        let Some(layer) = self.layers.pop() else {
            return;
        };
        let saved_scissor = self.scissor;
        let extra: Vec<Target> = self
            .masks
            .split_off(layer.masks.min(self.masks.len()))
            .into_iter()
            .filter_map(|m| m.target)
            .collect();
        self.retired.extend(extra);
        let extra: Vec<Target> = Vec::new();
        self.scissor = layer.scissor;
        let lr = layer.target.region;
        let (w, h) = (lr[2], lr[3]);
        let mut paint = identity_paint(w, h);
        paint.g1 = [4.0, 0.0, if self.masks.is_empty() { 0.0 } else { 1.0 }, 0.0];
        paint.g0 = [w as f32, h as f32, 0.0, 0.0];
        paint.color = [1.0, 0.0, 0.0, 0.0];
        let mut view = layer.target.view.clone();
        let mut blend = Blend::Over;
        let mut area = self.scissor_as_rect();
        let mut origin = (lr[0], lr[1]);
        let mut size = (w, h);
        match layer.kind {
            LayerKind::Opacity(o) => paint.color[0] = o,
            LayerKind::Additive => blend = Blend::Add,
            LayerKind::Blur(sigma) => {
                // Only the region the layer was drawn in, grown by the blur's reach.
                let reach = sigma * 3.0 + 2.0;
                let Some(b) = layer.bounds else {
                    self.scissor = saved_scissor;
                    self.free.extend(extra);
                    self.free.push(layer.target);
                    return;
                };
                let r = b
                    .inflate(reach, reach)
                    .intersect(Rect::new(lr[0] as f32, lr[1] as f32, w as f32, h as f32))
                    .intersect(area);
                if r.is_empty() {
                    self.scissor = saved_scissor;
                    self.free.extend(extra);
                    self.free.push(layer.target);
                    return;
                }
                let (x0, y0) = (r.x.floor() as u32, r.y.floor() as u32);
                let (rw, rh) = (
                    (r.right().ceil() as u32).saturating_sub(x0).max(1),
                    (r.bottom().ceil() as u32).saturating_sub(y0).max(1),
                );
                let ctx = self.ctx.clone();
                let tex = ctx.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("blur region"),
                    size: wgpu::Extent3d {
                        width: rw,
                        height: rh,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: COLOR,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                let src = layer.target.color.clone();
                self.encoder().copy_texture_to_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &src,
                        mip_level: 0,
                        origin: wgpu::Origin3d {
                            x: x0 - lr[0],
                            y: y0 - lr[1],
                            z: 0,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    tex.as_image_copy(),
                    wgpu::Extent3d {
                        width: rw,
                        height: rh,
                        depth_or_array_layers: 1,
                    },
                );
                let tv = tex.create_view(&wgpu::TextureViewDescriptor::default());
                if let Some(v) = self.blur_texture(&tv, rw, rh, sigma) {
                    view = v;
                    origin = (x0, y0);
                    size = (rw, rh);
                    area = Rect::new(x0 as f32, y0 as f32, rw as f32, rh as f32).intersect(area);
                }
            }
        }
        paint.g0 = [size.0 as f32, size.1 as f32, 0.0, 0.0];
        paint.inv_b[0] = -(origin.0 as f32);
        paint.inv_b[1] = -(origin.1 as f32);
        self.draw(&quad(area), Mode::Simple, blend, &paint, Some(&view));
        self.mark(area);
        self.scissor = saved_scissor;
        self.free.extend(extra);
        self.free.push(layer.target);
    }

    /// Runs a full-screen filter pass from `src` into a new `w` × `h` texture.
    fn filter_pass(
        &mut self,
        src: &wgpu::TextureView,
        w: u32,
        h: u32,
        blur: Option<(f32, f32, f32, i32)>,
    ) -> Option<wgpu::TextureView> {
        if !self.aux {
            self.end_pass();
        }
        let ctx = self.ctx.clone();
        let tex = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("appkit filter"),
            size: wgpu::Extent3d {
                width: w.max(1),
                height: h.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: COLOR,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let mut paint = identity_paint(w.max(1), h.max(1));
        match blur {
            Some((dx, dy, sigma, r)) => paint.g0 = [dx, dy, sigma, r as f32],
            None => paint.g0 = [w as f32, h as f32, 0.0, 0.0],
        }
        let pipeline = ctx.pipeline(Key {
            mode: Mode::Filter {
                blur: blur.is_some(),
            },
            blend: Blend::Replace,
            format: COLOR,
        });
        let verts = quad(Rect::new(0.0, 0.0, w as f32, h as f32));
        let vb = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&verts),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let ub = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::bytes_of(&paint),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let bind = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &ctx.bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ub.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(src),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&ctx.white),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&ctx.sampler),
                },
            ],
        });
        let aux = self.aux;
        if aux {
            let ctx = self.ctx.clone();
            self.mask_encoder.get_or_insert_with(|| {
                ctx.device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("appkit masks"),
                    })
            });
        } else {
            self.encoder();
        }
        let encoder = if aux {
            self.mask_encoder.as_mut()?
        } else {
            self.encoder.as_mut()?
        };
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("appkit filter"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind, &[0]);
            pass.set_vertex_buffer(0, vb.slice(..));
            pass.draw(0..6, 0..1);
        }
        Some(view)
    }

    /// `src` (`w` × `h`) blurred by a Gaussian of σ pixels, reduced first for a large σ.
    fn blur_texture(
        &mut self,
        src: &wgpu::TextureView,
        w: u32,
        h: u32,
        sigma: f32,
    ) -> Option<wgpu::TextureView> {
        if sigma.is_nan() || sigma <= 0.05 || !sigma.is_finite() {
            return self.filter_pass(src, w, h, None);
        }
        let mut k = 1u32;
        while sigma / (k as f32) >= 6.0 && k < 8 {
            k *= 2;
        }
        let mut cur = src.clone();
        let (mut cw, mut ch) = (w, h);
        let mut f = 1;
        while f < k {
            cw = cw.div_ceil(2);
            ch = ch.div_ceil(2);
            cur = self.filter_pass(&cur, cw, ch, None)?;
            f *= 2;
        }
        let s = sigma / k as f32;
        let r = ((s * 3.0).ceil() as i32).clamp(1, 96);
        let a = self.filter_pass(&cur, cw, ch, Some((1.0, 0.0, s, r)))?;
        self.filter_pass(&a, cw, ch, Some((0.0, 1.0, s, r)))
    }

    /// Pushes a clip by `outline` (device pixels) or by its complement. The mask itself is
    /// rendered when something is drawn under it, for the region of that draw.
    fn push_mask(&mut self, outline: &Path, invert: bool) {
        if outline.is_empty() {
            return;
        }
        self.masks.push(MaskEntry {
            path: outline.clone(),
            invert,
            target: None,
        });
    }

    /// Makes every mask of the stack cover `want` (device pixels).
    fn ensure_masks(&mut self, want: Rect) {
        let region = [
            want.x.floor().max(0.0) as u32,
            want.y.floor().max(0.0) as u32,
            (want.right().ceil().max(0.0) as u32)
                .saturating_sub(want.x.floor().max(0.0) as u32)
                .max(1),
            (want.bottom().ceil().max(0.0) as u32)
                .saturating_sub(want.y.floor().max(0.0) as u32)
                .max(1),
        ];
        let covers = |r: &[u32; 4]| {
            r[0] <= region[0]
                && r[1] <= region[1]
                && r[0] + r[2] >= region[0] + region[2]
                && r[1] + r[3] >= region[1] + region[3]
        };
        let mut redo = false;
        for i in 0..self.masks.len() {
            let ok = self
                .masks
                .get(i)
                .and_then(|m| m.target.as_ref())
                .is_some_and(|t| covers(&t.region));
            if ok && !redo {
                continue;
            }
            redo = true;
            self.render_mask(i, region);
        }
    }

    /// Renders mask `i` for `region`, under the masks below it.
    fn render_mask(&mut self, i: usize, region: [u32; 4]) {
        // The frame's open pass keeps its draws: the mask goes to its own encoder.
        let held = (
            self.pass_on.take(),
            std::mem::take(&mut self.pending),
            std::mem::take(&mut self.uniforms),
            std::mem::take(&mut self.vertices),
        );
        let mut tail = self.masks.split_off(i);
        let Some(mut entry) = (!tail.is_empty()).then(|| tail.remove(0)) else {
            self.masks.extend(tail);
            (self.pass_on, self.pending, self.uniforms, self.vertices) = held;
            return;
        };
        if let Some(old) = entry.target.take() {
            self.retired.push(old);
        }
        let (w, h) = self.target_size();
        let mask = self.take_target(MASK, region);
        let saved_scissor = self.scissor;
        self.scissor = [
            region[0],
            region[1],
            region[0] + region[2],
            region[1] + region[3],
        ];
        let previous = !self.masks.is_empty();
        self.scratch = Some(mask);
        self.pass_on = Some(Where::Scratch);
        let mut white = identity_paint(w, h);
        white.color = [1.0; 4];
        white.g1[2] = if previous { 1.0 } else { 0.0 };
        let saved_transform = std::mem::replace(&mut self.transform, Affine::IDENTITY);
        if entry.invert {
            // The clip so far, minus the outline.
            let full = self.scissor_as_rect();
            self.draw(&quad(full), Mode::Simple, Blend::Over, &white, None);
            self.cover_fill(
                &entry.path.bez,
                entry.path.even_odd,
                &white,
                None,
                Blend::Erase,
            );
        } else {
            // The outline, times the clip so far (the mask multiplies in the fragment stage).
            self.cover_fill(
                &entry.path.bez,
                entry.path.even_odd,
                &white,
                None,
                Blend::Over,
            );
        }
        self.transform = saved_transform;
        self.end_pass();
        entry.target = self.scratch.take();
        self.scissor = saved_scissor;
        self.masks.push(entry);
        self.masks.extend(tail);
        (self.pass_on, self.pending, self.uniforms, self.vertices) = held;
    }

    fn rect_clip(&mut self, rect: Rect) -> bool {
        let [a, b, c, d, e, f] = self.transform.as_coeffs();
        if b.abs() > 1e-6 || c.abs() > 1e-6 || !(a.is_finite() && d.is_finite()) {
            return false;
        }
        let x0 = a * f64::from(rect.x) + e;
        let x1 = a * f64::from(rect.right()) + e;
        let y0 = d * f64::from(rect.y) + f;
        let y1 = d * f64::from(rect.bottom()) + f;
        self.intersect_scissor(Rect::new(
            x0.min(x1) as f32,
            y0.min(y1) as f32,
            (x0 - x1).abs() as f32,
            (y0 - y1).abs() as f32,
        ));
        true
    }

    fn rect_path(rect: Rect) -> Path {
        let mut b = crate::render::path::PathBuilder::new();
        b.add_rect(rect);
        b.detach()
    }

    /// The glyph outlines of a run, in current coordinates.
    fn glyph_outlines(font: &FontData, size: f32, coords: &[i16], glyphs: &[Glyph]) -> BezPath {
        use skrifa::MetadataProvider;
        let mut out = BezPath::new();
        let Ok(fref) = skrifa::FontRef::from_index(font.data.data(), font.index) else {
            return out;
        };
        let normalized: Vec<skrifa::instance::NormalizedCoord> = coords
            .iter()
            .map(|c| skrifa::instance::NormalizedCoord::from_bits(*c))
            .collect();
        let location = skrifa::instance::LocationRef::new(&normalized);
        let outlines = fref.outline_glyphs();
        for g in glyphs {
            let Some(glyph) = outlines.get(skrifa::GlyphId::new(g.id)) else {
                continue;
            };
            let mut pen = Pen(BezPath::new());
            let settings = skrifa::outline::DrawSettings::unhinted(
                skrifa::instance::Size::new(size),
                location,
            );
            if glyph.draw(settings, &mut pen).is_err() {
                continue;
            }
            let place = Affine::translate((f64::from(g.x), f64::from(g.y)))
                * Affine::scale_non_uniform(1.0, -1.0);
            out.extend((place * pen.0).elements().iter().copied());
        }
        out
    }

    /// Reads the frame back: the CPU canvas's pixmap (premultiplied RGBA8).
    pub fn into_pixmap(&mut self) -> Pixmap {
        self.restore_to(0);
        self.submit();
        let (w, h) = (self.width, self.height);
        let mut pm = Pixmap::new(w.min(65535) as u16, h.min(65535) as u16);
        let row = (w * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buf = self.ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: u64::from(row) * u64::from(h),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self
            .ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        enc.copy_texture_to_buffer(
            self.main.color.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.ctx.queue.submit([enc.finish()]);
        let slice = buf.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.ctx.device.poll(wgpu::PollType::wait_indefinitely());
        {
            let Ok(data) = slice.get_mapped_range() else {
                return pm;
            };
            let dst = pm.data_as_u8_slice_mut();
            for y in 0..h as usize {
                let s = y * row as usize;
                let d = y * w as usize * 4;
                if let (Some(a), Some(b)) = (
                    data.get(s..s + w as usize * 4),
                    dst.get_mut(d..d + w as usize * 4),
                ) {
                    b.copy_from_slice(a);
                }
            }
        }
        buf.unmap();
        pm
    }
}

/// A skrifa pen into kurbo.
struct Pen(BezPath);

impl skrifa::outline::OutlinePen for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to((f64::from(x), f64::from(y)));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to((f64::from(x), f64::from(y)));
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.0.quad_to(
            (f64::from(cx0), f64::from(cy0)),
            (f64::from(x), f64::from(y)),
        );
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.0.curve_to(
            (f64::from(cx0), f64::from(cy0)),
            (f64::from(cx1), f64::from(cy1)),
            (f64::from(x), f64::from(y)),
        );
    }
    fn close(&mut self) {
        self.0.close_path();
    }
}

impl Painter for GpuCanvas {
    fn as_canvas(&mut self) -> Option<&mut crate::render::canvas::Canvas> {
        None
    }

    fn save(&mut self) {
        self.stack.push(Saved {
            transform: self.transform,
            scissor: self.scissor,
            masks: self.masks.len(),
            layers: self.layers.len(),
        });
    }

    fn restore(&mut self) {
        let Some(s) = self.stack.pop() else {
            return;
        };
        while self.layers.len() > s.layers {
            self.close_layer();
        }
        let dropped = self.masks.split_off(s.masks.min(self.masks.len()));
        self.free
            .extend(dropped.into_iter().filter_map(|m| m.target));
        self.scissor = s.scissor;
        self.transform = s.transform;
    }

    fn save_count(&self) -> usize {
        self.stack.len()
    }

    fn restore_to(&mut self, count: usize) {
        while self.stack.len() > count {
            self.restore();
        }
    }

    fn transform(&self) -> Transform {
        Transform {
            m: self.transform.as_coeffs().map(|v| v as f32),
        }
    }

    fn concat(&mut self, t: Transform) {
        self.transform *= t.to_affine();
    }

    fn translate(&mut self, (x, y): (f32, f32)) {
        self.transform *= Affine::translate((f64::from(x), f64::from(y)));
    }

    fn scale(&mut self, (sx, sy): (f32, f32)) {
        self.transform *= Affine::scale_non_uniform(f64::from(sx), f64::from(sy));
    }

    fn clip_rect(&mut self, rect: Rect) {
        if !self.rect_clip(rect) {
            self.push_mask(
                &GpuCanvas::rect_path(rect).transformed(self.transform_t()),
                false,
            );
        }
    }

    fn clip_path(&mut self, path: &Path) {
        self.push_mask(&path.transformed(self.transform_t()), false);
    }

    fn clip_out_path(&mut self, path: &Path) {
        self.push_mask(&path.transformed(self.transform_t()), true);
    }

    fn push_opacity(&mut self, opacity: f32) {
        let o = if opacity.is_finite() {
            opacity.clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.push_layer(LayerKind::Opacity(o));
    }

    fn push_additive(&mut self) {
        self.push_layer(LayerKind::Additive);
    }

    fn push_blur(&mut self, sigma: f32) {
        if sigma > 0.0 && sigma.is_finite() {
            self.push_layer(LayerKind::Blur(sigma));
        } else {
            self.push_layer(LayerKind::Opacity(1.0));
        }
    }

    fn backdrop(&mut self, outline: &Path, mut bd: Backdrop) {
        if outline.is_empty() {
            return;
        }
        bd.to_device = self.transform_t();
        let dev = outline.transformed(bd.to_device);
        let b = dev.bounds().intersect(self.scissor_as_rect());
        if b.is_empty() {
            return;
        }
        // Where samples can come from: the region grown by the blur's and the refraction's reach.
        let refract_reach = bd.glass.map_or(0.0, |(r, g)| {
            let band = g.depth.max(0.0) * r.width.min(r.height) / 2.0;
            let lens = if (g.magnification - 1.0).abs() > 1e-3 {
                r.width.max(r.height)
            } else {
                0.0
            };
            (band * (g.distortion.max(0.0) + g.chromatic_aberration.max(0.0)) + lens)
                * bd.to_device.m[0].abs().max(bd.to_device.m[3].abs())
        });
        let reach = (bd.sigma.max(0.0) * 3.0 + refract_reach).ceil() + 2.0;
        let (fw, fh) = (self.width as f32, self.height as f32);
        let sx0 = (b.x - reach).floor().max(0.0) as u32;
        let sy0 = (b.y - reach).floor().max(0.0) as u32;
        let sx1 = ((b.right() + reach).ceil().min(fw)) as u32;
        let sy1 = ((b.bottom() + reach).ceil().min(fh)) as u32;
        if sx1 <= sx0 || sy1 <= sy0 {
            return;
        }
        let (sw, sh) = (sx1 - sx0, sy1 - sy0);
        // What is drawn so far: the frame (the groups open now are composited later).
        self.end_pass();
        let ctx = self.ctx.clone();
        let region = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("backdrop"),
            size: wgpu::Extent3d {
                width: sw,
                height: sh,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: COLOR,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let src = self.main.color.clone();
        self.encoder().copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &src,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: sx0,
                    y: sy0,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            region.as_image_copy(),
            wgpu::Extent3d {
                width: sw,
                height: sh,
                depth_or_array_layers: 1,
            },
        );
        let rview = region.create_view(&wgpu::TextureViewDescriptor::default());
        let Some(plane) = self.blur_texture(&rview, sw, sh, bd.sigma) else {
            return;
        };
        let (w, h) = self.target_size();
        let mut p = identity_paint(w, h);
        p.g1 = [5.0, 0.0, if self.masks.is_empty() { 0.0 } else { 1.0 }, 0.0];
        p.color = [1.0, 0.0, 0.0, 0.0];
        p.g2 = [sx0 as f32, sy0 as f32, sw as f32, sh as f32];
        let inv = bd.to_device.invert().unwrap_or(Transform::IDENTITY).m;
        p.inv_a = [inv[0], inv[1], inv[2], inv[3]];
        p.inv_b[0] = inv[4];
        p.inv_b[1] = inv[5];
        p.fwd_a = [
            bd.to_device.m[0],
            bd.to_device.m[1],
            bd.to_device.m[2],
            bd.to_device.m[3],
        ];
        p.fwd_b = [bd.to_device.m[4], bd.to_device.m[5], 0.0, 0.0];
        let m = backdrop::saturation_brightness_matrix(bd.saturation, bd.brightness);
        p.m0 = [m[0][0], m[0][1], m[0][2], 0.0];
        p.m1 = [m[1][0], m[1][1], m[1][2], 0.0];
        p.m2 = [m[2][0], m[2][1], m[2][2], 0.0];
        if let Some((rect, g)) = bd.glass {
            let band = g.depth.max(0.0) * rect.width.min(rect.height) / 2.0;
            let magnify = g.magnification.is_finite() && (g.magnification - 1.0).abs() > 1e-3;
            if (band > 0.0 && g.distortion > 0.0) || magnify {
                p.g3 = [rect.x, rect.y, rect.width, rect.height];
                p.g4 = [
                    g.distortion,
                    g.depth,
                    g.chromatic_aberration,
                    g.magnification,
                ];
                p.g5 = [
                    rect.width.min(rect.height) / 2.0,
                    crate::shape::AMALFI_SMOOTHING,
                    1.0,
                    0.0,
                ];
            }
        }
        self.cover_fill(
            &outline.bez,
            outline.even_odd,
            &p,
            Some(&plane),
            Blend::Over,
        );
    }

    fn fill_blurred_path(&mut self, path: &Path, sigma: f32, color: Color) {
        if path.is_empty() || color.a == 0 {
            return;
        }
        if !(sigma > 0.0 && sigma.is_finite()) {
            self.fill_path(path, &Brush::Solid(color), 1.0);
            return;
        }
        let dev = path.transformed(self.transform_t());
        let k = self.transform.determinant().abs().sqrt() as f32;
        let s = sigma * k;
        let reach = s * 3.0 + 2.0;
        let b = dev
            .bounds()
            .inflate(reach, reach)
            .intersect(self.scissor_as_rect());
        if b.is_empty() {
            return;
        }
        let region = [
            b.x.floor() as u32,
            b.y.floor() as u32,
            ((b.right().ceil() as u32).saturating_sub(b.x.floor() as u32)).max(1),
            ((b.bottom().ceil() as u32).saturating_sub(b.y.floor() as u32)).max(1),
        ];
        // The shape's coverage in a target of its own, blurred there, then drawn in the colour.
        let held = (
            self.pass_on.take(),
            std::mem::take(&mut self.pending),
            std::mem::take(&mut self.uniforms),
            std::mem::take(&mut self.vertices),
        );
        let target = self.take_target(COLOR, region);
        let saved_scissor = self.scissor;
        let saved_masks = std::mem::take(&mut self.masks);
        let saved_transform = std::mem::replace(&mut self.transform, Affine::IDENTITY);
        self.scissor = [
            region[0],
            region[1],
            region[0] + region[2],
            region[1] + region[3],
        ];
        self.scratch = Some(target);
        self.pass_on = Some(Where::Scratch);
        let (w, h) = self.target_size();
        let mut white = identity_paint(w, h);
        white.color = [1.0; 4];
        self.cover_fill(&dev.bez, dev.even_odd, &white, None, Blend::Over);
        self.aux = true;
        self.end_pass();
        let scratch = self.scratch.take();
        let blurred = scratch
            .as_ref()
            .and_then(|t| self.blur_texture(&t.view.clone(), region[2], region[3], s));
        self.aux = false;
        self.transform = saved_transform;
        self.masks = saved_masks;
        self.scissor = saved_scissor;
        (self.pass_on, self.pending, self.uniforms, self.vertices) = held;
        self.retired.extend(scratch);
        let Some(view) = blurred else {
            return;
        };
        let mut p = identity_paint(w, h);
        p.g1 = [4.0, 0.0, if self.masks.is_empty() { 0.0 } else { 1.0 }, 0.0];
        p.g0 = [region[2] as f32, region[3] as f32, 0.0, 0.0];
        p.inv_b[0] = -(region[0] as f32);
        p.inv_b[1] = -(region[1] as f32);
        // The blurred coverage is white: the colour is applied by the shader through `color`.
        p.color = premul(color, 1.0);
        p.g1[3] = 1.0;
        let area = Rect::new(
            region[0] as f32,
            region[1] as f32,
            region[2] as f32,
            region[3] as f32,
        )
        .intersect(self.scissor_as_rect());
        self.mark(area);
        self.draw(&quad(area), Mode::Simple, Blend::Over, &p, Some(&view));
    }

    fn fill_path(&mut self, path: &Path, brush: &Brush, opacity: f32) {
        if path.is_empty() {
            return;
        }
        let p = self.uniform_for(brush, opacity);
        self.cover_fill(&path.bez, path.even_odd, &p, None, Blend::Over);
    }

    fn fill_rect(&mut self, rect: Rect, color: Color) {
        if rect.is_empty() || color.a == 0 {
            return;
        }
        let p = self.uniform_for(&Brush::Solid(color), 1.0);
        let t = self.transform;
        let pt = |x: f32, y: f32| {
            let q = t * kurbo::Point::new(f64::from(x), f64::from(y));
            [q.x as f32, q.y as f32]
        };
        let corners = [
            pt(rect.x, rect.y),
            pt(rect.right(), rect.y),
            pt(rect.right(), rect.bottom()),
            pt(rect.x, rect.bottom()),
        ];
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for c in corners {
            x0 = x0.min(c[0]);
            y0 = y0.min(c[1]);
            x1 = x1.max(c[0]);
            y1 = y1.max(c[1]);
        }
        self.mark(Rect::new(x0, y0, x1 - x0, y1 - y0));
        self.draw(&quad_of(corners), Mode::Simple, Blend::Over, &p, None);
    }

    fn fill_blurred_rounded_rect(
        &mut self,
        rect: Rect,
        radius: f32,
        sigma: f32,
        color: Color,
        invert: bool,
    ) {
        if rect.is_empty() || color.a == 0 {
            return;
        }
        let t = self.transform;
        let c = t.as_coeffs();
        let k = (c[0] * c[3] - c[1] * c[2]).abs().sqrt() as f32;
        let a = t * kurbo::Point::new(f64::from(rect.x), f64::from(rect.y));
        let b = t * kurbo::Point::new(f64::from(rect.right()), f64::from(rect.bottom()));
        let dev = Rect::new(
            a.x.min(b.x) as f32,
            a.y.min(b.y) as f32,
            (a.x - b.x).abs() as f32,
            (a.y - b.y).abs() as f32,
        );
        let s = sigma.abs() * k;
        let mut p = self.uniform_for(&Brush::Solid(color), 1.0);
        p.inv_a = [1.0, 0.0, 0.0, 1.0];
        p.inv_b[0] = 0.0;
        p.inv_b[1] = 0.0;
        p.g1[0] = 6.0;
        p.g0 = [dev.x, dev.y, dev.width, dev.height];
        p.g2 = [
            s.max(0.01),
            radius.max(0.0) * k,
            if invert { 1.0 } else { 0.0 },
            0.0,
        ];
        let area = if invert {
            self.scissor_as_rect()
        } else {
            dev.inflate(s * 3.0 + 1.0, s * 3.0 + 1.0)
                .intersect(self.scissor_as_rect())
        };
        if area.is_empty() {
            return;
        }
        self.mark(area);
        self.draw(&quad(area), Mode::Simple, Blend::Over, &p, None);
    }

    fn stroke_path(&mut self, path: &Path, brush: &Brush, opacity: f32, style: &StrokeStyle) {
        if path.is_empty() || style.width.is_nan() || style.width <= 0.0 {
            return;
        }
        let mut s = kurbo::Stroke::new(f64::from(style.width));
        if style.round {
            s = s.with_caps(kurbo::Cap::Round).with_join(kurbo::Join::Round);
        } else {
            s = s.with_caps(kurbo::Cap::Butt).with_join(kurbo::Join::Miter);
        }
        if style.dash.len() >= 2 {
            s = s.with_dashes(0.0, style.dash.iter().map(|d| f64::from(*d)));
        }
        let scale = self.transform.determinant().abs().sqrt().max(1e-3);
        let outline = kurbo::stroke(
            path.bez.iter(),
            &s,
            &kurbo::StrokeOpts::default(),
            0.1 / scale,
        );
        let p = self.uniform_for(brush, opacity);
        self.cover_fill(&outline, false, &p, None, Blend::Over);
    }

    fn draw_image(&mut self, image: &Image, dst: Rect, opacity: f32) {
        if dst.is_empty() || image.width() == 0 || image.height() == 0 {
            return;
        }
        let Some(view) = self.ctx.image_view(image) else {
            return;
        };
        let (iw, ih) = (image.width() as f32, image.height() as f32);
        let m = self.transform
            * Affine::translate((f64::from(dst.x), f64::from(dst.y)))
            * Affine::scale_non_uniform(f64::from(dst.width / iw), f64::from(dst.height / ih));
        let inv = m.inverse().as_coeffs();
        let (w, h) = self.target_size();
        let mut p = identity_paint(w, h);
        p.inv_a = [inv[0] as f32, inv[1] as f32, inv[2] as f32, inv[3] as f32];
        p.inv_b[0] = inv[4] as f32;
        p.inv_b[1] = inv[5] as f32;
        p.g1 = [4.0, 0.0, if self.masks.is_empty() { 0.0 } else { 1.0 }, 0.0];
        p.g0 = [iw, ih, 0.0, 0.0];
        p.color = [opacity.clamp(0.0, 1.0), 0.0, 0.0, 0.0];
        let t = self.transform;
        let pt = |x: f32, y: f32| {
            let q = t * kurbo::Point::new(f64::from(x), f64::from(y));
            [q.x as f32, q.y as f32]
        };
        let corners = [
            pt(dst.x, dst.y),
            pt(dst.right(), dst.y),
            pt(dst.right(), dst.bottom()),
            pt(dst.x, dst.bottom()),
        ];
        self.draw(
            &quad_of(corners),
            Mode::Simple,
            Blend::Over,
            &p,
            Some(&view),
        );
        let b = kurbo::Rect::new(
            f64::from(dst.x),
            f64::from(dst.y),
            f64::from(dst.right()),
            f64::from(dst.bottom()),
        );
        let bb = t.transform_rect_bbox(b);
        self.mark(Rect::new(
            bb.x0 as f32,
            bb.y0 as f32,
            bb.width() as f32,
            bb.height() as f32,
        ));
    }

    fn fill_glyphs(
        &mut self,
        font: &FontData,
        size: f32,
        coords: &[i16],
        glyphs: &[Glyph],
        color: Color,
    ) {
        if glyphs.is_empty() || color.a == 0 {
            return;
        }
        let outline = GpuCanvas::glyph_outlines(font, size, coords, glyphs);
        if outline.elements().is_empty() {
            return;
        }
        let p = self.uniform_for(&Brush::Solid(color), 1.0);
        self.cover_fill(&outline, false, &p, None, Blend::Over);
    }
}

impl GpuCanvas {
    /// A target for the device `region`, reusing a closed one of the same size.
    fn take_target(&mut self, format: wgpu::TextureFormat, region: [u32; 4]) -> Target {
        let size = |r: &[u32; 4]| (r[2], r[3]);
        if let Some(i) = self
            .free
            .iter()
            .position(|t| t.format == format && size(&t.region) == size(&region))
        {
            let mut t = self.free.swap_remove(i);
            t.region = region;
            return t;
        }
        Target::new(&self.ctx, region, format)
    }

    fn transform_t(&self) -> Transform {
        Transform {
            m: self.transform.as_coeffs().map(|v| v as f32),
        }
    }

    fn push_layer(&mut self, kind: LayerKind) {
        self.end_pass();
        let region = self.scissor_region();
        let target = self.take_target(COLOR, region);
        self.layers.push(Layer {
            kind,
            target,
            scissor: self.scissor,
            masks: self.masks.len(),
            bounds: None,
        });
        // Even a layer nothing is drawn in is cleared.
        self.pass_on = Some(self.current());
    }
}

#[cfg(test)]
mod tests;
