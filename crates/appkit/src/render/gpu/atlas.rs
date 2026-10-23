//! The glyph atlas: the coverage of a glyph, rasterized once (by the CPU canvas, so the edges
//! are the CPU path's) and kept in an `R8` texture. A run then costs one textured quad per
//! glyph instead of a stencil-and-cover fill.
//!
//! Entries are keyed like the outline cache of [`glyphs`] (font blob, face, glyph, variation
//! coordinates) plus what a raster depends on: the size in device pixels (1/8 px steps) and
//! the position inside the pixel (quarter-pixel steps on both axes). When the texture is full
//! a new one replaces it: draws already recorded keep the old texture alive.

use std::collections::HashMap;

use super::*;
use crate::render::glyphs;

/// The side of an atlas texture.
pub(super) const SIDE: u32 = 2048;
/// Largest cell (a glyph bigger than this in device pixels is filled as a path).
const MAX_CELL: u32 = 192;
/// Subpixel steps per pixel on each axis.
pub(super) const STEPS: u32 = 4;

#[derive(Hash, PartialEq, Eq)]
struct GlyphKey {
    blob: u64,
    index: u32,
    glyph: u32,
    /// A hash of the variation coordinates.
    coords: u64,
    /// The size in device pixels, in eighths.
    size8: u32,
    /// `y * STEPS + x`: the quarter pixel the glyph's origin falls on.
    sub: u8,
}

/// Where a glyph is in the atlas and how its cell sits against the glyph's origin pixel.
#[derive(Debug, Clone, Copy)]
pub(super) struct Slot {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    /// The cell's offset from the pixel the origin falls in.
    pub ox: i32,
    pub oy: i32,
}

/// What looking a path up in the path atlas gave.
pub(super) enum Cell {
    /// The coverage of the rectangle (device pixels) is at the slot of the texture.
    Hit(Rect, Slot, wgpu::TextureView),
    /// The path covers nothing inside the clip rectangle.
    Empty,
    /// The transform, the size or this frame's budget does not suit the atlas.
    No,
}

/// What looking a glyph up gave.
pub(super) enum Lookup {
    /// In the atlas texture `view`.
    Hit(Slot, wgpu::TextureView),
    /// Nothing to draw (a space, or no outline).
    Empty,
    /// Too big for a cell: fill the outline.
    TooBig,
}

struct Shelf {
    y: u32,
    h: u32,
    x: u32,
}

pub(super) struct Atlas {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    shelves: Vec<Shelf>,
    next_y: u32,
    slots: HashMap<GlyphKey, Option<Slot>>,
    /// Path coverages, by [`path_key`].
    cells: HashMap<u64, Option<Slot>>,
}

