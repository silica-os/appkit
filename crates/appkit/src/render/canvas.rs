//! A drawing context for a layer's contents, on vello_cpu.
//!
//! Views record display lists; a layer's backing store is made by replaying them here.
//! The canvas keeps a save/restore stack like the toolkit's old Skia one: a transform, and
//! the clip, opacity, blend and filter layers pushed since the save.

use std::sync::Arc;

use vello_cpu::kurbo::{self, Affine, Cap, Join, Stroke};
use vello_cpu::peniko::{
    self, BlendMode, Compose, Extend, Fill, FontData, Gradient, ImageQuality, ImageSampler, Mix,
};
use vello_cpu::{Glyph, Pixmap, RenderContext, RenderSettings, Resources};

use crate::color::Color;
use crate::geom::{Point, Rect, Transform};
use crate::render::image::Image;
use crate::render::path::Path;

/// What a fill or a stroke paints with, in the canvas's current coordinates.
#[derive(Debug, Clone, PartialEq)]
pub enum Brush {
    /// One colour.
    Solid(Color),
    /// A linear gradient.
    Linear {
        /// Start.
        from: Point,
        /// End.
        to: Point,
        /// Stops (position 0–1, colour).
        stops: Vec<(f32, Color)>,
    },
    /// A radial gradient.
    Radial {
        /// Centre.
        center: Point,
        /// Radius.
        radius: f32,
        /// Stops.
        stops: Vec<(f32, Color)>,
    },
    /// A sweep gradient around `center` from `start` degrees (0 at 3 o'clock, clockwise).
    Sweep {
        /// Centre.
        center: Point,
        /// Start angle in degrees.
        start: f32,
        /// Stops.
        stops: Vec<(f32, Color)>,
    },
}

impl From<Color> for Brush {
    fn from(c: Color) -> Self {
        Brush::Solid(c)
    }
}

fn kp(p: Point) -> kurbo::Point {
    kurbo::Point::new(f64::from(p.x), f64::from(p.y))
}

fn stops(stops: &[(f32, Color)], opacity: f32) -> Vec<peniko::ColorStop> {
    stops
        .iter()
        .map(|(o, c)| {
            peniko::ColorStop::from((o.clamp(0.0, 1.0), c.with_opacity(opacity).to_peniko()))
        })
        .collect()
}

/// How a stroke looks.
#[derive(Debug, Clone, PartialEq)]
pub struct StrokeStyle {
    /// Width.
    pub width: f32,
    /// Round caps and joins (square caps and miter joins otherwise).
    pub round: bool,
    /// Dash pattern (on, off, …); empty for a solid line.
    pub dash: Vec<f32>,
}

impl StrokeStyle {
    /// A solid stroke of `width`.
    pub fn new(width: f32) -> Self {
        StrokeStyle {
            width,
            round: false,
            dash: Vec::new(),
        }
    }
}

/// A layer pushed on the context, kept so that it can be pushed again after a backdrop
/// splits the drawing.
#[derive(Debug, Clone)]
enum Open {
    Clip(kurbo::BezPath, Fill, Affine),
    Opacity(f32),
    Additive,
    Blur(f32),
}

#[derive(Debug, Clone, Copy)]
struct Saved {
    transform: Affine,
    layers: usize,
}

/// A drawing context over a pixmap of a fixed size.
///
/// Drawing is recorded and rasterized on [`Canvas::flush`] (or when a backdrop needs the
/// pixels below it), composited over what the target already holds.
pub struct Canvas {
    ctx: RenderContext,
    resources: Resources,
    target: Pixmap,
    transform: Affine,
    stack: Vec<Saved>,
    open: Vec<Open>,
    dirty: bool,
    /// The lowest device row reached by what was drawn since the last flush (conservative).
    reach: f64,
    width: u16,
    height: u16,
    /// While a region is open: the full target and the region's device origin.
    full: Option<(Pixmap, u16, u16)>,
}

impl std::fmt::Debug for Canvas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Canvas({}×{})", self.width, self.height)
    }
}

/// Copies a `size` block of pixels from `src` at `from` to `dst` at `to` (clipped to both).
fn copy_rect(src: &Pixmap, from: (u16, u16), dst: &mut Pixmap, to: (u16, u16), size: (u16, u16)) {
    let (sw, dw) = (usize::from(src.width()), usize::from(dst.width()));
    let w = usize::from(size.0)
        .min(sw.saturating_sub(usize::from(from.0)))
        .min(dw.saturating_sub(usize::from(to.0)));
    let rows = usize::from(size.1)
        .min(usize::from(src.height()).saturating_sub(usize::from(from.1)))
        .min(usize::from(dst.height()).saturating_sub(usize::from(to.1)));
    let s = src.data_as_u8_slice();
    let d = dst.data_as_u8_slice_mut();
    for y in 0..rows {
        let si = ((usize::from(from.1) + y) * sw + usize::from(from.0)) * 4;
        let di = ((usize::from(to.1) + y) * dw + usize::from(to.0)) * 4;
        if let (Some(a), Some(b)) = (s.get(si..si + w * 4), d.get_mut(di..di + w * 4)) {
            b.copy_from_slice(a);
        }
    }
}

