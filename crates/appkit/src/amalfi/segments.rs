//! Segmented controls (kit `Segments`): the segmented control (select one or any), the
//! action segments (toolbar capsules, the Align group), the Icon + Label control for window
//! panes, and the single items the kit also has as masters.

use crate::amalfi::foundation::{ControlSize, Label, control_shadow, focus_halo_named, whole};
use crate::amalfi::materials::Tier;
use crate::amalfi::sliders::{ThumbRest, morph_style};
use crate::amalfi::theme::Theme;
use crate::amalfi::tokens::{self, TypeStyle, control_heights, typography};
use crate::color::Color;
use crate::core::NodeFlags;
use crate::core::a11y::{A11y, Action, Role};
use crate::core::event::{Button, Event, Handled, Key};
use crate::core::layout::Proposal;
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, FocusKind, Interaction, LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Point, Rect, Size};
use crate::motion::{Animated, Timing};
use crate::shape::Shape;
use crate::style::{Border, Style};
use crate::text::Align;

/// How much a segmented control grows while held down (ui-kit `--press-scale-segmented`).
const PRESS_SCALE_SEGMENTED: f32 = 1.035;
/// The lens is `Glass/Regular`, with corners 1.1 × the item's height.
const LENS_RADIUS: f32 = 1.1;

/// Fixed measures.
pub mod metrics {
    /// Space between a control's track and its items.
    pub const TRACK_PADDING: f32 = crate::amalfi::tokens::spacing::XS;
    /// Horizontal padding of a text item.
    pub const TEXT_ITEM_PADDING: f32 = crate::amalfi::tokens::spacing::S;
    /// Separator width.
    pub const SEPARATOR_WIDTH: f32 = 1.0;
    /// Inset of a text highlight at its sides.
    pub const TEXT_HIGHLIGHT_INSET: f32 = crate::amalfi::tokens::spacing::XS;
    /// Text boxes are a multiple of this wide (the 4 pt grid).
    pub const STEP: f32 = crate::amalfi::tokens::grid::BASE;
    /// The Icon + Label control: bar height.
    pub const PANE_BAR_HEIGHT: f32 = 52.0;
    /// The Icon + Label control: pane height.
    pub const PANE_HEIGHT: f32 = 44.0;
    /// The Icon + Label control: narrowest pane.
    pub const MIN_PANE_WIDTH: f32 = 72.0;
    /// The Icon + Label control: glyph size.
    pub const PANE_GLYPH: f32 = crate::amalfi::tokens::control_icons::LARGE;
    /// The Icon + Label control: top of the label in a pane.
    pub const PANE_LABEL_TOP: f32 = 26.0;
}

/// The highlight fades out in this many seconds (`motion.md`: push and icon buttons).
pub const HIGHLIGHT_OUT: f32 = 0.06;

/// The measures of segments at one control size (silica_ui's `AmalfiSegmentSizeMetrics`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SizeMetrics {
    /// The size.
    pub size: ControlSize,
    /// Track height.
    pub height: f32,
    /// Width of an action text segment.
    pub text_segment_width: f32,
    /// Separator height.
    pub separator_height: f32,
    /// Inset of a hover highlight.
    pub hover_inset: f32,
    /// Width of a text item.
    pub text_item_width: f32,
}

impl SizeMetrics {
    /// The metrics of `size`; a toolbar's Large is 36 tall.
    pub fn of(size: ControlSize, toolbar: bool) -> SizeMetrics {
        match size {
            ControlSize::Mini => SizeMetrics {
                size,
                height: control_heights::MINI,
                text_segment_width: 40.0,
                separator_height: 12.0,
                hover_inset: 0.0,
                text_item_width: 44.0,
            },
            ControlSize::Small => SizeMetrics {
                size,
                height: control_heights::SMALL,
                text_segment_width: 44.0,
                separator_height: 12.0,
                hover_inset: 0.0,
                text_item_width: 56.0,
            },
            ControlSize::Regular => SizeMetrics {
                size,
                height: control_heights::REGULAR,
                text_segment_width: 52.0,
                separator_height: 16.0,
                hover_inset: 4.0,
                text_item_width: 72.0,
            },
            ControlSize::Large => SizeMetrics {
                size,
                height: if toolbar {
                    control_heights::TOOLBAR
                } else {
                    control_heights::LARGE
                },
                text_segment_width: 56.0,
                separator_height: 16.0,
                hover_inset: 4.0,
                text_item_width: 88.0,
            },
        }
    }

    /// Whether separators take a place of their own between icon segments (toolbar items).
    pub fn separator_takes_space(&self) -> bool {
        self.height == control_heights::TOOLBAR
    }

    /// The glyph's size.
    pub fn glyph(&self) -> f32 {
        self.size.icon()
    }

    /// An item's height inside a track.
    pub fn item_height(&self) -> f32 {
        self.height - 2.0 * metrics::TRACK_PADDING
    }

    /// The label token of an action segment.
    pub fn action_token(&self) -> &'static TypeStyle {
        match self.size {
            ControlSize::Mini | ControlSize::Small => &typography::CALLOUT,
            _ => &typography::BODY,
        }
    }

    /// The label token of an item.
    pub fn item_token(&self) -> &'static TypeStyle {
        match self.size {
            ControlSize::Mini => &typography::CAPTION,
            ControlSize::Small => &typography::LABEL,
            _ => &typography::BODY,
        }
    }
}

/// What a segment shows.
#[derive(Debug, Clone, PartialEq)]
pub enum Content {
    /// A label.
    Text(String),
    /// A symbol (Silica Symbols name).
    Icon(String),
}

/// A segment of a control.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    /// What it shows.
    pub content: Content,
    /// Whether it can be chosen.
    pub enabled: bool,
    /// The accessible label (icons need one).
    pub label: Option<String>,
    /// The tooltip.
    pub tooltip: Option<String>,
}

impl Segment {
    /// A text segment.
    pub fn text(label: &str) -> Segment {
        Segment {
            content: Content::Text(label.to_string()),
            enabled: true,
            label: None,
            tooltip: None,
        }
    }

    /// An icon segment with its accessible label.
    pub fn icon(symbol: &str, label: &str) -> Segment {
        Segment {
            content: Content::Icon(symbol.to_string()),
            enabled: true,
            label: Some(label.to_string()),
            tooltip: None,
        }
    }

    /// Disabled.
    pub fn disabled(mut self) -> Segment {
        self.enabled = false;
        self
    }

    fn a11y_label(&self) -> String {
        match (&self.label, &self.content) {
            (Some(l), _) => l.clone(),
            (None, Content::Text(t)) => t.clone(),
            (None, Content::Icon(_)) => String::new(),
        }
    }

    fn is_icon(&self) -> bool {
        matches!(self.content, Content::Icon(_))
    }
}

/// How an item looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ItemLook {
    /// No fill.
    #[default]
    Rest,
    /// `Line/Separator`.
    Hover,
    /// `Line/Border`.
    Pressed,
    /// `Surface/Control` with a border and the level 1 shadow.
    Selected,
    /// Selected and pressed.
    SelectedPressed,
}

impl ItemLook {
    fn style(self, theme: &Theme) -> Style {
        let c = &theme.colors;
        match self {
            ItemLook::Rest => Style::default(),
            ItemLook::Hover => Style::fill(c.line_separator),
            ItemLook::Pressed => Style::fill(c.line_border),
            ItemLook::Selected | ItemLook::SelectedPressed => {
                let fill = if self == ItemLook::Selected {
                    c.surface_control
                } else {
                    c.surface_control_pressed
                };
                Style::fill(fill)
                    .with_border(Border::inside(c.line_border, 1.0))
                    .with_shadow(control_shadow(theme))
            }
        }
    }
}

/// The thumb at `t` = 0 and the glass lens at `t` = 1: one element whose every property moves
/// together (ui-kit `.s-segmented__thumb` and `.is-lens`). At rest: `Surface/Control`, a 1 pt
/// `Line/Border` ring and `0 1 2` shadow, corners `item / 2`. As the lens: `Glass/Regular`
/// (its tint, rims, glow and `shadows.glass`), corners `item * 1.1`. Always under the labels.
fn paint_thumb(cx: &mut PaintCx<'_>, theme: &Theme, rect: Rect, item_h: f32, t: f32) {
    let c = &theme.colors;
    let rest = ThumbRest {
        fill: c.surface_control,
        border: Some(c.line_border),
        shadow: control_shadow(theme),
    };
    let style = morph_style(theme, t, rest, Tier::Regular);
    let radius = item_h / 2.0 + (item_h * LENS_RADIUS - item_h / 2.0) * t;
    cx.rec.layer("Thumb", rect, Shape::rounded(radius), style);
}

/// Text laid out for a segment, one cache per segment.
#[derive(Debug, Default)]
struct Labels {
    items: Vec<Label>,
    fallback: Label,
}

impl Labels {
    fn get(&mut self, i: usize) -> &mut Label {
        if self.items.len() <= i {
            self.items.resize_with(i + 1, Label::default);
        }
        match self.items.get_mut(i) {
            Some(l) => l,
            None => &mut self.fallback,
        }
    }
}

/// Records one label centred in `rect` (the layer's box is the item's width).
fn paint_label(
    cx: &mut PaintCx<'_>,
    cache: &mut Label,
    name: &str,
    rect: Rect,
    text: &str,
    token: &'static TypeStyle,
    color: Color,
) {
    let theme = *cx.theme();
    let style = theme.text(token, color);
    let lh = style.line_height.unwrap_or(token.size * 1.3);
    let layout = cache.layout(text, &style, Align::Center, rect.width, Some(1));
    cx.rec.text(
        name,
        Rect::new(rect.x, rect.y + (rect.height - lh) / 2.0, rect.width, lh),
        layout,
        Some(token.name),
        color,
    );
}

fn is_disabled(cx: &mut EventCx<'_>) -> bool {
    let id = cx.id();
    cx.ui().is_disabled(id)
}

fn text_width(cache: &mut Label, theme: &Theme, token: &'static TypeStyle, text: &str) -> f32 {
    let style = theme.text(token, theme.colors.text_primary);
    whole(cache.width(text, &style), metrics::STEP)
}

// ---------------------------------------------------------------------------------------
// The segmented control.
// ---------------------------------------------------------------------------------------

/// Whether a segmented control selects one segment or any number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Selection {
    /// Exactly one (a radio group).
    #[default]
    One,
    /// Each segment on or off.
    Any,
}