impl Atlas {
    pub(super) fn new(device: &wgpu::Device) -> Atlas {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph atlas"),
            size: wgpu::Extent3d {
                width: SIDE,
                height: SIDE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: MASK,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Atlas {
            texture,
            view,
            shelves: Vec::new(),
            next_y: 0,
            slots: HashMap::new(),
            cells: HashMap::new(),
        }
    }

    /// A `w` × `h` cell on a shelf, or `None` when the texture is full.
    fn alloc(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        let hh = h.next_multiple_of(8);
        for s in &mut self.shelves {
            if s.h == hh && s.x + w <= SIDE {
                let at = (s.x, s.y);
                s.x += w;
                return Some(at);
            }
        }
        if self.next_y + hh > SIDE || w > SIDE {
            return None;
        }
        let y = self.next_y;
        self.next_y += hh;
        self.shelves.push(Shelf { y, h: hh, x: w });
        Some((0, y))
    }
}

/// A rasterized cell.
struct Raster {
    w: u32,
    h: u32,
    ox: i32,
    oy: i32,
    coverage: Vec<u8>,
}

enum Rastered {
    Cell(Raster),
    Empty,
    TooBig,
}

/// Rasterizes `glyph` at `size8`/8 device pixels with its origin at the quarter pixel `sub`.
fn rasterize(font: &FontData, coords: &[i16], glyph: u32, size8: u32, sub: u8) -> Rastered {
    use kurbo::Shape;
    let Some(path) = glyphs::outline(font, coords, glyph) else {
        return Rastered::Empty;
    };
    let ds = f64::from(size8) / 8.0;
    let step = f64::from(STEPS);
    let fx = f64::from(u32::from(sub) % STEPS) / step;
    let fy = f64::from(u32::from(sub) / STEPS) / step;
    let bb = path.bounding_box();
    if !(bb.width() > 0.0 || bb.height() > 0.0) || !bb.x0.is_finite() || !bb.y1.is_finite() {
        return Rastered::Empty;
    }
    let ox = (bb.x0 * ds + fx).floor() as i32 - 1;
    let oy = (bb.y0 * ds + fy).floor() as i32 - 1;
    let w = ((bb.x1 * ds + fx).ceil() as i32 + 1 - ox).max(1) as u32;
    let h = ((bb.y1 * ds + fy).ceil() as i32 + 1 - oy).max(1) as u32;
    if w > MAX_CELL || h > MAX_CELL {
        return Rastered::TooBig;
    }
    let t = Affine::translate((f64::from(-ox) + fx, f64::from(-oy) + fy)) * Affine::scale(ds);
    let mut cpu = Canvas::new(w, h);
    let shape = Path {
        bez: t * (*path).clone(),
        even_odd: false,
    };
    Painter::fill_path(
        &mut cpu,
        &shape,
        &Brush::Solid(Color::rgb(255, 255, 255)),
        1.0,
    );
    let pm = cpu.into_pixmap();
    let coverage = pm
        .data_as_u8_slice()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|&[_, _, _, a]| a)
        .collect();
    Rastered::Cell(Raster {
        w,
        h,
        ox,
        oy,
        coverage,
    })
}

impl GpuContext {
    /// The atlas cell of a glyph, rasterized and uploaded on first use.
    pub(super) fn glyph_slot(
        &self,
        font: &FontData,
        coords: &[i16],
        coords_key: u64,
        glyph: u32,
        size8: u32,
        sub: u8,
    ) -> Lookup {
        let key = GlyphKey {
            blob: font.data.id(),
            index: font.index,
            glyph,
            coords: coords_key,
            size8,
            sub,
        };
        let mut atlas = self.atlas.borrow_mut();
        if let Some(hit) = atlas.slots.get(&key) {
            return match hit {
                Some(s) => Lookup::Hit(*s, atlas.view.clone()),
                None => Lookup::Empty,
            };
        }
        let started_at = std::time::Instant::now();
        let rastered = rasterize(font, coords, glyph, size8, sub);
        super::profile(|p| {
            p.glyphs += 1;
            p.glyph_us += started_at.elapsed().as_micros() as u64;
        });
        let r = match rastered {
            Rastered::Cell(r) => r,
            Rastered::Empty => {
                atlas.slots.insert(key, None);
                return Lookup::Empty;
            }
            Rastered::TooBig => return Lookup::TooBig,
        };
        let Some(slot) = self.upload(&mut atlas, &r) else {
            return Lookup::TooBig;
        };
        atlas.slots.insert(key, Some(slot));
        Lookup::Hit(slot, atlas.view.clone())
    }

    /// Puts a cell in `atlas` (a new texture when it is full: recorded draws keep the old
    /// one).
    fn upload(&self, atlas: &mut Atlas, r: &Raster) -> Option<Slot> {
        let at = match atlas.alloc(r.w, r.h) {
            Some(at) => at,
            None => {
                *atlas = Atlas::new(&self.device);
                atlas.alloc(r.w, r.h)?
            }
        };
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &atlas.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: at.0,
                    y: at.1,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &r.coverage,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(r.w),
                rows_per_image: Some(r.h),
            },
            wgpu::Extent3d {
                width: r.w,
                height: r.h,
                depth_or_array_layers: 1,
            },
        );
        Some(Slot {
            x: at.0,
            y: at.1,
            w: r.w,
            h: r.h,
            ox: r.ox,
            oy: r.oy,
        })
    }
}

