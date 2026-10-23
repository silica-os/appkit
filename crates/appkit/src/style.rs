//! Layer styles as the Amalfi kit defines them, and their painting on a canvas.
//!
//! A style is what a kit layer carries: fills, borders, drop shadows, inner shadows and
//! background or glass blurs. Painting follows the kit (and silica_ui's foundation):
//!
//! - drop shadows have CSS `box-shadow` semantics, a Gaussian of σ = blur / 2, drawn outside
//!   the shape only (a shadow never shows through a translucent fill);
//! - then the backdrop blur (glass), clipped to the shape;
//! - then the fills in order, bottom first;
//! - then inner shadows, clipped to the shape;
//! - then borders: inside borders are stroked within the outline, outside ones beyond it.

use crate::color::Color;
use crate::geom::{Point, Rect};
use crate::render::backdrop::Backdrop;
use crate::render::canvas::{Brush, StrokeStyle};
use crate::render::path::{Path, PathBuilder};
use crate::shape::Shape;

/// A gradient stop: position 0–1 and colour.
pub type Stop = (f32, Color);

/// What a fill or a border paints with.
#[derive(Debug, Clone, PartialEq)]
pub enum Paint {
    /// One colour.
    Solid(Color),
    /// A linear gradient between two points given in unit coordinates of the layer's frame.
    Linear {
        /// Start, in 0–1 of the frame.
        from: Point,
        /// End, in 0–1 of the frame.
        to: Point,
        /// Stops.
        stops: Vec<Stop>,
    },
    /// A radial gradient from `from` (centre) to `to` (a point on the outer circle).
    Radial {
        /// Centre, in 0–1 of the frame.
        from: Point,
        /// A point of the outer circle, in 0–1 of the frame.
        to: Point,
        /// Stops.
        stops: Vec<Stop>,
    },
    /// An angular (sweep) gradient around the frame's centre, starting at 12 o'clock.
    Angular {
        /// Stops.
        stops: Vec<Stop>,
    },
}

impl Paint {
    /// The colour of a solid paint.
    pub fn solid(&self) -> Option<Color> {
        match self {
            Paint::Solid(c) => Some(*c),
            _ => None,
        }
    }

    /// Whether the paint draws nothing.
    pub fn is_invisible(&self) -> bool {
        match self {
            Paint::Solid(c) => c.a == 0,
            Paint::Linear { stops, .. }
            | Paint::Radial { stops, .. }
            | Paint::Angular { stops } => stops.iter().all(|(_, c)| c.a == 0),
        }
    }

    /// The brush of this paint on a layer in `rect`.
    pub fn brush(&self, rect: Rect) -> Brush {
        let unit = |p: Point| Point::new(rect.x + p.x * rect.width, rect.y + p.y * rect.height);
        match self {
            Paint::Solid(c) => Brush::Solid(*c),
            Paint::Linear { from, to, stops } => Brush::Linear {
                from: unit(*from),
                to: unit(*to),
                stops: stops.clone(),
            },
            Paint::Radial { from, to, stops } => {
                let center = unit(*from);
                let edge = unit(*to);
                Brush::Radial {
                    center,
                    radius: (edge.x - center.x).hypot(edge.y - center.y).max(0.001),
                    stops: stops.clone(),
                }
            }
            // The kit sweeps from 12 o'clock.
            Paint::Angular { stops } => Brush::Sweep {
                center: rect.center(),
                start: -90.0,
                stops: stops.clone(),
            },
        }
    }
}

impl From<Color> for Paint {
    fn from(c: Color) -> Self {
        Paint::Solid(c)
    }
}

/// Where a border is stroked relative to the outline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BorderPosition {
    /// Within the outline (the kit's default).
    #[default]
    Inside,
    /// Centred on the outline.
    Center,
    /// Beyond the outline.
    Outside,
}

/// A border.
#[derive(Debug, Clone, PartialEq)]
pub struct Border {
    /// What it paints with.
    pub paint: Paint,
    /// Thickness in points.
    pub thickness: f32,
    /// Position.
    pub position: BorderPosition,
    /// Dash pattern (on, off, …); empty for a solid line.
    pub dash: Vec<f32>,
    /// Round line ends (for dashes).
    pub round_ends: bool,
}

