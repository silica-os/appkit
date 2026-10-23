//! Sliders (kit `Sliders`): the linear slider (horizontal or vertical, plain, with tick marks
//! or with glyphs), the circular dial, and the round thumb that turns into a `Glass/Regular`
//! lens while it is dragged (`materials.md`, lens rule).

use crate::render::path::PathBuilder;

use crate::amalfi::foundation::{control_shadow, focus_halo_named};
use crate::amalfi::materials::Tier;
use crate::amalfi::theme::Theme;
use crate::amalfi::tokens;
use crate::color::Color;
use crate::core::a11y::{A11y, Action, Role};
use crate::core::event::{Button, Event, Handled, Key};
use crate::core::layout::Proposal;
use crate::core::tree::NodeFlags;
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, FocusKind, Interaction, LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Point, Rect, Size};
use crate::motion::{Animated, Timing};
use crate::shape::Shape;
use crate::style::{Blur, Border, BorderPosition, Glass, Shadow, Style};

/// Fixed measures (silica_ui's `AmalfiSliderMetrics`).
pub mod metrics {
    /// The thumb's diameter.
    pub const THUMB: f32 = crate::amalfi::tokens::control_heights::MINI;
    /// The track's thickness.
    pub const TRACK: f32 = 4.0;
    /// Default length of a horizontal slider.
    pub const HORIZONTAL_LENGTH: f32 = 200.0;
    /// Default length of a vertical slider.
    pub const VERTICAL_LENGTH: f32 = 160.0;
    /// Thickness of a slider.
    pub const THICKNESS: f32 = crate::amalfi::tokens::control_heights::MINI;
    /// Thickness with tick marks.
    pub const THICKNESS_WITH_TICKS: f32 = 36.0;
    /// Gap between the track and the tick marks.
    pub const TICK_GAP: f32 = crate::amalfi::tokens::spacing::S;
    /// A tick mark's width.
    pub const TICK_WIDTH: f32 = 2.0;
    /// A tick mark's length.
    pub const TICK_LENGTH: f32 = crate::amalfi::tokens::spacing::S;
    /// Glyph size at the ends.
    pub const GLYPH: f32 = crate::amalfi::tokens::control_icons::SMALL;
    /// Gap between a glyph and the track.
    pub const GLYPH_GAP: f32 = crate::amalfi::tokens::spacing::S;
    /// The lens: 8 beyond the thumb along the track, 12 beyond the track across it.
    pub const LENS_LONG: f32 = THUMB + 16.0;
    /// The lens across the track.
    pub const LENS_SHORT: f32 = TRACK + 24.0;
    /// The dial's side.
    pub const DIAL: f32 = 56.0;
    /// The dial's ring diameter.
    pub const DIAL_TRACK: f32 = 44.0;
    /// The distance of the dial's thumb from its centre.
    pub const DIAL_ORBIT: f32 = 20.0;
    /// The thumb is grabbed within this distance of its centre.
    pub const GRAB_RADIUS: f32 = 14.0;
    /// Movement before a press becomes a drag.
    pub const DRAG_SLOP: f32 = 2.0;
}

/// How a thumb looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ThumbLook {
    /// A press turns it `Surface/Control Pressed`.
    pub pressed: bool,
    /// Hover lays `Accent/Amalfi Soft` over it.
    pub hovered: bool,
    /// Keyboard focus adds the halo.
    pub focused: bool,
}

/// Records a round thumb at `rect`: `Thumb`, `State`, `Focus Halo` (names under `prefix`).
pub fn paint_thumb(cx: &mut PaintCx<'_>, prefix: &str, rect: Rect, look: ThumbLook) {
    let theme = *cx.theme();
    let c = theme.colors;
    let shape = Shape::rounded(metrics::THUMB / 2.0);
    let fill = if look.pressed {
        c.surface_control_pressed
    } else {
        c.surface_control
    };
    cx.rec.layer(
        &format!("{prefix}Thumb"),
        rect,
        shape.clone(),
        Style::fill(fill)
            .with_border(Border::inside(c.line_border, 1.0))
            .with_shadow(control_shadow(&theme)),
    );
    if look.hovered && !look.pressed {
        cx.rec.layer(
            &format!("{prefix}State"),
            rect,
            shape.clone(),
            Style::fill(c.accent_amalfi_soft),
        );
    }
    if look.focused {
        focus_halo_named(cx.rec, &format!("{prefix}Focus Halo"), rect, shape, &theme);
    }
}

/// Records the thumb as the one element it is while it turns into the lens: `Thumb`, in
/// `rect`, its corners half the shorter side, at morph `t`.
pub fn paint_morph(cx: &mut PaintCx<'_>, prefix: &str, rect: Rect, t: f32, pressed: bool) {
    let theme = *cx.theme();
    let c = theme.colors;
    let rest = ThumbRest {
        fill: if pressed {
            c.surface_control_pressed
        } else {
            c.surface_control
        },
        border: Some(c.line_border),
        shadow: control_shadow(&theme),
    };
    cx.rec.layer(
        &format!("{prefix}Thumb"),
        rect,
        Shape::rounded(rect.width.min(rect.height) / 2.0),
        morph_style(&theme, t, rest, Tier::Clear),
    );
}

/// The glass of a lens at morph `t` (0: none, 1: full), growing with the morph.
///
/// - [`Tier::Regular`] (`--s-fx-regular`): the `Glass/Regular` blur, saturation, brightness
///   and refraction, the glass of every control at rest.
/// - [`Tier::Clear`] (slider, switch and segmented control, ui-kit `--s-fx-lens` on
///   `.is-lens`): the `Glass/Clear` refraction at the rim and its slight magnification,
///   `blur(1px) saturate(1.2)`, so the labels under the lens stay sharp, only bent and
///   magnified.
pub fn lens_glass(theme: &Theme, t: f32, tier: Tier) -> Blur {
    let t = t.clamp(0.0, 1.0);
    let k = |v: f32| 1.0 + (v - 1.0) * t;
    let glass = match tier {
        Tier::Regular => {
            let m = theme.material("Glass/Regular");
            Glass {
                radius: m.map_or(0.0, |m| m.blur) * t,
                saturation: k(m.map_or(1.0, |m| m.saturation)),
                brightness: k(m.map_or(1.0, |m| m.brightness)),
                distortion: m.map_or(0.0, |m| m.distortion) * t,
                depth: m.map_or(0.0, |m| m.depth),
                chromatic_aberration: m.map_or(0.0, |m| m.chromatic_aberration) * t,
                // The rims are the CSS ones in `morph_style`.
                specular: false,
                magnification: k(m.map_or(1.0, |m| m.magnification)),
            }
        }
        Tier::Clear => {
            let m = theme.material("Glass/Clear");
            Glass {
                radius: t,
                saturation: k(1.2),
                brightness: 1.0,
                distortion: m.map_or(0.3, |m| m.distortion) * t,
                depth: m.map_or(0.5, |m| m.depth),
                chromatic_aberration: m.map_or(0.08, |m| m.chromatic_aberration) * t,
                specular: false,
                magnification: k(m.map_or(1.1, |m| m.magnification)),
            }
        }
    };
    Blur::Glass(glass)
}

