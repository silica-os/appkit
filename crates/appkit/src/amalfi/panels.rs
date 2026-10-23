//! Panels (ui-kit `SColorPanel`, `SFilePanel` and `SFileBrowser`): the colour panel's content
//! (swatches, a saturation / brightness pad, a hue slider and a hex field), the striped file
//! list of the open and save panels, and the open / save panel in the Finder's layout.

use crate::amalfi::button::{PushButton, PushVariant};
use crate::amalfi::buttons::{IconButton, MenuButton};
use crate::amalfi::fields::TextField;
use crate::amalfi::forms::form_row;
use crate::amalfi::foundation::{Interaction as Transitions, Label, focus_halo_named};
use crate::amalfi::segments::{ActionSegments, Segment, SegmentedControl};
use crate::amalfi::sidebars;
use crate::amalfi::sliders::Slider;
use crate::amalfi::text::{Text, TextRole};
use crate::amalfi::tokens::typography;
use crate::amalfi::wells::swatch_row;
use crate::color::Color;
use crate::core::a11y::{A11y, Action, Role};
use crate::core::event::{Button, Event, Handled, Key, Mods};
use crate::core::layout::{Alignment, Proposal, Spacer, Stack};
use crate::core::tree::{NodeFlags, ViewId};
use crate::core::ui::Ui;
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, FocusKind, Interaction, LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Insets, Point, Rect, Size};
use crate::shape::Shape;
use crate::style::{Border, Paint, Style};
use crate::text::Align;

/// Fixed measures of the kit.
pub mod metrics {
    /// The colour panel's width.
    pub const PICKER_WIDTH: f32 = 192.0;
    /// The saturation / brightness pad's height.
    pub const PAD_HEIGHT: f32 = 96.0;
    /// The pad's corner radius.
    pub const PAD_RADIUS: f32 = 8.0;
    /// The pad's knob.
    pub const KNOB: f32 = 14.0;
    /// The side margin of the pad, the hue slider and the hex row.
    pub const MARGIN: f32 = 4.0;
    /// The hex row's colour chip.
    pub const CHIP: f32 = 28.0;
    /// A file row's height.
    pub const ROW: f32 = 32.0;
    /// The file list's column header height.
    pub const HEADER: f32 = 28.0;
    /// The file list's default height.
    pub const LIST_HEIGHT: f32 = 220.0;
    /// Where the Name, Kind and Date Modified columns start.
    pub const COLUMNS: (f32, f32, f32) = (44.0, 300.0, 396.0);
    /// The file panel's places column.
    pub const SIDEBAR: f32 = 177.0;
    /// A file panel's width, expanded and compact.
    pub const PANEL_WIDTH: (f32, f32) = (720.0, 440.0);
    /// The file panel's corner radius.
    pub const PANEL_RADIUS: f32 = 20.0;
}

// ---------------------------------------------------------------------------------------------
// Colour maths
// ---------------------------------------------------------------------------------------------