/// Single-threaded: vello_cpu's workers cannot run filter layers (blurs).
fn settings() -> RenderSettings {
    RenderSettings {
        num_threads: 0,
        ..RenderSettings::default()
    }
}

impl Canvas {
    /// A transparent canvas of `width` × `height` pixels (clamped to 1–65535).
    pub fn new(width: u32, height: u32) -> Canvas {
        let w = width.clamp(1, u32::from(u16::MAX)) as u16;
        let h = height.clamp(1, u32::from(u16::MAX)) as u16;
        Canvas::on(Pixmap::new(w, h))
    }

    /// A canvas drawing over `target` (a retained frame).
    pub fn on(target: Pixmap) -> Canvas {
        let (w, h) = (target.width().max(1), target.height().max(1));
        Canvas {
            ctx: RenderContext::new_with(w, h, settings()),
            resources: Resources::new(),
            target,
            transform: Affine::IDENTITY,
            stack: Vec::new(),
            open: Vec::new(),
            dirty: false,
            reach: 0.0,
            width: w,
            height: h,
            full: None,
        }
    }

    /// Confines drawing to `rect` (device pixels) until [`Canvas::end_region`]: the region is
    /// rasterized on its own, so a flush costs its size rather than the target's. Device
    /// coordinates keep their meaning; a region already open is closed first.
    pub fn begin_region(&mut self, rect: Rect) {
        self.end_region();
        self.flush();
        let r = rect.intersect(Rect::new(
            0.0,
            0.0,
            f32::from(self.width),
            f32::from(self.height),
        ));
        if r.is_empty() {
            return;
        }
        let (x0, y0) = (r.x.floor() as u16, r.y.floor() as u16);
        let x1 = (r.right().ceil() as u16).min(self.width);
        let y1 = (r.bottom().ceil() as u16).min(self.height);
        let (w, h) = (x1.saturating_sub(x0).max(1), y1.saturating_sub(y0).max(1));
        let mut band = Pixmap::new(w, h);
        copy_rect(&self.target, (x0, y0), &mut band, (0, 0), (w, h));
        let full = std::mem::replace(&mut self.target, band);
        self.full = Some((full, x0, y0));
        self.width = w;
        self.height = h;
        self.shift(Affine::translate((-f64::from(x0), -f64::from(y0))));
    }

    /// Puts the region's pixels back into the target (a no-op without a region).
    pub fn end_region(&mut self) {
        let Some((mut full, x0, y0)) = self.full.take() else {
            return;
        };
        self.flush();
        copy_rect(
            &self.target,
            (0, 0),
            &mut full,
            (x0, y0),
            (self.width, self.height),
        );
        self.width = full.width();
        self.height = full.height();
        self.target = full;
        self.shift(Affine::translate((f64::from(x0), f64::from(y0))));
    }

    /// The open region's device origin ((0, 0) without one).
    fn origin(&self) -> (f64, f64) {
        self.full
            .as_ref()
            .map_or((0.0, 0.0), |(_, x, y)| (f64::from(*x), f64::from(*y)))
    }

    /// Moves device space under every transform, current and saved.
    fn shift(&mut self, by: Affine) {
        self.transform = by * self.transform;
        for s in &mut self.stack {
            s.transform = by * s.transform;
        }
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        u32::from(self.width)
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        u32::from(self.height)
    }

    /// Saves the transform and opens a group that `restore` closes.
    pub fn save(&mut self) {
        self.stack.push(Saved {
            transform: self.transform,
            layers: self.open.len(),
        });
    }

    /// Closes the layers opened since the matching `save` and restores its transform.
    pub fn restore(&mut self) {
        let Some(s) = self.stack.pop() else {
            return;
        };
        while self.open.len() > s.layers {
            self.ctx.pop_layer();
            self.open.pop();
        }
        self.transform = s.transform;
    }

    /// Nesting depth of saves.
    pub fn save_count(&self) -> usize {
        self.stack.len()
    }

    /// Restores until `count` saves remain.
    pub fn restore_to(&mut self, count: usize) {
        while self.stack.len() > count {
            self.restore();
        }
    }

    /// The current transform.
    pub fn transform(&self) -> Transform {
        let (x, y) = self.origin();
        let c = (Affine::translate((x, y)) * self.transform).as_coeffs();
        Transform {
            m: c.map(|v| v as f32),
        }
    }

    /// The transform to the target's pixels (the region's while one is open).
    fn device(&self) -> Transform {
        let c = self.transform.as_coeffs();
        Transform {
            m: c.map(|v| v as f32),
        }
    }

    /// Replaces the current transform.
    pub fn set_transform(&mut self, t: Transform) {
        let (x, y) = self.origin();
        self.transform = Affine::translate((-x, -y)) * t.to_affine();
    }

