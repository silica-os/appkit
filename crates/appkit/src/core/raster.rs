//! Raster caches of static subtrees: a subtree that has not changed for a few frames, draws
//! enough and reads nothing behind it (no backdrop blur, no compositor glass, no custom
//! drawing) is drawn once into an offscreen image and then drawn as that image, until
//! something in it changes. The view's own position, scale and opacity are applied around
//! the image, so a static panel that moves or fades keeps its cache.
//!
//! The cache is on for the platform's windows ([`Ui::set_raster_cache`]) and off in headless
//! tests, whose pixels must not depend on how many frames were drawn.

use std::cell::RefCell;
use std::hash::{Hash, Hasher};

use crate::render::canvas::Canvas;
use crate::render::image::Image;

use crate::core::display::{DisplayItem, DisplayList};
use crate::geom::Rect;

/// Frames a subtree must stay unchanged before it is cached.
pub const STABLE_FRAMES: u64 = 3;
/// Display items a subtree must draw to be worth an image.
pub const MIN_ITEMS: u32 = 24;
/// The most device pixels one image may have.
pub const MAX_PIXELS: i64 = 4096 * 4096;
/// The most bytes all the images of a process may take.
pub const BUDGET: usize = 96 << 20;

/// A subtree's image.
#[derive(Debug)]
pub struct Cached {
    /// The image, `bounds` × the device scale.
    pub image: Image,
    /// Device pixels per point it was drawn at.
    pub scale: f32,
    /// The subtree's bounds (view coordinates) it covers.
    pub bounds: Rect,
    /// `changed_at` when it was drawn.
    pub made_at: u64,
    /// Drawn under a dimmed ancestor.
    pub dimmed: bool,
}

impl Cached {
    /// Bytes the image takes.
    pub fn bytes(&self) -> usize {
        self.image.width() as usize * self.image.height() as usize * 4
    }
}

/// What the frame learns about a view's subtree (kept on its node).
#[derive(Debug, Default)]
pub struct RasterState {
    /// The frame in which the subtree's content last changed.
    pub changed_at: u64,
    /// The view's own placement (position, offset, scale, opacity, animation) last frame.
    pub placement: u64,
    /// The children last frame.
    pub children: u64,
    /// The view's own drawing bounds last frame (view coordinates).
    pub own: Rect,
    /// The subtree's bounds (view coordinates).
    pub bounds: Rect,
    /// Display items drawn by the subtree.
    pub items: u32,
    /// The subtree reads nothing behind it and has no custom drawing.
    pub cacheable: bool,
    /// Its image.
    pub image: RefCell<Option<Cached>>,
}

/// A hash of anything hashable (with floats by their bits).
pub fn hash_of(f: impl FnOnce(&mut std::collections::hash_map::DefaultHasher)) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    f(&mut h);
    h.finish()
}

/// Feeds floats to a hasher.
pub fn hash_f32s(h: &mut impl Hasher, values: &[f32]) {
    for v in values {
        v.to_bits().hash(h);
    }
}

/// Whether a display list can go into an image (it reads nothing behind it).
pub fn list_cacheable(list: &DisplayList) -> bool {
    list.items.iter().all(|i| match i {
        DisplayItem::Glass { .. } | DisplayItem::Custom { .. } => false,
        DisplayItem::Layer { style, .. } => style.blurs.is_empty(),
        _ => true,
    })
}

/// The device scale of `canvas` when its matrix is an axis-aligned uniform scale and
/// translation that puts `bounds` on whole device pixels (an image then draws exactly
/// what the subtree would); `None` otherwise.
pub fn aligned_scale(canvas: &Canvas, bounds: Rect) -> Option<f32> {
    let m = canvas.transform().m;
    if m[1].abs() > 1e-6 || m[2].abs() > 1e-6 {
        return None;
    }
    let (sx, sy) = (m[0], m[3]);
    if sx <= 0.0 || (sx - sy).abs() > 1e-5 {
        return None;
    }
    let x = m[4] + bounds.x * sx;
    let y = m[5] + bounds.y * sy;
    let whole = |v: f32| (v - v.round()).abs() < 1e-3;
    (whole(x) && whole(y)).then_some(sx)
}