/// The lens a pressed segment of a select-one control swells into (ui-kit `SSegmented`
/// `measure`, `materials.md` lens rule): 8 beyond the item on each side, 12 above and below.
pub fn lens_rect(item: Rect) -> Rect {
    Rect::new(
        item.x - 8.0,
        item.y - 12.0,
        item.width + 16.0,
        item.height + 24.0,
    )
}

/// A segmented control.
#[derive(Debug)]
pub struct SegmentedControl {
    /// The segments.
    pub segments: Vec<Segment>,
    /// Select one or any.
    pub mode: Selection,
    /// Selected indices, ascending.
    pub selected: Vec<usize>,
    /// Size.
    pub size: ControlSize,
    /// A toolbar's Large control is 36 tall.
    pub toolbar: bool,
    /// Minimum width of a text item.
    pub text_item_width: Option<f32>,
    labels: Labels,
    items: Vec<Rect>,
    width: f32,
    thumb: Animated<Rect>,
    /// 0 the thumb, 1 the glass lens (the same element morphing, `spring.snappy`).
    lens: Animated<f32>,
    /// The item held down: the lens is around it (it follows the pointer item to item).
    hot: Option<usize>,
    /// Held down by `Prop::Pressed` (the showcase's `force="pressed"`), not by the pointer.
    forced: bool,
    hovered: Option<usize>,
    down: bool,
    pressed_item: Option<usize>,
    pressed_inside: bool,
    focus_index: usize,
}

impl SegmentedControl {
    /// A control of `segments`, select one, nothing selected.
    pub fn new(segments: Vec<Segment>) -> SegmentedControl {
        SegmentedControl {
            segments,
            mode: Selection::One,
            selected: Vec::new(),
            size: ControlSize::Regular,
            toolbar: false,
            text_item_width: None,
            labels: Labels::default(),
            items: Vec::new(),
            width: 0.0,
            thumb: Animated::new(Rect::ZERO),
            lens: Animated::new(0.0),
            hot: None,
            forced: false,
            hovered: None,
            down: false,
            pressed_item: None,
            pressed_inside: false,
            focus_index: 0,
        }
    }

    /// Text segments.
    pub fn texts(labels: &[&str]) -> SegmentedControl {
        SegmentedControl::new(labels.iter().map(|l| Segment::text(l)).collect())
    }

    /// With a selection mode.
    pub fn mode(mut self, mode: Selection) -> SegmentedControl {
        self.mode = mode;
        self
    }

    /// With the selected segments.
    pub fn selected(mut self, selected: &[usize]) -> SegmentedControl {
        self.selected = selected.to_vec();
        self.selected.sort_unstable();
        self.selected.dedup();
        if let Some(i) = self.selected.first() {
            self.focus_index = *i;
        }
        self
    }

    /// With a size.
    pub fn size(mut self, size: ControlSize) -> SegmentedControl {
        self.size = size;
        self
    }

    fn metrics(&self) -> SizeMetrics {
        SizeMetrics::of(self.size, self.toolbar)
    }

    fn can_choose(&self, i: usize, enabled: bool) -> bool {
        enabled && self.segments.get(i).is_some_and(|s| s.enabled)
    }

    /// Lays the items out: text items share the widest width.
    fn compute(&mut self, theme: &Theme) {
        let m = self.metrics();
        let mut text_w = self.text_item_width.unwrap_or(m.text_item_width);
        let token = m.item_token();
        for i in 0..self.segments.len() {
            let Some(Content::Text(t)) = self.segments.get(i).map(|s| s.content.clone()) else {
                continue;
            };
            let w =
                text_width(self.labels.get(i), theme, token, &t) + 2.0 * metrics::TEXT_ITEM_PADDING;
            text_w = text_w.max(w);
        }
        self.items.clear();
        let mut x = metrics::TRACK_PADDING;
        for s in &self.segments {
            let w = if s.is_icon() { m.height } else { text_w };
            self.items
                .push(Rect::new(x, metrics::TRACK_PADDING, w, m.item_height()));
            x += w;
        }
        self.width = x + metrics::TRACK_PADDING;
        if !self.thumb.is_animating() && self.hot.is_none() && self.lens.get() <= 0.0 {
            let r = self.single().and_then(|i| self.items.get(i)).copied();
            self.thumb.jump(r.unwrap_or(Rect::ZERO));
        }
    }

    fn single(&self) -> Option<usize> {
        match self.mode {
            Selection::One => self.selected.first().copied(),
            Selection::Any => None,
        }
    }

    fn index_at_x(&self, x: f32) -> Option<usize> {
        if self.items.is_empty() {
            return None;
        }
        let mut best = 0;
        let mut distance = f32::INFINITY;
        for (i, r) in self.items.iter().enumerate() {
            if x >= r.x && x < r.right() {
                return Some(i);
            }
            let d = (x - r.x).abs().min((x - r.right()).abs());
            if d < distance {
                distance = d;
                best = i;
            }
        }
        Some(best)
    }

    fn index_at(&self, p: Point) -> Option<usize> {
        let h = self.metrics().height;
        if p.y < 0.0 || p.y > h || p.x < 0.0 || p.x > self.width {
            return None;
        }
        self.index_at_x(p.x)
    }

    fn payload(&self) -> Value {
        match self.mode {
            Selection::One => self
                .selected
                .first()
                .map_or(Value::None, |i| Value::Int(*i as i64)),
            Selection::Any => Value::IntList(self.selected.iter().map(|i| *i as i64).collect()),
        }
    }

    fn choose(&mut self, cx: &mut EventCx<'_>, i: usize) {
        if !self.can_choose(i, !is_disabled(cx)) {
            return;
        }
        match self.mode {
            Selection::One => {
                if self.selected == [i] {
                    return;
                }
                let from = self.thumb.get();
                self.selected = vec![i];
                self.focus_index = i;
                if let Some(to) = self.items.get(i).copied() {
                    if cx.theme().appearance.reduce_motion || from == Rect::ZERO {
                        self.thumb.jump(to);
                    } else {
                        self.thumb
                            .animate_to(to, Timing::Spring(tokens::motion::SPRING_SNAPPY));
                    }
                }
            }
            Selection::Any => {
                if let Some(at) = self.selected.iter().position(|s| *s == i) {
                    self.selected.remove(at);
                } else {
                    self.selected.push(i);
                    self.selected.sort_unstable();
                }
            }
        }
        let payload = self.payload();
        cx.emit(Signal::ValueChanged, payload.clone());
        cx.emit(Signal::SelectionChanged, payload);
        cx.request_paint();
        cx.animate();
    }

    fn spring(cx: &EventCx<'_>) -> Timing {
        if cx.theme().appearance.reduce_motion {
            // The short fade, no overshoot (ui-kit: 150 ms `cubic-bezier(0.2, 0, 0, 1)`).
            Timing::curve(tokens::motion::FADE_QUICK_CURVE, tokens::motion::FADE_QUICK)
        } else {
            Timing::Spring(tokens::motion::SPRING_SNAPPY)
        }
    }

    /// The item swells into the lens (or the lens moves to it): the thumb's rect and the
    /// morph both animate with `spring.snappy`.
    fn lens_to(&mut self, cx: &mut EventCx<'_>, i: usize) {
        let Some(item) = self.items.get(i).copied() else {
            return;
        };
        let spring = Self::spring(cx);
        if self.hot.is_none() && self.thumb.get() == Rect::ZERO {
            self.thumb.jump(item);
        }
        self.hot = Some(i);
        self.thumb.animate_to(lens_rect(item), spring);
        self.lens.animate_to(1.0, spring);
        cx.request_paint();
        cx.animate();
    }

    /// The lens turns back into the thumb under the selected item.
    fn lens_off(&mut self, cx: &mut EventCx<'_>) {
        let spring = Self::spring(cx);
        self.hot = None;
        let to = self
            .single()
            .and_then(|i| self.items.get(i))
            .copied()
            .unwrap_or(Rect::ZERO);
        if to == Rect::ZERO {
            self.thumb.jump(to);
        } else {
            self.thumb.animate_to(to, spring);
        }
        self.lens.animate_to(0.0, spring);
        cx.request_paint();
        cx.animate();
    }

    /// The enabled item nearest to `x` (ui-kit `move`: the distance to the item's span).
    fn nearest_enabled(&self, x: f32, enabled: bool) -> Option<usize> {
        let mut best = None;
        let mut dist = f32::INFINITY;
        for (i, r) in self.items.iter().enumerate() {
            if !self.can_choose(i, enabled) {
                continue;
            }
            let d = if x < r.x {
                r.x - x
            } else if x > r.right() {
                x - r.right()
            } else {
                0.0
            };
            if d < dist {
                dist = d;
                best = Some(i);
            }
        }
        best
    }

    fn step(&mut self, cx: &mut EventCx<'_>, step: isize) {
        let n = self.segments.len() as isize;
        let enabled = !is_disabled(cx);
        // The focus wraps round the ends (ui-kit `onKey`), skipping disabled items.
        for k in 1..=n {
            let idx = (self.focus_index as isize + step * k).rem_euclid(n.max(1)) as usize;
            if self.can_choose(idx, enabled) {
                self.focus_index = idx;
                if self.mode == Selection::One {
                    self.choose(cx, idx);
                }
                cx.request_paint();
                return;
            }
        }
    }
}