    /// Pre-concatenates `t` (applied to what is drawn before the current transform).
    pub fn concat(&mut self, t: Transform) {
        self.transform *= t.to_affine();
    }

    /// Moves the origin.
    pub fn translate(&mut self, (x, y): (f32, f32)) {
        self.transform *= Affine::translate((f64::from(x), f64::from(y)));
    }

    /// Scales.
    pub fn scale(&mut self, (sx, sy): (f32, f32)) {
        self.transform *= Affine::scale_non_uniform(f64::from(sx), f64::from(sy));
    }

    fn sync(&mut self) {
        self.ctx.set_transform(clean(self.transform));
        self.dirty = true;
    }

    /// Records that `local` (current coordinates) is drawn into: a flush rasterizes the rows
    /// down to the lowest one reached (a blur layer may spread anywhere).
    fn reach(&mut self, local: kurbo::Rect) {
        let bottom = if self.open.iter().any(|o| matches!(o, Open::Blur(_))) {
            f64::INFINITY
        } else {
            self.transform.transform_rect_bbox(local).y1 + 2.0
        };
        self.reach = if bottom.is_nan() {
            f64::INFINITY
        } else {
            self.reach.max(bottom)
        };
    }

    /// Clips what follows (until `restore`) to `rect`.
    pub fn clip_rect(&mut self, rect: Rect) {
        let mut b = kurbo::BezPath::new();
        let r = rect.to_kurbo();
        b.move_to((r.x0, r.y0));
        b.line_to((r.x1, r.y0));
        b.line_to((r.x1, r.y1));
        b.line_to((r.x0, r.y1));
        b.close_path();
        self.clip_bez(b, Fill::NonZero);
    }

    /// Clips what follows to the inside of `path`.
    pub fn clip_path(&mut self, path: &Path) {
        let rule = if path.even_odd {
            Fill::EvenOdd
        } else {
            Fill::NonZero
        };
        let fine = self.fine(&path.bez).into_owned();
        self.clip_bez(fine, rule);
    }

    /// Clips what follows to the outside of `path` (within the canvas).
    pub fn clip_out_path(&mut self, path: &Path) {
        let mut b = kurbo::BezPath::new();
        // The whole canvas in the current coordinates, then the hole.
        let inv = self.transform.inverse();
        let (w, h) = (f64::from(self.width), f64::from(self.height));
        let corners = [(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)].map(|p| inv * kurbo::Point::from(p));
        b.move_to(corners[0]);
        for p in &corners[1..] {
            b.line_to(*p);
        }
        b.close_path();
        for el in self.fine(&path.bez).elements() {
            b.push(*el);
        }
        self.clip_bez(b, Fill::EvenOdd);
    }

    fn push_open(&mut self, op: &Open) {
        match op {
            Open::Clip(b, rule, t) => {
                self.ctx.set_transform(clean(*t));
                self.ctx.set_fill_rule(*rule);
                self.ctx.push_clip_layer(b);
                self.ctx.set_fill_rule(Fill::NonZero);
            }
            Open::Opacity(o) => self.ctx.push_opacity_layer(*o),
            Open::Additive => self
                .ctx
                .push_blend_layer(BlendMode::new(Mix::Normal, Compose::Plus)),
            Open::Blur(sigma) => {
                let filter = vello_cpu::filter_effects::Filter::from_primitive(
                    vello_cpu::filter_effects::FilterPrimitive::GaussianBlur {
                        std_deviation: *sigma,
                        edge_mode: vello_cpu::filter_effects::EdgeMode::None,
                    },
                );
                self.ctx.push_filter_layer(filter);
            }
        }
    }

    fn open_layer(&mut self, op: Open) {
        self.sync();
        self.push_open(&op);
        self.open.push(op);
    }

    fn clip_bez(&mut self, b: kurbo::BezPath, rule: Fill) {
        let t = self.transform;
        self.open_layer(Open::Clip(b, rule, t));
    }

