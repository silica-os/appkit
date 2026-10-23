//! Rendering: paths, images, the canvas (vello_cpu), backdrop filters, and the frame's
//! compositors.

pub mod backdrop;
pub mod canvas;
mod glyphs;
pub mod gpu;
pub mod image;
pub mod path;

use vello_cpu::Glyph;
use vello_cpu::peniko::FontData;

use crate::color::Color;
use crate::geom::{Rect, Transform};
use canvas::{Brush, Canvas, StrokeStyle};

/// What drawing needs of a render backend: the CPU [`Canvas`] and the GPU canvas both implement
/// it, so that views, styles and text paint the same way on either.
pub trait Painter {
    /// The CPU canvas behind this painter, when it is one (custom drawing, the raster cache).
    fn as_canvas(&mut self) -> Option<&mut Canvas>;
    /// Pushes the transform and the open layers.
    fn save(&mut self);
    /// Pops to the matching save.
    fn restore(&mut self);
    /// The depth of the save stack.
    fn save_count(&self) -> usize;
    /// Pops down to `count` saves.
    fn restore_to(&mut self, count: usize);
    /// The current transform.
    fn transform(&self) -> Transform;
    /// Multiplies the transform.
    fn concat(&mut self, t: Transform);
    /// Translates.
    fn translate(&mut self, by: (f32, f32));
    /// Scales.
    fn scale(&mut self, by: (f32, f32));
    /// Clips to a rectangle.
    fn clip_rect(&mut self, rect: Rect);
    /// Clips to a path.
    fn clip_path(&mut self, path: &path::Path);
    /// Clips out a path.
    fn clip_out_path(&mut self, path: &path::Path);
    /// Draws what follows into a group faded by `opacity`.
    fn push_opacity(&mut self, opacity: f32);
    /// Draws what follows additively.
    fn push_additive(&mut self);
    /// Blurs what follows (σ in current units).
    fn push_blur(&mut self, sigma: f32);
    /// Fills `path` with `color` blurred by a Gaussian of σ `sigma` (shadows of any shape).
    fn fill_blurred_path(&mut self, path: &path::Path, sigma: f32, color: Color) {
        self.save();
        self.push_blur(sigma);
        self.fill_path(path, &Brush::Solid(color), 1.0);
        self.restore();
    }
    /// Applies a backdrop filter under `outline`.
    fn backdrop(&mut self, outline: &path::Path, bd: backdrop::Backdrop);
    /// Fills a path.
    fn fill_path(&mut self, path: &path::Path, brush: &Brush, opacity: f32);
    /// Fills a rectangle.
    fn fill_rect(&mut self, rect: Rect, color: Color);
    /// Fills a blurred rounded rectangle (or its complement).
    fn fill_blurred_rounded_rect(
        &mut self,
        rect: Rect,
        radius: f32,
        sigma: f32,
        color: Color,
        invert: bool,
    );
    /// Strokes a path.
    fn stroke_path(&mut self, path: &path::Path, brush: &Brush, opacity: f32, style: &StrokeStyle);
    /// Draws an image into `dst`.
    fn draw_image(&mut self, image: &image::Image, dst: Rect, opacity: f32);
    /// Draws glyphs of a font.
    fn fill_glyphs(
        &mut self,
        font: &FontData,
        size: f32,
        coords: &[i16],
        glyphs: &[Glyph],
        color: Color,
    );
}

impl Painter for Canvas {
    fn as_canvas(&mut self) -> Option<&mut Canvas> {
        Some(self)
    }
    fn save(&mut self) {
        Canvas::save(self);
    }
    fn restore(&mut self) {
        Canvas::restore(self);
    }
    fn save_count(&self) -> usize {
        Canvas::save_count(self)
    }
    fn restore_to(&mut self, count: usize) {
        Canvas::restore_to(self, count);
    }
    fn transform(&self) -> Transform {
        Canvas::transform(self)
    }
    fn concat(&mut self, t: Transform) {
        Canvas::concat(self, t);
    }
    fn translate(&mut self, by: (f32, f32)) {
        Canvas::translate(self, by);
    }
    fn scale(&mut self, by: (f32, f32)) {
        Canvas::scale(self, by);
    }
    fn clip_rect(&mut self, rect: Rect) {
        Canvas::clip_rect(self, rect);
    }
    fn clip_path(&mut self, path: &path::Path) {
        Canvas::clip_path(self, path);
    }
    fn clip_out_path(&mut self, path: &path::Path) {
        Canvas::clip_out_path(self, path);
    }
    fn push_opacity(&mut self, opacity: f32) {
        Canvas::push_opacity(self, opacity);
    }
    fn push_additive(&mut self) {
        Canvas::push_additive(self);
    }
    fn push_blur(&mut self, sigma: f32) {
        Canvas::push_blur(self, sigma);
    }
    fn backdrop(&mut self, outline: &path::Path, bd: backdrop::Backdrop) {
        Canvas::backdrop(self, outline, bd);
    }
    fn fill_path(&mut self, path: &path::Path, brush: &Brush, opacity: f32) {
        Canvas::fill_path(self, path, brush, opacity);
    }
    fn fill_rect(&mut self, rect: Rect, color: Color) {
        Canvas::fill_rect(self, rect, color);
    }
    fn fill_blurred_rounded_rect(
        &mut self,
        rect: Rect,
        radius: f32,
        sigma: f32,
        color: Color,
        invert: bool,
    ) {
        Canvas::fill_blurred_rounded_rect(self, rect, radius, sigma, color, invert);
    }
    fn stroke_path(&mut self, path: &path::Path, brush: &Brush, opacity: f32, style: &StrokeStyle) {
        Canvas::stroke_path(self, path, brush, opacity, style);
    }
    fn draw_image(&mut self, image: &image::Image, dst: Rect, opacity: f32) {
        Canvas::draw_image(self, image, dst, opacity);
    }
    fn fill_glyphs(
        &mut self,
        font: &FontData,
        size: f32,
        coords: &[i16],
        glyphs: &[Glyph],
        color: Color,
    ) {
        Canvas::fill_glyphs(self, font, size, coords, glyphs.iter().copied(), color);
    }
}