/// What a solid thumb looks like at rest, for [`morph_style`].
#[derive(Debug, Clone, Copy)]
pub struct ThumbRest {
    /// The fill.
    pub fill: Color,
    /// The 1 pt ring (none for the switch's white thumb).
    pub border: Option<Color>,
    /// The drop shadow.
    pub shadow: Shadow,
}

fn faded(c: Color, k: f32) -> Color {
    Color::rgba(
        c.r,
        c.g,
        c.b,
        (f32::from(c.a) * k.clamp(0.0, 1.0)).round() as u8,
    )
}

/// The style of a thumb that is turning into a lens, `t` = 0 the solid thumb and 1 the lens: one
/// element whose fill, ring, shadows and glass all move together (ui-kit `.s-slider__thumb`,
/// `.s-switch__thumb`, `.s-segmented__thumb`). The lens: a 1 pt `Glass/Rim Dark` ring,
/// `Glass/Rim Light` (0 1 1) and a white 25 % glow (0 0 3) inside and the [`lens_glass`] of
/// `lens`; a Regular lens also takes the Regular tint and the glass drop shadow
/// (`shadows.glass`), a Clear one stays transparent with no shadow.
pub fn morph_style(theme: &Theme, t: f32, rest: ThumbRest, lens: Tier) -> Style {
    let t = t.clamp(0.0, 1.0);
    let c = theme.colors;
    let mut style = Style::fill(faded(rest.fill, 1.0 - t));
    let regular = lens == Tier::Regular;
    if let Some(tint) = theme.material("Glass/Regular").and_then(|m| m.tint)
        && regular
        && t > 0.0
    {
        style.fills.push(faded(tint, t).into());
    }
    let lens_drop = theme.shadow(4).filter(|_| regular);
    let ring = match rest.border {
        Some(b) => b.lerp(c.glass_rim_dark, t),
        None => faded(c.glass_rim_dark, t),
    };
    style = style.with_border(Border::inside(ring, 1.0));
    let mut drop = rest.shadow;
    drop.color = faded(drop.color, 1.0 - t);
    style = style.with_shadow(drop);
    if let Some(mut d) = lens_drop.filter(|_| t > 0.0) {
        d.color = faded(d.color, t);
        style = style.with_shadow(d);
    }
    if t > 0.0 {
        style = style
            .with_inner_shadow(Shadow::new(faded(c.glass_rim_light, t), 0.0, 1.0, 1.0))
            .with_inner_shadow(Shadow::new(
                faded(Color::rgba(255, 255, 255, 0x40), t),
                0.0,
                0.0,
                3.0,
            ))
            .with_blur(lens_glass(theme, t, lens));
    }
    style
}

/// The timing of a lens morph: `spring.snappy`, or with Reduce Motion the 150 ms fade (and the
/// geometry then stays put).
pub fn morph_timing(reduce: bool) -> Timing {
    if reduce {
        Timing::curve(tokens::motion::FADE_QUICK_CURVE, tokens::motion::FADE_QUICK)
    } else {
        Timing::Spring(tokens::motion::SPRING_SNAPPY)
    }
}

/// The value, the pointer and the keyboard of a slider, shared by the line and the dial.
#[derive(Debug)]
struct Core {
    value: f32,
    min: f32,
    max: f32,
    divisions: Option<u32>,
    pressed: bool,
    dragged: bool,
    active: bool,
    grab: Point,
    down_at: Point,
    growth: Animated<f32>,
    /// Reduce Motion: the lens does not grow.
    reduced: bool,
    keyed: Option<f32>,
}

impl Core {
    fn new() -> Core {
        Core {
            value: 0.0,
            min: 0.0,
            max: 1.0,
            divisions: None,
            pressed: false,
            dragged: false,
            active: false,
            grab: Point::ZERO,
            down_at: Point::ZERO,
            growth: Animated::new(0.0),
            reduced: false,
            keyed: None,
        }
    }

    fn fraction(&self) -> f32 {
        let span = self.max - self.min;
        if !span.is_finite() || span <= 0.0 || !self.value.is_finite() {
            return 0.0;
        }
        ((self.value - self.min) / span).clamp(0.0, 1.0)
    }

    fn value_of(&self, f: f32) -> f32 {
        let mut t = if f.is_finite() {
            f.clamp(0.0, 1.0)
        } else {
            0.0
        };
        if let Some(d) = self.divisions.filter(|d| *d > 0) {
            let d = d as f32;
            t = (t * d).round() / d;
        }
        self.min + t * (self.max - self.min)
    }

    fn emit(&mut self, cx: &mut EventCx<'_>, f: f32) {
        let v = self.value_of(f);
        if v != self.value {
            self.value = v;
            cx.emit(Signal::ValueChanged, Value::Float(f64::from(v)));
            cx.request_paint();
        }
    }

    /// Holds the lens up (or lets it go) without a pointer: `Prop::Dragging`.
    fn force_lens(&mut self, on: bool) {
        self.dragged = on;
        let v = if on { 1.0 } else { 0.0 };
        self.growth.jump(v);
    }

    /// The morph, 0 (the thumb) to 1 (the lens); the spring may overshoot a little.
    fn lens_growth(&self) -> f32 {
        self.growth.get().clamp(0.0, 1.2)
    }

    /// How far the lens is the lens, for its style.
    fn lens_opacity(&self) -> f32 {
        self.growth.get().clamp(0.0, 1.0)
    }