    /// Draws what follows (until `restore`) at `opacity`, as a group.
    pub fn push_opacity(&mut self, opacity: f32) {
        let o = if opacity.is_finite() {
            opacity.clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.open_layer(Open::Opacity(o));
    }

    /// Draws what follows (until `restore`) added to what is below (light).
    pub fn push_additive(&mut self) {
        self.open_layer(Open::Additive);
    }

    /// Draws what follows (until `restore`) blurred with a Gaussian of `sigma` pixels.
    pub fn push_blur(&mut self, sigma: f32) {
        if sigma > 0.0 && sigma.is_finite() {
            self.open_layer(Open::Blur(sigma));
        } else {
            // Keep the stack balanced: a plain group.
            self.open_layer(Open::Opacity(1.0));
        }
    }

    /// The opacity of the groups open now.
    fn group_opacity(&self) -> f32 {
        self.open
            .iter()
            .map(|o| match o {
                Open::Opacity(v) => *v,
                _ => 1.0,
            })
            .product()
    }

    /// Rasterizes what was drawn so far over the target; the open layers stay open.
    pub fn flush(&mut self) {
        if !self.dirty {
            return;
        }

        for _ in 0..self.open.len() {
            self.ctx.pop_layer();
        }
        self.ctx.flush();
        let settings = vello_cpu::RasterizerSettings {
            target_init: vello_cpu::TargetInit::SrcOver,
            ..vello_cpu::RasterizerSettings::default()
        };
        // Only the rows down to the lowest one drawn: rasterizing costs every row it covers.
        let rows = self.reach.ceil().clamp(0.0, f64::from(self.height)) as u16;
        self.reach = 0.0;
        if rows == self.height {
            self.ctx
                .render_with(&mut self.target, &mut self.resources, settings);
        } else if rows > 0 {
            let w = self.width;
            let len = usize::from(w) * usize::from(rows) * 4;
            if let Some(view) = self
                .target
                .data_as_u8_slice_mut()
                .get_mut(..len)
                .and_then(|buf| vello_cpu::PixmapMut::new(w, rows, buf))
            {
                self.ctx.render_with(view, &mut self.resources, settings);
            }
        }
        self.ctx.reset();
        self.dirty = false;
        let open = std::mem::take(&mut self.open);
        for op in &open {
            self.push_open(op);
        }
        self.open = open;
    }

    /// Makes `rect` (device pixels) transparent in the target.
    pub fn clear_rect(&mut self, rect: Rect) {
        self.flush();
        let (ox, oy) = self.origin();
        let rect = Rect::new(
            rect.x - ox as f32,
            rect.y - oy as f32,
            rect.width,
            rect.height,
        );
        let r = rect.intersect(Rect::new(0.0, 0.0, self.width as f32, self.height as f32));
        if r.is_empty() {
            return;
        }
        let (x0, y0) = (r.x.floor() as usize, r.y.floor() as usize);
        let (x1, y1) = (r.right().ceil() as usize, r.bottom().ceil() as usize);
        let w = usize::from(self.width);
        let data = self.target.data_as_u8_slice_mut();
        for y in y0..y1 {
            if let Some(row) = data.get_mut((y * w + x0) * 4..(y * w + x1) * 4) {
                row.fill(0);
            }
        }
    }

    /// Applies a backdrop filter under `outline` (current coordinates, inside the open
    /// clips): what is drawn so far is rasterized, filtered there and drawn over.
    pub fn backdrop(&mut self, outline: &Path, mut bd: crate::render::backdrop::Backdrop) {
        if outline.is_empty() {
            return;
        }
        self.flush();
        let dev = outline.transformed(self.device());
        let b = dev
            .bounds()
            .intersect(Rect::new(0.0, 0.0, self.width as f32, self.height as f32));
        if b.is_empty() {
            return;
        }
        let (x0, y0) = (b.x.floor().max(0.0) as u16, b.y.floor().max(0.0) as u16);
        let x1 = (b.right().ceil() as u16).min(self.width);
        let y1 = (b.bottom().ceil() as u16).min(self.height);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let (mw, mh) = (x1 - x0, y1 - y0);
        // The coverage: the outline inside the open clips, rasterized in the region.
        let mut mctx = RenderContext::new_with(mw, mh, settings());
        let shift = Affine::translate((-f64::from(x0), -f64::from(y0)));
        let mut clips = 0;
        for op in &self.open {
            if let Open::Clip(path, rule, t) = op {
                mctx.set_transform(clean(shift * *t));
                mctx.set_fill_rule(*rule);
                mctx.push_clip_layer(path);
                clips += 1;
            }
        }
        mctx.set_transform(clean(shift * self.transform));
        mctx.set_fill_rule(if outline.even_odd {
            Fill::EvenOdd
        } else {
            Fill::NonZero
        });
        mctx.set_paint(peniko::Color::WHITE);
        mctx.fill_path(&self.fine(&outline.bez));
        for _ in 0..clips {
            mctx.pop_layer();
        }
        mctx.flush();
        let mut mp = Pixmap::new(mw, mh);
        mctx.render(&mut mp, &mut self.resources);
        let mask: Vec<u8> = mp
            .data_as_u8_slice()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|&[_, _, _, a]| a)
            .collect();
        bd.to_device = self.device();
        let opacity = self.group_opacity();
        let (w, h) = (usize::from(self.width), usize::from(self.height));
        crate::render::backdrop::apply(
            self.target.data_as_u8_slice_mut(),
            w,
            h,
            (
                usize::from(x0),
                usize::from(y0),
                usize::from(mw),
                usize::from(mh),
            ),
            &mask,
            &bd,
            opacity,
        );
    }