/// The largest side of a path cell, in device pixels.
const PATH_SIDE: u32 = 1024;
/// The most device pixels a path cell may have.
const PATH_AREA: u32 = 256 * 1024;
/// The most device pixels of path coverage rasterized in one frame; past it a path is
/// filled by stencil and cover.
pub(super) const PATH_BUDGET: u64 = 1 << 20;
/// The most time a frame spends rasterizing path cells: a page shown for the first time
/// fills the atlas over a few frames instead of stalling one (on a slow CPU, blurred cells
/// cost milliseconds each).
pub(super) const PATH_TIME: std::time::Duration = std::time::Duration::from_millis(3);

/// A hash of what a path cell depends on: the path, the linear part of the transform, the
/// quarter pixel its box starts on, the fill rule and the stroke.
fn path_key(
    bez: &BezPath,
    scale: (f64, f64),
    sub: (u32, u32),
    even_odd: bool,
    stroke: Option<&StrokeStyle>,
    blur: f32,
) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for el in bez.elements() {
        let pts: &[kurbo::Point] = match el {
            PathEl::MoveTo(p) | PathEl::LineTo(p) => {
                0u8.hash(&mut h);
                std::slice::from_ref(p)
            }
            PathEl::QuadTo(a, b) => {
                1u8.hash(&mut h);
                &[*a, *b]
            }
            PathEl::CurveTo(a, b, c) => {
                2u8.hash(&mut h);
                &[*a, *b, *c]
            }
            PathEl::ClosePath => {
                3u8.hash(&mut h);
                &[]
            }
        };
        for p in pts {
            p.x.to_bits().hash(&mut h);
            p.y.to_bits().hash(&mut h);
        }
    }
    scale.0.to_bits().hash(&mut h);
    scale.1.to_bits().hash(&mut h);
    sub.hash(&mut h);
    even_odd.hash(&mut h);
    blur.to_bits().hash(&mut h);
    if let Some(s) = stroke {
        s.width.to_bits().hash(&mut h);
        s.round.hash(&mut h);
        for d in &s.dash {
            d.to_bits().hash(&mut h);
        }
    }
    h.finish()
}

/// A hash of variation coordinates.
fn coords_key(coords: &[i16]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    coords.hash(&mut h);
    h.finish()
}

