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
    coords: Vec<i16>,
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
        glyph: u32,
        size8: u32,
        sub: u8,
    ) -> Lookup {
        let key = GlyphKey {
            blob: font.data.id(),
            index: font.index,
            glyph,
            coords: coords.to_vec(),
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
        let r = match rasterize(font, coords, glyph, size8, sub) {
            Rastered::Cell(r) => r,
            Rastered::Empty => {
                atlas.slots.insert(key, None);
                return Lookup::Empty;
            }
            Rastered::TooBig => return Lookup::TooBig,
        };
        let at = match atlas.alloc(r.w, r.h) {
            Some(at) => at,
            None => {
                // Full: start another texture; recorded draws keep the old one.
                *atlas = Atlas::new(&self.device);
                match atlas.alloc(r.w, r.h) {
                    Some(at) => at,
                    None => return Lookup::TooBig,
                }
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
        let slot = Slot {
            x: at.0,
            y: at.1,
            w: r.w,
            h: r.h,
            ox: r.ox,
            oy: r.oy,
        };
        atlas.slots.insert(key, Some(slot));
        Lookup::Hit(slot, atlas.view.clone())
    }
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
            match self.ctx.glyph_slot(font, coords, g.id, size8, sub) {
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
                    let mut p = base;
                    p.inv_a = [1.0, 0.0, 0.0, 1.0];
                    p.inv_b[0] = s.x as f32 - x0 as f32;
                    p.inv_b[1] = s.y as f32 - y0 as f32;
                    p.g0 = [SIDE as f32, SIDE as f32, 0.0, 0.0];
                    p.g1[0] = 4.0;
                    // Kind 4 with 2 in w: the red channel is a coverage, tinted by the colour.
                    p.g1[3] = 2.0;
                    let r = Rect::new(x0 as f32, y0 as f32, s.w as f32, s.h as f32);
                    self.mark(r);
                    self.draw(&quad(r), Mode::Simple, Blend::Over, &p, Some(&view));
                }
            }
        }
        true
    }
}

#[cfg(test)]
impl GpuContext {
    /// The glyphs held by the current atlas texture.
    pub(super) fn atlas_len(&self) -> usize {
        self.atlas.borrow().slots.len()
    }
}