    /// How far its rectangle has grown (none with Reduce Motion).
    fn grown(&self) -> f32 {
        if self.reduced {
            0.0
        } else {
            self.lens_growth()
        }
    }

    fn shows_lens(&self) -> bool {
        self.growth.get() > 0.0
    }

    fn start_drag(&mut self, reduce: bool) {
        self.dragged = true;
        self.reduced = reduce;
        self.growth.animate_to(1.0, morph_timing(reduce));
    }

    fn end(&mut self, cx: &mut EventCx<'_>, reduce: bool) {
        if !self.active {
            return;
        }
        self.active = false;
        self.pressed = false;
        if self.dragged {
            self.dragged = false;
            self.reduced = reduce;
            self.growth.animate_to(0.0, morph_timing(reduce));
            cx.animate();
        }
        cx.emit(Signal::EditingEnded, Value::Float(f64::from(self.value)));
        cx.request_paint();
    }

    fn tick(&mut self, dt: f32) -> bool {
        self.growth.tick(dt)
    }

    fn step(&self) -> f32 {
        self.divisions
            .filter(|d| *d > 0)
            .map_or(0.05, |d| 1.0 / d as f32)
    }

    fn key(&mut self, cx: &mut EventCx<'_>, next: impl Fn(f32) -> f32) {
        let from = self.keyed.unwrap_or_else(|| self.fraction());
        let span = self.max - self.min;
        let to = self.value_of(next(from));
        cx.emit(Signal::EditingBegan, Value::Float(f64::from(self.value)));
        if to != self.value {
            self.value = to;
            cx.emit(Signal::ValueChanged, Value::Float(f64::from(to)));
        }
        self.keyed = Some(if span > 0.0 && span.is_finite() {
            (to - self.min) / span
        } else {
            0.0
        });
        cx.emit(Signal::EditingEnded, Value::Float(f64::from(to)));
        cx.request_paint();
    }

    /// Handles a key; `rtl` is ignored (left-to-right layouts).
    fn handle_key(&mut self, cx: &mut EventCx<'_>, key: &Key) -> Handled {
        let step = self.step();
        match key {
            Key::Up | Key::Right => self.key(cx, |f| f + step),
            Key::Down | Key::Left => self.key(cx, |f| f - step),
            // A tenth of the range, or a division when that is larger (ui-kit `big`).
            Key::PageUp => {
                let big = step.max(0.1);
                self.key(cx, |f| f + big);
            }
            Key::PageDown => {
                let big = step.max(0.1);
                self.key(cx, |f| f - big);
            }
            Key::Home => self.key(cx, |_| 0.0),
            Key::End => self.key(cx, |_| 1.0),
            _ => return Handled::No,
        }
        Handled::Yes
    }

    fn set_common(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::Value => match value.as_f32() {
                Some(v) if v.is_finite() => {
                    self.value = v;
                    self.keyed = None;
                    PropChange::Paint
                }
                _ => PropChange::Unknown,
            },
            Prop::MinValue => match value.as_f32() {
                Some(v) if v.is_finite() => {
                    self.min = v;
                    PropChange::Paint
                }
                _ => PropChange::Unknown,
            },
            Prop::MaxValue => match value.as_f32() {
                Some(v) if v.is_finite() => {
                    self.max = v;
                    PropChange::Paint
                }
                _ => PropChange::Unknown,
            },
            Prop::Step => match value.as_f32() {
                Some(s) if s.is_finite() && s > 0.0 => {
                    let n = ((self.max - self.min) / s).round();
                    self.divisions = (1.0..10_000.0).contains(&n).then_some(n as u32);
                    PropChange::Paint
                }
                _ => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn get_common(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Value => Some(Value::Float(f64::from(self.value))),
            Prop::MinValue => Some(Value::Float(f64::from(self.min))),
            Prop::MaxValue => Some(Value::Float(f64::from(self.max))),
            Prop::Step => Some(self.divisions.map_or(Value::None, |d| {
                Value::Float(f64::from((self.max - self.min) / d as f32))
            })),
            _ => None,
        }
    }

    fn a11y(&self, a: &mut A11y) {
        a.role = Role::Slider;
        a.numeric = Some((
            f64::from(self.value),
            f64::from(self.min),
            f64::from(self.max),
            f64::from(self.step() * (self.max - self.min)),
        ));
        a.value = Some(format!("{}%", (self.fraction() * 100.0).round()));
        a.actions = vec![Action::Increment, Action::Decrement, Action::Focus];
    }
}

fn reduce_motion(cx: &mut EventCx<'_>) -> bool {
    cx.theme().appearance.reduce_motion
}

fn whole(r: Rect) -> Rect {
    let (l, t) = (r.x.round(), r.y.round());
    Rect::new(l, t, r.right().round() - l, r.bottom().round() - t)
}

// ---------------------------------------------------------------------------------------
// The linear slider.
// ---------------------------------------------------------------------------------------

/// A linear slider.
#[derive(Debug)]
pub struct Slider {
    core: Core,
    /// Vertical (0 at the bottom).
    pub vertical: bool,
    /// Tick marks (needs divisions).
    pub tick_marks: bool,
    /// A glyph at the minimum end.
    pub min_glyph: Option<String>,
    /// A glyph at the maximum end.
    pub max_glyph: Option<String>,
    /// A read-only level indicator: no thumb, no input.
    pub indicator: bool,
    length: f32,
}

impl Slider {
    /// A horizontal slider from 0 to 1 at 0.
    pub fn new() -> Slider {
        Slider {
            core: Core::new(),
            vertical: false,
            tick_marks: false,
            min_glyph: None,
            max_glyph: None,
            indicator: false,
            length: metrics::HORIZONTAL_LENGTH,
        }
    }

    /// With a value.
    pub fn value(mut self, v: f32) -> Slider {
        self.core.value = v;
        self
    }

    /// With a range.
    pub fn range(mut self, min: f32, max: f32) -> Slider {
        self.core.min = min;
        self.core.max = max;
        self
    }

    /// With discrete steps.
    pub fn divisions(mut self, n: u32) -> Slider {
        self.core.divisions = Some(n);
        self
    }

    /// With tick marks (one per division and one more).
    pub fn ticks(mut self) -> Slider {
        self.tick_marks = true;
        self
    }