impl Border {
    /// A solid inside border.
    pub fn inside(color: Color, thickness: f32) -> Self {
        Border {
            paint: Paint::Solid(color),
            thickness,
            position: BorderPosition::Inside,
            dash: Vec::new(),
            round_ends: false,
        }
    }

    /// A solid border at `position`.
    pub fn at(color: Color, thickness: f32, position: BorderPosition) -> Self {
        Border {
            position,
            ..Border::inside(color, thickness)
        }
    }
}

/// A drop or inner shadow, with the kit's (CSS) blur.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadow {
    /// Colour.
    pub color: Color,
    /// Offset.
    pub x: f32,
    /// Offset.
    pub y: f32,
    /// CSS blur (σ = blur / 2).
    pub blur: f32,
    /// Spread.
    pub spread: f32,
}

impl Shadow {
    /// A shadow without spread.
    pub const fn new(color: Color, x: f32, y: f32, blur: f32) -> Self {
        Shadow {
            color,
            x,
            y,
            blur,
            spread: 0.0,
        }
    }
}

/// The parameters of a glass material (kit blur type `Glass`, tokens' materials).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glass {
    /// Backdrop blur σ in points.
    pub radius: f32,
    /// Saturation of the backdrop.
    pub saturation: f32,
    /// Brightness of the backdrop.
    pub brightness: f32,
    /// Refraction strength at the edge.
    pub distortion: f32,
    /// Width of the refracting band, as a fraction of half the shorter side.
    pub depth: f32,
    /// Chromatic split.
    pub chromatic_aberration: f32,
    /// Specular highlight on the rim.
    pub specular: bool,
    /// Magnification of a lens (1: none).
    pub magnification: f32,
}

impl Default for Glass {
    fn default() -> Self {
        Glass {
            radius: 0.0,
            saturation: 1.0,
            brightness: 1.0,
            distortion: 0.0,
            depth: 0.0,
            chromatic_aberration: 0.0,
            specular: false,
            magnification: 1.0,
        }
    }
}

/// A blur of what is behind the layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Blur {
    /// A plain background blur (CSS `blur()`, σ = radius) with saturation and brightness.
    Background {
        /// σ in points.
        radius: f32,
        /// Saturation.
        saturation: f32,
        /// Brightness.
        brightness: f32,
    },
    /// Glass: blur, refraction at the edge, saturation, brightness, specular rim.
    Glass(Glass),
}

/// Everything a layer paints around and inside its shape.
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    /// Opacity of the whole layer, 0–1.
    pub opacity: f32,
    /// Fills, bottom first.
    pub fills: Vec<Paint>,
    /// Borders, bottom first.
    pub borders: Vec<Border>,
    /// Drop shadows (outside the shape).
    pub shadows: Vec<Shadow>,
    /// Inner shadows (inside the shape).
    pub inner_shadows: Vec<Shadow>,
    /// Backdrop blurs.
    pub blurs: Vec<Blur>,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            opacity: 1.0,
            fills: Vec::new(),
            borders: Vec::new(),
            shadows: Vec::new(),
            inner_shadows: Vec::new(),
            blurs: Vec::new(),
        }
    }
}

impl Style {
    /// A style with one solid fill.
    pub fn fill(color: Color) -> Self {
        Style {
            fills: vec![Paint::Solid(color)],
            ..Style::default()
        }
    }

    /// Adds a fill.
    pub fn with_fill(mut self, paint: impl Into<Paint>) -> Self {
        self.fills.push(paint.into());
        self
    }

    /// Adds a border.
    pub fn with_border(mut self, border: Border) -> Self {
        self.borders.push(border);
        self
    }

    /// Adds a drop shadow.
    pub fn with_shadow(mut self, shadow: Shadow) -> Self {
        self.shadows.push(shadow);
        self
    }

    /// Adds an inner shadow.
    pub fn with_inner_shadow(mut self, shadow: Shadow) -> Self {
        self.inner_shadows.push(shadow);
        self
    }

    /// Adds a blur.
    pub fn with_blur(mut self, blur: Blur) -> Self {
        self.blurs.push(blur);
        self
    }

    /// Sets the opacity.
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    /// Whether painting this style draws nothing.
    pub fn is_invisible(&self) -> bool {
        self.opacity <= 0.0
            || (self.fills.iter().all(Paint::is_invisible)
                && self.borders.iter().all(|b| b.paint.is_invisible())
                && self.shadows.iter().all(|s| s.color.a == 0)
                && self.inner_shadows.iter().all(|s| s.color.a == 0)
                && self.blurs.is_empty())
    }