/// Hue (0–360), saturation and value (0–1) of a colour.
pub fn to_hsv(c: Color) -> (f32, f32, f32) {
    let (r, g, b) = (
        f32::from(c.r) / 255.0,
        f32::from(c.g) / 255.0,
        f32::from(c.b) / 255.0,
    );
    let max = r.max(g).max(b);
    let d = max - r.min(g).min(b);
    let h = if d <= f32::EPSILON {
        0.0
    } else if (max - r).abs() <= f32::EPSILON {
        60.0 * (((g - b) / d).rem_euclid(6.0))
    } else if (max - g).abs() <= f32::EPSILON {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    (h, if max <= f32::EPSILON { 0.0 } else { d / max }, max)
}

/// The colour of a hue (0–360), saturation and value (0–1).
pub fn from_hsv(h: f32, s: f32, v: f32) -> Color {
    let (s, v) = (s.clamp(0.0, 1.0), v.clamp(0.0, 1.0));
    let h = if h.is_finite() {
        h.rem_euclid(360.0)
    } else {
        0.0
    };
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let q = |f: f32| ((f + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    Color::rgb(q(r), q(g), q(b))
}

// ---------------------------------------------------------------------------------------------
// The pad
// ---------------------------------------------------------------------------------------------

/// The saturation / brightness pad: white to the hue left to right, black at the bottom, with a
/// knob. Dragging or the arrow keys (Shift: ten times more) move it; `ValueChanged` carries
/// the new colour.
#[derive(Debug)]
pub struct ColorPad {
    /// Hue, saturation, value.
    pub hsv: (f32, f32, f32),
    dragging: bool,
}

impl ColorPad {
    /// A pad at `color`.
    pub fn new(color: Color) -> ColorPad {
        ColorPad {
            hsv: to_hsv(color),
            dragging: false,
        }
    }

    fn set_from(&mut self, cx: &mut EventCx<'_>, p: Point) {
        let s = cx.size();
        if s.width <= 0.0 || s.height <= 0.0 {
            return;
        }
        let sat = (p.x / s.width).clamp(0.0, 1.0);
        let val = 1.0 - (p.y / s.height).clamp(0.0, 1.0);
        self.hsv = (self.hsv.0, sat, val);
        cx.emit(
            Signal::ValueChanged,
            Value::Color(from_hsv(self.hsv.0, sat, val)),
        );
        cx.request_paint();
    }

    fn nudge(&mut self, cx: &mut EventCx<'_>, dx: f32, dy: f32, k: f32) {
        let sat = (self.hsv.1 + dx * k).clamp(0.0, 1.0);
        let val = (self.hsv.2 + dy * k).clamp(0.0, 1.0);
        self.hsv = (self.hsv.0, sat, val);
        cx.emit(
            Signal::ValueChanged,
            Value::Color(from_hsv(self.hsv.0, sat, val)),
        );
        cx.request_paint();
    }
}

impl Widget for ColorPad {
    fn kind(&self) -> &'static str {
        "ColorPad"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let w = p
            .width
            .filter(|w| w.is_finite())
            .unwrap_or(metrics::PICKER_WIDTH - 2.0 * metrics::MARGIN);
        Size::new(w.max(0.0), metrics::PAD_HEIGHT)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let r = cx.bounds();
        let hue = from_hsv(self.hsv.0, 1.0, 1.0);
        let shape = Shape::rounded(metrics::PAD_RADIUS);
        let style = Style::default()
            .with_fill(Paint::Linear {
                from: Point::new(0.0, 0.0),
                to: Point::new(1.0, 0.0),
                stops: vec![(0.0, Color::rgb(255, 255, 255)), (1.0, hue)],
            })
            .with_fill(Paint::Linear {
                from: Point::new(0.0, 1.0),
                to: Point::new(0.0, 0.0),
                stops: vec![(0.0, Color::rgb(0, 0, 0)), (1.0, Color::rgba(0, 0, 0, 0))],
            })
            .with_border(Border::inside(c.line_border, 1.0));
        cx.rec.layer("Pad", r, shape.clone(), style);
        let k = metrics::KNOB;
        let knob = Rect::new(
            r.x + self.hsv.1 * r.width - k / 2.0,
            r.y + (1.0 - self.hsv.2) * r.height - k / 2.0,
            k,
            k,
        );
        let cur = from_hsv(self.hsv.0, self.hsv.1, self.hsv.2);
        cx.rec.layer(
            "Knob",
            knob,
            Shape::Ellipse,
            Style::fill(cur)
                .with_border(Border::at(
                    Color::rgb(255, 255, 255),
                    2.0,
                    crate::style::BorderPosition::Outside,
                ))
                .with_shadow(crate::style::Shadow::new(
                    Color::rgba(0, 0, 0, 0x66),
                    0.0,
                    1.0,
                    3.0,
                )),
        );
        if cx.focus_visible() && !cx.is_disabled() {
            focus_halo_named(cx.rec, "Focus Halo", r, shape, &theme);
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        match event {
            Event::PointerDown {
                pos,
                button: Button::Left,
                ..
            } => {
                self.dragging = true;
                cx.capture_pointer();
                self.set_from(cx, *pos);
                Handled::Yes
            }
            Event::PointerMove {
                pos,
                dragging: true,
                ..
            } if self.dragging => {
                self.set_from(cx, *pos);
                Handled::Yes
            }
            Event::PointerUp {
                button: Button::Left,
                ..
            } => {
                self.dragging = false;
                Handled::Yes
            }
            Event::Key(k) if k.pressed => {
                let step = if k.mods.contains(Mods::SHIFT) {
                    0.1
                } else {
                    0.02
                };
                let (dx, dy) = match k.key {
                    Key::Left => (-1.0, 0.0),
                    Key::Right => (1.0, 0.0),
                    Key::Up => (0.0, 1.0),
                    Key::Down => (0.0, -1.0),
                    _ => return Handled::No,
                };
                self.nudge(cx, dx, dy, step);
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn interaction(&self) -> Interaction {
        Interaction::None
    }

    fn focus(&self) -> FocusKind {
        FocusKind::Control
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match (prop, value.as_color()) {
            (Prop::ColorValue, Some(c)) => {
                let (h, s, v) = to_hsv(c);
                // A grey keeps the hue it had.
                let h = if s > 0.0 && v > 0.0 { h } else { self.hsv.0 };
                self.hsv = (h, s, v);
                PropChange::Paint
            }
            (Prop::Value, _) => match value.as_f32() {
                Some(h) => {
                    self.hsv.0 = h.clamp(0.0, 360.0);
                    PropChange::Paint
                }
                None => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Slider;
        a.label = Some("Saturation and brightness".to_string());
        a.value = Some(from_hsv(self.hsv.0, self.hsv.1, self.hsv.2).to_hex());
        a.actions = vec![Action::Focus];
    }
}

/// A 1 pt rule.
#[derive(Debug, Clone, Copy)]
pub struct Hairline;

impl Widget for Hairline {
    fn kind(&self) -> &'static str {
        "Hairline"
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        Size::new(
            p.width.filter(|w| w.is_finite()).unwrap_or(0.0).max(0.0),
            1.0,
        )
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let c = cx.theme().colors;
        cx.rec.layer(
            "Separator",
            cx.bounds(),
            Shape::RECT,
            Style::fill(c.line_separator),
        );
    }

    fn accessibility(&self, a: &mut A11y) {
        a.hidden = true;
    }
}

/// The 28 pt colour chip beside the hex field.
#[derive(Debug)]
pub struct ColorChip {
    /// The colour.
    pub color: Color,
}

impl Widget for ColorChip {
    fn kind(&self) -> &'static str {
        "ColorChip"
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        Size::new(metrics::CHIP, metrics::CHIP)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let c = cx.theme().colors;
        cx.rec.layer(
            "Chip",
            cx.bounds(),
            Shape::rounded(9.0),
            Style::fill(self.color).with_border(Border::inside(c.line_border, 1.0)),
        );
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match (prop, value.as_color()) {
            (Prop::ColorValue, Some(c)) => {
                self.color = c;
                PropChange::Paint
            }
            _ => PropChange::Unknown,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.hidden = true;
    }
}

// ---------------------------------------------------------------------------------------------
// The colour panel's content
// ---------------------------------------------------------------------------------------------

/// The colour panel's content: 192 wide, the swatches, and unless swatches-only a rule, the
/// pad, the hue slider and the hex row. It has no surface (put it in a popover). `ValueChanged`
/// carries the chosen `Color`.
#[derive(Debug)]
pub struct ColorPicker {
    /// The current colour.
    pub color: Color,
    hue: Option<ViewId>,
    pad: Option<ViewId>,
    chip: Option<ViewId>,
    hex: Option<ViewId>,
    chips: Vec<(ViewId, Color)>,
}

impl Widget for ColorPicker {
    fn kind(&self) -> &'static str {
        "ColorPicker"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        let mut h = 0.0;
        for c in cx.children() {
            h += cx
                .measure(c, Proposal::new(Some(metrics::PICKER_WIDTH), None))
                .height;
        }
        Size::new(metrics::PICKER_WIDTH, h + 4.0)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, _size: Size) {
        let mut y = 0.0;
        for c in cx.children() {
            let s = cx.measure(c, Proposal::new(Some(metrics::PICKER_WIDTH), None));
            cx.place(
                c,
                Rect::new(0.0, y, s.width.min(metrics::PICKER_WIDTH), s.height),
            );
            y += s.height;
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Group;
        a.label = Some("Colours".to_string());
        a.value = Some(self.color.to_hex());
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::ColorValue => Some(Value::Color(self.color)),
            _ => None,
        }
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match (prop, value.as_color()) {
            (Prop::ColorValue, Some(c)) => {
                self.color = c;
                PropChange::Paint
            }
            _ => PropChange::Unknown,
        }
    }
}

fn pad_of(ui: &mut Ui, v: ViewId, top: f32) -> ViewId {
    let _ = ui.set(
        v,
        Prop::Padding,
        Value::Insets(Insets::new(top, metrics::MARGIN, 0.0, metrics::MARGIN)),
    );
    v
}

/// Makes the colour panel's content over `rows` of swatches (name, colour), showing `color`.
/// `swatches_only` leaves out the pad, the hue slider and the hex field.
pub fn color_picker(
    ui: &mut Ui,
    rows: &[Vec<(String, Color)>],
    color: Color,
    swatches_only: bool,
) -> ViewId {
    let picker = ui.create(ColorPicker {
        color,
        hue: None,
        pad: None,
        chip: None,
        hex: None,
        chips: Vec::new(),
    });
    let title = ui.create(
        Text::new("Swatches")
            .token(&typography::HEADLINE)
            .role(TextRole::Primary)
            .layer("Title"),
    );
    let _ = ui.set(
        title,
        Prop::Padding,
        Value::Insets(Insets::new(2.0, 0.0, 0.0, 0.0)),
    );
    ui.add_child(picker, title);
    let mut chips = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let (r, cs) = swatch_row(ui, row, row.iter().position(|(_, c)| *c == color));
        let _ = ui.set(r, Prop::Identifier, Value::from(format!("Row {}", i + 1)));
        ui.add_child(picker, r);
        for (chip, (_, c)) in cs.iter().zip(row) {
            chips.push((*chip, *c));
        }
    }
    let _ = ui.with_widget::<ColorPicker, _>(picker, |p| p.chips.clone_from(&chips));
    if !swatches_only {
        let rule = ui.create(Hairline);
        let _ = ui.set(
            rule,
            Prop::Padding,
            Value::Insets(Insets::new(4.0, 4.0, 8.0, 4.0)),
        );
        ui.add_child(picker, rule);
        let pad = ui.create(ColorPad::new(color));
        pad_of(ui, pad, 0.0);
        ui.add_child(picker, pad);
        let hue = ui.create(Slider::new().range(0.0, 360.0).value(to_hsv(color).0));
        let _ = ui.set(hue, Prop::Identifier, Value::from("Hue"));
        let _ = ui.set(hue, Prop::AccessibilityLabel, Value::from("Hue"));
        pad_of(ui, hue, 8.0);
        let _ = ui.set(
            hue,
            Prop::Width,
            Value::from(metrics::PICKER_WIDTH - 2.0 * metrics::MARGIN),
        );
        ui.add_child(picker, hue);
        let row = ui.create(Stack::row(8.0));
        pad_of(ui, row, 8.0);
        let chip = ui.create(ColorChip { color });
        let hex = ui.create(TextField::text().with_text(&color.to_hex()));
        let _ = ui.set(hex, Prop::Width, Value::from(120.0));
        let _ = ui.set(hex, Prop::AccessibilityLabel, Value::from("Hex colour"));
        ui.add_child(row, chip);
        ui.add_child(row, hex);
        ui.add_child(picker, row);
        let _ = ui.with_widget::<ColorPicker, _>(picker, |p| {
            p.hue = Some(hue);
            p.pad = Some(pad);
            p.chip = Some(chip);
            p.hex = Some(hex);
        });
        ui.connect(pad, Signal::ValueChanged, move |ui, _, v| {
            if let Some(c) = v.as_color() {
                set_color(ui, picker, c, true);
            }
        });
        ui.connect(hue, Signal::ValueChanged, move |ui, _, v| {
            let Some(h) = v.as_f32() else { return };
            let (s, val, cur) = ui
                .with_widget::<ColorPicker, _>(picker, |p| {
                    let (_, s, v) = to_hsv(p.color);
                    (s, v, p.color)
                })
                .unwrap_or((1.0, 1.0, color));
            // A grey has no hue to carry: give it the saturation to show the slider's effect.
            let (s, val) = if s <= 0.0 || val <= 0.0 {
                (1.0, val.max(1.0))
            } else {
                (s, val)
            };
            let _ = cur;
            let c = from_hsv(h, s, val);
            set_color(ui, picker, c, true);
            let _ = ui.with_widget::<ColorPad, _>(pad, |p| p.hsv.0 = h);
        });
        ui.connect(hex, Signal::TextChanged, move |ui, _, v| {
            let Some(t) = v.as_str() else { return };
            let t = t.trim();
            let digits = t.trim_start_matches('#');
            if digits.len() == 6
                && let Some(c) = Color::parse(t)
            {
                set_color(ui, picker, c, true);
            }
        });
    }
    for (chip, c) in chips {
        ui.connect(chip, Signal::Activated, move |ui, _, _| {
            set_color(ui, picker, c, true);
        });
    }
    picker
}

/// Sets the panel's colour: the swatch ring, the pad, the hue slider, the chip and the hex
/// field follow; with `emit`, the panel emits `ValueChanged`.
pub fn set_color(ui: &mut Ui, picker: ViewId, color: Color, emit: bool) {
    let Some((hue, pad, chip, hex, chips)) = ui.with_widget::<ColorPicker, _>(picker, |p| {
        p.color = color;
        (p.hue, p.pad, p.chip, p.hex, p.chips.clone())
    }) else {
        return;
    };
    for (v, c) in chips {
        let _ = ui.set(v, Prop::State, Value::Bool(c == color));
    }
    if let Some(p) = pad {
        let _ = ui.set(p, Prop::ColorValue, Value::Color(color));
    }
    if let Some(c) = chip {
        let _ = ui.set(c, Prop::ColorValue, Value::Color(color));
    }
    if let Some(h) = hue {
        let (hv, s, v) = to_hsv(color);
        if s > 0.0 && v > 0.0 {
            let _ = ui.set(h, Prop::Value, Value::from(hv));
        }
    }
    if let Some(x) = hex {
        let cur = ui
            .get(x, Prop::Text)
            .and_then(|v| v.as_str().map(str::to_string));
        if cur.and_then(|t| Color::parse(&t)) != Some(color) {
            let _ = ui.set(x, Prop::Text, Value::from(color.to_hex()));
        }
    }
    if emit {
        ui.emit(picker, Signal::ValueChanged, Value::Color(color));
    }
}

// ---------------------------------------------------------------------------------------------
// The file list
// ---------------------------------------------------------------------------------------------

/// A file of the list.
#[derive(Debug, Clone, PartialEq)]
pub struct FileEntry {
    /// Identifies it.
    pub value: String,
    /// The name.
    pub label: String,
    /// The icon (a symbol).
    pub icon: String,
    /// The kind ("Folder", "Text Document").
    pub kind: String,
    /// The date modified.
    pub date: String,
}

impl FileEntry {
    /// A file.
    pub fn new(value: &str, label: &str, icon: &str, kind: &str, date: &str) -> FileEntry {
        FileEntry {
            value: value.to_string(),
            label: label.to_string(),
            icon: icon.to_string(),
            kind: kind.to_string(),
            date: date.to_string(),
        }
    }
}

/// The file list of the open and save panels: a column header (Name, Kind, Date Modified),
/// 32 pt rows with every second row striped (the stripes carry on through the empty space),
/// hairlines above and below. One row is selected (`SelectionChanged` with its value); a
/// double click or Return on it emits `Activated` with the value. Up / Down / Home / End move
/// the selection; the wheel scrolls.
#[derive(Debug)]
pub struct FileBrowser {
    /// The files.
    pub files: Vec<FileEntry>,
    /// The selected file's value.
    pub selected: Option<String>,
    /// The height.
    pub height: f32,
    scroll: f32,
    hover: Option<usize>,
    transitions: Transitions,
    cache: Label,
}

impl FileBrowser {
    /// A list of `files`, 220 high.
    pub fn new(files: Vec<FileEntry>) -> FileBrowser {
        FileBrowser {
            files,
            selected: None,
            height: metrics::LIST_HEIGHT,
            scroll: 0.0,
            hover: None,
            transitions: Transitions::default(),
            cache: Label::default(),
        }
    }

    /// With a selection.
    pub fn select(mut self, value: &str) -> FileBrowser {
        self.selected = Some(value.to_string());
        self
    }

    fn viewport(&self, h: f32) -> f32 {
        (h - metrics::HEADER).max(0.0)
    }

    fn max_scroll(&self, h: f32) -> f32 {
        (self.files.len() as f32 * metrics::ROW - self.viewport(h)).max(0.0)
    }

    fn row_at(&self, p: Point, h: f32) -> Option<usize> {
        if p.y < metrics::HEADER || p.y >= h {
            return None;
        }
        let i = ((p.y - metrics::HEADER + self.scroll) / metrics::ROW).floor();
        (i >= 0.0 && (i as usize) < self.files.len()).then_some(i as usize)
    }

    fn index_of_selected(&self) -> Option<usize> {
        let s = self.selected.as_ref()?;
        self.files.iter().position(|f| &f.value == s)
    }

    fn choose(&mut self, cx: &mut EventCx<'_>, i: usize) {
        let Some(f) = self.files.get(i) else { return };
        let v = f.value.clone();
        // Keep it in view.
        let h = cx.size().height;
        let top = i as f32 * metrics::ROW;
        if top < self.scroll {
            self.scroll = top;
        } else if top + metrics::ROW > self.scroll + self.viewport(h) {
            self.scroll = (top + metrics::ROW - self.viewport(h)).max(0.0);
        }
        if self.selected.as_deref() != Some(&v) {
            self.selected = Some(v.clone());
            cx.emit(Signal::SelectionChanged, Value::Str(v));
        }
        cx.request_paint();
    }
}

impl Widget for FileBrowser {
    fn kind(&self) -> &'static str {
        "FileBrowser"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let w = p
            .width
            .filter(|w| w.is_finite())
            .unwrap_or(metrics::PANEL_WIDTH.0 - metrics::SIDEBAR);
        Size::new(w.max(0.0), self.height.max(metrics::HEADER))
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let r = cx.bounds();
        cx.rec
            .layer("Surface", r, Shape::RECT, Style::fill(c.surface_raised));
        // The list: stripes first, clipped to the viewport.
        let view = Rect::new(r.x, r.y + metrics::HEADER, r.width, self.viewport(r.height));
        cx.rec.push_clip(view, Shape::RECT);
        let first = (self.scroll / metrics::ROW).floor().max(0.0) as usize;
        let last = ((self.scroll + view.height) / metrics::ROW).ceil() as usize + 1;
        let stripe = c.line_separator.with_opacity(0.55);
        for i in first..last {
            let y = view.y + i as f32 * metrics::ROW - self.scroll;
            let row = Rect::new(r.x, y, r.width, metrics::ROW);
            if i % 2 == 1 {
                cx.rec
                    .layer("Stripe", row, Shape::RECT, Style::fill(stripe));
            }
            let Some(f) = self.files.get(i).cloned() else {
                continue;
            };
            let selected = self.selected.as_deref() == Some(&f.value);
            let inner = Rect::new(row.x + 8.0, row.y, row.width - 16.0, row.height);
            let active = cx.window_active();
            let (label, icon) = if selected {
                let (fill, label, icon) = if active {
                    (c.accent_amalfi_soft, c.text_primary, c.accent_amalfi)
                } else {
                    let fill = if theme.dark() {
                        c.surface_control
                    } else {
                        c.surface_control_pressed
                    };
                    (fill, c.text_secondary, c.text_secondary)
                };
                cx.rec.layer(
                    "Selection",
                    inner,
                    Shape::rounded(sidebars::metrics::ROW_RADIUS),
                    Style::fill(fill),
                );
                (label, icon)
            } else {
                if self.hover == Some(i) {
                    cx.rec.layer(
                        "Hover",
                        inner,
                        Shape::rounded(sidebars::metrics::ROW_RADIUS),
                        Style::fill(c.line_separator).with_opacity(self.transitions.hover()),
                    );
                }
                (c.text_primary, c.text_secondary)
            };
            cx.rec.glyph(
                "Icon",
                Rect::new(inner.x + 12.0, inner.y + 6.0, 20.0, 20.0),
                &f.icon,
                400,
                icon,
            );
            let (nx, kx, dx) = metrics::COLUMNS;
            let mut put =
                |cache: &mut Label, name: &str, text: &str, x: f32, w: f32, col: Color| {
                    let style = theme.text(&typography::BODY, col);
                    let layout = cache.layout(text, &style, Align::Start, w.max(0.0), Some(1));
                    cx.rec.text(
                        name,
                        Rect::new(r.x + x, row.y + 7.0, w.max(0.0), 18.0),
                        layout,
                        Some(typography::BODY.name),
                        style.color,
                    );
                };
            put(&mut self.cache, "Name", &f.label, nx, kx - nx - 8.0, label);
            put(
                &mut self.cache,
                "Kind",
                &f.kind,
                kx,
                dx - kx - 8.0,
                c.text_secondary,
            );
            put(
                &mut self.cache,
                "Date",
                &f.date,
                dx,
                r.width - dx - 8.0,
                c.text_secondary,
            );
        }
        cx.rec.pop();
        // The header and the hairlines.
        let head = Rect::new(r.x, r.y, r.width, metrics::HEADER);
        cx.rec
            .layer("Header", head, Shape::RECT, Style::fill(c.surface_raised));
        let (nx, kx, dx) = metrics::COLUMNS;
        for (name, text, x) in [
            ("Name", "Name", nx),
            ("Kind", "Kind", kx),
            ("Date", "Date Modified", dx),
        ] {
            let style = theme.text(&typography::CAPTION, c.text_secondary);
            let layout = Label::default().layout(
                text,
                &style,
                Align::Start,
                (r.width - x).max(0.0),
                Some(1),
            );
            cx.rec.text(
                &format!("Header {name}"),
                Rect::new(r.x + x, r.y + 6.0, (r.width - x).max(0.0), 14.0),
                layout,
                Some(typography::CAPTION.name),
                style.color,
            );
        }
        for (name, y) in [
            ("Header rule", r.y + metrics::HEADER - 1.0),
            ("Top rule", r.y),
            ("Bottom rule", r.bottom() - 1.0),
        ] {
            cx.rec.layer(
                name,
                Rect::new(r.x, y, r.width, 1.0),
                Shape::RECT,
                Style::fill(c.line_separator),
            );
        }
        if cx.focus_visible() && !cx.is_disabled() {
            focus_halo_named(cx.rec, "Focus Halo", r, Shape::RECT, &theme);
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let h = cx.size().height;
        match event {
            Event::PointerDown {
                pos,
                button: Button::Left,
                clicks,
                ..
            } => {
                if let Some(i) = self.row_at(*pos, h) {
                    self.choose(cx, i);
                    cx.focus_self();
                    if *clicks >= 2
                        && let Some(f) = self.files.get(i)
                    {
                        cx.emit(Signal::Activated, Value::Str(f.value.clone()));
                    }
                }
                Handled::Yes
            }
            Event::PointerMove { pos, .. } => {
                let i = self.row_at(*pos, h);
                if i != self.hover {
                    self.hover = i;
                    cx.request_paint();
                }
                Handled::No
            }
            Event::PointerLeave => {
                self.hover = None;
                cx.request_paint();
                Handled::No
            }
            Event::Scroll { dy, .. } => {
                let max = self.max_scroll(h);
                let to = (self.scroll + dy).clamp(0.0, max);
                if (to - self.scroll).abs() > f32::EPSILON {
                    self.scroll = to;
                    cx.request_paint();
                    return Handled::Yes;
                }
                Handled::No
            }
            Event::Key(k) if k.pressed => {
                let n = self.files.len();
                let cur = self.index_of_selected();
                let to = match k.key {
                    Key::Down => Some(cur.map_or(0, |i| (i + 1).min(n.saturating_sub(1)))),
                    Key::Up => Some(cur.map_or(0, |i| i.saturating_sub(1))),
                    Key::Home => Some(0),
                    Key::End => Some(n.saturating_sub(1)),
                    Key::Enter => {
                        if let Some(f) = cur.and_then(|i| self.files.get(i)) {
                            cx.emit(Signal::Activated, Value::Str(f.value.clone()));
                            return Handled::Yes;
                        }
                        return Handled::No;
                    }
                    _ => return Handled::No,
                };
                if let Some(i) = to.filter(|_| n > 0) {
                    self.choose(cx, i);
                }
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn state_changed(&mut self, cx: &mut EventCx<'_>, _old: NodeFlags, new: NodeFlags) {
        let reduce = cx.theme().appearance.reduce_motion;
        if self.transitions.update(new, false, reduce) {
            cx.animate();
        }
    }

    fn tick(&mut self, _cx: &mut EventCx<'_>, dt: f32) -> bool {
        self.transitions.tick(dt)
    }

    fn interaction(&self) -> Interaction {
        Interaction::Hover
    }

    fn focus(&self) -> FocusKind {
        FocusKind::Always
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::Text => match value.as_str() {
                Some(t) => {
                    self.selected = Some(t.to_string()).filter(|s| !s.is_empty());
                    PropChange::Paint
                }
                None => PropChange::Unknown,
            },
            Prop::Height => match value.as_f32() {
                Some(h) if h > 0.0 => {
                    self.height = h;
                    PropChange::Layout
                }
                _ => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Text => Some(Value::from(self.selected.clone().unwrap_or_default())),
            Prop::RowCount => Some(Value::Int(self.files.len() as i64)),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::List;
        a.label = Some("Files".to_string());
        a.value.clone_from(&self.selected);
        a.actions = vec![Action::Focus];
    }
}

/// Makes a file list.
pub fn file_browser(ui: &mut Ui, files: Vec<FileEntry>, selected: Option<&str>) -> ViewId {
    let mut b = FileBrowser::new(files);
    b.selected = selected.map(str::to_string);
    ui.create(b)
}

// ---------------------------------------------------------------------------------------------
// The open and save panel
// ---------------------------------------------------------------------------------------------

/// Which panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// A save panel.
    #[default]
    Save,
    /// An open panel.
    Open,
}

/// A group of places: its title and (value, label, icon) rows.
pub type PlaceGroup = (String, Vec<(String, String, String)>);

/// What a file panel shows.
#[derive(Debug, Clone)]
pub struct FilePanelSpec {
    /// Save or open.
    pub mode: Mode,
    /// Expanded (the browser shows) or the save panel's compact form.
    pub expanded: bool,
    /// The title ("Save").
    pub title: String,
    /// The "Save As:" name.
    pub name: String,
    /// The places: (group title, [(value, label, icon)]).
    pub places: Vec<PlaceGroup>,
    /// The locations of the pop-up.
    pub locations: Vec<String>,
    /// The files.
    pub files: Vec<FileEntry>,
}

impl FilePanelSpec {
    /// A spec with nothing in it.
    pub fn new(mode: Mode) -> FilePanelSpec {
        FilePanelSpec {
            mode,
            expanded: true,
            title: String::new(),
            name: String::new(),
            places: Vec::new(),
            locations: Vec::new(),
            files: Vec::new(),
        }
    }
}

/// The panel: a raised dialog surface (radius 20, a border and the level 3 shadow) around its
/// two children, the places column and the main column. Emits `Activated` with `"primary"`,
/// `"secondary"` (Cancel), `"new-folder"`, `"back"` or `"forward"`, and `CloseRequested` on Escape.
#[derive(Debug)]
pub struct FilePanel {
    /// Save or open.
    pub mode: Mode,
    /// Expanded.
    pub expanded: bool,
}

impl FilePanel {
    fn compact(&self) -> bool {
        self.mode == Mode::Save && !self.expanded
    }
}

impl Widget for FilePanel {
    fn kind(&self) -> &'static str {
        "FilePanel"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let w = p
            .width
            .filter(|w| w.is_finite())
            .unwrap_or(if self.compact() {
                metrics::PANEL_WIDTH.1
            } else {
                metrics::PANEL_WIDTH.0
            });
        let mut h = 0.0f32;
        for c in cx.children() {
            h = h.max(cx.measure(c, Proposal::new(Some(w), None)).height);
        }
        Size::new(w.max(0.0), h)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        for c in cx.children() {
            let s = cx.measure(c, Proposal::new(Some(size.width), None));
            cx.place(
                c,
                Rect::new(0.0, 0.0, size.width, s.height.max(size.height)),
            );
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let mut style =
            Style::fill(c.surface_raised).with_border(Border::inside(c.line_border, 1.0));
        if let Some(s) = theme.shadow(3) {
            style.shadows.push(s);
        }
        cx.rec.layer(
            "Surface",
            cx.bounds(),
            Shape::rounded(metrics::PANEL_RADIUS),
            style,
        );
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        match event {
            Event::Key(k) if k.pressed && matches!(k.key, Key::Escape) => {
                cx.emit(Signal::CloseRequested, Value::None);
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Dialog;
        a.label = Some(
            if self.mode == Mode::Save {
                "Save"
            } else {
                "Open"
            }
            .to_string(),
        );
    }
}

/// The places column: `Surface/Sidebar` with a hairline at its right.
#[derive(Debug)]
struct PlacesColumn {
    top: f32,
}

impl Widget for PlacesColumn {
    fn kind(&self) -> &'static str {
        "PlacesColumn"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let mut h = self.top;
        for c in cx.children() {
            h += cx
                .measure(c, Proposal::new(Some(metrics::SIDEBAR - 16.0), None))
                .height;
        }
        Size::new(
            metrics::SIDEBAR,
            p.height.filter(|h| h.is_finite()).unwrap_or(h),
        )
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, _size: Size) {
        let mut y = self.top;
        for c in cx.children() {
            let s = cx.measure(c, Proposal::new(Some(metrics::SIDEBAR - 16.0), None));
            cx.place(c, Rect::new(8.0, y, metrics::SIDEBAR - 16.0, s.height));
            y += s.height;
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let c = cx.theme().colors;
        let r = cx.bounds();
        cx.rec
            .layer("Surface", r, Shape::RECT, Style::fill(c.surface_sidebar));
        cx.rec.layer(
            "Edge",
            Rect::new(r.right() - 1.0, r.y, 1.0, r.height),
            Shape::RECT,
            Style::fill(c.line_separator),
        );
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::List;
        a.label = Some("Places".to_string());
    }
}

/// The views of a built file panel.
#[derive(Debug, Clone)]
pub struct FilePanelParts {
    /// The panel.
    pub root: ViewId,
    /// The "Save As:" field (save panel).
    pub name: Option<ViewId>,
    /// The file list (expanded).
    pub browser: Option<ViewId>,
    /// The search field (expanded).
    pub search: Option<ViewId>,
    /// The location pop-up.
    pub location: ViewId,
    /// The primary button.
    pub primary: ViewId,
    /// The Cancel button.
    pub secondary: ViewId,
    /// The place rows, in order.
    pub places: Vec<ViewId>,
}

fn named(ui: &mut Ui, v: ViewId, name: &str) -> ViewId {
    let _ = ui.set(v, Prop::Identifier, Value::from(name));
    v
}

/// Makes the open or save panel of `spec`. The primary and Cancel buttons, "New Folder", the
/// navigation arrows and the compact / expanded switch all work: they emit on the panel, and
/// the owner builds the panel again at the new form on `ExpandedChanged`.
pub fn file_panel(ui: &mut Ui, spec: &FilePanelSpec) -> FilePanelParts {
    let root = ui.create(FilePanel {
        mode: spec.mode,
        expanded: spec.expanded,
    });
    let compact = spec.mode == Mode::Save && !spec.expanded;
    let row = ui.create(Stack::row(0.0).aligned(Alignment::TOP_LEADING));
    ui.add_child(root, row);
    let mut places = Vec::new();
    if !compact {
        let top = if spec.mode == Mode::Save { 48.0 } else { 12.0 };
        let col = ui.create(PlacesColumn { top });
        let _ = ui.set(col, Prop::MaxHeight, Value::from(f32::INFINITY));
        // The save form leaves the column's top 48 pt to the window's own controls.
        for (title, items) in &spec.places {
            let hd = ui.create(sidebars::Header::new(title));
            ui.add_child(col, hd);
            for (value, label, icon) in items {
                let r = sidebars::row(ui, icon, label, None, false);
                named(ui, r, value);
                ui.add_child(col, r);
                places.push(r);
            }
        }
        ui.add_child(row, col);
    }
    // The main column.
    let main = ui.create(Stack::column(12.0).aligned(Alignment::LEADING));
    let _ = ui.set(main, Prop::MaxWidth, Value::from(f32::INFINITY));
    let pad = if compact {
        Insets::new(14.0, 20.0, 20.0, 20.0)
    } else if spec.mode == Mode::Save {
        Insets::new(14.0, 0.0, 12.0, 0.0)
    } else {
        Insets::new(16.0, 0.0, 12.0, 0.0)
    };
    let _ = ui.set(main, Prop::Padding, Value::Insets(pad));
    let side = if compact { 0.0 } else { 16.0 };
    let inset = |ui: &mut Ui, v: ViewId| {
        if side > 0.0 {
            let _ = ui.set(
                v,
                Prop::Padding,
                Value::Insets(Insets::symmetric(side, 0.0)),
            );
        }
        v
    };
    let mut name = None;
    if spec.mode == Mode::Save {
        let t = ui.create(
            Text::new(if spec.title.is_empty() {
                "Save"
            } else {
                &spec.title
            })
            .token(&typography::BODY_EMPHASIS)
            .layer("Title"),
        );
        let v = inset(ui, t);
        ui.add_child(main, v);
        let f = ui.create(TextField::text().with_text(&spec.name));
        let _ = ui.set(f, Prop::Width, Value::from(204.0));
        name = Some(f);
        let tags = ui.create(TextField::text());
        let _ = ui.set(tags, Prop::Width, Value::from(204.0));
        let a = form_row(ui, "Save As:", f, None, false);
        let b = form_row(ui, "Tags:", tags, None, false);
        let fields = ui.create(Stack::column(8.0).aligned(Alignment::LEADING));
        ui.add_child(fields, a);
        ui.add_child(fields, b);
        if compact {
            let wr = ui.create(Stack::row(8.0));
            let loc = location_menu(spec);
            let loc = ui.create(loc);
            let more = ui.create(IconButton::new("chevron-down").square());
            let _ = ui.set(
                more,
                Prop::AccessibilityLabel,
                Value::from("Show more options"),
            );
            ui.add_child(wr, loc);
            ui.add_child(wr, more);
            let w = form_row(ui, "Where:", wr, None, false);
            ui.add_child(fields, w);
            ui.connect(more, Signal::Activated, move |ui, _, _| {
                set_expanded(ui, root, true)
            });
        }
        let v = inset(ui, fields);
        ui.add_child(main, v);
    }
    let loc_view;
    let mut browser = None;
    let mut search = None;
    if compact {
        loc_view = ui
            .tree
            .children(
                ui.tree
                    .children(
                        ui.tree
                            .children(main)
                            .iter()
                            .copied()
                            .last()
                            .unwrap_or(main),
                    )
                    .first()
                    .copied()
                    .unwrap_or(main),
            )
            .first()
            .copied()
            .unwrap_or(main);
    } else {
        let bar = ui.create(Stack::row(12.0));
        let nav = ui.create(ActionSegments::new(vec![
            Segment::icon("chevron-left", "Back"),
            Segment::icon("chevron-right", "Forward"),
        ]));
        let views = ui.create(SegmentedControl::new(vec![
            Segment::icon("square-grid-2x2", "Icons"),
            Segment::icon("list-bullet", "List"),
            Segment::icon("gallery", "Gallery"),
        ]));
        let _ = ui.set(views, Prop::SelectedIndex, Value::Int(1));
        let loc = ui.create(location_menu(spec));
        let _ = ui.set(loc, Prop::LayoutPriority, Value::from(1.0));
        loc_view = loc;
        let s = ui.create(TextField::search());
        let _ = ui.set(s, Prop::Width, Value::from(108.0));
        search = Some(s);
        ui.add_child(bar, nav);
        ui.add_child(bar, views);
        ui.add_child(bar, loc);
        if spec.mode == Mode::Save {
            let less = ui.create(IconButton::new("chevron-up").square());
            let _ = ui.set(less, Prop::AccessibilityLabel, Value::from("Hide options"));
            ui.connect(less, Signal::Activated, move |ui, _, _| {
                set_expanded(ui, root, false)
            });
            ui.add_child(bar, less);
        }
        ui.add_child(bar, s);
        let v = inset(ui, bar);
        ui.add_child(main, v);
        ui.connect(nav, Signal::Activated, move |ui, _, v| {
            let which = if v.as_int() == Some(0) {
                "back"
            } else {
                "forward"
            };
            ui.emit(root, Signal::Activated, Value::from(which));
        });
        let b = ui.create(FileBrowser::new(spec.files.clone()));
        let _ = ui.set(b, Prop::MaxWidth, Value::from(f32::INFINITY));
        ui.add_child(main, b);
        browser = Some(b);
        // The search field filters the list.
        let all = spec.files.clone();
        ui.connect(s, Signal::TextChanged, move |ui, _, v| {
            let q = v.as_str().unwrap_or("").to_lowercase();
            let _ = ui.with_widget::<FileBrowser, _>(b, |fb| {
                fb.files = all
                    .iter()
                    .filter(|f| q.is_empty() || f.label.to_lowercase().contains(&q))
                    .cloned()
                    .collect();
            });
        });
    }
    // The footer.
    let footer = ui.create(Stack::row(8.0));
    if spec.mode == Mode::Save && !compact {
        let nf = ui.create(PushButton::new("New Folder").variant(PushVariant::Secondary));
        named(ui, nf, "New Folder");
        ui.connect(nf, Signal::Activated, move |ui, _, _| {
            ui.emit(root, Signal::Activated, Value::from("new-folder"));
        });
        ui.add_child(footer, nf);
    }
    let sp = ui.create(Spacer::default());
    ui.add_child(footer, sp);
    let secondary = ui.create(PushButton::new("Cancel").variant(PushVariant::Secondary));
    let primary = ui.create(
        PushButton::new(if spec.mode == Mode::Save {
            "Save"
        } else {
            "Open"
        })
        .variant(PushVariant::Primary),
    );
    named(ui, secondary, "Cancel");
    named(ui, primary, "Primary");
    ui.connect(secondary, Signal::Activated, move |ui, _, _| {
        ui.emit(root, Signal::Activated, Value::from("secondary"));
    });
    ui.connect(primary, Signal::Activated, move |ui, _, _| {
        ui.emit(root, Signal::Activated, Value::from("primary"));
    });
    ui.add_child(footer, secondary);
    ui.add_child(footer, primary);
    let v = inset(ui, footer);
    ui.add_child(main, v);
    // A double click or Return on a file opens it.
    if let Some(b) = browser {
        ui.connect(b, Signal::Activated, move |ui, _, _| {
            ui.emit(root, Signal::Activated, Value::from("primary"));
        });
    }
    ui.add_child(row, main);
    FilePanelParts {
        root,
        name,
        browser,
        search,
        location: loc_view,
        primary,
        secondary,
        places,
    }
}

fn location_menu(spec: &FilePanelSpec) -> MenuButton {
    let items: Vec<&str> = spec.locations.iter().map(String::as_str).collect();
    MenuButton::pop_up(&items, 0)
}

/// Expands or collapses a save panel (what the chevron beside "Where:" and the one in the
/// toolbar do): the panel records the form and emits `ExpandedChanged`, on which its owner
/// builds it again.
pub fn set_expanded(ui: &mut Ui, panel: ViewId, expanded: bool) {
    let changed = ui
        .with_widget::<FilePanel, _>(panel, |p| {
            let c = p.mode == Mode::Save && p.expanded != expanded;
            if c {
                p.expanded = expanded;
            }
            c
        })
        .unwrap_or(false);
    if changed {
        ui.emit(panel, Signal::ExpandedChanged, Value::Bool(expanded));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::window::{WindowId, WindowKind};
    use std::cell::RefCell;
    use std::rc::Rc;

    fn host(ui: &mut Ui, v: ViewId, w: f32, h: f32) -> WindowId {
        let win = ui.create_window(WindowKind::Normal, Size::new(w, h));
        ui.set_content(win, v);
        ui.show_window(win);
        ui.prepare_frame(win);
        win
    }

    fn click(ui: &mut Ui, win: WindowId, p: Point, clicks: u32) {
        ui.pointer_move(win, p, Mods::empty());
        let _ = clicks;
        ui.pointer_down(win, p, Button::Left, Mods::empty());
        ui.pointer_up(win, p, Button::Left, Mods::empty());
        ui.flush();
    }

    fn log_of(ui: &mut Ui, v: ViewId, s: Signal) -> Rc<RefCell<Vec<Value>>> {
        let log = Rc::new(RefCell::new(Vec::new()));
        let l = log.clone();
        ui.connect(v, s, move |_, _, val| l.borrow_mut().push(val.clone()));
        log
    }

    fn files() -> Vec<FileEntry> {
        vec![
            FileEntry::new("a", "Lemon cake.recipe", "doc", "Recipe", "Today"),
            FileEntry::new("b", "Focaccia.recipe", "doc", "Recipe", "Today"),
            FileEntry::new(
                "c",
                "Limoncello notes.txt",
                "doc",
                "Text Document",
                "Yesterday",
            ),
        ]
    }

    #[test]
    fn hsv_round_trips() {
        for c in [
            Color::rgb(14, 119, 230),
            Color::rgb(255, 0, 0),
            Color::rgb(0, 0, 0),
            Color::rgb(200, 200, 200),
        ] {
            let (h, s, v) = to_hsv(c);
            assert_eq!(from_hsv(h, s, v), c);
        }
        assert_eq!(from_hsv(f32::NAN, 2.0, -1.0), Color::rgb(0, 0, 0));
    }

    #[test]
    fn the_pad_follows_the_pointer_and_the_arrows() {
        let mut ui = Ui::new_headless();
        let pad = ui.create(ColorPad::new(Color::rgb(255, 0, 0)));
        let win = host(&mut ui, pad, 184.0, 96.0);
        let log = log_of(&mut ui, pad, Signal::ValueChanged);
        let r = ui.tree.window_rect(pad);
        click(&mut ui, win, Point::new(r.x + r.width - 0.5, r.y + 0.5), 1);
        let Some(Value::Color(top)) = log.borrow().last().cloned() else {
            panic!("no colour")
        };
        assert!(top.r > 250 && top.g < 5 && top.b < 5);
        click(&mut ui, win, Point::new(r.x, r.y + r.height - 1.0), 1);
        let Some(Value::Color(c)) = log.borrow().last().cloned() else {
            panic!("no colour")
        };
        assert!(c.r < 10 && c.g < 10 && c.b < 10);
        ui.set_focus(pad, true);
        ui.key(
            win,
            crate::core::event::KeyEvent::press(Key::Up, Mods::SHIFT),
        );
        let n = log.borrow().len();
        assert!(n >= 3);
    }

    #[test]
    fn the_picker_follows_a_swatch_the_hex_field_and_the_hue() {
        let mut ui = Ui::new_headless();
        let rows = vec![vec![
            ("Red".to_string(), Color::rgb(255, 0, 0)),
            ("Blue".to_string(), Color::rgb(0, 0, 255)),
        ]];
        let p = color_picker(&mut ui, &rows, Color::rgb(255, 0, 0), false);
        let win = host(&mut ui, p, 300.0, 400.0);
        let log = log_of(&mut ui, p, Signal::ValueChanged);
        let size = ui.measure(p, Proposal::UNSPECIFIED);
        assert_eq!(size.width, 192.0);
        // A swatch.
        let chip = *ui
            .tree
            .children(ui.tree.children(p)[1])
            .get(1)
            .expect("blue");
        let c = ui.tree.window_rect(chip).center();
        click(&mut ui, win, c, 1);
        assert_eq!(
            log.borrow().last(),
            Some(&Value::Color(Color::rgb(0, 0, 255)))
        );
        assert_eq!(
            ui.get(p, Prop::ColorValue),
            Some(Value::Color(Color::rgb(0, 0, 255)))
        );
        // The hex field.
        let row = *ui.tree.children(p).last().expect("hex row");
        let hex = ui.tree.children(row)[1];
        ui.emit(hex, Signal::TextChanged, Value::from("#00FF00"));
        ui.flush();
        assert_eq!(
            ui.get(p, Prop::ColorValue),
            Some(Value::Color(Color::rgb(0, 255, 0)))
        );
        // An incomplete one changes nothing.
        ui.emit(hex, Signal::TextChanged, Value::from("#00F"));
        ui.flush();
        assert_eq!(
            ui.get(p, Prop::ColorValue),
            Some(Value::Color(Color::rgb(0, 255, 0)))
        );
        // Swatches only.
        let q = color_picker(&mut ui, &rows, Color::rgb(0, 0, 255), true);
        assert_eq!(ui.tree.children(q).len(), 2);
    }

    #[test]
    fn the_list_selects_moves_and_opens() {
        let mut ui = Ui::new_headless();
        let b = file_browser(&mut ui, files(), None);
        let win = host(&mut ui, b, 560.0, 220.0);
        let sel = log_of(&mut ui, b, Signal::SelectionChanged);
        let open = log_of(&mut ui, b, Signal::Activated);
        let r = ui.tree.window_rect(b);
        // The second row.
        click(
            &mut ui,
            win,
            Point::new(r.x + 100.0, r.y + 28.0 + 32.0 + 16.0),
            1,
        );
        assert_eq!(*sel.borrow(), vec![Value::from("b")]);
        ui.key(
            win,
            crate::core::event::KeyEvent::press(Key::Down, Mods::empty()),
        );
        assert_eq!(sel.borrow().last(), Some(&Value::from("c")));
        ui.key(
            win,
            crate::core::event::KeyEvent::press(Key::Enter, Mods::empty()),
        );
        ui.flush();
        assert_eq!(*open.borrow(), vec![Value::from("c")]);
        // The header and the empty space select nothing.
        let n = sel.borrow().len();
        click(&mut ui, win, Point::new(r.x + 100.0, r.y + 10.0), 1);
        click(&mut ui, win, Point::new(r.x + 100.0, r.y + 200.0), 1);
        assert_eq!(sel.borrow().len(), n);
        assert_eq!(ui.accessibility_of(b).map(|a| a.role), Some(Role::List));
    }

    #[test]
    fn the_list_scrolls_and_survives_odd_sizes() {
        let mut ui = Ui::new_headless();
        let many: Vec<FileEntry> = (0..30)
            .map(|i| FileEntry::new(&i.to_string(), &format!("File {i}"), "doc", "Doc", "Today"))
            .collect();
        let b = file_browser(&mut ui, many, Some("0"));
        let win = host(&mut ui, b, 560.0, 220.0);
        ui.scroll(
            win,
            Point::new(100.0, 100.0),
            0.0,
            100.0,
            true,
            crate::core::event::Phase::Changed,
        );
        for (w, h) in [(0.0, 0.0), (1.0, 1.0), (1e6, 1e6)] {
            let s = ui.measure(b, Proposal::new(Some(w), Some(h)));
            assert!(s.width.is_finite() && s.height.is_finite());
        }
        let e = file_browser(&mut ui, Vec::new(), None);
        let _ = host(&mut ui, e, 560.0, 220.0);
    }

    fn spec(mode: Mode, expanded: bool) -> FilePanelSpec {
        let mut s = FilePanelSpec::new(mode);
        s.expanded = expanded;
        s.name = "Untitled".to_string();
        s.locations = vec!["Recipes".to_string(), "Documents".to_string()];
        s.files = files();
        s.places = vec![(
            "Favourites".to_string(),
            vec![(
                "docs".to_string(),
                "Documents".to_string(),
                "folder".to_string(),
            )],
        )];
        s
    }

    #[test]
    fn the_save_panel_buttons_work() {
        let mut ui = Ui::new_headless();
        let parts = file_panel(&mut ui, &spec(Mode::Save, true));
        let win = host(&mut ui, parts.root, 720.0, 600.0);
        let log = log_of(&mut ui, parts.root, Signal::Activated);
        let c = ui.tree.window_rect(parts.primary).center();
        click(&mut ui, win, c, 1);
        let c = ui.tree.window_rect(parts.secondary).center();
        click(&mut ui, win, c, 1);
        assert_eq!(
            *log.borrow(),
            vec![Value::from("primary"), Value::from("secondary")]
        );
        let size = ui.measure(parts.root, Proposal::UNSPECIFIED);
        assert_eq!(size.width, 720.0);
        assert!(parts.browser.is_some() && parts.name.is_some());
        assert_eq!(
            ui.accessibility_of(parts.root).map(|a| a.role),
            Some(Role::Dialog)
        );
    }

    #[test]
    fn the_open_panel_has_no_fields_and_a_double_click_opens() {
        let mut ui = Ui::new_headless();
        let parts = file_panel(&mut ui, &spec(Mode::Open, true));
        assert!(parts.name.is_none());
        let log = log_of(&mut ui, parts.root, Signal::Activated);
        let b = parts.browser.expect("browser");
        ui.emit(b, Signal::Activated, Value::from("a"));
        ui.flush();
        assert_eq!(*log.borrow(), vec![Value::from("primary")]);
    }

    #[test]
    fn the_compact_save_panel_has_no_browser_and_expands() {
        let mut ui = Ui::new_headless();
        let parts = file_panel(&mut ui, &spec(Mode::Save, false));
        assert!(parts.browser.is_none());
        let size = ui.measure(parts.root, Proposal::UNSPECIFIED);
        assert_eq!(size.width, 440.0);
        let log = log_of(&mut ui, parts.root, Signal::ExpandedChanged);
        set_expanded(&mut ui, parts.root, true);
        ui.flush();
        assert_eq!(*log.borrow(), vec![Value::Bool(true)]);
    }
}