impl GpuCanvas {
    /// Draws a run from the atlas; `false` when the transform or the size does not suit it
    /// (the caller fills outlines).
    pub(super) fn fill_glyphs_atlas(
        &mut self,
        font: &FontData,
        size: f32,
        coords: &[i16],
        glyphs: &[Glyph],
        color: Color,
    ) -> bool {
        let [a, b, c, d, e, f] = self.transform.as_coeffs();
        // Axis-aligned, uniformly scaled, not mirrored.
        if b.abs() > 1e-4 || c.abs() > 1e-4 || a <= 0.0 || d <= 0.0 || (a - d).abs() > 1e-3 * a {
            return false;
        }
        let ds = f64::from(size) * a;
        if !ds.is_finite() || !(2.0..=120.0).contains(&ds) || !crate::render::glyphs::plain(font) {
            return false;
        }
        let size8 = (ds * 8.0).round() as u32;
        let base = self.uniform_for(&Brush::Solid(color), 1.0);
        let step = f64::from(STEPS);
        let ck = coords_key(coords);
        let side = SIDE as f32;
        let mut quads: Vec<BatchV> = Vec::with_capacity(glyphs.len() * 6);
        let mut quads_view: Option<wgpu::TextureView> = None;
        for g in glyphs {
            let px = a * f64::from(g.x) + e;
            let py = d * f64::from(g.y) + f;
            if !px.is_finite() || !py.is_finite() {
                continue;
            }
            let split = |v: f64| {
                let mut i = v.floor();
                let mut q = ((v - i) * step).round();
                if q >= step {
                    i += 1.0;
                    q = 0.0;
                }
                (i as i32, q as u32)
            };
            let ((ix, qx), (iy, qy)) = (split(px), split(py));
            let sub = (qy * STEPS + qx) as u8;
            match self.ctx.glyph_slot(font, coords, ck, g.id, size8, sub) {
                Lookup::Empty => {}
                Lookup::TooBig => {
                    let one =
                        GpuCanvas::glyph_outlines(font, size, coords, std::slice::from_ref(g));
                    if !one.elements().is_empty() {
                        self.cover_fill(&one, false, &base, None, Blend::Over);
                    }
                }
                Lookup::Hit(s, view) => {
                    let (x0, y0) = (ix + s.ox, iy + s.oy);
                    let r = Rect::new(x0 as f32, y0 as f32, s.w as f32, s.h as f32);
                    if !r.intersects(self.scissor_as_rect()) {
                        continue;
                    }
                    self.mark(r);
                    if self.scratch.is_some() {
                        let mut p = base;
                        p.inv_a = [1.0, 0.0, 0.0, 1.0];
                        p.inv_b[0] = s.x as f32 - x0 as f32;
                        p.inv_b[1] = s.y as f32 - y0 as f32;
                        p.g0 = [side, side, 0.0, 0.0];
                        p.g1[0] = 4.0;
                        // Kind 4 with 2 in w: the red channel is a coverage, tinted.
                        p.g1[3] = 2.0;
                        self.draw(&quad(r), Mode::Simple, Blend::Over, &p, Some(&view));
                        continue;
                    }
                    // A new atlas texture ends the run so far.
                    if quads_view.as_ref().is_some_and(|v| *v != view) {
                        let done = std::mem::take(&mut quads);
                        self.draw_batch(&done, quads_view.as_ref());
                    }
                    let uv = [
                        s.x as f32 / side,
                        s.y as f32 / side,
                        (s.x + s.w) as f32 / side,
                        (s.y + s.h) as f32 / side,
                    ];
                    quads.extend(batch_quad(r, uv, base.color));
                    quads_view = Some(view);
                }
            }
        }
        if !quads.is_empty() {
            self.draw_batch(&quads, quads_view.as_ref());
        }
        true
    }

    /// Fills a path (or strokes it with `stroke`) with a solid colour from the path atlas:
    /// its coverage is rasterized once by the CPU canvas (so the edges are the CPU path's)
    /// and drawn as a batched quad. `false` when the transform, the size or this frame's
    /// budget does not suit it (the caller fills by stencil and cover).
    pub(super) fn fill_path_atlas(
        &mut self,
        bez: &BezPath,
        even_odd: bool,
        stroke: Option<&StrokeStyle>,
        color: Color,
        opacity: f32,
    ) -> bool {
        if self.scratch.is_some() {
            return false;
        }
        match self.path_cell(bez, even_odd, stroke, 0.0) {
            Cell::No => false,
            Cell::Empty => true,
            Cell::Hit(r, slot, view) => {
                self.cell_quad(r, slot, &view, premul(color, opacity.clamp(0.0, 1.0)));
                true
            }
        }
    }

    /// Fills a path blurred by σ `sigma` (device pixels) in a solid colour from the path
    /// atlas, blurred once by the CPU canvas; `false` when the atlas does not suit it.
    pub(super) fn fill_blurred_atlas(&mut self, path: &Path, sigma: f32, color: Color) -> bool {
        if self.scratch.is_some() {
            return false;
        }
        match self.path_cell(&path.bez, path.even_odd, None, sigma) {
            Cell::No => false,
            Cell::Empty => true,
            Cell::Hit(r, slot, view) => {
                self.cell_quad(r, slot, &view, premul(color, 1.0));
                true
            }
        }
    }

    /// Draws the coverage of a cell at `r` (device pixels), tinted by `color`.
    fn cell_quad(&mut self, r: Rect, slot: Slot, view: &wgpu::TextureView, color: [f32; 4]) {
        self.mark(r);
        let side = SIDE as f32;
        let uv = [
            slot.x as f32 / side,
            slot.y as f32 / side,
            (slot.x as f32 + r.width) / side,
            (slot.y as f32 + r.height) / side,
        ];
        self.draw_batch(&batch_quad(r, uv, color), Some(view));
    }

