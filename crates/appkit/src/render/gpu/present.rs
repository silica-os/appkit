//! Presenting a [`GpuCanvas`] through a wgpu surface (a Wayland `wl_surface`): the retained
//! frame is copied into the swapchain image, which is presented (the driver attaches the buffer
//! and commits the surface).

use std::ffi::c_void;
use std::ptr::NonNull;

use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, RawDisplayHandle, RawWindowHandle,
    WaylandDisplayHandle, WaylandWindowHandle,
};

use super::*;

/// The `wl_display` of the process, as wgpu wants it.
#[derive(Debug)]
struct WaylandDisplay(NonNull<c_void>);

// SAFETY: the pointer is only handed to the graphics driver, which uses the connection from
// the thread that owns the surface; the connection outlives the instance.
#[allow(unsafe_code)]
unsafe impl Send for WaylandDisplay {}
// SAFETY: as above, nothing is dereferenced here.
#[allow(unsafe_code)]
unsafe impl Sync for WaylandDisplay {}

impl HasDisplayHandle for WaylandDisplay {
    #[allow(unsafe_code)]
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        let raw = RawDisplayHandle::Wayland(WaylandDisplayHandle::new(self.0));
        // SAFETY: the pointer is a live `wl_display` for as long as the platform runs.
        Ok(unsafe { DisplayHandle::borrow_raw(raw) })
    }
}

/// What a present did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presented {
    /// The image was queued: the surface was committed.
    Done,
    /// Nothing was presented (occluded, timed out, still outdated): try again after the next
    /// frame callback. The canvas keeps its damage.
    Skipped,
    /// The surface is gone: drop it and make another.
    Lost,
}

/// A swapchain on a Wayland surface.
pub struct PresentSurface {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    configured: bool,
}

impl std::fmt::Debug for PresentSurface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PresentSurface({}×{} {:?})",
            self.config.width, self.config.height, self.config.format
        )
    }
}

impl GpuContext {
    /// A context on an adapter that can present to `window` (a `wl_surface`) on `display` (a
    /// `wl_display`), and that surface. `None` without a usable adapter or surface; the caller
    /// then keeps the shared-memory path.
    ///
    /// Both pointers must be live: the display for as long as the context exists, the surface
    /// for as long as the returned [`PresentSurface`] does.
    #[allow(unsafe_code)]
    pub fn for_wayland(
        display: NonNull<c_void>,
        window: NonNull<c_void>,
    ) -> Option<(Rc<GpuContext>, PresentSurface)> {
        let desc =
            wgpu::InstanceDescriptor::new_with_display_handle(Box::new(WaylandDisplay(display)));
        let (ctx, surface) = GpuContext::build(desc, |instance| {
            // SAFETY: the caller guarantees both pointers (see above).
            unsafe { instance.create_surface_unsafe(wayland_target(display, window)) }.ok()
        })?;
        let surface = PresentSurface::new(&ctx, surface?)?;
        Some((ctx, surface))
    }

    /// A swapchain for another `wl_surface` of the same display; same contract as
    /// [`GpuContext::for_wayland`].
    #[allow(unsafe_code)]
    pub fn present_surface(
        &self,
        display: NonNull<c_void>,
        window: NonNull<c_void>,
    ) -> Option<PresentSurface> {
        // SAFETY: the caller guarantees both pointers (see above).
        let surface = unsafe {
            self.instance
                .create_surface_unsafe(wayland_target(display, window))
        }
        .ok()?;
        PresentSurface::new(self, surface)
    }
}

fn wayland_target(display: NonNull<c_void>, window: NonNull<c_void>) -> wgpu::SurfaceTargetUnsafe {
    wgpu::SurfaceTargetUnsafe::RawHandle {
        raw_display_handle: Some(RawDisplayHandle::Wayland(WaylandDisplayHandle::new(
            display,
        ))),
        raw_window_handle: RawWindowHandle::Wayland(WaylandWindowHandle::new(window)),
    }
}