    /// Vertical.
    pub fn vertical(mut self) -> Slider {
        self.vertical = true;
        self
    }

    /// With glyphs at the ends.
    pub fn glyphs(mut self, min: &str, max: &str) -> Slider {
        self.min_glyph = Some(min.to_string());
        self.max_glyph = Some(max.to_string());
        self
    }

    /// A read-only indicator.
    pub fn indicator(mut self) -> Slider {
        self.indicator = true;
        self
    }

    fn has_glyphs(&self) -> bool {
        !self.vertical && (self.min_glyph.is_some() || self.max_glyph.is_some())
    }

    fn glyph_offset(&self) -> f32 {
        if self.has_glyphs() {
            metrics::GLYPH + metrics::GLYPH_GAP
        } else {
            0.0
        }
    }

    fn ticks_count(&self) -> usize {
        match (self.tick_marks, self.core.divisions) {
            (true, Some(d)) => d as usize + 1,
            _ => 0,
        }
    }

    fn across(&self) -> f32 {
        if self.ticks_count() > 0 {
            metrics::THICKNESS_WITH_TICKS
        } else {
            metrics::THICKNESS
        }
    }

    fn track_at() -> f32 {
        (metrics::THUMB - metrics::TRACK) / 2.0
    }

    /// A rectangle `along` the slider, `across` it, in slider coordinates.
    fn rect(&self, along: f32, across: f32, extent: f32, thickness: f32) -> Rect {
        if self.vertical {
            Rect::new(across, self.length - along - extent, thickness, extent)
        } else {
            Rect::new(along, across, extent, thickness)
        }
    }

    fn thumb_rect(&self) -> Rect {
        let along = (self.core.fraction() * (self.length - metrics::THUMB)).round();
        self.rect(along, 0.0, metrics::THUMB, metrics::THUMB)
    }

    fn lens_rect(&self, growth: f32) -> Rect {
        let (w, h) = if self.vertical {
            (metrics::LENS_SHORT, metrics::LENS_LONG)
        } else {
            (metrics::LENS_LONG, metrics::LENS_SHORT)
        };
        let t = self.thumb_rect();
        let c = t.center();
        let to = Rect::new(c.x - w / 2.0, c.y - h / 2.0, w, h);
        whole(Rect::new(
            t.x + (to.x - t.x) * growth,
            t.y + (to.y - t.y) * growth,
            t.width + (to.width - t.width) * growth,
            t.height + (to.height - t.height) * growth,
        ))
    }

    fn fraction_at(&self, p: Point) -> f32 {
        let span = self.length - metrics::THUMB;
        if span <= 0.0 {
            return 0.0;
        }
        let along = if self.vertical {
            self.length - p.y
        } else {
            p.x
        };
        ((along - metrics::THUMB / 2.0) / span).clamp(0.0, 1.0)
    }

    fn size(&self) -> Size {
        let across = self.across();
        let slider = if self.vertical {
            Size::new(across, self.length)
        } else {
            Size::new(self.length, across)
        };
        if self.has_glyphs() {
            Size::new(slider.width + 2.0 * self.glyph_offset(), slider.height)
        } else {
            slider
        }
    }

    fn bed(&self, cx: &mut PaintCx<'_>, prefix: &str, o: Point) {
        let theme = *cx.theme();
        let c = theme.colors;
        let at = Slider::track_at();
        let shift = |r: Rect| r.translate(o);
        let radius = Shape::rounded(metrics::TRACK / 2.0);
        cx.rec.layer(
            &format!("{prefix}Track"),
            shift(self.rect(0.0, at, self.length, metrics::TRACK)),
            radius.clone(),
            Style::fill(c.surface_sunken).with_border(Border::inside(c.line_separator, 1.0)),
        );
        let fill = (self.core.fraction() * self.length).round();
        cx.rec.layer(
            &format!("{prefix}Fill"),
            shift(self.rect(0.0, at, fill, metrics::TRACK)),
            radius,
            Style::fill(c.accent_amalfi),
        );
        let n = self.ticks_count();
        for i in 0..n {
            let f = if n <= 1 {
                0.0
            } else {
                i as f32 / (n - 1) as f32
            };
            let centre = metrics::THUMB / 2.0 + f * (self.length - metrics::THUMB);
            let r = self.rect(
                (centre - metrics::TICK_WIDTH / 2.0).round(),
                at + metrics::TRACK + metrics::TICK_GAP,
                metrics::TICK_WIDTH,
                metrics::TICK_LENGTH,
            );
            // A tick is `TICK_WIDTH` across the track and `TICK_LENGTH` along the drop.
            let r = if self.vertical {
                Rect::new(r.x, r.y, metrics::TICK_LENGTH, metrics::TICK_WIDTH)
            } else {
                r
            };
            cx.rec.layer(
                &format!("{prefix}Tick {i}"),
                shift(r),
                Shape::RECT,
                Style::fill(c.text_tertiary),
            );
        }
    }
}

impl Default for Slider {
    fn default() -> Self {
        Slider::new()
    }
}

impl Widget for Slider {
    fn kind(&self) -> &'static str {
        "Slider"
    }