    /// How far beyond the frame the style can paint (shadows, outside borders).
    pub fn overflow(&self) -> f32 {
        let mut reach: f32 = 0.0;
        for s in &self.shadows {
            reach = reach.max(s.blur * 1.5 + s.spread.abs() + s.x.abs().max(s.y.abs()) + 1.0);
        }
        for b in &self.borders {
            reach = reach.max(match b.position {
                BorderPosition::Inside => 0.0,
                BorderPosition::Center => b.thickness / 2.0,
                BorderPosition::Outside => b.thickness,
            });
        }
        reach
    }
}

/// Options of a painting pass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaintOptions {
    /// Draw backdrop blurs (off: Reduce Transparency, or a backend without them, which then
    /// fills with `blur_fallback`).
    pub backdrop: bool,
    /// The opaque fill that replaces glass when backdrops are off (tint over Surface/Window).
    pub blur_fallback: Option<Color>,
    /// The compositor draws the window's glass regions (`silica-glass-v1`): glass layers
    /// declared as compositor glass are not painted by the client.
    pub compositor_glass: bool,
}

impl Default for PaintOptions {
    fn default() -> Self {
        PaintOptions {
            backdrop: true,
            blur_fallback: None,
            compositor_glass: false,
        }
    }
}

/// Paints `style` for `shape` in `rect`.
pub fn paint_style(
    canvas: &mut dyn crate::render::Painter,
    rect: Rect,
    shape: &Shape,
    style: &Style,
    opts: PaintOptions,
) {
    if rect.is_empty() || style.opacity <= 0.0 {
        return;
    }
    let outline = shape.path(rect);
    let opacity = if style.opacity.is_finite() {
        style.opacity.clamp(0.0, 1.0)
    } else {
        1.0
    };
    paint_shadows(canvas, rect, shape, &outline, &style.shadows, opacity);
    for blur in &style.blurs {
        paint_backdrop(canvas, rect, &outline, blur, opts);
    }
    for fill in &style.fills {
        canvas.fill_path(&outline, &fill.brush(rect), opacity);
    }
    paint_inner_shadows(canvas, rect, shape, &outline, &style.inner_shadows, opacity);
    for border in &style.borders {
        paint_border(canvas, rect, shape, &outline, border, opacity);
    }
}

/// The blur σ of a kit (CSS) shadow blur.
pub fn shadow_sigma(blur: f32) -> f32 {
    blur / 2.0
}

/// The radius a blurred rounded rect stands in for `shape` with: shadows of rectangles,
/// capsules and rounded rects are drawn analytically, others through a blur layer.
fn analytic_radius(shape: &Shape, rect: Rect) -> Option<f32> {
    let limit = rect.width.min(rect.height) / 2.0;
    match shape {
        // Continuous corners are not circular: their shadows blur the outline itself.
        Shape::Rect { corners, smoothing } => {
            let a = corners.to_array();
            let r = a[0];
            ((*smoothing <= 1e-3 || r <= 1e-3) && a.iter().all(|v| (v - r).abs() < 1e-3))
                .then_some(r.clamp(0.0, limit.max(0.0)))
        }
        Shape::Capsule => Some(limit.max(0.0)),
        Shape::Ellipse if (rect.width - rect.height).abs() < 1e-3 => Some(limit.max(0.0)),
        _ => None,
    }
}

/// Drop shadows, outside the shape only.
pub fn paint_shadows(
    canvas: &mut dyn crate::render::Painter,
    rect: Rect,
    shape: &Shape,
    outline: &Path,
    shadows: &[Shadow],
    opacity: f32,
) {
    if shadows.iter().all(|s| s.color.a == 0) {
        return;
    }
    canvas.save();
    canvas.clip_out_path(outline);
    for s in shadows {
        if s.color.a == 0 {
            continue;
        }
        let r = rect
            .translate(Point::new(s.x, s.y))
            .inflate(s.spread, s.spread);
        if r.is_empty() {
            continue;
        }
        let grown = shape.grown(s.spread);
        let color = s.color.with_opacity(opacity);
        let sigma = shadow_sigma(s.blur);
        match analytic_radius(&grown, r) {
            Some(radius) if sigma > 0.0 => {
                canvas.fill_blurred_rounded_rect(r, radius, sigma, color, false);
            }
            _ if sigma > 0.0 && sigma.is_finite() => {
                canvas.fill_blurred_path(&grown.path(r), sigma, color);
            }
            _ => canvas.fill_path(&grown.path(r), &Brush::Solid(color), 1.0),
        }
    }
    canvas.restore();
}