impl PresentSurface {
    /// The swapchain's configuration: premultiplied alpha (the windows are translucent) in
    /// a format the copy can write; `None` when the surface cannot do that.
    fn new(ctx: &GpuContext, surface: wgpu::Surface<'static>) -> Option<PresentSurface> {
        let caps = surface.get_capabilities(&ctx.adapter_handle);
        let format = [
            wgpu::TextureFormat::Bgra8Unorm,
            wgpu::TextureFormat::Rgba8Unorm,
        ]
        .into_iter()
        .find(|f| caps.formats.contains(f))?;
        if !caps
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PreMultiplied)
            || !caps.usages.contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
        {
            return None;
        }
        // Frames are paced by the frame callback: no second wait in the swapchain when it can
        // be avoided.
        let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
            wgpu::PresentMode::Mailbox
        } else {
            wgpu::PresentMode::Fifo
        };
        Some(PresentSurface {
            surface,
            config: wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                width: 1,
                height: 1,
                present_mode,
                desired_maximum_frame_latency: 2,
                alpha_mode: wgpu::CompositeAlphaMode::PreMultiplied,
                color_space: wgpu::SurfaceColorSpace::default(),
                view_formats: Vec::new(),
            },
            configured: false,
        })
    }

    /// The swapchain's size in device pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    /// Sizes the swapchain (a resize, a new scale); a no-op when it already has that size.
    pub fn resize(&mut self, ctx: &GpuContext, width: u32, height: u32) {
        let (w, h) = (width.clamp(1, 16384), height.clamp(1, 16384));
        if self.configured && (self.config.width, self.config.height) == (w, h) {
            return;
        }
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&ctx.device, &self.config);
        self.configured = true;
    }

    /// Copies the canvas's frame into the next swapchain image and presents it. The canvas
    /// must have the swapchain's size. `before` runs right before the present, only when there
    /// is one: it sends the `damage_buffer` and `frame` requests, because presenting commits
    /// the surface.
    pub fn present(
        &mut self,
        ctx: &GpuContext,
        canvas: &mut GpuCanvas,
        before: impl FnOnce(),
    ) -> Presented {
        if !self.configured || canvas.size() != self.size() {
            return Presented::Skipped;
        }
        let mut tries = 0;
        loop {
            let (tex, stale) = match self.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(t) => (t, false),
                wgpu::CurrentSurfaceTexture::Suboptimal(t) => (t, true),
                wgpu::CurrentSurfaceTexture::Outdated => {
                    // Reconfigure and try once more.
                    self.surface.configure(&ctx.device, &self.config);
                    tries += 1;
                    if tries > 1 {
                        return Presented::Skipped;
                    }
                    continue;
                }
                wgpu::CurrentSurfaceTexture::Lost => return Presented::Lost,
                wgpu::CurrentSurfaceTexture::Timeout
                | wgpu::CurrentSurfaceTexture::Occluded
                | wgpu::CurrentSurfaceTexture::Validation => return Presented::Skipped,
            };
            let view = tex
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            canvas.blit_to(&view, self.config.format);
            before();
            ctx.queue.present(tex);
            if stale {
                self.surface.configure(&ctx.device, &self.config);
            }
            return Presented::Done;
        }
    }
}

impl GpuCanvas {
    /// The size in device pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Clears `rect` (device pixels) to transparent, so that a region can be repainted over a
    /// retained frame. Honours the clip.
    pub fn clear_rect(&mut self, rect: Rect) {
        if rect.is_empty() {
            return;
        }
        self.mark(rect);
        let p = self.uniform_for(&Brush::Solid(Color::TRANSPARENT), 1.0);
        let saved = self.transform;
        self.transform = Affine::IDENTITY;
        self.draw(&quad(rect), Mode::Simple, Blend::Replace, &p, None);
        self.transform = saved;
    }

    /// Records the copy of the frame into `target` (a view of `format`, the canvas's size)
    /// and submits everything.
    pub(super) fn blit_to(&mut self, target: &wgpu::TextureView, format: wgpu::TextureFormat) {
        self.restore_to(0);
        self.end_pass();
        let ctx = self.ctx.clone();
        let (w, h) = (self.width, self.height);
        let mut paint = identity_paint(w, h);
        paint.g0 = [w as f32, h as f32, 0.0, 0.0];
        let pipeline = ctx.pipeline(Key {
            mode: Mode::Filter { blur: false },
            blend: Blend::Replace,
            format,
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
                    resource: wgpu::BindingResource::TextureView(&self.main.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&ctx.white),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&ctx.nearest),
                },
            ],
        });
        let encoder = self.encoder();
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("appkit present"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
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
        self.submit();
    }

    /// The frame as `format` pixels, for tests of [`GpuCanvas::blit_to`].
    #[cfg(test)]
    pub(super) fn blit_readback(&mut self, format: wgpu::TextureFormat) -> Vec<u8> {
        let (w, h) = (self.width, self.height);
        let tex = self.ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        self.blit_to(&view, format);
        let row = (w * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buf = self.ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(row) * u64::from(h),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self
            .ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        enc.copy_texture_to_buffer(
            tex.as_image_copy(),
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
        let mut out = Vec::new();
        if let Ok(data) = slice.get_mapped_range() {
            for y in 0..h as usize {
                let s = y * row as usize;
                if let Some(r) = data.get(s..s + w as usize * 4) {
                    out.extend_from_slice(r);
                }
            }
        }
        buf.unmap();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_copy_into_the_swapchain_swaps_channels_for_bgra() {
        let Some(ctx) = GpuContext::new() else { return };
        let mut gpu = GpuCanvas::new(&ctx, 8, 8);
        gpu.begin(true);
        gpu.fill_rect(Rect::new(0.0, 0.0, 8.0, 8.0), Color::rgb(200, 100, 50));
        let rgba = gpu.blit_readback(wgpu::TextureFormat::Rgba8Unorm);
        assert_eq!(&rgba[0..4], &[200, 100, 50, 255]);
        let bgra = gpu.blit_readback(wgpu::TextureFormat::Bgra8Unorm);
        assert_eq!(&bgra[0..4], &[50, 100, 200, 255]);
    }

    #[test]
    fn clearing_a_region_makes_it_transparent_again() {
        let Some(ctx) = GpuContext::new() else { return };
        let mut gpu = GpuCanvas::new(&ctx, 16, 16);
        gpu.begin(true);
        gpu.fill_rect(Rect::new(0.0, 0.0, 16.0, 16.0), Color::rgb(255, 0, 0));
        gpu.submit();
        gpu.begin(false);
        gpu.clear_rect(Rect::new(0.0, 0.0, 8.0, 8.0));
        let px = gpu.into_pixmap();
        let d = px.data_as_u8_slice();
        assert_eq!(&d[0..4], &[0, 0, 0, 0]);
        assert_eq!(
            &d[(12 * 16 + 12) * 4..(12 * 16 + 12) * 4 + 4],
            &[255, 0, 0, 255]
        );
    }
}