    /// The path atlas cell of a path (or of its stroke, or of the path blurred by σ `blur`
    /// in device pixels) under the transform, rasterized on a miss.
    pub(super) fn path_cell(
        &mut self,
        bez: &BezPath,
        even_odd: bool,
        stroke: Option<&StrokeStyle>,
        blur: f32,
    ) -> Cell {
        let [a, b, c, d, e, f] = self.transform.as_coeffs();
        // Axis-aligned and not mirrored: under a rotation every frame would be a new cell.
        if b.abs() > 1e-9 || c.abs() > 1e-9 || a <= 0.0 || d <= 0.0 {
            return Cell::No;
        }
        let cb = bez.control_box();
        // Miter joins reach up to the miter limit (4) times the half width.
        let grow = stroke.map_or(0.0, |s| {
            f64::from(s.width) * if s.round { 0.5 } else { 2.0 }
        });
        // A blur reaches 3σ, plus the edge's own pixel.
        let reach = if blur > 0.0 {
            f64::from(blur) * 3.0 + 2.0
        } else {
            0.0
        };
        let (ox, oy) = (a * (cb.x0 - grow) - reach, d * (cb.y0 - grow) - reach);
        let (dx0, dy0) = (ox + e, oy + f);
        let (dx1, dy1) = (
            a * (cb.x1 + grow) + reach + e,
            d * (cb.y1 + grow) + reach + f,
        );
        if ![dx0, dy0, dx1, dy1].iter().all(|v| v.is_finite()) {
            return Cell::No;
        }
        let (bw, bh) = (dx1 - dx0, dy1 - dy0);
        if bw <= 0.0 || bh <= 0.0 {
            // Nothing covered.
            return if stroke.is_none() {
                Cell::Empty
            } else {
                Cell::No
            };
        }
        let dev = Rect::new(dx0 as f32, dy0 as f32, bw as f32, bh as f32);
        if !dev.intersects(self.scissor_as_rect()) {
            return Cell::Empty;
        }
        let step = f64::from(STEPS);
        let split = |v: f64| {
            let mut i = v.floor();
            let mut q = ((v - i) * step).round();
            if q >= step {
                i += 1.0;
                q = 0.0;
            }
            (i, q)
        };
        let ((ix, qx), (iy, qy)) = (split(dx0), split(dy0));
        // One pixel of margin on every side.
        let w = (qx / step + bw).ceil() as u32 + 2;
        let h = (qy / step + bh).ceil() as u32 + 2;
        if w > PATH_SIDE || h > PATH_SIDE || w * h > PATH_AREA {
            return Cell::No;
        }
        let key = path_key(bez, (a, d), (qx as u32, qy as u32), even_odd, stroke, blur);
        let found = self.ctx.paths.borrow().cells.get(&key).copied();
        let slot = match found {
            Some(Some(s)) => s,
            Some(None) => return Cell::Empty,
            None => {
                let px = u64::from(w) * u64::from(h);
                if self.path_budget < px || std::time::Instant::now() >= self.path_deadline {
                    return Cell::No;
                }
                self.path_budget -= px;
                // The box moves to its quarter pixel (at most 1/8 px), inside the margin.
                let to_cell = Affine::translate((qx / step - ox + 1.0, qy / step - oy + 1.0))
                    * Affine::new([a, 0.0, 0.0, d, 0.0, 0.0]);
                let started_at = std::time::Instant::now();
                let rastered = rasterize_path(bez, even_odd, stroke, blur, to_cell, w, h);
                super::profile(|p| {
                    p.path_cells += 1;
                    p.path_us += started_at.elapsed().as_micros() as u64;
                });
                let Some(r) = rastered else {
                    self.ctx.paths.borrow_mut().cells.insert(key, None);
                    return Cell::Empty;
                };
                let mut paths = self.ctx.paths.borrow_mut();
                let Some(s) = self.ctx.upload(&mut paths, &r) else {
                    return Cell::No;
                };
                paths.cells.insert(key, Some(s));
                s
            }
        };
        let view = self.ctx.paths.borrow().view.clone();
        let r = Rect::new(ix as f32 - 1.0, iy as f32 - 1.0, w as f32, h as f32);
        Cell::Hit(r, slot, view)
    }
}