    fn set_brush(&mut self, brush: &Brush, opacity: f32) {
        match brush {
            Brush::Solid(c) => self.ctx.set_paint(c.with_opacity(opacity).to_peniko()),
            Brush::Linear { from, to, stops: s } => {
                if s.len() < 2 {
                    let c = s.first().map_or(Color::TRANSPARENT, |(_, c)| *c);
                    self.ctx.set_paint(c.with_opacity(opacity).to_peniko());
                    return;
                }
                let g = Gradient::new_linear(kp(*from), kp(*to))
                    .with_extend(Extend::Pad)
                    .with_stops(stops(s, opacity).as_slice());
                self.ctx.set_paint(g);
            }
            Brush::Radial {
                center,
                radius,
                stops: s,
            } => {
                if s.len() < 2 {
                    let c = s.first().map_or(Color::TRANSPARENT, |(_, c)| *c);
                    self.ctx.set_paint(c.with_opacity(opacity).to_peniko());
                    return;
                }
                let g = Gradient::new_radial(kp(*center), radius.max(0.001))
                    .with_stops(stops(s, opacity).as_slice());
                self.ctx.set_paint(g);
            }
            Brush::Sweep {
                center,
                start,
                stops: s,
            } => {
                if s.len() < 2 {
                    let c = s.first().map_or(Color::TRANSPARENT, |(_, c)| *c);
                    self.ctx.set_paint(c.with_opacity(opacity).to_peniko());
                    return;
                }
                let a = start.to_radians();
                let g = Gradient::new_sweep(kp(*center), a, a + std::f32::consts::TAU)
                    .with_stops(stops(s, opacity).as_slice());
                self.ctx.set_paint(g);
            }
        }
    }

    /// Fills `path`.
    pub fn fill_path(&mut self, path: &Path, brush: &Brush, opacity: f32) {
        if path.is_empty() {
            return;
        }
        self.sync();
        self.set_brush(brush, opacity);
        self.ctx.set_fill_rule(if path.even_odd {
            Fill::EvenOdd
        } else {
            Fill::NonZero
        });
        self.reach(kurbo::Shape::bounding_box(&path.bez));
        let fine = self.fine(&path.bez);
        self.ctx.fill_path(&fine);
        self.ctx.set_fill_rule(Fill::NonZero);
    }