/// Draws `subtree` (in view coordinates) into a new image of `bounds` at `scale`.
pub fn draw_image(bounds: Rect, scale: f32, subtree: impl FnOnce(&mut Canvas)) -> Option<Image> {
    let w = (bounds.width * scale).ceil() as i64;
    let h = (bounds.height * scale).ceil() as i64;
    if w <= 0 || h <= 0 || w * h > MAX_PIXELS || w > 65535 || h > 65535 {
        return None;
    }
    let mut c = Canvas::new(w as u32, h as u32);
    c.scale((scale, scale));
    c.translate((-bounds.x, -bounds.y));
    subtree(&mut c);
    Some(c.finish())
}

/// Draws a cached image at its bounds (the canvas is in view coordinates).
pub fn blit(canvas: &mut Canvas, cached: &Cached) {
    let w = cached.image.width() as f32 / cached.scale;
    let h = cached.image.height() as f32 / cached.scale;
    canvas.draw_image(
        &cached.image,
        Rect::new(cached.bounds.x, cached.bounds.y, w, h),
        1.0,
    );
}

#[cfg(test)]
mod tests {
    use crate::amalfi::button::{PushButton, PushVariant};
    use crate::amalfi::progress::Spinner;
    use crate::core::layout::{Alignment, Grid, Stack};
    use crate::core::{Prop, Ui, Value, WindowKind};
    use crate::geom::Size;

    fn scene(cache: bool) -> (Ui, crate::core::window::WindowId) {
        let mut ui = Ui::new_headless();
        ui.set_raster_cache(cache);
        let w = ui.create_window(WindowKind::Normal, Size::new(600.0, 400.0));
        let col = ui.create(Stack::column(8.0));
        let grid = ui.create(Grid {
            columns: 6,
            column_spacing: 8.0,
            row_spacing: 8.0,
            alignment: Alignment::TOP_LEADING,
        });
        for i in 0..36 {
            let b = ui.create(PushButton::new(&format!("B{i}")).variant(PushVariant::Secondary));
            ui.add_child(grid, b);
        }
        let spin = ui.create(Spinner::new());
        ui.add_child(col, grid);
        ui.add_child(col, spin);
        let _ = ui.set(col, Prop::Padding, Value::from(12.0));
        ui.set_content(w, col);
        ui.show_window(w);
        (ui, w)
    }

    fn pixels(img: &crate::render::image::Image) -> Vec<u8> {
        img.bytes().to_vec()
    }

    #[test]
    fn a_static_subtree_is_drawn_from_an_image_with_the_same_pixels() {
        let (mut on, w1) = scene(true);
        let (mut off, w2) = scene(false);
        let mut last = (Vec::new(), Vec::new());
        for _ in 0..8 {
            on.advance(1.0 / 60.0);
            off.advance(1.0 / 60.0);
            let a = on.snapshot(w1, 2.0).unwrap();
            let b = off.snapshot(w2, 2.0).unwrap();
            last = (pixels(&a), pixels(&b));
        }
        // The grid of buttons is cached while the spinner turns beside it.
        assert!(on.raster_cache_bytes() > 0);
        assert_eq!(off.raster_cache_bytes(), 0);
        // Compositing an image of the subtree differs from drawing it in place only by the
        // rounding of a few antialiased edge pixels.
        let diffs: Vec<u8> = last
            .0
            .iter()
            .zip(&last.1)
            .map(|(a, b)| a.abs_diff(*b))
            .filter(|d| *d > 3)
            .collect();
        let max = diffs.iter().copied().max().unwrap_or(0);
        assert!(
            diffs.len() * 10_000 < last.0.len() && max <= 8,
            "{} bytes differ (max {max})",
            diffs.len()
        );
    }
}