/// The coverage of a path (stroked, or blurred by σ `blur` in cell pixels) in a `w` × `h`
/// cell, through `to_cell`; `None` if it covers nothing.
fn rasterize_path(
    bez: &BezPath,
    even_odd: bool,
    stroke: Option<&StrokeStyle>,
    blur: f32,
    to_cell: Affine,
    w: u32,
    h: u32,
) -> Option<Raster> {
    let mut cpu = Canvas::new(w, h);
    let white = Brush::Solid(Color::rgb(255, 255, 255));
    match stroke {
        Some(style) => {
            let m = to_cell.as_coeffs().map(|v| v as f32);
            Painter::concat(&mut cpu, crate::geom::Transform { m });
            let path = Path {
                bez: bez.clone(),
                even_odd: false,
            };
            Painter::stroke_path(&mut cpu, &path, &white, 1.0, style);
        }
        None => {
            let path = Path {
                bez: to_cell * bez.clone(),
                even_odd,
            };
            Painter::fill_path(&mut cpu, &path, &white, 1.0);
        }
    }
    let pm = cpu.into_pixmap();
    let mut coverage: Vec<u8> = pm
        .data_as_u8_slice()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|&[_, _, _, a]| a)
        .collect();
    if blur > 0.0 {
        coverage = gaussian_blur(&coverage, w as usize, blur);
    }
    if coverage.iter().all(|&c| c == 0) {
        return None;
    }
    Some(Raster {
        w,
        h,
        ox: 0,
        oy: 0,
        coverage,
    })
}

/// `coverage` (rows of `w`) blurred by a Gaussian of σ `sigma` pixels, in two separable
/// passes; what falls outside the cell counts as empty.
fn gaussian_blur(coverage: &[u8], w: usize, sigma: f32) -> Vec<u8> {
    if w == 0 {
        return coverage.to_vec();
    }
    let r = (sigma * 3.0).ceil().max(1.0) as usize;
    let mut kernel: Vec<f32> = (0..=2 * r)
        .map(|i| {
            let x = i as f32 - r as f32;
            (-x * x / (2.0 * sigma * sigma)).exp()
        })
        .collect();
    let sum: f32 = kernel.iter().sum();
    kernel.iter_mut().for_each(|k| *k /= sum);
    // One pass along rows of `n`: zero padded on both sides.
    let pass = |data: &[f32], n: usize| -> Vec<f32> {
        let mut out = Vec::with_capacity(data.len());
        let mut padded = vec![0.0f32; n + 2 * r];
        for row in data.chunks(n) {
            padded.iter_mut().for_each(|v| *v = 0.0);
            for (d, s) in padded.iter_mut().skip(r).zip(row) {
                *d = *s;
            }
            out.extend(
                padded
                    .windows(2 * r + 1)
                    .take(row.len())
                    .map(|win| win.iter().zip(&kernel).map(|(a, k)| a * k).sum::<f32>()),
            );
        }
        out
    };
    let transpose = |data: &[f32], n: usize| -> Vec<f32> {
        let rows = data.len() / n;
        (0..n)
            .flat_map(|x| (0..rows).map(move |y| data.get(y * n + x).copied().unwrap_or(0.0)))
            .collect()
    };
    let h = coverage.len() / w;
    let data: Vec<f32> = coverage.iter().map(|&c| f32::from(c)).collect();
    let across = pass(&data, w);
    let down = pass(&transpose(&across, w), h);
    transpose(&down, h)
        .iter()
        .map(|v| v.round().clamp(0.0, 255.0) as u8)
        .collect()
}

#[cfg(test)]
impl GpuContext {
    /// The glyphs held by the current atlas texture.
    pub(super) fn atlas_len(&self) -> usize {
        self.atlas.borrow().slots.len()
    }
}