    /// `bez` with its curves flattened finely enough for exact edges: vello_cpu flattens
    /// with a quarter-pixel tolerance, its chords cutting curves (continuous corners, rings)
    /// visibly thinner. Paths of lines are passed as they are.
    fn fine<'a>(&self, bez: &'a kurbo::BezPath) -> std::borrow::Cow<'a, kurbo::BezPath> {
        let curved = bez
            .elements()
            .iter()
            .any(|e| matches!(e, kurbo::PathEl::QuadTo(..) | kurbo::PathEl::CurveTo(..)));
        if !curved {
            return std::borrow::Cow::Borrowed(bez);
        }
        let scale = self.transform.determinant().abs().sqrt().max(1e-3);
        let mut out = kurbo::BezPath::new();
        kurbo::flatten(bez, 0.1 / scale, |el| out.push(el));
        std::borrow::Cow::Owned(out)
    }

    /// Fills `rect` with a colour.
    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        if rect.is_empty() || color.a == 0 {
            return;
        }
        self.sync();
        self.ctx.set_paint(color.to_peniko());
        self.reach(rect.to_kurbo());
        self.ctx.fill_rect(&rect.to_kurbo());
    }

    /// Fills a Gaussian-blurred rounded rect of σ `sigma` (current units) analytically; with
    /// `invert`, its complement (inner shadows).
    pub fn fill_blurred_rounded_rect(
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
        self.sync();
        self.ctx.set_paint(color.to_peniko());
        if invert {
            self.reach = f64::INFINITY;
        } else {
            self.reach(
                rect.to_kurbo()
                    .inflate(f64::from(sigma.abs() * 3.0), f64::from(sigma.abs() * 3.0)),
            );
        }
        self.ctx
            .fill_blurred_rounded_rect(&rect.to_kurbo(), radius.max(0.0), sigma, invert);
    }

    /// Strokes `path`.
    pub fn stroke_path(&mut self, path: &Path, brush: &Brush, opacity: f32, style: &StrokeStyle) {
        if path.is_empty() || style.width.is_nan() || style.width <= 0.0 {
            return;
        }
        self.sync();
        self.set_brush(brush, opacity);
        let mut s = Stroke::new(f64::from(style.width));
        if style.round {
            s = s.with_caps(Cap::Round).with_join(Join::Round);
        } else {
            s = s.with_caps(Cap::Butt).with_join(Join::Miter);
        }
        if style.dash.len() >= 2 {
            s = s.with_dashes(0.0, style.dash.iter().map(|d| f64::from(*d)));
        }
        // Expanded here, finely, then filled (vello_cpu's own expansion is as coarse as its
        // flattening).
        let scale = self.transform.determinant().abs().sqrt().max(1e-3);
        let outline = kurbo::stroke(
            path.bez.iter(),
            &s,
            &kurbo::StrokeOpts::default(),
            0.1 / scale,
        );
        self.reach(kurbo::Shape::bounding_box(&outline));
        let fine = self.fine(&outline);
        self.ctx.set_fill_rule(Fill::NonZero);
        self.ctx.fill_path(&fine);
    }

    /// Draws `image` scaled into `dst` at `opacity` (bilinear).
    pub fn draw_image(&mut self, image: &Image, dst: Rect, opacity: f32) {
        if dst.is_empty() || image.width() == 0 || image.height() == 0 {
            return;
        }
        let (w, h) = (image.width() as f32, image.height() as f32);
        self.save();
        self.translate((dst.x, dst.y));
        self.scale((dst.width / w, dst.height / h));
        self.sync();
        // Pixel-aligned at 1:1 (the raster cache's blits): copied, not resampled.
        let [a, b, c, d, e, f] = self.transform.as_coeffs();
        let aligned = (a - 1.0).abs() < 1e-4
            && (d - 1.0).abs() < 1e-4
            && b.abs() < 1e-4
            && c.abs() < 1e-4
            && (e - e.round()).abs() < 1e-3
            && (f - f.round()).abs() < 1e-3;
        if aligned && let Some(clip) = self.aligned_clip() {
            self.flush();
            self.blit(image, (e.round() as i64, f.round() as i64), clip, opacity);
            self.restore();
            return;
        }
        let img = vello_cpu::Image {
            image: vello_cpu::ImageSource::Pixmap(Arc::clone(image.pixmap())),
            sampler: ImageSampler {
                x_extend: Extend::Pad,
                y_extend: Extend::Pad,
                quality: if aligned {
                    ImageQuality::Low
                } else {
                    ImageQuality::Medium
                },
                alpha: opacity.clamp(0.0, 1.0),
            },
        };
        self.ctx.set_paint(img);
        self.reach(kurbo::Rect::new(0.0, 0.0, f64::from(w), f64::from(h)));
        self.ctx
            .fill_rect(&kurbo::Rect::new(0.0, 0.0, f64::from(w), f64::from(h)));
        self.restore();
    }

    /// The device rectangle the open layers clip to, when they are all clips to
    /// pixel-aligned rectangles (the image fast path); `None` otherwise.
    fn aligned_clip(&self) -> Option<(i64, i64, i64, i64)> {
        let mut clip = (0i64, 0i64, i64::from(self.width), i64::from(self.height));
        for op in &self.open {
            let Open::Clip(b, _, t) = op else {
                return None;
            };
            let [a, sb, sc, d, _, _] = t.as_coeffs();
            if sb.abs() > 1e-6 || sc.abs() > 1e-6 || a <= 0.0 || d <= 0.0 {
                return None;
            }
            let els = b.elements();
            if els.len() != 5 || !matches!(els.get(4), Some(kurbo::PathEl::ClosePath)) {
                return None;
            }
            let mut pts = Vec::with_capacity(4);
            for el in els.iter().take(4) {
                match el {
                    kurbo::PathEl::MoveTo(p) | kurbo::PathEl::LineTo(p) => pts.push(*t * *p),
                    _ => return None,
                }
            }
            let (x0, x1) = pts.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
                (lo.min(p.x), hi.max(p.x))
            });
            let (y0, y1) = pts.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
                (lo.min(p.y), hi.max(p.y))
            });
            // A rectangle: every corner on its bounds.
            let on_edges = pts.iter().all(|p| {
                ((p.x - x0).abs() < 1e-6 || (p.x - x1).abs() < 1e-6)
                    && ((p.y - y0).abs() < 1e-6 || (p.y - y1).abs() < 1e-6)
            });
            let whole = |v: f64| (v - v.round()).abs() < 1e-3;
            if !on_edges || ![x0, y0, x1, y1].into_iter().all(whole) {
                return None;
            }
            clip = (
                clip.0.max(x0.round() as i64),
                clip.1.max(y0.round() as i64),
                clip.2.min(x1.round() as i64),
                clip.3.min(y1.round() as i64),
            );
        }
        Some(clip)
    }

    /// Source-over of `image` at device `origin` inside `clip`, at `opacity`: rows composited
    /// directly into the target (opaque runs copied).
    fn blit(
        &mut self,
        image: &Image,
        origin: (i64, i64),
        clip: (i64, i64, i64, i64),
        opacity: f32,
    ) {
        let (iw, ih) = (i64::from(image.width()), i64::from(image.height()));
        let x0 = origin.0.max(clip.0);
        let y0 = origin.1.max(clip.1);
        let x1 = (origin.0 + iw).min(clip.2);
        let y1 = (origin.1 + ih).min(clip.3);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let op = (opacity.clamp(0.0, 1.0) * 255.0).round() as u32;
        if op == 0 {
            return;
        }
        let tw = usize::from(self.width);
        let iw = iw as usize;
        let (ox, oy) = (origin.0 as usize, origin.1 as usize);
        let (x0, y0, x1, y1) = (x0 as usize, y0 as usize, x1 as usize, y1 as usize);
        let src = image.bytes();
        let n = (x1 - x0) * 4;
        // Rows y0..y1 of the target, split in bands for big blits (memory-bound: one core
        // cannot saturate the bus).
        let Some(dst) = self
            .target
            .data_as_u8_slice_mut()
            .get_mut(y0 * tw * 4..y1 * tw * 4)
        else {
            return;
        };
        let rows = y1 - y0;
        let bands = if rows * (x1 - x0) >= 1 << 20 {
            std::thread::available_parallelism().map_or(1, |n| n.get().min(8))
        } else {
            1
        };
        let per = rows.div_ceil(bands).max(1);
        let band = |first: usize, dst: &mut [u8]| {
            for (k, row) in dst.chunks_exact_mut(tw * 4).enumerate() {
                let y = first + k;
                let si = ((y - oy) * iw + (x0 - ox)) * 4;
                let (Some(s), Some(d)) = (src.get(si..si + n), row.get_mut(x0 * 4..x0 * 4 + n))
                else {
                    continue;
                };
                let (s, _) = s.as_chunks::<4>();
                let (d, _) = d.as_chunks_mut::<4>();
                if op == 255 {
                    blend_row(d, s);
                } else {
                    for (d, s) in d.iter_mut().zip(s) {
                        *d = over(s.map(|v| ((u32::from(v) * op + 127) / 255) as u8), *d);
                    }
                }
            }
        };
        if bands == 1 {
            band(y0, dst);
            return;
        }
        std::thread::scope(|scope| {
            for (i, part) in dst.chunks_mut(per * tw * 4).enumerate() {
                let band = &band;
                scope.spawn(move || band(y0 + i * per, part));
            }
        });
    }

    /// Fills glyphs of `font` at `size` (positions in the current coordinates, baselines).
    pub fn fill_glyphs(
        &mut self,
        font: &FontData,
        size: f32,
        coords: &[i16],
        glyphs: impl Iterator<Item = Glyph> + Clone,
        color: Color,
    ) {
        if color.a == 0 {
            return;
        }
        self.sync();
        self.ctx.set_paint(color.to_peniko());
        let span = glyphs.clone().fold(
            kurbo::Rect::new(
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ),
            |r, g| r.union_pt(kurbo::Point::new(f64::from(g.x), f64::from(g.y))),
        );
        if span.x0.is_finite() {
            let s = f64::from(size.abs()) * 2.0;
            self.reach(span.inflate(s, s));
        }
        if crate::render::glyphs::plain(font) {
            // Cached outlines, each filled where its glyph goes.
            let s = f64::from(size);
            for g in glyphs {
                let Some(path) = crate::render::glyphs::outline(font, coords, g.id) else {
                    continue;
                };
                let at = Affine::translate((f64::from(g.x), f64::from(g.y))) * Affine::scale(s);
                self.ctx.set_transform(clean(self.transform * at));
                self.ctx.fill_path(&path);
            }
            self.ctx.set_transform(clean(self.transform));
            return;
        }
        let res = self
            .ctx
            .glyph_run(&mut self.resources, font)
            .font_size(size)
            .normalized_coords(coords)
            .hint(false)
            .fill_glyphs(glyphs);
        if let Err(e) = res {
            tracing::debug!("glyphs: {e:?}");
        }
    }

    /// Rasterizes what is left, closes every layer and gives the target back.
    pub fn into_pixmap(mut self) -> Pixmap {
        self.restore_to(0);
        while let Some(_op) = self.open.pop() {
            self.ctx.pop_layer();
        }
        self.end_region();
        self.flush();
        self.target
    }

    /// The target so far (flushed; an open region is closed).
    pub fn pixmap(&mut self) -> &Pixmap {
        self.end_region();
        self.flush();
        &self.target
    }

    /// Rasterizes what is left into an image.
    pub fn finish(self) -> Image {
        Image::from_pixmap(self.into_pixmap())
    }
}