impl Widget for SegmentedControl {
    fn kind(&self) -> &'static str {
        "SegmentedControl"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        let theme = cx.theme();
        self.compute(&theme);
        Size::new(self.width, self.metrics().height)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        self.compute(&theme);
        let m = self.metrics();
        let b = cx.bounds();
        let o = Point::new(b.x, b.y);
        let shift = |r: Rect| r.translate(o);
        let h = m.height;
        let dim = !cx.is_disabled();
        cx.rec.layer(
            "Track",
            Rect::new(b.x, b.y, self.width, h),
            Shape::rounded(h / 2.0),
            theme.material_style("Glass/Regular"),
        );
        let one = self.mode == Selection::One;
        let item_radius = m.item_height() / 2.0;
        // The selection of select one: a slide, or the chosen item's own background. While an
        // item is held the thumb is the lens: one element, morphing (ui-kit `.s-segmented__thumb`
        // and `.is-lens`), always below the items (Track, Lens, Segments from bottom to top).
        let lens_t = self.lens.get().clamp(0.0, 1.0);
        let morphing = one && (self.hot.is_some() || lens_t > 0.0);
        let sliding = one && (self.thumb.is_animating() || morphing);
        let thumb_rect =
            (sliding && self.thumb.get() != Rect::ZERO).then(|| shift(self.thumb.get()));
        if let Some(rect) = thumb_rect {
            paint_thumb(cx, &theme, rect, m.item_height(), lens_t);
        }
        for (i, seg) in self.segments.iter().enumerate() {
            let Some(item) = self.items.get(i).copied() else {
                continue;
            };
            let chosen = self.selected.contains(&i);
            let look = if one {
                if chosen && !sliding {
                    ItemLook::Selected
                } else if self.hovered == Some(i) && !self.down && !chosen {
                    ItemLook::Hover
                } else {
                    ItemLook::Rest
                }
            } else {
                let down = self.pressed_item == Some(i) && self.pressed_inside;
                match (chosen, down) {
                    (true, true) => ItemLook::SelectedPressed,
                    (true, false) => ItemLook::Selected,
                    (false, true) => ItemLook::Pressed,
                    (false, false) if self.hovered == Some(i) && !self.down => ItemLook::Hover,
                    _ => ItemLook::Rest,
                }
            };
            let dimmed = dim && !seg.enabled;
            if dimmed {
                cx.rec.push_opacity(theme.disabled_opacity());
            }
            let prefix = format!("Segment {}/", i + 1);
            let rect = shift(item);
            cx.rec.layer(
                &format!("{prefix}Background"),
                rect,
                Shape::rounded(item_radius),
                look.style(&theme),
            );
            let color = theme.colors.text_primary;
            match &seg.content {
                Content::Text(t) => {
                    let cache = self.labels.get(i);
                    paint_label(
                        cx,
                        cache,
                        &format!("{prefix}Label"),
                        rect,
                        t,
                        m.item_token(),
                        color,
                    );
                }
                Content::Icon(symbol) => {
                    let g = m.glyph();
                    let r = Rect::new(
                        rect.x + (rect.width - g) / 2.0,
                        rect.y + (rect.height - g) / 2.0,
                        g,
                        g,
                    );
                    cx.rec
                        .glyph(&format!("{prefix}Glyph Sidebar"), r, symbol, 400, color);
                }
            }
            if dimmed {
                cx.rec.pop();
            }
        }
        if cx.focus_visible()
            && !cx.is_disabled()
            && let Some(item) = self.items.get(self.focus_index)
        {
            focus_halo_named(
                cx.rec,
                "Focus Halo",
                shift(*item),
                Shape::rounded(item_radius),
                &theme,
            );
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let theme = cx.theme();
        let origin = cx.bounds().origin();
        let enabled = !is_disabled(cx);
        match event {
            Event::PointerMove {
                pos,
                dragging: false,
                ..
            } => {
                self.compute(&theme);
                let local = Point::new(pos.x - origin.x, pos.y - origin.y);
                let i = self.index_at(local);
                let h = i.filter(|i| self.can_choose(*i, enabled));
                if h != self.hovered {
                    self.hovered = h;
                    cx.request_paint();
                }
                Handled::No
            }
            Event::PointerLeave => {
                if self.hovered.take().is_some() {
                    cx.request_paint();
                }
                Handled::No
            }
            Event::PointerDown {
                pos,
                button: Button::Left,
                ..
            } => {
                self.compute(&theme);
                if self.down {
                    return Handled::No;
                }
                let local = Point::new(pos.x - origin.x, pos.y - origin.y);
                let Some(i) = self
                    .index_at(local)
                    .filter(|i| self.can_choose(*i, enabled))
                else {
                    return Handled::No;
                };
                self.down = true;
                cx.grow(PRESS_SCALE_SEGMENTED);
                self.focus_index = i;
                if self.mode == Selection::Any {
                    // Select any has no lens: a flat pressed item, toggled on release.
                    self.pressed_item = Some(i);
                    self.pressed_inside = true;
                    cx.request_paint();
                    return Handled::Yes;
                }
                self.lens_to(cx, i);
                Handled::Yes
            }
            Event::PointerMove {
                pos,
                dragging: true,
                ..
            } if self.down => {
                let local = Point::new(pos.x - origin.x, pos.y - origin.y);
                if let Some(p) = self.pressed_item {
                    let inside = self.index_at(local) == Some(p);
                    if inside != self.pressed_inside {
                        self.pressed_inside = inside;
                        cx.request_paint();
                    }
                } else if self.mode == Selection::One {
                    // The lens follows the pointer from item to item.
                    if let Some(n) = self.nearest_enabled(local.x, enabled)
                        && self.hot != Some(n)
                    {
                        self.focus_index = n;
                        self.lens_to(cx, n);
                    }
                }
                Handled::Yes
            }
            Event::PointerUp {
                pos,
                button: Button::Left,
                ..
            } if self.down => {
                self.down = false;
                cx.grow(1.0);
                let local = Point::new(pos.x - origin.x, pos.y - origin.y);
                if let Some(p) = self.pressed_item.take() {
                    if self.index_at(local) == Some(p) {
                        self.choose(cx, p);
                    }
                    cx.request_paint();
                    return Handled::Yes;
                }
                // Released anywhere (the pointer is captured): the item under the lens is chosen.
                if let Some(t) = self.hot.filter(|t| self.can_choose(*t, enabled)) {
                    self.choose(cx, t);
                }
                self.lens_off(cx);
                Handled::Yes
            }
            Event::Key(k) if k.pressed => {
                let step = match k.key {
                    Key::Left | Key::Up => -1,
                    Key::Right | Key::Down => 1,
                    Key::Space | Key::Enter => {
                        let i = self.focus_index;
                        self.choose(cx, i);
                        return Handled::Yes;
                    }
                    _ => return Handled::No,
                };
                self.step(cx, step);
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn tick(&mut self, _cx: &mut EventCx<'_>, dt: f32) -> bool {
        let a = self.thumb.tick(dt);
        let b = self.lens.tick(dt);
        a || b
    }

    fn state_changed(&mut self, cx: &mut EventCx<'_>, old: NodeFlags, new: NodeFlags) {
        // `Prop::Pressed` holds the control down without a pointer: the second item (the
        // first when there is one) is the pressed one, as in the ui-kit showcase.
        let held = new.contains(NodeFlags::PRESSED) && !self.down;
        if held && !old.contains(NodeFlags::PRESSED) {
            self.forced = true;
            self.compute(&cx.theme());
            cx.grow(PRESS_SCALE_SEGMENTED);
            if self.mode == Selection::One {
                let i = usize::from(self.items.len() > 1);
                self.lens_to(cx, i);
            }
        } else if self.forced && !new.contains(NodeFlags::PRESSED) {
            self.forced = false;
            cx.grow(1.0);
            self.lens_off(cx);
        }
    }

    fn interaction(&self) -> Interaction {
        Interaction::Hover
    }

    fn focus(&self) -> FocusKind {
        FocusKind::Control
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::Items => match value.as_str_list() {
                Some(items) => {
                    let icons: Vec<String> = self
                        .segments
                        .iter()
                        .map(|s| match &s.content {
                            Content::Icon(i) => i.clone(),
                            Content::Text(_) => String::new(),
                        })
                        .collect();
                    self.segments = items
                        .iter()
                        .enumerate()
                        .map(|(i, t)| match icons.get(i).filter(|s| !s.is_empty()) {
                            Some(sym) => Segment::icon(sym, t),
                            None => Segment::text(t),
                        })
                        .collect();
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::ItemSymbols => match value.as_str_list() {
                Some(symbols) => {
                    for (s, sym) in self.segments.iter_mut().zip(symbols) {
                        if !sym.is_empty() {
                            let label = s.a11y_label();
                            *s = Segment::icon(sym, &label);
                        }
                    }
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::SelectedIndex => match value.as_int() {
                Some(i) => {
                    self.selected = usize::try_from(i).map(|i| vec![i]).unwrap_or_default();
                    self.thumb.jump(Rect::ZERO);
                    PropChange::Paint
                }
                None => PropChange::Unknown,
            },
            // An integer list, or (through the ABI) the indices as a list of strings.
            Prop::SelectedRows => {
                let list: Option<Vec<usize>> = match value {
                    Value::IntList(l) => {
                        Some(l.iter().filter_map(|i| usize::try_from(*i).ok()).collect())
                    }
                    other => other
                        .as_str_list()
                        .map(|l| l.iter().filter_map(|s| s.parse().ok()).collect()),
                };
                match list {
                    Some(mut l) => {
                        l.sort_unstable();
                        l.dedup();
                        self.selected = l;
                        PropChange::Paint
                    }
                    None => PropChange::Unknown,
                }
            }
            Prop::ControlSize => match value.as_int().and_then(ControlSize::from_int) {
                Some(s) => {
                    self.size = s;
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::Style => match value.as_int() {
                Some(0) => {
                    self.mode = Selection::One;
                    PropChange::Paint
                }
                Some(1) => {
                    self.mode = Selection::Any;
                    PropChange::Paint
                }
                _ => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Items => Some(Value::StrList(
                self.segments.iter().map(Segment::a11y_label).collect(),
            )),
            Prop::SelectedIndex => Some(
                self.selected
                    .first()
                    .map_or(Value::Int(-1), |i| Value::Int(*i as i64)),
            ),
            Prop::SelectedRows => Some(Value::IntList(
                self.selected.iter().map(|i| *i as i64).collect(),
            )),
            Prop::ControlSize => Some(Value::Int(self.size.to_int())),
            Prop::Style => Some(Value::Int(match self.mode {
                Selection::One => 0,
                Selection::Any => 1,
            })),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::SegmentedControl;
        a.value = Some(
            self.selected
                .iter()
                .filter_map(|i| self.segments.get(*i))
                .map(Segment::a11y_label)
                .collect::<Vec<_>>()
                .join(", "),
        );
        a.actions = vec![Action::Click, Action::Focus];
    }
}

// ---------------------------------------------------------------------------------------
// Action segments.
// ---------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Look {
    #[default]
    None,
    Hover,
    Pressed,
    Selected,
}

/// A capsule of buttons (Back / Forward, Reply / Reply All / Forward, text alignment): a
/// `Glass/Regular` track split by separators. A segment fires on release over it
/// (`Signal::Activated`, payload: its index); the selected ones (the Align group) keep a
/// highlight and an accent glyph.
#[derive(Debug)]
pub struct ActionSegments {
    /// The segments.
    pub segments: Vec<Segment>,
    /// Selected segments.
    pub selected: Vec<usize>,
    /// Size (a toolbar's Large is 36 tall).
    pub size: ControlSize,
    /// Whether the window is active (`None`: ask the window).
    pub active: Option<bool>,
    labels: Labels,
    lefts: Vec<f32>,
    widths: Vec<f32>,
    width: f32,
    highlight: Vec<Animated<f32>>,
    separator: Vec<Animated<f32>>,
    last_look: Vec<Look>,
    hovered: Option<usize>,
    pressed: Option<usize>,
    inside: bool,
    focus_index: usize,
}

impl ActionSegments {
    /// A capsule of `segments`, Large (the toolbar size).
    pub fn new(segments: Vec<Segment>) -> ActionSegments {
        ActionSegments {
            segments,
            selected: Vec::new(),
            size: ControlSize::Large,
            active: None,
            labels: Labels::default(),
            lefts: Vec::new(),
            widths: Vec::new(),
            width: 0.0,
            highlight: Vec::new(),
            separator: Vec::new(),
            last_look: Vec::new(),
            hovered: None,
            pressed: None,
            inside: false,
            focus_index: 0,
        }
    }

    /// With a size.
    pub fn size(mut self, size: ControlSize) -> ActionSegments {
        self.size = size;
        self
    }

    /// With selected segments.
    pub fn selected(mut self, selected: &[usize]) -> ActionSegments {
        self.selected = selected.to_vec();
        self
    }

    /// With the window's active state fixed.
    pub fn active(mut self, active: bool) -> ActionSegments {
        self.active = Some(active);
        self
    }

    fn metrics(&self) -> SizeMetrics {
        SizeMetrics::of(self.size, true)
    }

    fn can_press(&self, i: usize, enabled: bool) -> bool {
        enabled && self.segments.get(i).is_some_and(|s| s.enabled)
    }

    fn compute(&mut self, theme: &Theme) {
        let m = self.metrics();
        let token = m.action_token();
        let mut text_w = m.text_segment_width;
        for i in 0..self.segments.len() {
            if let Some(Content::Text(t)) = self.segments.get(i).map(|s| s.content.clone()) {
                text_w = text_w.max(text_width(self.labels.get(i), theme, token, &t));
            }
        }
        self.lefts.clear();
        self.widths.clear();
        let mut x = 0.0;
        for (i, s) in self.segments.iter().enumerate() {
            let w = if s.is_icon() { m.height } else { text_w };
            // Large icon segments are 1 apart: the separator has a place of its own.
            if i > 0 && s.is_icon() && m.separator_takes_space() {
                x += metrics::SEPARATOR_WIDTH;
            }
            self.lefts.push(x);
            self.widths.push(w);
            x += w;
        }
        self.width = x;
        let n = self.segments.len();
        self.highlight.resize_with(n, || Animated::new(0.0));
        self.separator.resize_with(n, || Animated::new(1.0));
        self.last_look.resize(n, Look::Hover);
        // At rest the fades sit on their targets (a selected segment starts highlighted).
        let looks: Vec<Look> = (0..n).map(|i| self.look(i)).collect();
        for (i, look) in looks.iter().enumerate() {
            if *look != Look::None
                && let Some(l) = self.last_look.get_mut(i)
            {
                *l = *look;
            }
            if let Some(a) = self.highlight.get_mut(i)
                && !a.is_animating()
            {
                a.jump(if *look == Look::None { 0.0 } else { 1.0 });
            }
            if i > 0
                && let Some(a) = self.separator.get_mut(i)
                && !a.is_animating()
            {
                let shown = looks.get(i - 1) == Some(&Look::None) && *look == Look::None;
                a.jump(if shown { 1.0 } else { 0.0 });
            }
        }
    }

    fn index_at(&self, p: Point) -> Option<usize> {
        if p.y < 0.0 || p.y > self.metrics().height {
            return None;
        }
        self.lefts
            .iter()
            .zip(&self.widths)
            .position(|(l, w)| p.x >= *l && p.x < *l + *w)
    }

    fn look(&self, i: usize) -> Look {
        if self.pressed == Some(i) && self.inside {
            Look::Pressed
        } else if self.selected.contains(&i) {
            Look::Selected
        } else if self.hovered == Some(i) && self.pressed.is_none() {
            Look::Hover
        } else {
            Look::None
        }
    }

    fn highlight_rect(&self, i: usize, look: Look) -> Rect {
        let m = self.metrics();
        let x = self.lefts.get(i).copied().unwrap_or(0.0);
        let w = self.widths.get(i).copied().unwrap_or(0.0);
        let h = m.height;
        // Pressed keeps the margins of hover (ui-kit 03e08f8).
        let _ = look;
        let inset = m.hover_inset;
        if self.segments.get(i).is_some_and(Segment::is_icon) {
            return Rect::new(x + inset, inset, w - 2.0 * inset, h - 2.0 * inset);
        }
        let side = metrics::TEXT_HIGHLIGHT_INSET;
        Rect::new(x + side, inset, w - 2.0 * side, h - 2.0 * inset)
    }

    /// Moves the fades toward the current looks.
    fn sync(&mut self, cx: &mut EventCx<'_>) {
        let reduce = cx.theme().appearance.reduce_motion;
        let n = self.segments.len();
        let looks: Vec<Look> = (0..n).map(|i| self.look(i)).collect();
        let timing = |shown: bool| {
            if reduce {
                Timing::None
            } else {
                Timing::curve(
                    tokens::motion::FADE_QUICK_CURVE,
                    if shown {
                        tokens::motion::FADE_QUICK
                    } else {
                        HIGHLIGHT_OUT
                    },
                )
            }
        };
        let mut busy = false;
        for (i, look) in looks.iter().enumerate() {
            let shown = *look != Look::None;
            if shown && let Some(l) = self.last_look.get_mut(i) {
                *l = *look;
            }
            if let Some(a) = self.highlight.get_mut(i) {
                let t = if shown { 1.0 } else { 0.0 };
                if a.target() != t {
                    a.animate_to(t, timing(shown));
                }
                busy |= a.is_animating();
            }
            if i > 0 {
                let sep_shown = looks.get(i - 1) == Some(&Look::None) && *look == Look::None;
                if let Some(a) = self.separator.get_mut(i) {
                    let t = if sep_shown { 1.0 } else { 0.0 };
                    if a.target() != t {
                        a.animate_to(t, timing(sep_shown));
                    }
                    busy |= a.is_animating();
                }
            }
        }
        cx.request_paint();
        if busy {
            cx.animate();
        }
    }

    fn fire(&mut self, cx: &mut EventCx<'_>, i: usize) {
        if self.can_press(i, !is_disabled(cx)) {
            cx.emit(Signal::Activated, Value::Int(i as i64));
        }
    }
}

impl Widget for ActionSegments {
    fn kind(&self) -> &'static str {
        "ActionSegments"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        let theme = cx.theme();
        self.compute(&theme);
        Size::new(self.width, self.metrics().height)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        self.compute(&theme);
        let c = theme.colors;
        let m = self.metrics();
        let b = cx.bounds();
        let h = m.height;
        let active = self.active.unwrap_or_else(|| cx.window_active());
        cx.rec.layer(
            "Glass",
            Rect::new(b.x, b.y, self.width, h),
            Shape::rounded(h / 2.0),
            theme.material_style("Glass/Regular"),
        );
        let n = self.segments.len();
        for i in 0..n {
            let op = self.highlight.get(i).map_or(0.0, Animated::get);
            if op <= 0.0 {
                continue;
            }
            let look = self.last_look.get(i).copied().unwrap_or(Look::Hover);
            let rect = self.highlight_rect(i, look).translate(Point::new(b.x, b.y));
            let color = if look == Look::Pressed {
                c.line_border
            } else {
                c.line_separator
            };
            let name = if look == Look::Selected {
                "Selected highlight"
            } else {
                "Highlight"
            };
            cx.rec.layer(
                name,
                rect,
                Shape::rounded(rect.height / 2.0),
                Style::fill(color).with_opacity(op),
            );
        }
        for i in 1..n {
            let op = self.separator.get(i).map_or(0.0, Animated::get);
            if op <= 0.0 {
                continue;
            }
            let left = self.lefts.get(i).copied().unwrap_or(0.0)
                - if self.segments.get(i).is_some_and(Segment::is_icon) && m.separator_takes_space()
                {
                    metrics::SEPARATOR_WIDTH
                } else {
                    0.0
                };
            cx.rec.layer(
                &format!("Separator {i}"),
                Rect::new(
                    b.x + left,
                    b.y + (h - m.separator_height) / 2.0,
                    metrics::SEPARATOR_WIDTH,
                    m.separator_height,
                ),
                Shape::RECT,
                Style::fill(c.line_separator).with_opacity(op),
            );
        }
        let enabled = !cx.is_disabled();
        for i in 0..n {
            let Some(seg) = self.segments.get(i) else {
                continue;
            };
            let x = b.x + self.lefts.get(i).copied().unwrap_or(0.0);
            let w = self.widths.get(i).copied().unwrap_or(0.0);
            let color = if !active {
                c.text_tertiary
            } else if self.selected.contains(&i) {
                c.accent_amalfi
            } else {
                c.text_primary
            };
            let dim = enabled && !seg.enabled;
            if dim {
                cx.rec.push_opacity(theme.disabled_opacity());
            }
            match &seg.content {
                Content::Icon(symbol) => {
                    let g = m.glyph();
                    cx.rec.glyph(
                        &format!("Glyph {}", i + 1),
                        Rect::new(x + (w - g) / 2.0, b.y + (h - g) / 2.0, g, g),
                        symbol,
                        400,
                        color,
                    );
                }
                Content::Text(t) => {
                    let cache = self.labels.get(i);
                    paint_label(
                        cx,
                        cache,
                        &format!("Label {}", i + 1),
                        Rect::new(x, b.y, w, h),
                        t,
                        m.action_token(),
                        color,
                    );
                }
            }
            if dim {
                cx.rec.pop();
            }
        }
        if cx.focus_visible() && !cx.is_disabled() && self.focus_index < n {
            let rect = self
                .highlight_rect(self.focus_index, Look::Hover)
                .translate(Point::new(b.x, b.y));
            focus_halo_named(
                cx.rec,
                "Focus Halo",
                rect,
                Shape::rounded(rect.height / 2.0),
                &theme,
            );
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let theme = cx.theme();
        let origin = cx.bounds().origin();
        let enabled = !is_disabled(cx);
        let local = |p: &Point| Point::new(p.x - origin.x, p.y - origin.y);
        match event {
            Event::PointerMove {
                pos,
                dragging: false,
                ..
            } => {
                self.compute(&theme);
                let i = self.index_at(local(pos));
                let h = i.filter(|i| self.can_press(*i, enabled));
                if h != self.hovered {
                    self.hovered = h;
                    self.sync(cx);
                }
                Handled::No
            }
            Event::PointerLeave => {
                if self.hovered.take().is_some() {
                    self.sync(cx);
                }
                Handled::No
            }
            Event::PointerDown {
                pos,
                button: Button::Left,
                ..
            } => {
                self.compute(&theme);
                let Some(i) = self
                    .index_at(local(pos))
                    .filter(|i| self.can_press(*i, enabled))
                else {
                    return Handled::No;
                };
                self.pressed = Some(i);
                cx.grow(PRESS_SCALE_SEGMENTED);
                self.inside = true;
                self.focus_index = i;
                self.sync(cx);
                Handled::Yes
            }
            Event::PointerMove {
                pos,
                dragging: true,
                ..
            } if self.pressed.is_some() => {
                let inside = self.index_at(local(pos)) == self.pressed;
                if inside != self.inside {
                    self.inside = inside;
                    self.sync(cx);
                }
                Handled::Yes
            }
            Event::PointerUp {
                pos,
                button: Button::Left,
                ..
            } if self.pressed.is_some() => {
                let pressed = self.pressed.take();
                cx.grow(1.0);
                let at = self.index_at(local(pos));
                self.inside = false;
                self.hovered = at.filter(|i| self.can_press(*i, enabled));
                self.sync(cx);
                if let Some(p) = pressed
                    && at == Some(p)
                {
                    self.fire(cx, p);
                }
                Handled::Yes
            }
            Event::Key(k) if k.pressed => {
                let step: isize = match k.key {
                    Key::Left | Key::Up => -1,
                    Key::Right | Key::Down => 1,
                    Key::Space | Key::Enter => {
                        let i = self.focus_index;
                        self.fire(cx, i);
                        return Handled::Yes;
                    }
                    _ => return Handled::No,
                };
                let n = self.segments.len() as isize;
                for k in 1..=n {
                    let idx = (self.focus_index as isize + step * k).rem_euclid(n.max(1)) as usize;
                    if self.can_press(idx, enabled) {
                        self.focus_index = idx;
                        cx.request_paint();
                        break;
                    }
                }
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn tick(&mut self, _cx: &mut EventCx<'_>, dt: f32) -> bool {
        let mut busy = false;
        for a in self.highlight.iter_mut().chain(self.separator.iter_mut()) {
            busy |= a.tick(dt);
        }
        busy
    }

    fn interaction(&self) -> Interaction {
        Interaction::Hover
    }

    fn focus(&self) -> FocusKind {
        FocusKind::Control
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::SelectedRows => match value.as_int_list() {
                Some(l) => {
                    self.selected = l.iter().filter_map(|i| usize::try_from(*i).ok()).collect();
                    PropChange::Paint
                }
                None => PropChange::Unknown,
            },
            Prop::ControlSize => match value.as_int().and_then(ControlSize::from_int) {
                Some(s) => {
                    self.size = s;
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            // Through the ABI: icon segments from symbols (their accessible labels from
            // `Items`), or text segments from `Items` alone.
            Prop::ItemSymbols => match value.as_str_list() {
                Some(l) => {
                    let labels: Vec<String> = self
                        .segments
                        .iter()
                        .map(|s| s.label.clone().unwrap_or_default())
                        .collect();
                    self.segments = l
                        .iter()
                        .enumerate()
                        .map(|(i, sym)| {
                            Segment::icon(sym, labels.get(i).map_or("", String::as_str))
                        })
                        .collect();
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::Items => match value.as_str_list() {
                Some(l) => {
                    let icons = self
                        .segments
                        .iter()
                        .any(|s| matches!(s.content, Content::Icon(_)));
                    if icons {
                        for (s, label) in self.segments.iter_mut().zip(l) {
                            s.label = Some(label.clone());
                        }
                    } else {
                        self.segments = l.iter().map(|t| Segment::text(t)).collect();
                    }
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::SelectedRows => Some(Value::IntList(
                self.selected.iter().map(|i| *i as i64).collect(),
            )),
            Prop::ControlSize => Some(Value::Int(self.size.to_int())),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Toolbar;
        a.actions = vec![Action::Click, Action::Focus];
    }
}

// ---------------------------------------------------------------------------------------
// The Icon + Label control (window panes).
// ---------------------------------------------------------------------------------------

/// A pane of the Icon + Label control.
#[derive(Debug, Clone, PartialEq)]
pub struct Pane {
    /// The label under the glyph.
    pub label: String,
    /// The symbol.
    pub symbol: String,
    /// Whether it can be chosen.
    pub enabled: bool,
}

impl Pane {
    /// An enabled pane.
    pub fn new(label: &str, symbol: &str) -> Pane {
        Pane {
            label: label.to_string(),
            symbol: symbol.to_string(),
            enabled: true,
        }
    }
}

/// The segmented control for switching the panes of a window (settings windows always use
/// it): a `Glass/Regular` capsule 52 tall of equal panes, each a 20 pt glyph over a Caption
/// label. As in every select-one segmented control (ui-kit `SSegmented`, stacked) the thumb
/// slides to the chosen pane, and while held it swells into the glass lens, which follows the
/// pointer from pane to pane; the pane under it when released is chosen.
#[derive(Debug)]
pub struct PaneControl {
    /// The panes.
    pub panes: Vec<Pane>,
    /// The chosen pane.
    pub selected: usize,
    /// Pane width (`None`: the widest label plus 24, on the 8 pt grid, at least 72).
    pub pane_width: Option<f32>,
    labels: Labels,
    pw: f32,
    hovered: Option<usize>,
    /// Held down by the pointer.
    down: bool,
    thumb: Animated<Rect>,
    /// 0 the thumb, 1 the glass lens (`spring.snappy`).
    lens: Animated<f32>,
    /// The pane under the lens.
    hot: Option<usize>,
}

impl PaneControl {
    /// A control of `panes` with the first chosen.
    pub fn new(panes: Vec<Pane>) -> PaneControl {
        PaneControl {
            panes,
            selected: 0,
            pane_width: None,
            labels: Labels::default(),
            pw: metrics::MIN_PANE_WIDTH,
            hovered: None,
            down: false,
            thumb: Animated::new(Rect::ZERO),
            lens: Animated::new(0.0),
            hot: None,
        }
    }

    /// With a chosen pane.
    pub fn selected(mut self, i: usize) -> PaneControl {
        self.selected = i;
        self
    }

    /// With a fixed pane width.
    pub fn pane_width(mut self, w: f32) -> PaneControl {
        self.pane_width = Some(w);
        self
    }

    fn compute(&mut self, theme: &Theme) {
        self.pw = match self.pane_width {
            Some(w) => w,
            None => {
                let mut widest = 0.0f32;
                for i in 0..self.panes.len() {
                    let Some(l) = self.panes.get(i).map(|p| p.label.clone()) else {
                        continue;
                    };
                    let style = theme.text(&typography::CAPTION, theme.colors.text_primary);
                    widest = widest.max(self.labels.get(i).width(&l, &style).ceil());
                }
                whole(widest + 24.0, 8.0).max(metrics::MIN_PANE_WIDTH)
            }
        };
        if !self.thumb.is_animating() && self.hot.is_none() && self.lens.get() <= 0.0 {
            let r = self.item(self.selected).unwrap_or(Rect::ZERO);
            self.thumb.jump(r);
        }
    }

    /// The box of pane `i`, from the control's origin.
    fn item(&self, i: usize) -> Option<Rect> {
        (i < self.panes.len()).then(|| {
            Rect::new(
                metrics::TRACK_PADDING + i as f32 * self.pw,
                metrics::TRACK_PADDING,
                self.pw,
                metrics::PANE_HEIGHT,
            )
        })
    }

    /// The pane swells into the lens, or the lens moves to it.
    fn lens_to(&mut self, cx: &mut EventCx<'_>, i: usize) {
        let Some(item) = self.item(i) else {
            return;
        };
        let spring = SegmentedControl::spring(cx);
        if self.hot.is_none() && self.thumb.get() == Rect::ZERO {
            self.thumb.jump(item);
        }
        self.hot = Some(i);
        self.thumb.animate_to(lens_rect(item), spring);
        self.lens.animate_to(1.0, spring);
        cx.request_paint();
        cx.animate();
    }

    /// The lens turns back into the thumb under the chosen pane.
    fn lens_off(&mut self, cx: &mut EventCx<'_>) {
        let spring = SegmentedControl::spring(cx);
        self.hot = None;
        match self.item(self.selected) {
            Some(to) => self.thumb.animate_to(to, spring),
            None => self.thumb.jump(Rect::ZERO),
        }
        self.lens.animate_to(0.0, spring);
        cx.request_paint();
        cx.animate();
    }

    fn width(&self) -> f32 {
        self.panes.len() as f32 * self.pw + 2.0 * metrics::TRACK_PADDING
    }

    fn can(&self, i: usize, enabled: bool) -> bool {
        enabled && self.panes.get(i).is_some_and(|p| p.enabled)
    }

    fn index_at(&self, x: f32) -> usize {
        let i = ((x - metrics::TRACK_PADDING) / self.pw).floor();
        let n = self.panes.len().saturating_sub(1);
        if i.is_finite() && i > 0.0 {
            (i as usize).min(n)
        } else {
            0
        }
    }

    fn choose(&mut self, cx: &mut EventCx<'_>, i: usize) {
        if i == self.selected || !self.can(i, !is_disabled(cx)) {
            return;
        }
        let from = self.thumb.get();
        self.selected = i;
        if let Some(to) = self.item(i) {
            if cx.theme().appearance.reduce_motion || from == Rect::ZERO {
                self.thumb.jump(to);
            } else {
                self.thumb
                    .animate_to(to, Timing::Spring(tokens::motion::SPRING_SNAPPY));
            }
        }
        cx.emit(Signal::ValueChanged, Value::Int(i as i64));
        cx.emit(Signal::SelectionChanged, Value::Int(i as i64));
        cx.request_paint();
        cx.animate();
    }
}

impl Widget for PaneControl {
    fn kind(&self) -> &'static str {
        "PaneControl"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        let theme = cx.theme();
        self.compute(&theme);
        Size::new(self.width(), metrics::PANE_BAR_HEIGHT)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        self.compute(&theme);
        let c = theme.colors;
        let b = cx.bounds();
        cx.rec.layer(
            "Track",
            Rect::new(b.x, b.y, self.width(), metrics::PANE_BAR_HEIGHT),
            Shape::rounded(metrics::PANE_BAR_HEIGHT / 2.0),
            theme.material_style("Glass/Regular"),
        );
        let enabled = !cx.is_disabled();
        let selected = self.selected.min(self.panes.len().saturating_sub(1));
        // The thumb slides, or is the lens, below the panes; at rest the chosen pane carries
        // the thumb's look itself.
        let lens_t = self.lens.get().clamp(0.0, 1.0);
        let sliding = self.thumb.is_animating() || self.hot.is_some() || lens_t > 0.0;
        if sliding && self.thumb.get() != Rect::ZERO {
            let rect = self.thumb.get().translate(Point::new(b.x, b.y));
            paint_thumb(cx, &theme, rect, metrics::PANE_HEIGHT, lens_t);
        }
        for (i, pane) in self.panes.iter().enumerate() {
            let chosen = i == selected;
            let on = pane.enabled;
            let look = if chosen && !sliding {
                ItemLook::Selected
            } else if self.hovered == Some(i) && on && enabled && !self.down && !chosen {
                ItemLook::Hover
            } else {
                ItemLook::Rest
            };
            let x = b.x + metrics::TRACK_PADDING + i as f32 * self.pw;
            let y = b.y + metrics::TRACK_PADDING;
            let prefix = format!("Pane {}/", i + 1);
            if !on {
                cx.rec.push_opacity(theme.disabled_opacity());
            }
            cx.rec.layer(
                &format!("{prefix}Background"),
                Rect::new(x, y, self.pw, metrics::PANE_HEIGHT),
                Shape::rounded(metrics::PANE_HEIGHT / 2.0),
                look.style(&theme),
            );
            let g = metrics::PANE_GLYPH;
            cx.rec.glyph_sized(
                &format!("{prefix}Glyph"),
                Rect::new(x + (self.pw - g) / 2.0, y + 4.0, g, g + 1.0),
                g,
                &pane.symbol,
                400,
                if chosen {
                    c.accent_amalfi
                } else {
                    c.text_primary
                },
            );
            let label = pane.label.clone();
            paint_label_top(
                cx,
                self.labels.get(i),
                &format!("{prefix}Label"),
                Rect::new(x, y + metrics::PANE_LABEL_TOP, self.pw, 14.0),
                &label,
                &typography::CAPTION,
                c.text_primary,
            );
            if !on {
                cx.rec.pop();
            }
        }
        if cx.focus_visible() && !cx.is_disabled() && !self.panes.is_empty() {
            focus_halo_named(
                cx.rec,
                "Focus Halo",
                Rect::new(
                    b.x + metrics::TRACK_PADDING + selected as f32 * self.pw,
                    b.y + metrics::TRACK_PADDING,
                    self.pw,
                    metrics::PANE_HEIGHT,
                ),
                Shape::rounded(metrics::PANE_HEIGHT / 2.0),
                &theme,
            );
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let theme = cx.theme();
        let origin = cx.bounds().origin();
        let enabled = !is_disabled(cx);
        match event {
            Event::PointerMove {
                pos,
                dragging: false,
                ..
            } => {
                self.compute(&theme);
                let i = self.index_at(pos.x - origin.x);
                let h = Some(i).filter(|i| self.can(*i, enabled));
                if h != self.hovered {
                    self.hovered = h;
                    cx.request_paint();
                }
                Handled::No
            }
            Event::PointerLeave => {
                if self.hovered.take().is_some() {
                    cx.request_paint();
                }
                Handled::No
            }
            Event::PointerDown {
                pos,
                button: Button::Left,
                ..
            } => {
                self.compute(&theme);
                let i = self.index_at(pos.x - origin.x);
                if self.down || !self.can(i, enabled) {
                    return Handled::No;
                }
                self.down = true;
                cx.grow(PRESS_SCALE_SEGMENTED);
                self.lens_to(cx, i);
                Handled::Yes
            }
            Event::PointerMove {
                pos,
                dragging: true,
                ..
            } if self.down => {
                // The lens follows the pointer from pane to pane.
                let i = self.index_at(pos.x - origin.x);
                if self.can(i, enabled) && self.hot != Some(i) {
                    self.lens_to(cx, i);
                }
                Handled::Yes
            }
            Event::PointerUp {
                button: Button::Left,
                ..
            } if self.down => {
                self.down = false;
                cx.grow(1.0);
                // Released anywhere (the pointer is captured): the pane under the lens.
                if let Some(t) = self.hot {
                    self.choose(cx, t);
                }
                self.lens_off(cx);
                Handled::Yes
            }
            Event::Key(k) if k.pressed => {
                let step: isize = match k.key {
                    Key::Left | Key::Up => -1,
                    Key::Right | Key::Down => 1,
                    _ => return Handled::No,
                };
                let n = self.panes.len() as isize;
                let mut i = self.selected as isize + step;
                while i >= 0 && i < n {
                    if self.can(i as usize, enabled) {
                        self.choose(cx, i as usize);
                        break;
                    }
                    i += step;
                }
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn tick(&mut self, _cx: &mut EventCx<'_>, dt: f32) -> bool {
        let a = self.thumb.tick(dt);
        let b = self.lens.tick(dt);
        a || b
    }

    fn interaction(&self) -> Interaction {
        Interaction::Hover
    }

    fn focus(&self) -> FocusKind {
        FocusKind::Control
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::SelectedIndex => match value.as_int().and_then(|i| usize::try_from(i).ok()) {
                Some(i) => {
                    self.selected = i;
                    PropChange::Paint
                }
                None => PropChange::Unknown,
            },
            // The labels, then the symbols (through the ABI the panes are built from both).
            Prop::Items => match value.as_str_list() {
                Some(labels) => {
                    let symbols: Vec<String> =
                        self.panes.iter().map(|p| p.symbol.clone()).collect();
                    self.panes = labels
                        .iter()
                        .enumerate()
                        .map(|(i, l)| Pane::new(l, symbols.get(i).map_or("", String::as_str)))
                        .collect();
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::ItemSymbols => match value.as_str_list() {
                Some(symbols) => {
                    for (p, s) in self.panes.iter_mut().zip(symbols) {
                        p.symbol.clone_from(s);
                    }
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::SelectedIndex => Some(Value::Int(self.selected as i64)),
            Prop::Items => Some(Value::StrList(
                self.panes.iter().map(|p| p.label.clone()).collect(),
            )),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::SegmentedControl;
        a.value = self.panes.get(self.selected).map(|p| p.label.clone());
        a.actions = vec![Action::Click, Action::Focus];
    }
}

/// A label whose layer box is `rect` exactly (top aligned).
fn paint_label_top(
    cx: &mut PaintCx<'_>,
    cache: &mut Label,
    name: &str,
    rect: Rect,
    text: &str,
    token: &'static TypeStyle,
    color: Color,
) {
    let theme = *cx.theme();
    let style = theme.text(token, color);
    let layout = cache.layout(text, &style, Align::Center, rect.width, Some(1));
    cx.rec.text(name, rect, layout, Some(token.name), color);
}

// ---------------------------------------------------------------------------------------
// A single item.
// ---------------------------------------------------------------------------------------

/// What a standalone item shows.
#[derive(Debug, Clone, PartialEq)]
pub enum ItemContent {
    /// A label (`Segments/Text Item`).
    Text(String),
    /// A symbol (`Segments/Icon Item`).
    Icon(String),
    /// A glyph over a label (`Segments/Icon + Label Item`).
    IconLabel {
        /// The symbol.
        symbol: String,
        /// The label.
        label: String,
    },
}

/// One item of a segmented control as its own view: what the kit's item masters show. The
/// controls draw their items themselves.
#[derive(Debug)]
pub struct SegmentItem {
    /// What it shows.
    pub content: ItemContent,
    /// Size (text and icon items).
    pub size: ControlSize,
    /// How it looks.
    pub look: ItemLook,
    /// Width (`None`: the natural one).
    pub width: Option<f32>,
    label: Label,
}

impl SegmentItem {
    /// An item.
    pub fn new(content: ItemContent) -> SegmentItem {
        SegmentItem {
            content,
            size: ControlSize::Regular,
            look: ItemLook::Rest,
            width: None,
            label: Label::default(),
        }
    }

    /// With a size.
    pub fn size(mut self, size: ControlSize) -> SegmentItem {
        self.size = size;
        self
    }

    /// With a look.
    pub fn look(mut self, look: ItemLook) -> SegmentItem {
        self.look = look;
        self
    }

    /// With a width.
    pub fn width(mut self, width: f32) -> SegmentItem {
        self.width = Some(width);
        self
    }

    fn natural(&mut self, theme: &Theme) -> Size {
        let m = SizeMetrics::of(self.size, false);
        match self.content.clone() {
            ItemContent::Icon(_) => Size::new(self.width.unwrap_or(m.height), m.item_height()),
            ItemContent::Text(t) => {
                let w = text_width(&mut self.label, theme, m.item_token(), &t)
                    + 2.0 * metrics::TEXT_ITEM_PADDING;
                Size::new(
                    self.width.unwrap_or_else(|| w.max(m.text_item_width)),
                    m.item_height(),
                )
            }
            ItemContent::IconLabel { label, .. } => {
                let style = theme.text(&typography::CAPTION, theme.colors.text_primary);
                let w = whole(self.label.width(&label, &style).ceil() + 24.0, 8.0)
                    .max(metrics::MIN_PANE_WIDTH);
                Size::new(self.width.unwrap_or(w), metrics::PANE_HEIGHT)
            }
        }
    }
}

impl Widget for SegmentItem {
    fn kind(&self) -> &'static str {
        "SegmentItem"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        let theme = cx.theme();
        self.natural(&theme)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let r = cx.bounds();
        let m = SizeMetrics::of(self.size, false);
        let selected = matches!(self.look, ItemLook::Selected | ItemLook::SelectedPressed);
        match self.content.clone() {
            ItemContent::Text(t) => {
                cx.rec.layer(
                    "Background",
                    r,
                    Shape::rounded(r.height / 2.0),
                    self.look.style(&theme),
                );
                paint_label(
                    cx,
                    &mut self.label,
                    "Label",
                    r,
                    &t,
                    m.item_token(),
                    c.text_primary,
                );
            }
            ItemContent::Icon(symbol) => {
                cx.rec.layer(
                    "Background",
                    r,
                    Shape::rounded(r.height / 2.0),
                    self.look.style(&theme),
                );
                let g = m.glyph();
                cx.rec.glyph(
                    "Glyph Sidebar",
                    Rect::new(r.x + (r.width - g) / 2.0, r.y + (r.height - g) / 2.0, g, g),
                    &symbol,
                    400,
                    c.text_primary,
                );
            }
            ItemContent::IconLabel { symbol, label } => {
                cx.rec.layer(
                    "Background",
                    r,
                    Shape::rounded(r.height / 2.0),
                    self.look.style(&theme),
                );
                let g = metrics::PANE_GLYPH;
                cx.rec.glyph_sized(
                    "Glyph",
                    Rect::new(r.x + (r.width - g) / 2.0, r.y + 4.0, g, g + 1.0),
                    g,
                    &symbol,
                    400,
                    if selected {
                        c.accent_amalfi
                    } else {
                        c.text_primary
                    },
                );
                paint_label_top(
                    cx,
                    &mut self.label,
                    "Label",
                    Rect::new(r.x, r.y + metrics::PANE_LABEL_TOP, r.width, 14.0),
                    &label,
                    &typography::CAPTION,
                    c.text_primary,
                );
            }
        }
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::{KeyEvent, Mods};
    use crate::core::ui::Ui;
    use crate::core::window::{WindowId, WindowKind};
    use std::cell::RefCell;
    use std::rc::Rc;

    fn scene(
        w: impl Widget + 'static,
    ) -> (Ui, WindowId, crate::core::ViewId, Rc<RefCell<Vec<Value>>>) {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(400.0, 200.0));
        let id = ui.create(w);
        let log = Rc::new(RefCell::new(Vec::new()));
        let l = log.clone();
        ui.connect(id, Signal::ValueChanged, move |_, _, v| {
            l.borrow_mut().push(v.clone());
        });
        let l = log.clone();
        ui.connect(id, Signal::Activated, move |_, _, v| {
            l.borrow_mut().push(v.clone());
        });
        ui.set_content(win, id);
        ui.show_window(win);
        ui.prepare_frame(win);
        (ui, win, id, log)
    }

    fn at(ui: &Ui, id: crate::core::ViewId, x: f32, y: f32) -> Point {
        let r = ui.tree.window_rect(id);
        Point::new(r.x + x, r.y + y)
    }

    fn click(ui: &mut Ui, win: WindowId, p: Point) {
        ui.pointer_move(win, p, Mods::empty());
        ui.pointer_down(win, p, Button::Left, Mods::empty());
        ui.pointer_up(win, p, Button::Left, Mods::empty());
    }

    #[test]
    fn select_one_chooses_on_release_and_slides() {
        let (mut ui, win, id, log) =
            scene(SegmentedControl::texts(&["One", "Two", "Three"]).selected(&[0]));
        // The second item's centre: 4 + 72 + 36.
        let p = at(&ui, id, 112.0, 14.0);
        click(&mut ui, win, p);
        assert_eq!(*log.borrow(), vec![Value::Int(1)]);
        assert_eq!(ui.get(id, Prop::SelectedIndex), Some(Value::Int(1)));
        for _ in 0..60 {
            ui.advance(0.05);
        }
        assert!(
            ui.widget::<SegmentedControl>(id)
                .is_some_and(|c| c.hot.is_none() && c.lens.get() <= 0.0)
        );
        // Choosing the chosen one again changes nothing.
        let n = log.borrow().len();
        let p = at(&ui, id, 112.0, 14.0);
        click(&mut ui, win, p);
        assert_eq!(log.borrow().len(), n);
    }

    #[test]
    fn dragging_the_lens_chooses_the_item_under_it() {
        let (mut ui, win, id, _log) =
            scene(SegmentedControl::texts(&["One", "Two", "Three"]).selected(&[0]));
        let start = at(&ui, id, 40.0, 14.0);
        ui.pointer_move(win, start, Mods::empty());
        ui.pointer_down(win, start, Button::Left, Mods::empty());
        ui.pointer_move(win, at(&ui, id, 190.0, 14.0), Mods::empty());
        ui.pointer_up(win, at(&ui, id, 190.0, 14.0), Button::Left, Mods::empty());
        assert_eq!(ui.get(id, Prop::SelectedIndex), Some(Value::Int(2)));
        // The pointer is captured: released far below, the item under the lens is chosen
        // (ui-kit `up`).
        let start = at(&ui, id, 40.0, 14.0);
        ui.pointer_move(win, start, Mods::empty());
        ui.pointer_down(win, start, Button::Left, Mods::empty());
        ui.pointer_move(win, at(&ui, id, 40.0, 90.0), Mods::empty());
        ui.pointer_up(win, at(&ui, id, 40.0, 90.0), Button::Left, Mods::empty());
        assert_eq!(ui.get(id, Prop::SelectedIndex), Some(Value::Int(0)));
    }

    fn scale_of(ui: &Ui, id: crate::core::ViewId) -> f32 {
        let s = ui
            .tree
            .get(id)
            .and_then(|n| n.anim.as_ref())
            .map_or(1.0, |a| a.scale.get());
        (s * 1000.0).round() / 1000.0
    }

    fn settle(ui: &mut Ui) {
        for _ in 0..120 {
            ui.advance(0.02);
        }
    }

    #[test]
    fn the_thumb_swells_into_the_lens_and_jumps_from_item_to_item() {
        let (mut ui, win, id, _log) =
            scene(SegmentedControl::texts(&["One", "Two", "Three"]).selected(&[0]));
        let items = ui
            .widget::<SegmentedControl>(id)
            .map(|c| c.items.clone())
            .unwrap_or_default();
        let start = at(&ui, id, 40.0, 14.0);
        ui.pointer_move(win, start, Mods::empty());
        ui.pointer_down(win, start, Button::Left, Mods::empty());
        settle(&mut ui);
        let state = |ui: &Ui| {
            ui.widget::<SegmentedControl>(id)
                .map(|c| (c.thumb.get(), c.lens.get(), c.hot))
                .unwrap_or_default()
        };
        // Pressed: the lens is around the first item, 8 / 12 beyond it.
        let (rect, t, hot) = state(&ui);
        assert_eq!(rect, lens_rect(items[0]));
        assert!((t - 1.0).abs() < 1e-3);
        assert_eq!(hot, Some(0));
        // Dragged to the third: the same element moves there (it does not trail the pointer).
        ui.pointer_move(win, at(&ui, id, 190.0, 14.0), Mods::empty());
        let (_, _, hot) = state(&ui);
        assert_eq!(hot, Some(2));
        settle(&mut ui);
        assert_eq!(state(&ui).0, lens_rect(items[2]));
        // Released: the selection is the third, the lens is the thumb again under it.
        ui.pointer_up(win, at(&ui, id, 190.0, 14.0), Button::Left, Mods::empty());
        settle(&mut ui);
        let (rect, t, hot) = state(&ui);
        assert_eq!((rect, hot), (items[2], None));
        assert!(t.abs() < 1e-3);
        assert_eq!(ui.get(id, Prop::SelectedIndex), Some(Value::Int(2)));
    }

    #[test]
    fn the_whole_control_grows_while_held_and_not_with_reduced_motion() {
        let (mut ui, win, id, _log) =
            scene(SegmentedControl::texts(&["One", "Two"]).selected(&[0]));
        let p = at(&ui, id, 40.0, 14.0);
        ui.pointer_move(win, p, Mods::empty());
        ui.pointer_down(win, p, Button::Left, Mods::empty());
        settle(&mut ui);
        assert_eq!(scale_of(&ui, id), 1.035);
        ui.pointer_up(win, p, Button::Left, Mods::empty());
        settle(&mut ui);
        assert_eq!(scale_of(&ui, id), 1.0);
    }

    #[test]
    fn pressed_holds_the_second_item_without_a_pointer() {
        let (mut ui, win, id, log) =
            scene(SegmentedControl::texts(&["One", "Two", "Three"]).selected(&[0]));
        let items = ui
            .widget::<SegmentedControl>(id)
            .map(|c| c.items.clone())
            .unwrap_or_default();
        let _ = ui.set(id, Prop::Pressed, Value::Bool(true));
        ui.prepare_frame(win);
        settle(&mut ui);
        let thumb = ui.widget::<SegmentedControl>(id).map(|c| c.thumb.get());
        assert_eq!(thumb, Some(lens_rect(items[1])));
        assert_eq!(scale_of(&ui, id), 1.035);
        // Letting go chooses nothing.
        let _ = ui.set(id, Prop::Pressed, Value::Bool(false));
        ui.prepare_frame(win);
        settle(&mut ui);
        assert!(log.borrow().is_empty());
        assert_eq!(ui.get(id, Prop::SelectedIndex), Some(Value::Int(0)));
        assert_eq!(scale_of(&ui, id), 1.0);
    }

    fn thumb_layer(
        ui: &mut Ui,
        id: crate::core::ViewId,
    ) -> (usize, usize, crate::style::Style, [f32; 4]) {
        use crate::core::InspectedContent;
        let layers = ui.inspect_layers(id);
        let n = layers.len();
        let i = layers.iter().position(|l| l.name == "Thumb").unwrap_or(n);
        match layers.get(i).map(|l| &l.content) {
            Some(InspectedContent::Shape { style, radii, .. }) => (i, n, style.clone(), *radii),
            _ => (i, n, crate::style::Style::default(), [0.0; 4]),
        }
    }

    #[test]
    fn the_held_thumb_is_the_regular_glass_lens_under_the_labels() {
        let (mut ui, win, id, _log) =
            scene(SegmentedControl::texts(&["One", "Two", "Three"]).selected(&[0]));
        let (i, n, rest, _) = thumb_layer(&mut ui, id);
        // At rest the selected item carries the thumb look itself: no moving thumb.
        assert_eq!(i, n);
        assert!(rest.blurs.is_empty());
        let _ = ui.set(id, Prop::Pressed, Value::Bool(true));
        ui.prepare_frame(win);
        settle(&mut ui);
        ui.prepare_frame(win);
        let (i, n, lens, radii) = thumb_layer(&mut ui, id);
        let item_h = 20.0;
        // Settled: Regular glass (its tint, the rims inside, the glass shadow, its blur
        // behind), corners 1.1 x the item's height, and below the items' labels.
        let theme = Theme::light();
        let tint = theme.material("Glass/Regular").and_then(|m| m.tint);
        assert_eq!(lens.fills.last(), tint.map(Into::into).as_ref());
        assert_eq!(lens.inner_shadows.len(), 2);
        assert_eq!(lens.shadows.last().copied(), theme.shadow(4));
        assert!(matches!(
            lens.blurs.first(),
            Some(crate::style::Blur::Glass(g))
                if theme.material("Glass/Regular").is_some_and(|m| (g.radius - m.blur).abs() < 1e-3)
        ));
        assert!((radii[0] - item_h * 1.1).abs() < 0.01, "{radii:?}");
        assert!(i + 2 < n, "the lens is drawn before the items ({i} of {n})");
        // Let go: it is the thumb again, below the items.
        let _ = ui.set(id, Prop::Pressed, Value::Bool(false));
        ui.prepare_frame(win);
        settle(&mut ui);
        ui.prepare_frame(win);
        let (i, n, back, _) = thumb_layer(&mut ui, id);
        assert!(i == n || (back.blurs.is_empty() && i + 2 < n));
    }

    /// The luminance of the pixels of `r` (device pixels).
    fn grey(img: &crate::render::image::Image, r: Rect) -> Vec<f32> {
        let mut out = Vec::new();
        for y in (r.y as u32)..(r.bottom() as u32) {
            for x in (r.x as u32)..(r.right() as u32) {
                let p = img.pixel(x, y);
                out.push(
                    0.299 * f32::from(p[0]) + 0.587 * f32::from(p[1]) + 0.114 * f32::from(p[2]),
                );
            }
        }
        out
    }

    fn std_dev(v: &[f32]) -> f32 {
        let n = v.len().max(1) as f32;
        let m = v.iter().sum::<f32>() / n;
        (v.iter().map(|x| (x - m) * (x - m)).sum::<f32>() / n).sqrt()
    }

    #[test]
    fn the_label_over_the_lens_stays_sharp() {
        // Reduced motion: no scale, so the held and the resting renders line up.
        let (mut ui, win, id, _log) =
            scene(SegmentedControl::texts(&["Day", "Week", "Month"]).selected(&[1]));
        let mut a = ui.appearance;
        a.reduce_motion = true;
        ui.set_appearance(a);
        let rest = ui.snapshot(win, 2.0).expect("rest");
        let items = ui
            .widget::<SegmentedControl>(id)
            .map(|c| c.items.clone())
            .unwrap_or_default();
        let origin = ui.tree.window_rect(id).origin();
        let item = items[1];
        // The label's box, inset from the lens' rims, in device pixels.
        let r = Rect::new(
            (origin.x + item.x + 6.0) * 2.0,
            (origin.y + item.y + 3.0) * 2.0,
            (item.width - 12.0) * 2.0,
            (item.height - 6.0) * 2.0,
        );
        let reference = grey(&rest, r);

        let _ = ui.set(id, Prop::Pressed, Value::Bool(true));
        // Barely started, then settled: the lens only ever gets to its one blur.
        for _ in 0..3 {
            ui.advance(1.0 / 60.0);
            let _ = ui.prepare_frame(win);
        }
        let early = ui.snapshot(win, 2.0).expect("early");
        assert!(early.width() > 0);
        ui.settle_animations(win);
        let held = ui.snapshot(win, 2.0).expect("held");
        // Painting again, and again after more frames, gives the same pixels: nothing builds
        // up from one frame to the next.
        for _ in 0..30 {
            ui.advance(1.0 / 60.0);
            let _ = ui.prepare_frame(win);
        }
        let later = ui.snapshot(win, 2.0).expect("later");
        assert_eq!(held.bytes(), later.bytes());
        // The label is drawn over the glass, as sharp as at rest.
        let (got, want) = (std_dev(&grey(&held, r)), std_dev(&reference));
        assert!(
            got >= want * 0.85,
            "contrast {got:.2} over the lens, {want:.2} at rest"
        );
    }

    #[test]
    fn reduced_motion_morphs_the_lens_with_the_short_fade_and_no_scale() {
        let (mut ui, win, id, _log) =
            scene(SegmentedControl::texts(&["One", "Two", "Three"]).selected(&[0]));
        let mut a = ui.appearance;
        a.reduce_motion = true;
        ui.set_appearance(a);
        let _ = ui.set(id, Prop::Pressed, Value::Bool(true));
        ui.prepare_frame(win);
        // 150 ms in: a fade, not a spring: the lens is between the two looks (settles by
        // 160 ms) and the control does not scale.
        ui.advance(0.075);
        let mid = ui
            .widget::<SegmentedControl>(id)
            .map(|c| c.lens.get())
            .unwrap_or(-1.0);
        assert!(mid > 0.0 && mid < 1.0, "{mid}");
        ui.advance(0.1);
        let end = ui
            .widget::<SegmentedControl>(id)
            .map(|c| c.lens.get())
            .unwrap_or(-1.0);
        assert_eq!(end, 1.0);
        assert_eq!(scale_of(&ui, id), 1.0);
    }

    #[test]
    fn select_any_toggles_each_item() {
        let (mut ui, win, id, log) = scene(
            SegmentedControl::texts(&["Bold", "Italic"])
                .mode(Selection::Any)
                .selected(&[0]),
        );
        let p = at(&ui, id, 100.0, 14.0);
        click(&mut ui, win, p);
        let p = at(&ui, id, 30.0, 14.0);
        click(&mut ui, win, p);
        assert_eq!(
            *log.borrow(),
            vec![Value::IntList(vec![0, 1]), Value::IntList(vec![1])]
        );
    }

    #[test]
    fn arrows_move_the_choice_and_skip_disabled() {
        let (mut ui, win, id, _log) = scene(
            SegmentedControl::new(vec![
                Segment::text("A"),
                Segment::text("B").disabled(),
                Segment::text("C"),
            ])
            .selected(&[0]),
        );
        ui.set_focus(id, true);
        ui.key(win, KeyEvent::press(Key::Right, Mods::empty()));
        assert_eq!(ui.get(id, Prop::SelectedIndex), Some(Value::Int(2)));
        ui.key(win, KeyEvent::press(Key::Left, Mods::empty()));
        assert_eq!(ui.get(id, Prop::SelectedIndex), Some(Value::Int(0)));
    }

    #[test]
    fn up_and_down_arrows_move_the_choice_and_wrap_round_the_ends() {
        let (mut ui, win, id, _log) = scene(
            SegmentedControl::new(vec![
                Segment::text("A"),
                Segment::text("B"),
                Segment::text("C"),
            ])
            .selected(&[0]),
        );
        ui.set_focus(id, true);
        ui.key(win, KeyEvent::press(Key::Down, Mods::empty()));
        assert_eq!(ui.get(id, Prop::SelectedIndex), Some(Value::Int(1)));
        ui.key(win, KeyEvent::press(Key::Up, Mods::empty()));
        ui.key(win, KeyEvent::press(Key::Up, Mods::empty()));
        assert_eq!(
            ui.get(id, Prop::SelectedIndex),
            Some(Value::Int(2)),
            "wraps to the end"
        );
        ui.key(win, KeyEvent::press(Key::Right, Mods::empty()));
        assert_eq!(
            ui.get(id, Prop::SelectedIndex),
            Some(Value::Int(0)),
            "and back"
        );
    }

    #[test]
    fn action_segments_fire_on_release_over_the_pressed_one() {
        let (mut ui, win, id, log) = scene(ActionSegments::new(vec![
            Segment::icon("chevron-left", "Back"),
            Segment::icon("chevron-right", "Forward"),
        ]));
        let p = at(&ui, id, 55.0, 18.0);
        click(&mut ui, win, p);
        assert_eq!(*log.borrow(), vec![Value::Int(1)]);
        // Pressed on one, released on the other: nothing.
        let p = at(&ui, id, 18.0, 18.0);
        ui.pointer_move(win, p, Mods::empty());
        ui.pointer_down(win, p, Button::Left, Mods::empty());
        ui.pointer_move(win, at(&ui, id, 55.0, 18.0), Mods::empty());
        ui.pointer_up(win, at(&ui, id, 55.0, 18.0), Button::Left, Mods::empty());
        assert_eq!(log.borrow().len(), 1);
    }

    #[test]
    fn pane_control_chooses_a_pane() {
        let (mut ui, win, id, log) = scene(PaneControl::new(vec![
            Pane::new("General", "gear"),
            Pane::new("Network", "wifi"),
        ]));
        let p = at(&ui, id, 140.0, 20.0);
        click(&mut ui, win, p);
        assert_eq!(*log.borrow(), vec![Value::Int(1)]);
        assert_eq!(ui.get(id, Prop::SelectedIndex), Some(Value::Int(1)));
    }

    #[test]
    fn pane_control_slides_its_thumb_and_drags_the_lens() {
        let (mut ui, win, id, log) = scene(PaneControl::new(vec![
            Pane::new("General", "gear"),
            Pane::new("Network", "wifi"),
            Pane::new("Sound", "speaker"),
        ]));
        let lens = |ui: &Ui| {
            ui.widget::<PaneControl>(id)
                .map(|c| (c.hot, c.lens.target(), c.thumb.target()))
        };
        let start = at(&ui, id, 40.0, 26.0);
        ui.pointer_move(win, start, Mods::empty());
        ui.pointer_down(win, start, Button::Left, Mods::empty());
        // Held: the lens is around the first pane, 8 / 12 beyond it.
        let first = ui.widget::<PaneControl>(id).and_then(|c| c.item(0));
        assert_eq!(lens(&ui), first.map(|r| (Some(0), 1.0, lens_rect(r))));
        // Dragged to the last pane and released below the control: the last one is chosen
        // and the thumb slides back under it.
        let far = at(&ui, id, 250.0, 26.0);
        ui.pointer_move(win, far, Mods::empty());
        ui.pointer_up(win, at(&ui, id, 250.0, 120.0), Button::Left, Mods::empty());
        assert_eq!(*log.borrow(), vec![Value::Int(2)]);
        let last = ui.widget::<PaneControl>(id).and_then(|c| c.item(2));
        assert_eq!(lens(&ui), last.map(|r| (None, 0.0, r)));
        assert!(
            ui.widget::<PaneControl>(id)
                .is_some_and(|c| c.thumb.is_animating())
        );
    }

    #[test]
    fn lens_geometry() {
        let item = Rect::new(76.0, 8.0, 72.0, 20.0);
        assert_eq!(lens_rect(item), Rect::new(68.0, -4.0, 88.0, 44.0));
    }
}
