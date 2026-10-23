//! The layer texture cache: a subtree that has not changed for a few frames is rendered once
//! into a texture and composited from it, until something in it changes. This replaces the
//! CPU raster cache (`core::raster`) on the GPU path: nothing is rasterized on the CPU and
//! uploaded.
//!
//! The texture is a single-sample copy of the layer target, at whole device pixels (the
//! caller checked the alignment), kept by the context so that it outlives the canvas that
//! made it. Entries unused for a long while, or over the budget, are dropped.

use std::collections::HashMap;

use super::*;

/// The most bytes all the layer textures of a context may take.
const BUDGET: usize = 96 << 20;
/// The most device pixels one layer texture may have.
const MAX_PIXELS: u64 = 4096 * 4096;
/// Frames (canvas `begin` calls) an unused entry is kept.
const KEEP_FRAMES: u64 = 1800;

struct Entry {
    view: wgpu::TextureView,
    version: u64,
    scale: f32,
    size: (u32, u32),
    used: u64,
}

impl Entry {
    fn bytes(&self) -> usize {
        self.size.0 as usize * self.size.1 as usize * 4
    }
}

/// The cached layer textures of a context.
#[derive(Default)]
pub(super) struct LayerCache {
    entries: HashMap<u64, Entry>,
    clock: u64,
}

impl LayerCache {
    /// Counts a frame and drops what has not been used for long.
    pub(super) fn tick(&mut self) {
        self.clock += 1;
        let clock = self.clock;
        self.entries.retain(|_, e| e.used + KEEP_FRAMES >= clock);
    }

    fn bytes(&self) -> usize {
        self.entries.values().map(Entry::bytes).sum()
    }

    /// The number of textures held.
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }
}

impl GpuContext {
    /// The texture of `key` when it is `version`, `scale` and `size`.
    fn layer_hit(
        &self,
        key: u64,
        version: u64,
        scale: f32,
        size: (u32, u32),
    ) -> Option<wgpu::TextureView> {
        let mut cache = self.layers.borrow_mut();
        let clock = cache.clock;
        let e = cache.entries.get_mut(&key)?;
        if e.version != version || (e.scale - scale).abs() > 1e-4 || e.size != size {
            return None;
        }
        e.used = clock;
        Some(e.view.clone())
    }

    /// Keeps `texture` for `key`, dropping the least recently used entries over the budget.
    fn layer_store(
        &self,
        key: u64,
        texture: wgpu::Texture,
        version: u64,
        scale: f32,
        size: (u32, u32),
    ) -> wgpu::TextureView {
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut cache = self.layers.borrow_mut();
        let used = cache.clock;
        cache.entries.remove(&key);
        let new = Entry {
            view: view.clone(),
            version,
            scale,
            size,
            used,
        };
        let add = new.bytes();
        while cache.bytes() + add > BUDGET {
            let Some(oldest) = cache
                .entries
                .iter()
                .min_by_key(|(_, e)| e.used)
                .map(|(k, _)| *k)
            else {
                break;
            };
            cache.entries.remove(&oldest);
        }
        cache.entries.insert(key, new);
        view
    }
}

impl GpuCanvas {
    /// Composites the cached texture of the subtree `key` over `bounds` (view coordinates),
    /// rendering it with `paint` first when there is none for `version`. `false` when the
    /// subtree cannot be a texture here (not on whole device pixels inside the frame, too
    /// big): the caller then paints it directly.
    pub(super) fn layer_texture(
        &mut self,
        key: u64,
        version: u64,
        bounds: Rect,
        scale: f32,
        paint: &mut dyn FnMut(&mut dyn Painter),
    ) -> bool {
        let [a, b, c, d, e, f] = self.transform.as_coeffs();
        if b.abs() > 1e-6 || c.abs() > 1e-6 || a <= 0.0 || (a - d).abs() > 1e-5 {
            return false;
        }
        if !(scale.is_finite() && scale > 0.0) || (f64::from(scale) - a).abs() > 1e-4 {
            return false;
        }
        let wpx = (f64::from(bounds.width) * a).ceil();
        let hpx = (f64::from(bounds.height) * a).ceil();
        let (fx, fy) = (e + f64::from(bounds.x) * a, f + f64::from(bounds.y) * d);
        let (x0, y0) = (fx.round(), fy.round());
        if (fx - x0).abs() > 1e-3 || (fy - y0).abs() > 1e-3 {
            return false;
        }
        // Where the subtree lands may be partly (or wholly) outside the frame: it is rendered
        // in a space of its own, whose origin is its top-left pixel.
        if !(wpx >= 1.0 && hpx >= 1.0)
            || !(x0.abs() < 1e6 && y0.abs() < 1e6)
            || (wpx as u64) * (hpx as u64) > MAX_PIXELS
        {
            return false;
        }
        let (w, h) = (wpx as u32, hpx as u32);
        let origin = (x0, y0);
        let dst = Rect::new(bounds.x, bounds.y, w as f32 / scale, h as f32 / scale);
        let view = match self.ctx.layer_hit(key, version, scale, (w, h)) {
            Some(v) => v,
            None => {
                let Some(v) = self.render_layer_texture(key, version, scale, origin, (w, h), paint)
                else {
                    return false;
                };
                v
            }
        };
        self.draw_view(&view, w, h, dst, 1.0);
        true
    }

    /// Renders the subtree into a layer covering the device `region`, and keeps a copy.
    fn render_layer_texture(
        &mut self,
        key: u64,
        version: u64,
        scale: f32,
        origin: (f64, f64),
        (w, h): (u32, u32),
        paint: &mut dyn FnMut(&mut dyn Painter),
    ) -> Option<wgpu::TextureView> {
        self.save();
        // The subtree is drawn alone: the clips around it apply when it is composited.
        let held_masks = std::mem::take(&mut self.masks);
        self.scissor = [0, 0, w, h];
        self.transform = Affine::translate((-origin.0, -origin.1)) * self.transform;
        let depth = self.stack.len();
        self.push_layer(LayerKind::Opacity(1.0));
        paint(self);
        self.restore_to(depth);
        self.end_pass();
        let layer = self.layers.pop();
        let rest = std::mem::replace(&mut self.masks, held_masks);
        self.retired
            .extend(rest.into_iter().filter_map(|m| m.target));
        self.restore();
        let layer = layer?;
        let ctx = self.ctx.clone();
        let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("layer cache"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
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
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            texture.as_image_copy(),
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.free.push(layer.target);
        Some(ctx.layer_store(key, texture, version, scale, (w, h)))
    }
}