/// Inner shadows: the outside of the shape, offset and blurred, seen through the shape.
pub fn paint_inner_shadows(
    canvas: &mut dyn crate::render::Painter,
    rect: Rect,
    shape: &Shape,
    outline: &Path,
    shadows: &[Shadow],
    opacity: f32,
) {
    if shadows.iter().all(|s| s.color.a == 0) {
        return;
    }
    canvas.save();
    canvas.clip_path(outline);
    for s in shadows {
        if s.color.a == 0 {
            continue;
        }
        let hole_rect = rect
            .translate(Point::new(s.x, s.y))
            .inflate(-s.spread, -s.spread);
        let hole_shape = shape.grown(-s.spread);
        let color = s.color.with_opacity(opacity);
        let sigma = shadow_sigma(s.blur);
        if let Some(radius) = analytic_radius(&hole_shape, hole_rect)
            && sigma > 0.0
            && !hole_rect.is_empty()
        {
            canvas.fill_blurred_rounded_rect(hole_rect, radius, sigma, color, true);
            continue;
        }
        let reach = s.blur * 2.0 + s.spread.abs() + s.x.abs() + s.y.abs() + 2.0;
        let mut outer = PathBuilder::new();
        outer.add_rect(rect.inflate(reach, reach));
        let ring = outer.detach().with(&hole_shape.path(hole_rect), true);
        if sigma > 0.0 && sigma.is_finite() {
            canvas.fill_blurred_path(&ring, sigma, color);
        } else {
            canvas.fill_path(&ring, &Brush::Solid(color), 1.0);
        }
    }
    canvas.restore();
}

fn paint_border(
    canvas: &mut dyn crate::render::Painter,
    rect: Rect,
    shape: &Shape,
    outline: &Path,
    border: &Border,
    opacity: f32,
) {
    let t = border.thickness;
    if t <= 0.0 || !t.is_finite() || border.paint.is_invisible() {
        return;
    }
    let brush = border.paint.brush(rect);
    let mut stroke = StrokeStyle::new(t);
    stroke.round = border.round_ends;
    if border.dash.len() >= 2 {
        stroke.dash = border.dash.clone();
    }
    match border.position {
        BorderPosition::Inside if border.dash.is_empty() => {
            // The ring between the outline and the outline shrunk by the thickness: exact on
            // every rounded outline, as the kit's inside borders.
            let inner_rect = rect.inflate(-t, -t);
            if inner_rect.is_empty() {
                canvas.fill_path(outline, &brush, opacity);
                return;
            }
            let ring = outline.with(&shape.grown(-t).path(inner_rect), true);
            canvas.fill_path(&ring, &brush, opacity);
        }
        BorderPosition::Outside if border.dash.is_empty() => {
            let outer = shape.grown(t).path(rect.inflate(t, t));
            let ring = outer.with(outline, true);
            canvas.fill_path(&ring, &brush, opacity);
        }
        position => {
            // Centred, or dashed: stroke a path offset by half the width so that dashes keep
            // their round or square ends.
            let offset = match position {
                BorderPosition::Inside => -t / 2.0,
                BorderPosition::Center => 0.0,
                BorderPosition::Outside => t / 2.0,
            };
            let path = shape.grown(offset).path(rect.inflate(offset, offset));
            canvas.stroke_path(&path, &brush, opacity, &stroke);
        }
    }
}

pub use crate::render::backdrop::saturation_brightness_matrix;