/// Premultiplied source-over of one pixel.
#[inline]
fn over(s: [u8; 4], d: [u8; 4]) -> [u8; 4] {
    let keep = 255 - u32::from(s[3]);
    let [s0, s1, s2, s3] = s;
    let [d0, d1, d2, d3] = d;
    let f = |sv: u8, dv: u8| (u32::from(sv) + (u32::from(dv) * keep + 127) / 255).min(255) as u8;
    [f(s0, d0), f(s1, d1), f(s2, d2), f(s3, d3)]
}

/// Source-over of a row, eight pixels at a time: opaque blocks are copied, transparent ones
/// skipped, the rest blended.
fn blend_row(d: &mut [[u8; 4]], s: &[[u8; 4]]) {
    let (db, dt) = d.as_chunks_mut::<8>();
    let (sb, st) = s.as_chunks::<8>();
    for (d, s) in db.iter_mut().zip(sb) {
        let (lo, hi) = s
            .iter()
            .fold((255u8, 0u8), |(lo, hi), p| (lo.min(p[3]), hi.max(p[3])));
        if lo == 255 {
            *d = *s;
        } else if hi != 0 {
            blend_px(d, s);
        }
    }
    blend_px(dt, st);
}

fn blend_px(d: &mut [[u8; 4]], s: &[[u8; 4]]) {
    for (d, s) in d.iter_mut().zip(s) {
        match s[3] {
            255 => *d = *s,
            0 => {}
            _ => *d = over(*s, *d),
        }
    }
}