    fn paints_unclipped(&self) -> bool {
        // The lens grows past the control: over a scroll view's or a group's edge.
        self.core.growth.get() > 0.0
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let room = if self.vertical { p.height } else { p.width };
        let glyphs = 2.0 * self.glyph_offset();
        let default = if self.vertical {
            metrics::VERTICAL_LENGTH
        } else {
            metrics::HORIZONTAL_LENGTH
        };
        self.length = match room {
            Some(r) if r.is_finite() => (r - glyphs).max(metrics::THUMB),
            _ => default,
        };
        self.size()
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let b = cx.bounds();
        // Placed in a frame of another size: the slider takes the room it is given.
        let glyphs = 2.0 * self.glyph_offset();
        let room = if self.vertical { b.height } else { b.width } - glyphs;
        if room.is_finite() && room >= metrics::THUMB {
            self.length = room;
        }
        let theme = *cx.theme();
        let glyph_off = self.glyph_offset();
        let o = Point::new(b.x + glyph_off, b.y);
        let prefix = if self.has_glyphs() { "Slider/" } else { "" };
        if self.has_glyphs() {
            let y = b.y + (self.across() - metrics::GLYPH) / 2.0;
            let g = Rect::new(b.x, y, metrics::GLYPH, metrics::GLYPH);
            if let Some(s) = &self.min_glyph {
                cx.rec
                    .glyph("Minimum glyph", g, s, 400, theme.colors.text_secondary);
            }
            if let Some(s) = &self.max_glyph {
                cx.rec.glyph(
                    "Maximum glyph",
                    Rect::new(
                        b.right() - metrics::GLYPH,
                        y,
                        metrics::GLYPH,
                        metrics::GLYPH,
                    ),
                    s,
                    400,
                    theme.colors.text_secondary,
                );
            }
        }
        self.bed(cx, prefix, o);
        if self.indicator {
            return;
        }
        let thumb = self.thumb_rect().translate(o);
        let focused = cx.focus_visible() && !cx.is_disabled();
        if self.core.shows_lens() {
            let lens = self.lens_rect(self.core.grown()).translate(o);
            paint_morph(
                cx,
                &format!("{prefix}Thumb/"),
                lens,
                self.core.lens_opacity(),
                self.core.pressed,
            );
        } else {
            let look = ThumbLook {
                pressed: self.core.pressed,
                hovered: cx.is_hovered() && !cx.is_disabled(),
                focused,
            };
            paint_thumb(cx, &format!("{prefix}Thumb/"), thumb, look);
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        if self.indicator {
            return Handled::No;
        }
        let b = cx.bounds();
        let o = Point::new(b.x + self.glyph_offset(), b.y);
        let local = |p: &Point| Point::new(p.x - o.x, p.y - o.y);
        let reduce = reduce_motion(cx);
        match event {
            Event::PointerDown {
                pos,
                button: Button::Left,
                ..
            } => {
                let p = local(pos);
                self.core.active = true;
                self.core.pressed = true;
                self.core.down_at = p;
                cx.emit(
                    Signal::EditingBegan,
                    Value::Float(f64::from(self.core.value)),
                );
                // ui-kit `onDown`: the thumb jumps under the pointer (its centre on it) and
                // turns into the lens at once.
                self.core.grab = Point::ZERO;
                let f = self.fraction_at(p);
                self.core.emit(cx, f);
                self.core.start_drag(reduce);
                cx.animate();
                cx.request_paint();
                Handled::Yes
            }
            Event::PointerMove {
                pos,
                dragging: true,
                ..
            } if self.core.active => {
                let p = local(pos);
                if !self.core.dragged
                    && Point::new(p.x - self.core.down_at.x, p.y - self.core.down_at.y).length()
                        >= metrics::DRAG_SLOP
                {
                    self.core.start_drag(reduce);
                    cx.animate();
                }
                let g = self.core.grab;
                let f = self.fraction_at(Point::new(p.x - g.x, p.y - g.y));
                self.core.emit(cx, f);
                Handled::Yes
            }
            Event::PointerUp {
                button: Button::Left,
                ..
            } if self.core.active => {
                self.core.end(cx, reduce);
                Handled::Yes
            }
            Event::Key(k) if k.pressed => {
                let key = k.key.clone();
                self.core.handle_key(cx, &key)
            }
            _ => Handled::No,
        }
    }

    fn tick(&mut self, _cx: &mut EventCx<'_>, dt: f32) -> bool {
        self.core.tick(dt)
    }

    fn state_changed(&mut self, cx: &mut EventCx<'_>, _old: NodeFlags, new: NodeFlags) {
        // `Prop::Pressed`: the pressed thumb without a pointer (ui-kit `force="pressed"`; with a
        // pointer the thumb is the lens at once).
        if !self.core.active && self.core.pressed != new.contains(NodeFlags::PRESSED) {
            self.core.pressed = new.contains(NodeFlags::PRESSED);
            cx.request_paint();
        }
    }

    fn interaction(&self) -> Interaction {
        Interaction::Hover
    }

    fn focus(&self) -> FocusKind {
        if self.indicator {
            FocusKind::None
        } else {
            FocusKind::Control
        }
    }

    fn hit(&self, p: Point, size: Size) -> bool {
        !self.indicator && Rect::from_size(size).contains(p)
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::TickMarks => match value.as_int() {
                Some(n) if n >= 2 => {
                    self.core.divisions = u32::try_from(n - 1).ok();
                    self.tick_marks = true;
                    PropChange::Layout
                }
                Some(_) => {
                    self.tick_marks = false;
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::Vertical => match value.as_bool() {
                Some(v) => {
                    self.vertical = v;
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::ItemSymbols => match value.as_str_list() {
                Some(l) => {
                    self.min_glyph = l.first().filter(|s| !s.is_empty()).cloned();
                    self.max_glyph = l.get(1).filter(|s| !s.is_empty()).cloned();
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::Dragging => match value.as_bool() {
                Some(d) => {
                    self.core.force_lens(d);
                    PropChange::Paint
                }
                None => PropChange::Unknown,
            },
            other => self.core.set_common(other, value),
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Dragging => Some(Value::Bool(self.core.dragged)),
            Prop::Vertical => Some(Value::Bool(self.vertical)),
            Prop::TickMarks => Some(Value::Int(self.ticks_count() as i64)),
            other => self.core.get_common(other),
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        self.core.a11y(a);
    }

    fn dims_when_disabled(&self) -> bool {
        !self.indicator
    }
}

// ---------------------------------------------------------------------------------------
// The dial.
// ---------------------------------------------------------------------------------------

/// A circular slider: a 56 pt dial with a 4 pt ring, the value drawn as an arc from the top
/// clockwise and the round thumb on the ring at the value.
#[derive(Debug)]
pub struct Dial {
    core: Core,
}

impl Dial {
    /// A dial from 0 to 1 at 0.
    pub fn new() -> Dial {
        Dial { core: Core::new() }
    }

    /// With a value.
    pub fn value(mut self, v: f32) -> Dial {
        self.core.value = v;
        self
    }

    fn centre() -> Point {
        Point::new(metrics::DIAL / 2.0, metrics::DIAL / 2.0)
    }

    fn thumb(f: f32) -> Rect {
        let a = f * std::f32::consts::TAU;
        let c = Dial::centre();
        let (x, y) = (
            c.x + a.sin() * metrics::DIAL_ORBIT,
            c.y - a.cos() * metrics::DIAL_ORBIT,
        );
        Rect::new(
            (x - metrics::THUMB / 2.0).round(),
            (y - metrics::THUMB / 2.0).round(),
            metrics::THUMB,
            metrics::THUMB,
        )
    }

    fn fraction_at(&self, p: Point) -> Option<f32> {
        let c = Dial::centre();
        let (vx, vy) = (p.x - c.x, p.y - c.y);
        if vx.hypot(vy) < 1.0 {
            return None;
        }
        let mut f = vx.atan2(-vy) / std::f32::consts::TAU;
        if f < 0.0 {
            f += 1.0;
        }
        // While dragging, no wrap across the top: past it, stay at the end the value is at.
        let now = self.core.fraction();
        if self.core.dragged && (f - now).abs() > 0.5 {
            f = if now > 0.5 { 1.0 } else { 0.0 };
        }
        Some(f)
    }
}

impl Default for Dial {
    fn default() -> Self {
        Dial::new()
    }
}

impl Widget for Dial {
    fn kind(&self) -> &'static str {
        "Dial"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        Size::new(metrics::DIAL, metrics::DIAL)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme: Theme = *cx.theme();
        let c = theme.colors;
        let b = cx.bounds();
        let o = Point::new(b.x, b.y);
        let ring = Rect::new(
            (metrics::DIAL - metrics::DIAL_TRACK) / 2.0,
            (metrics::DIAL - metrics::DIAL_TRACK) / 2.0,
            metrics::DIAL_TRACK,
            metrics::DIAL_TRACK,
        );
        let f = self.core.fraction();
        cx.rec.layer(
            "Track",
            ring.translate(o),
            Shape::Ellipse,
            Style::default().with_border(Border::at(
                c.surface_sunken,
                metrics::TRACK,
                BorderPosition::Center,
            )),
        );
        if f > 0.0 {
            let mut border = Border::at(c.accent_amalfi, metrics::TRACK, BorderPosition::Center);
            border.round_ends = true;
            if f >= 1.0 {
                cx.rec.layer(
                    "Progress",
                    ring.translate(o),
                    Shape::Ellipse,
                    Style::default().with_border(border),
                );
            } else {
                let mut pb = PathBuilder::new();
                pb.add_arc(ring, -90.0, f * 360.0);
                let path = pb.detach();
                let r = path.bounds();
                let local = path.make_offset((-r.x, -r.y));
                cx.rec.layer(
                    "Progress",
                    r.translate(o),
                    Shape::Path(local),
                    Style::default().with_border(border),
                );
            }
        }
        let thumb = Dial::thumb(f).translate(o);
        if self.core.shows_lens() {
            let g = self.core.grown();
            let c = thumb.center();
            let (w, h) = (
                metrics::THUMB + (metrics::LENS_LONG - metrics::THUMB) * g,
                metrics::THUMB + (metrics::LENS_SHORT - metrics::THUMB) * g,
            );
            let lens = whole(Rect::new(c.x - w / 2.0, c.y - h / 2.0, w, h));
            paint_morph(
                cx,
                "Thumb/",
                lens,
                self.core.lens_opacity(),
                self.core.pressed,
            );
        } else {
            let look = ThumbLook {
                pressed: self.core.pressed,
                hovered: cx.is_hovered() && !cx.is_disabled(),
                focused: cx.focus_visible() && !cx.is_disabled(),
            };
            paint_thumb(cx, "Thumb/", thumb, look);
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let b = cx.bounds();
        let local = |p: &Point| Point::new(p.x - b.x, p.y - b.y);
        let reduce = reduce_motion(cx);
        match event {
            Event::PointerDown {
                pos,
                button: Button::Left,
                ..
            } => {
                let p = local(pos);
                self.core.active = true;
                self.core.pressed = true;
                self.core.down_at = p;
                cx.emit(
                    Signal::EditingBegan,
                    Value::Float(f64::from(self.core.value)),
                );
                let t = Dial::thumb(self.core.fraction()).center();
                let from_thumb = Point::new(p.x - t.x, p.y - t.y);
                if from_thumb.length() <= metrics::GRAB_RADIUS {
                    self.core.grab = from_thumb;
                } else {
                    self.core.grab = Point::ZERO;
                    if let Some(f) = self.fraction_at(p) {
                        self.core.emit(cx, f);
                    }
                }
                cx.request_paint();
                Handled::Yes
            }
            Event::PointerMove {
                pos,
                dragging: true,
                ..
            } if self.core.active => {
                let p = local(pos);
                if !self.core.dragged
                    && Point::new(p.x - self.core.down_at.x, p.y - self.core.down_at.y).length()
                        >= metrics::DRAG_SLOP
                {
                    self.core.start_drag(reduce);
                    cx.animate();
                }
                let g = self.core.grab;
                if let Some(f) = self.fraction_at(Point::new(p.x - g.x, p.y - g.y)) {
                    self.core.emit(cx, f);
                }
                Handled::Yes
            }
            Event::PointerUp {
                button: Button::Left,
                ..
            } if self.core.active => {
                self.core.end(cx, reduce);
                Handled::Yes
            }
            Event::Key(k) if k.pressed => {
                let key = k.key.clone();
                self.core.handle_key(cx, &key)
            }
            _ => Handled::No,
        }
    }

    fn tick(&mut self, _cx: &mut EventCx<'_>, dt: f32) -> bool {
        self.core.tick(dt)
    }

    fn state_changed(&mut self, cx: &mut EventCx<'_>, _old: NodeFlags, new: NodeFlags) {
        if !self.core.active && self.core.pressed != new.contains(NodeFlags::PRESSED) {
            self.core.pressed = new.contains(NodeFlags::PRESSED);
            cx.request_paint();
        }
    }

    fn interaction(&self) -> Interaction {
        Interaction::Hover
    }

    fn focus(&self) -> FocusKind {
        FocusKind::Control
    }

    fn hit(&self, p: Point, size: Size) -> bool {
        Rect::from_size(size).contains(p)
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        self.core.set_common(prop, value)
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        self.core.get_common(prop)
    }

    fn accessibility(&self, a: &mut A11y) {
        self.core.a11y(a);
    }
}

// ---------------------------------------------------------------------------------------
// The thumbs as views of their own (the kit has masters for them).
// ---------------------------------------------------------------------------------------

/// A round thumb on its own (`Sliders/Thumb/Round`).
#[derive(Debug, Default)]
pub struct SliderThumb {
    /// How it looks.
    pub look: ThumbLook,
}

impl Widget for SliderThumb {
    fn kind(&self) -> &'static str {
        "SliderThumb"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        Size::new(metrics::THUMB, metrics::THUMB)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let b = cx.bounds();
        paint_thumb(cx, "", b, self.look);
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }
}

/// The lens thumb on its own (`Sliders/Thumb/Lens/Dragging`).
#[derive(Debug, Default)]
pub struct LensThumb;

impl Widget for LensThumb {
    fn kind(&self) -> &'static str {
        "LensThumb"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        Size::new(metrics::LENS_LONG, metrics::LENS_SHORT)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let b = cx.bounds();
        let theme = *cx.theme();
        let rest = ThumbRest {
            fill: theme.colors.surface_control,
            border: Some(theme.colors.line_border),
            shadow: control_shadow(&theme),
        };
        cx.rec.layer(
            "Lens",
            b,
            Shape::rounded(b.width.min(b.height) / 2.0),
            morph_style(&theme, 1.0, rest, Tier::Clear),
        );
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rest(t: &Theme) -> ThumbRest {
        ThumbRest {
            fill: t.colors.surface_control,
            border: Some(t.colors.line_border),
            shadow: control_shadow(t),
        }
    }

    #[test]
    fn the_lens_is_clear_glass_with_the_css_rims() {
        let t = Theme::light();
        let lens = morph_style(&t, 1.0, rest(&t), Tier::Clear);
        // The thumb's fill is gone, no tint and no glass drop shadow, only the rims inside,
        // and Clear glass behind (`blur(1px) saturate(1.2)`, bent at the rim).
        assert!(
            lens.fills
                .iter()
                .all(|f| f.solid().is_some_and(|c| c.a == 0))
        );
        assert!(lens.shadows.iter().all(|s| s.color.a == 0));
        assert_eq!(lens.borders.len(), 1);
        assert_eq!(lens.inner_shadows.len(), 2);
        let Some(Blur::Glass(g)) = lens.blurs.first().copied() else {
            panic!("no glass");
        };
        assert!((g.radius - 1.0).abs() < 1e-6 && (g.saturation - 1.2).abs() < 1e-6);
        assert!(g.distortion > 0.0);
    }

    #[test]
    fn a_dragged_slider_thumb_morphs_into_one_glass_element() {
        let (mut ui, win, id, _log) = scene(Slider::new().value(0.5));
        let thumbs = |ui: &mut Ui| -> Vec<(Rect, bool)> {
            ui.inspect_layers(id)
                .iter()
                .filter(|l| l.path.ends_with("Thumb/Thumb"))
                .map(|l| {
                    let glass = matches!(
                        &l.content,
                        crate::core::InspectedContent::Shape { style, .. }
                            if matches!(style.blurs.first(), Some(Blur::Glass(_)))
                    );
                    (l.frame, glass)
                })
                .collect()
        };
        let rest = thumbs(&mut ui);
        assert_eq!(rest.len(), 1);
        assert!(!rest[0].1);
        let p = at(&ui, id, 100.0, 10.0);
        ui.pointer_move(win, p, Mods::empty());
        ui.pointer_down(win, p, Button::Left, Mods::empty());
        // Mid-spring: still one element, already growing.
        ui.advance(0.05);
        ui.prepare_frame(win);
        let mid = thumbs(&mut ui);
        assert_eq!(mid.len(), 1, "{mid:?}");
        assert!(mid[0].0.width > rest[0].0.width);
        for _ in 0..60 {
            ui.advance(1.0 / 60.0);
        }
        ui.prepare_frame(win);
        let held = thumbs(&mut ui);
        assert_eq!(held.len(), 1);
        assert!(held[0].1, "the held thumb is glass");
        assert!(
            (held[0].0.width - metrics::LENS_LONG).abs() < 0.6,
            "{held:?}"
        );
        ui.pointer_up(win, p, Button::Left, Mods::empty());
        for _ in 0..60 {
            ui.advance(1.0 / 60.0);
        }
        ui.prepare_frame(win);
        let back = thumbs(&mut ui);
        assert!(!back[0].1 && (back[0].0.width - rest[0].0.width).abs() < 0.6);
    }

    #[test]
    fn reduced_motion_morphs_the_slider_thumb_without_growing() {
        let (mut ui, win, id, _log) = scene(Slider::new().value(0.5));
        let mut a = ui.appearance;
        a.reduce_motion = true;
        ui.set_appearance(a);
        let p = at(&ui, id, 100.0, 10.0);
        ui.pointer_move(win, p, Mods::empty());
        ui.pointer_down(win, p, Button::Left, Mods::empty());
        for _ in 0..30 {
            ui.advance(1.0 / 60.0);
        }
        ui.prepare_frame(win);
        let w = ui
            .inspect_layers(id)
            .iter()
            .find(|l| l.path.ends_with("Thumb/Thumb"))
            .map(|l| l.frame.width)
            .unwrap_or(0.0);
        assert!((w - metrics::THUMB).abs() < 0.6, "no scale: {w}");
    }

    #[test]
    fn the_thumb_and_the_lens_are_one_element() {
        let t = Theme::light();
        let at_rest = morph_style(&t, 0.0, rest(&t), Tier::Regular);
        assert!(at_rest.blurs.is_empty() && at_rest.inner_shadows.is_empty());
        let half = morph_style(&t, 0.5, rest(&t), Tier::Regular);
        let Some(Blur::Glass(g)) = half.blurs.first().copied() else {
            panic!("no glass");
        };
        let full = t.material("Glass/Regular").map_or(0.0, |m| m.blur);
        assert!(g.radius > 0.0 && g.radius < full);
        // A Clear lens (every control's) barely frosts and magnifies a little.
        let clear = morph_style(&t, 0.5, rest(&t), Tier::Clear);
        let Some(Blur::Glass(g)) = clear.blurs.first().copied() else {
            panic!("no glass");
        };
        assert!((g.radius - 0.5).abs() < 1e-6);
        assert!(g.magnification > 1.0 && g.magnification < 1.1 + 1e-3);
        assert!(
            half.fills
                .iter()
                .any(|f| f.solid().is_some_and(|c| c.a > 0))
        );
    }
    use crate::core::event::{KeyEvent, Mods};
    use crate::core::ui::Ui;
    use crate::core::window::{WindowId, WindowKind};
    use std::cell::RefCell;
    use std::rc::Rc;

    type Log = Rc<RefCell<Vec<f64>>>;

    fn scene(w: impl Widget + 'static) -> (Ui, WindowId, crate::core::ViewId, Log) {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(300.0, 200.0));
        let id = ui.create(w);
        let log: Log = Rc::new(RefCell::new(Vec::new()));
        let l = log.clone();
        ui.connect(id, Signal::ValueChanged, move |_, _, v| {
            l.borrow_mut().push(v.as_f64().unwrap_or(f64::NAN));
        });
        let _ = ui.set(id, Prop::MaxWidth, Value::from(200.0));
        ui.set_content(win, id);
        ui.show_window(win);
        ui.prepare_frame(win);
        (ui, win, id, log)
    }

    fn at(ui: &Ui, id: crate::core::ViewId, x: f32, y: f32) -> Point {
        let r = ui.tree.window_rect(id);
        Point::new(r.x + x, r.y + y)
    }

    #[test]
    fn the_thumb_becomes_the_lens_at_once_under_the_pointer() {
        let (mut ui, win, id, log) = scene(Slider::new().value(0.0));
        // Pressed on the thumb's edge: the thumb's centre goes under the pointer (ui-kit
        // `fromPointer`), and the lens is up without moving first.
        let p = at(&ui, id, 18.0, 10.0);
        ui.pointer_move(win, p, Mods::empty());
        ui.pointer_down(win, p, Button::Left, Mods::empty());
        assert!(
            log.borrow()
                .last()
                .is_some_and(|v| (*v - 8.0 / 180.0).abs() < 0.01)
        );
        assert_eq!(ui.get(id, Prop::Dragging), Some(Value::Bool(true)));
        for _ in 0..60 {
            ui.advance(0.02);
        }
        let shown = ui.widget::<Slider>(id).is_some_and(|s| s.core.shows_lens());
        assert!(shown);
        ui.pointer_up(win, p, Button::Left, Mods::empty());
        for _ in 0..60 {
            ui.advance(0.02);
        }
        assert_eq!(ui.get(id, Prop::Dragging), Some(Value::Bool(false)));
    }

    #[test]
    fn dragging_can_be_held_without_a_pointer() {
        let (mut ui, _win, id, _log) = scene(Slider::new().value(0.5));
        let _ = ui.set(id, Prop::Dragging, Value::Bool(true));
        assert!(ui.widget::<Slider>(id).is_some_and(|s| s.core.shows_lens()));
        let _ = ui.set(id, Prop::Dragging, Value::Bool(false));
        assert!(
            ui.widget::<Slider>(id)
                .is_some_and(|s| !s.core.shows_lens())
        );
    }

    #[test]
    fn clicking_the_track_jumps_and_dragging_follows() {
        let (mut ui, win, id, log) = scene(Slider::new().value(0.0));
        // The track at 100 of 200: the centre of the thumb range is 10 + 90.
        let p = at(&ui, id, 100.0, 10.0);
        ui.pointer_move(win, p, Mods::empty());
        ui.pointer_down(win, p, Button::Left, Mods::empty());
        assert_eq!(log.borrow().len(), 1);
        assert!((log.borrow()[0] - 0.5).abs() < 0.01, "{:?}", log.borrow());
        let q = at(&ui, id, 190.0, 10.0);
        ui.pointer_move(win, q, Mods::empty());
        ui.pointer_up(win, q, Button::Left, Mods::empty());
        assert_eq!(ui.get(id, Prop::Value), Some(Value::Float(1.0)));
        for _ in 0..60 {
            ui.advance(0.05);
        }
        assert!(
            ui.widget::<Slider>(id)
                .is_some_and(|s| !s.core.shows_lens())
        );
    }

    #[test]
    fn steps_snap_and_keys_move_by_a_division() {
        let (mut ui, win, id, log) = scene(Slider::new().divisions(4).value(0.5));
        ui.set_focus(id, true);
        ui.key(win, KeyEvent::press(Key::Right, Mods::empty()));
        ui.key(win, KeyEvent::press(Key::Right, Mods::empty()));
        ui.key(win, KeyEvent::press(Key::End, Mods::empty()));
        ui.key(win, KeyEvent::press(Key::Home, Mods::empty()));
        assert_eq!(*log.borrow(), vec![0.75, 1.0, 0.0]);
    }

    #[test]
    fn page_keys_move_a_tenth_of_the_range() {
        let (mut ui, win, id, log) = scene(Slider::new().value(0.5));
        ui.set_focus(id, true);
        ui.key(win, KeyEvent::press(Key::PageUp, Mods::empty()));
        ui.key(win, KeyEvent::press(Key::PageDown, Mods::empty()));
        ui.key(win, KeyEvent::press(Key::PageDown, Mods::empty()));
        let v: Vec<f64> = log
            .borrow()
            .iter()
            .map(|x| (x * 100.0).round() / 100.0)
            .collect();
        assert_eq!(v, vec![0.6, 0.5, 0.4]);
    }

    #[test]
    fn dial_follows_the_angle() {
        let (mut ui, win, id, _log) = scene(Dial::new());
        // Straight right of the centre is a quarter turn.
        let p = at(&ui, id, 50.0, 28.0);
        ui.pointer_move(win, p, Mods::empty());
        ui.pointer_down(win, p, Button::Left, Mods::empty());
        ui.pointer_up(win, p, Button::Left, Mods::empty());
        let v = ui
            .get(id, Prop::Value)
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        assert!((v - 0.25).abs() < 0.01, "{v}");
    }

    #[test]
    fn disabled_ignores_input() {
        let (mut ui, win, id, log) = scene(Slider::new());
        let _ = ui.set(id, Prop::Disabled, Value::Bool(true));
        let p = at(&ui, id, 100.0, 10.0);
        ui.pointer_move(win, p, Mods::empty());
        ui.pointer_down(win, p, Button::Left, Mods::empty());
        ui.pointer_up(win, p, Button::Left, Mods::empty());
        assert!(log.borrow().is_empty());
    }
}