fn paint_backdrop(
    canvas: &mut dyn crate::render::Painter,
    rect: Rect,
    outline: &Path,
    blur: &Blur,
    opts: PaintOptions,
) {
    if !opts.backdrop {
        if let Some(c) = opts.blur_fallback {
            canvas.fill_path(outline, &Brush::Solid(c), 1.0);
        }
        return;
    }
    let (sigma, saturation, brightness, glass) = match *blur {
        Blur::Background {
            radius,
            saturation,
            brightness,
        } => (radius, saturation, brightness, None),
        Blur::Glass(g) => (g.radius, g.saturation, g.brightness, Some(g)),
    };
    let t = canvas.transform();
    // σ is in points: in device pixels it scales with the canvas.
    let scale = (t.m[0] * t.m[3] - t.m[1] * t.m[2]).abs().sqrt();
    canvas.backdrop(
        outline,
        Backdrop {
            sigma: sigma.max(0.0) * scale,
            saturation,
            brightness,
            glass: glass.map(|g| (rect, g)),
            to_device: t,
        },
    );
    if let Some(g) = glass
        && g.specular
    {
        paint_specular(canvas, rect, outline);
    }
}

/// The specular highlight on a glass rim: a thin light along the top edge fading down the
/// sides (1.5 pt, added to what is below), clipped to the outline.
pub fn paint_specular(canvas: &mut dyn crate::render::Painter, rect: Rect, outline: &Path) {
    let brush = Brush::Linear {
        from: Point::new(rect.x, rect.y),
        to: Point::new(rect.x, rect.y + rect.height.min(24.0)),
        stops: vec![
            (0.0, Color::rgba(255, 255, 255, 46)),
            (1.0, Color::TRANSPARENT),
        ],
    };
    canvas.save();
    canvas.clip_path(outline);
    canvas.push_additive();
    canvas.stroke_path(outline, &brush, 1.0, &StrokeStyle::new(1.5));
    canvas.restore();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(
        w: u32,
        h: u32,
        f: impl FnOnce(&mut crate::render::canvas::Canvas),
    ) -> crate::render::image::Image {
        let mut c = crate::render::canvas::Canvas::new(w, h);
        f(&mut c);
        c.finish()
    }

    fn pixel(img: &crate::render::image::Image, x: u32, y: u32) -> [u8; 4] {
        img.pixel(x, y)
    }

    #[test]
    fn fill_border_shadow() {
        let (w, h) = (60, 60);
        let style = Style::fill(Color::rgb(255, 0, 0))
            .with_border(Border::inside(Color::rgb(0, 0, 255), 2.0))
            .with_shadow(Shadow::new(Color::rgba(0, 0, 0, 128), 0.0, 4.0, 4.0));
        let s = render(w, h, |c| {
            paint_style(
                c,
                Rect::new(10.0, 10.0, 40.0, 30.0),
                &Shape::rounded(6.0),
                &style,
                PaintOptions::default(),
            )
        });
        assert_eq!(pixel(&s, 30, 25), [255, 0, 0, 255]);
        assert_eq!(pixel(&s, 30, 10), [0, 0, 255, 255]);
        // Shadow below the shape, nothing above it.
        assert!(pixel(&s, 30, 42)[3] > 0);
        assert_eq!(pixel(&s, 30, 5)[3], 0);
    }

    #[test]
    fn inner_shadow_stays_inside() {
        let (w, h) = (40, 40);
        let style = Style::default().with_inner_shadow(Shadow::new(Color::WHITE, 0.0, 1.0, 1.0));
        let s = render(w, h, |c| {
            paint_style(
                c,
                Rect::new(10.0, 10.0, 20.0, 20.0),
                &Shape::rounded(4.0),
                &style,
                PaintOptions::default(),
            )
        });
        assert!(pixel(&s, 20, 10)[3] > 0);
        assert_eq!(pixel(&s, 20, 8)[3], 0);
        assert_eq!(pixel(&s, 20, 20)[3], 0);
    }

    #[test]
    fn linear_gradient() {
        let (w, h) = (10, 10);
        let style = Style::default().with_fill(Paint::Linear {
            from: Point::new(0.5, 0.0),
            to: Point::new(0.5, 1.0),
            stops: vec![(0.0, Color::BLACK), (1.0, Color::WHITE)],
        });
        let s = render(w, h, |c| {
            paint_style(
                c,
                Rect::new(0.0, 0.0, 10.0, 10.0),
                &Shape::RECT,
                &style,
                PaintOptions::default(),
            )
        });
        assert!(pixel(&s, 5, 0)[0] < 30);
        assert!(pixel(&s, 5, 9)[0] > 220);
    }
}