/// `t` with the f32 noise of right-angle rotations removed (sin π = -8.7e-8): vello_cpu
/// mis-rasterizes the nearly axis-aligned edges it makes, leaking whole tiles.
fn clean(t: Affine) -> Affine {
    Affine::new(t.as_coeffs().map(|v| {
        let r = v.round();
        if (v - r).abs() < 1e-5 { r } else { v }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Transform;
    use crate::render::path::PathBuilder;

    #[test]
    fn fills_clips_and_restores() {
        let mut c = Canvas::new(20, 10);
        c.save();
        c.clip_rect(Rect::new(0.0, 0.0, 10.0, 10.0));
        c.fill_rect(Rect::new(0.0, 0.0, 20.0, 10.0), Color::rgba(255, 0, 0, 255));
        c.restore();
        let img = c.finish();
        assert_eq!(img.pixel(5, 5), [255, 0, 0, 255]);
        assert_eq!(img.pixel(15, 5), [0, 0, 0, 0]);
    }

    #[test]
    fn clip_out_keeps_the_outside() {
        let mut c = Canvas::new(20, 20);
        let mut b = PathBuilder::new();
        b.add_rect(Rect::new(5.0, 5.0, 10.0, 10.0));
        c.save();
        c.clip_out_path(&b.detach());
        c.fill_rect(Rect::new(0.0, 0.0, 20.0, 20.0), Color::rgba(0, 0, 255, 255));
        c.restore();
        let img = c.finish();
        assert_eq!(img.pixel(1, 1), [0, 0, 255, 255]);
        assert_eq!(img.pixel(10, 10), [0, 0, 0, 0]);
    }

    #[test]
    fn opacity_and_transform() {
        let mut c = Canvas::new(10, 10);
        c.scale((2.0, 2.0));
        c.save();
        c.push_opacity(0.5);
        c.fill_rect(Rect::new(0.0, 0.0, 2.0, 2.0), Color::rgba(0, 255, 0, 255));
        c.restore();
        let img = c.finish();
        let p = img.pixel(3, 3);
        assert!((i32::from(p[3]) - 128).abs() <= 1, "{p:?}");
        assert_eq!(img.pixel(5, 5), [0, 0, 0, 0]);
    }

    #[test]
    fn blur_spreads() {
        let mut c = Canvas::new(40, 40);
        c.save();
        c.push_blur(3.0);
        c.fill_rect(Rect::new(15.0, 15.0, 10.0, 10.0), Color::rgba(0, 0, 0, 255));
        c.restore();
        let img = c.finish();
        assert!(img.pixel(13, 20)[3] > 0);
        assert!(img.pixel(20, 20)[3] > 200);
    }

    #[test]
    fn a_half_turn_does_not_leak_tiles() {
        let mut c = Canvas::new(48, 48);
        c.concat(
            Transform::translate(-24.0, -24.0)
                .then(Transform::rotate(std::f32::consts::PI))
                .then(Transform::translate(24.0, 24.0)),
        );
        let p = crate::shape::Shape::rounded(2.0).path(Rect::new(20.0, 0.0, 8.0, 16.0));
        c.fill_path(&p, &Brush::Solid(Color::rgba(0, 0, 0, 255)), 1.0);
        let img = c.finish();
        for y in 32..48 {
            assert_eq!(img.pixel(30, y)[3], 0, "row {y}");
            assert_eq!(img.pixel(24, y)[3], 255, "row {y}");
        }
    }

    #[test]
    fn a_region_paints_what_the_whole_canvas_does() {
        let scene = |c: &mut Canvas| {
            c.save();
            c.scale((2.0, 2.0));
            c.fill_rect(
                Rect::new(0.0, 0.0, 40.0, 30.0),
                Color::rgba(240, 240, 240, 255),
            );
            let mut b = PathBuilder::new();
            b.add_oval(Rect::new(6.0, 5.0, 20.0, 14.0));
            c.fill_path(
                &b.detach(),
                &Brush::Solid(Color::rgba(20, 90, 200, 200)),
                1.0,
            );
            c.restore();
        };
        let mut whole = Canvas::new(80, 60);
        scene(&mut whole);
        let whole = whole.finish();
        let mut part = Canvas::new(80, 60);
        let r = Rect::new(9.0, 7.0, 31.0, 23.0);
        part.begin_region(r);
        part.save();
        part.clip_rect(r);
        scene(&mut part);
        part.restore();
        part.end_region();
        assert_eq!(part.transform(), Transform::IDENTITY);
        let part = part.finish();
        for y in 0..60 {
            for x in 0..80 {
                let inside = (9..40).contains(&x) && (7..30).contains(&y);
                let want = if inside { whole.pixel(x, y) } else { [0; 4] };
                // Coverage is computed per tile: a shift may round one level apart.
                let got = part.pixel(x, y);
                let off = got.iter().zip(want).map(|(a, b)| a.abs_diff(b)).max();
                assert!(off <= Some(1), "({x}, {y}): {got:?} vs {want:?}");
            }
        }
    }
}
