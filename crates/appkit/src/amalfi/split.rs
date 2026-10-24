//! A window split into a full-height sidebar, its content and, optionally, an inspector (ui-kit
//! `SWindow` with a split toolbar): each column has its own section of the toolbar, the window
//! controls stay put at the leading edge, and the side panes fold away by their width on
//! `spring.panel` (Reduce Motion: at once). Folded, the content's section makes room for a
//! reveal button, which fades in (`fade.quick`): the sidebar's after the controls, the
//! inspector's at the end. A pane's inner edge is a grip that resizes it (sidebar 200–360,
//! inspector 240–400) and folds it when pulled 56 under its minimum. Control-Command-S toggles
//! the sidebar, Option-Command-I the inspector.
//!
//! The children, in order: the sidebar column (its toolbar section first), the content column
//! (its toolbar section first), the reveal button and the grip; then, for an inspector, its
//! column (its toolbar section first), its reveal button and its grip.

use crate::amalfi::tokens;
use crate::amalfi::toolbar::{self, Toolbar};
use crate::core::a11y::{A11y, Role};
use crate::core::event::{Button, Event, Handled, Key, Mods};
use crate::core::layout::Proposal;
use crate::core::tree::ViewId;
use crate::core::ui::{Cursor, Ui};
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Rect, Size};
use crate::motion::{Animated, Timing};
use crate::shape::Shape;
use crate::style::Style;

/// Measures of the split window (ui-kit `SWindow`).
pub mod metrics {
    /// The sidebar's width when nothing sets one.
    pub const SIDEBAR_WIDTH: f32 = 240.0;
    /// The narrowest the grip makes the sidebar.
    pub const MIN_WIDTH: f32 = 200.0;
    /// The widest the grip makes the sidebar.
    pub const MAX_WIDTH: f32 = 360.0;
    /// The inspector's width when nothing sets one.
    pub const INSPECTOR_WIDTH: f32 = 260.0;
    /// The narrowest the grip makes the inspector.
    pub const INSPECTOR_MIN: f32 = 240.0;
    /// The widest the grip makes the inspector.
    pub const INSPECTOR_MAX: f32 = 400.0;
    /// How far under the minimum a grip is pulled before its pane folds.
    pub const FOLD_PULL: f32 = 56.0;
    /// A grip's width, on its pane's inner edge.
    pub const GRIP: f32 = 8.0;
    /// A reveal button's slot in the content's section.
    pub const REVEAL: f32 = 44.0;
    /// The content section's end padding, after the inspector's reveal slot.
    pub const SECTION_END: f32 = crate::amalfi::tokens::spacing::S;
}

/// A side pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pane {
    Sidebar,
    Inspector,
}

/// The split window.
#[derive(Debug)]
pub struct SplitWindow {
    /// The sidebar is shown.
    pub open: bool,
    /// The sidebar's width when shown.
    pub width: f32,
    /// The inspector is shown (when the window has one).
    pub inspector_open: bool,
    /// The inspector's width when shown.
    pub inspector_width: f32,
    fold: Animated<f32>,
    reveal: Animated<f32>,
    inspector_fold: Animated<f32>,
    inspector_reveal: Animated<f32>,
    /// The window has an inspector (its column is a child).
    has_inspector: bool,
    /// The size it was placed at.
    size: Size,
    /// A grip's drag: its pane, the pointer's x and the width when it started.
    drag: Option<(Pane, f32, f32)>,
    /// A grip set the resize cursor.
    cursor: bool,
}

impl Default for SplitWindow {
    fn default() -> Self {
        SplitWindow::new(true)
    }
}

/// Starts a pane's fold on `spring.panel` and its reveal button's fade on `fade.quick`.
fn fold_to(fold: &mut Animated<f32>, reveal: &mut Animated<f32>, open: bool) {
    let f = if open { 1.0 } else { 0.0 };
    fold.animate_to(f, Timing::Spring(tokens::motion::SPRING_PANEL));
    reveal.animate_to(
        1.0 - f,
        Timing::curve(tokens::motion::FADE_QUICK_CURVE, tokens::motion::FADE_QUICK),
    );
}

impl SplitWindow {
    /// A split window with its sidebar shown or folded (and its inspector, if any, shown).
    pub fn new(open: bool) -> SplitWindow {
        let f = if open { 1.0 } else { 0.0 };
        SplitWindow {
            open,
            width: metrics::SIDEBAR_WIDTH,
            inspector_open: true,
            inspector_width: metrics::INSPECTOR_WIDTH,
            fold: Animated::new(f),
            reveal: Animated::new(1.0 - f),
            inspector_fold: Animated::new(1.0),
            inspector_reveal: Animated::new(0.0),
            has_inspector: false,
            size: Size::new(0.0, 0.0),
            drag: None,
            cursor: false,
        }
    }

    /// Shows or folds the sidebar, on `spring.panel`; the reveal button fades on `fade.quick`.
    pub fn set_open(&mut self, open: bool) {
        self.open = open;
        fold_to(&mut self.fold, &mut self.reveal, open);
    }

    /// Shows or folds the inspector, as [`SplitWindow::set_open`] does the sidebar.
    pub fn set_inspector_open(&mut self, open: bool) {
        self.inspector_open = open;
        fold_to(&mut self.inspector_fold, &mut self.inspector_reveal, open);
    }

    /// The sidebar's visible width.
    fn shown_width(&self) -> f32 {
        (self.width * self.fold.get()).max(0.0)
    }

    /// The inspector's visible width (none without one).
    fn inspector_shown(&self) -> f32 {
        if self.has_inspector {
            (self.inspector_width * self.inspector_fold.get()).max(0.0)
        } else {
            0.0
        }
    }

    /// The pane whose grip is under `x`.
    fn grip_hit(&self, x: f32) -> Option<Pane> {
        let s = self.shown_width();
        let i = self.size.width - self.inspector_shown();
        if self.open && x >= s - metrics::GRIP && x <= s {
            Some(Pane::Sidebar)
        } else if self.has_inspector && self.inspector_open && x >= i && x <= i + metrics::GRIP {
            Some(Pane::Inspector)
        } else {
            None
        }
    }

    fn toggle(&mut self, cx: &mut EventCx<'_>, pane: Pane) {
        match pane {
            Pane::Sidebar => {
                let open = !self.open;
                self.set_open(open);
                cx.emit(Signal::ExpandedChanged, Value::Bool(open));
            }
            Pane::Inspector => {
                let open = !self.inspector_open;
                self.set_inspector_open(open);
                cx.emit(Signal::InspectorExpandedChanged, Value::Bool(open));
            }
        }
        cx.animate();
        cx.request_layout();
    }

    /// Places a side pane's column, keeping its width so nothing in it reflows while it
    /// folds: the sidebar slides under the content's leading edge, the inspector out past the
    /// window's trailing edge.
    fn place_pane(&self, cx: &mut LayoutCx<'_>, column: ViewId, pane: Pane, size: Size) {
        let (open, fold, width, x) = match pane {
            Pane::Sidebar => {
                let s = self.shown_width();
                (self.open, &self.fold, self.width, s - self.width)
            }
            Pane::Inspector => (
                self.inspector_open,
                &self.inspector_fold,
                self.inspector_width,
                size.width - self.inspector_shown(),
            ),
        };
        // Folded away it leaves the key loop and the hit tests (the kit's `inert`).
        let gone = !open && !fold.is_animating();
        set_if(cx.ui, column, Prop::Hidden, Value::Bool(gone));
        cx.measure(column, Proposal::new(Some(width), Some(size.height)));
        cx.place(column, Rect::new(x, 0.0, width, size.height));
    }

    /// Places a reveal button centred in its slot, at `x`, fading with `reveal`.
    fn place_reveal(cx: &mut LayoutCx<'_>, button: ViewId, x: f32, reveal: &Animated<f32>) {
        let o = reveal.get().clamp(0.0, 1.0);
        set_if(cx.ui, button, Prop::Hidden, Value::Bool(o <= 0.0));
        set_if(cx.ui, button, Prop::Opacity, Value::from(o));
        let b = cx.measure(button, Proposal::new(None, Some(toolbar::metrics::HEIGHT)));
        let x = x + ((metrics::REVEAL - b.width) / 2.0).round();
        let y = ((toolbar::metrics::HEIGHT - b.height) / 2.0).round();
        cx.place(button, Rect::new(x, y, b.width, b.height));
    }

    /// Places a grip at `r` while its pane is shown.
    fn place_grip(cx: &mut LayoutCx<'_>, grip: ViewId, open: bool, r: Rect) {
        let r = if open {
            r
        } else {
            Rect::new(0.0, 0.0, 0.0, 0.0)
        };
        cx.measure(grip, Proposal::new(Some(r.width), Some(r.height)));
        cx.place(grip, r);
    }
}

fn set_if(ui: &mut Ui, id: ViewId, prop: Prop, value: Value) {
    if ui.get(id, prop) != Some(value.clone()) {
        let _ = ui.set(id, prop, value);
    }
}

/// The section of a column's toolbar (its first child), if it is one.
fn section(ui: &mut Ui, column: ViewId) -> Option<(ViewId, &mut Toolbar)> {
    let bar = ui.tree.children(column).first().copied()?;
    let t = ui
        .tree
        .widget_mut(bar)?
        .as_any_mut()
        .downcast_mut::<Toolbar>()?;
    Some((bar, t))
}

impl Widget for SplitWindow {
    fn kind(&self) -> &'static str {
        "SplitWindow"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let fin = |v: Option<f32>, d: f32| v.filter(|v| v.is_finite()).unwrap_or(d);
        Size::new(fin(p.width, 900.0), fin(p.height, 600.0))
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        self.size = size;
        // By position, hidden ones too (`cx.children()` skips them).
        let kids = cx.ui.tree.children(cx.id()).to_vec();
        if self.has_inspector != (kids.len() > 4) {
            // The inspector's surface and line are this view's.
            self.has_inspector = kids.len() > 4;
            let id = cx.id();
            cx.ui.request_paint(id);
        }
        let s = self.shown_width();
        let si = self.inspector_shown();
        let w = (size.width - s - si).max(0.0);
        if let Some(&side) = kids.first() {
            self.place_pane(cx, side, Pane::Sidebar, size);
        }
        if let Some(&main) = kids.get(1) {
            let f = self.fold.get();
            let fi = if self.has_inspector {
                self.inspector_fold.get()
            } else {
                1.0
            };
            if let Some((bar, t)) = section(cx.ui, main) {
                // The content's section moves its pieces past the controls and the reveal
                // slots as the panes go.
                let room =
                    toolbar::metrics::REGION_CONTROLS + metrics::REVEAL - toolbar::metrics::LEADING;
                let end = metrics::REVEAL + metrics::SECTION_END - toolbar::metrics::TRAILING;
                t.controls = false;
                t.inset = (room * (1.0 - f)).max(0.0);
                t.end = (end * (1.0 - fi)).max(0.0);
                cx.ui.request_layout(bar);
            }
            cx.measure(main, Proposal::new(Some(w), Some(size.height)));
            cx.place(main, Rect::new(s, 0.0, w, size.height));
        }
        if let Some(&reveal) = kids.get(2) {
            Self::place_reveal(cx, reveal, toolbar::metrics::REGION_CONTROLS, &self.reveal);
        }
        if let Some(&grip) = kids.get(3) {
            let r = Rect::new(s - metrics::GRIP, 0.0, metrics::GRIP, size.height);
            Self::place_grip(cx, grip, self.open, r);
        }
        if let Some(&inspector) = kids.get(4) {
            if let Some((bar, t)) = section(cx.ui, inspector) {
                t.controls = false;
                t.bare = true;
                cx.ui.request_layout(bar);
            }
            self.place_pane(cx, inspector, Pane::Inspector, size);
        }
        if let Some(&reveal) = kids.get(5) {
            let x = s + w - metrics::SECTION_END - metrics::REVEAL;
            Self::place_reveal(cx, reveal, x, &self.inspector_reveal);
        }
        if let Some(&grip) = kids.get(6) {
            let r = Rect::new(size.width - si, 0.0, metrics::GRIP, size.height);
            Self::place_grip(cx, grip, self.inspector_open, r);
        }
    }

    // The inspector's surface under its column, and the line on its inner edge over it.
    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let si = self.inspector_shown();
        if si > 0.0 {
            let r = cx.bounds();
            let c = cx.theme().colors;
            cx.rec.layer(
                "Inspector",
                Rect::new(r.right() - si, r.y, si, r.height),
                Shape::RECT,
                Style::fill(c.surface_sidebar),
            );
        }
    }

    fn paint_over(&mut self, cx: &mut PaintCx<'_>) {
        let si = self.inspector_shown();
        if si > 0.0 {
            let r = cx.bounds();
            let c = cx.theme().colors;
            cx.rec.layer(
                "Inspector Separator",
                Rect::new(r.right() - si, r.y, 1.0, r.height),
                Shape::RECT,
                Style::fill(c.line_separator),
            );
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        match event {
            Event::Key(k)
                if k.pressed
                    && k.key == Key::Char('s')
                    && (k.mods - Mods::CAPS) == Mods::CTRL | Mods::LOGO =>
            {
                self.toggle(cx, Pane::Sidebar);
                Handled::Yes
            }
            Event::Key(k)
                if k.pressed
                    && self.has_inspector
                    && matches!(k.key, Key::Char('i' | 'I'))
                    && (k.mods - Mods::CAPS) == Mods::ALT | Mods::LOGO =>
            {
                self.toggle(cx, Pane::Inspector);
                Handled::Yes
            }
            Event::PointerDown {
                pos,
                button: Button::Left,
                ..
            } => match self.grip_hit(pos.x) {
                Some(pane) => {
                    let w = match pane {
                        Pane::Sidebar => self.width,
                        Pane::Inspector => self.inspector_width,
                    };
                    self.drag = Some((pane, pos.x, w));
                    cx.set_cursor(Cursor::ColResize);
                    Handled::Yes
                }
                None => Handled::No,
            },
            Event::PointerMove { pos, dragging, .. } => {
                if let (true, Some((pane, x0, w0))) = (*dragging, self.drag) {
                    let (raw, min, max) = match pane {
                        Pane::Sidebar => (w0 + pos.x - x0, metrics::MIN_WIDTH, metrics::MAX_WIDTH),
                        Pane::Inspector => (
                            w0 - (pos.x - x0),
                            metrics::INSPECTOR_MIN,
                            metrics::INSPECTOR_MAX,
                        ),
                    };
                    if raw < min - metrics::FOLD_PULL {
                        self.drag = None;
                        self.toggle(cx, pane);
                    } else {
                        let w = raw.clamp(min, max).round();
                        let (current, signal) = match pane {
                            Pane::Sidebar => (&mut self.width, Signal::ValueChanged),
                            Pane::Inspector => {
                                (&mut self.inspector_width, Signal::InspectorResized)
                            }
                        };
                        if w != *current {
                            *current = w;
                            cx.emit(signal, Value::from(w));
                            cx.request_layout();
                        }
                    }
                    return Handled::Yes;
                }
                let over = self.grip_hit(pos.x).is_some();
                if over != self.cursor {
                    self.cursor = over;
                    cx.set_cursor(if over {
                        Cursor::ColResize
                    } else {
                        Cursor::Default
                    });
                }
                Handled::No
            }
            Event::PointerUp { .. } if self.drag.is_some() => {
                self.drag = None;
                Handled::Yes
            }
            Event::PointerLeave if self.cursor => {
                self.cursor = false;
                cx.set_cursor(Cursor::Default);
                Handled::No
            }
            _ => Handled::No,
        }
    }

    fn tick(&mut self, cx: &mut EventCx<'_>, dt: f32) -> bool {
        if cx.theme().appearance.reduce_motion {
            let f = |open: bool| if open { 1.0 } else { 0.0 };
            self.fold.jump(f(self.open));
            self.inspector_fold.jump(f(self.inspector_open));
        }
        let a = self.fold.tick(dt);
        let b = self.reveal.tick(dt);
        let c = self.inspector_fold.tick(dt);
        let d = self.inspector_reveal.tick(dt);
        cx.request_layout();
        cx.request_paint();
        a || b || c || d
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::Expanded => match value.as_bool() {
                Some(b) if b != self.open => {
                    self.set_open(b);
                    PropChange::Layout
                }
                Some(_) => PropChange::Nothing,
                None => PropChange::Unknown,
            },
            Prop::Value => match value.as_f32() {
                Some(w) => {
                    self.width = w.clamp(metrics::MIN_WIDTH, metrics::MAX_WIDTH);
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::InspectorOpen => match value.as_bool() {
                Some(b) if b != self.inspector_open => {
                    self.set_inspector_open(b);
                    PropChange::Layout
                }
                Some(_) => PropChange::Nothing,
                None => PropChange::Unknown,
            },
            Prop::InspectorWidth => match value.as_f32() {
                Some(w) => {
                    self.inspector_width = w.clamp(metrics::INSPECTOR_MIN, metrics::INSPECTOR_MAX);
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Expanded => Some(Value::Bool(self.open)),
            Prop::Value => Some(Value::from(self.width)),
            Prop::InspectorOpen => Some(Value::Bool(self.inspector_open)),
            Prop::InspectorWidth => Some(Value::from(self.inspector_width)),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Group;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::layout::Stack;
    use crate::core::window::WindowKind;

    fn window(open: bool) -> (Ui, ViewId, [ViewId; 4], crate::core::window::WindowId) {
        let mut ui = Ui::new_headless();
        let split = ui.create(SplitWindow::new(open));
        let side = ui.create(Stack::column(0.0));
        let main = ui.create(Stack::column(0.0));
        let bar = ui.create(Toolbar::default());
        ui.add_child(main, bar);
        let reveal = ui.create(crate::amalfi::buttons::ToolbarButton::new("sidebar"));
        let grip = ui.create(Stack::column(0.0));
        for v in [side, main, reveal, grip] {
            ui.add_child(split, v);
        }
        let w = ui.create_window(WindowKind::Normal, Size::new(1000.0, 600.0));
        ui.set_content(w, split);
        ui.show_window(w);
        ui.prepare_frame(w);
        (ui, split, [side, main, reveal, grip], w)
    }

    #[test]
    fn the_sidebar_runs_the_full_height_beside_the_content() {
        let (ui, _, [side, main, _, grip], _) = window(true);
        let r = |v| ui.tree.window_rect(v);
        assert_eq!(r(side), Rect::new(0.0, 0.0, 240.0, 600.0));
        assert_eq!(r(main), Rect::new(240.0, 0.0, 760.0, 600.0));
        assert_eq!(r(grip), Rect::new(232.0, 0.0, 8.0, 600.0));
    }

    #[test]
    fn folding_hands_the_controls_room_to_the_content() {
        let (mut ui, split, [side, main, reveal, _], w) = window(true);
        ui.set(split, Prop::Expanded, Value::Bool(false)).unwrap();
        for _ in 0..120 {
            ui.advance(1.0 / 60.0);
            ui.prepare_frame(w);
        }
        assert_eq!(ui.tree.window_rect(main).x, 0.0);
        assert_eq!(ui.get(side, Prop::Hidden), Some(Value::Bool(true)));
        assert_eq!(ui.get(reveal, Prop::Hidden), Some(Value::Bool(false)));
        let bar = ui.tree.children(main)[0];
        let t = ui.widget::<Toolbar>(bar).map(|t| t.inset);
        assert_eq!(t, Some(126.0));
        assert!(ui.tree.window_rect(reveal).x >= 94.0);
    }

    /// A split window with an inspector: the four children of [`window`], then the
    /// inspector's column (with its toolbar section), its reveal button and its grip.
    fn with_inspector() -> (Ui, ViewId, [ViewId; 7], crate::core::window::WindowId) {
        let (mut ui, split, [side, main, reveal, grip], w) = window(true);
        let col = ui.create(Stack::column(0.0));
        let bar = ui.create(Toolbar::default());
        ui.add_child(col, bar);
        let show = ui.create(crate::amalfi::buttons::ToolbarButton::new("sidebar-right"));
        let igrip = ui.create(Stack::column(0.0));
        for v in [col, show, igrip] {
            ui.add_child(split, v);
        }
        ui.prepare_frame(w);
        (ui, split, [side, main, reveal, grip, col, show, igrip], w)
    }

    fn settle(ui: &mut Ui, w: crate::core::window::WindowId) {
        for _ in 0..120 {
            ui.advance(1.0 / 60.0);
            ui.prepare_frame(w);
        }
    }

    #[test]
    fn the_inspector_runs_the_full_height_at_the_end() {
        let (ui, split, [_, main, _, _, col, show, igrip], _) = with_inspector();
        let r = |v| ui.tree.window_rect(v);
        assert_eq!(r(main), Rect::new(240.0, 0.0, 500.0, 600.0));
        assert_eq!(r(col), Rect::new(740.0, 0.0, 260.0, 600.0));
        assert_eq!(r(igrip), Rect::new(740.0, 0.0, 8.0, 600.0));
        assert_eq!(ui.get(show, Prop::Hidden), Some(Value::Bool(true)));
        let bar = ui.tree.children(col)[0];
        assert_eq!(ui.widget::<Toolbar>(bar).map(|t| t.bare), Some(true));
        let layers = ui.inspect_layers(split);
        assert!(layers.iter().any(|l| l.name == "Inspector Separator"));
    }

    #[test]
    fn option_command_i_folds_the_inspector_and_reveals_its_button() {
        let (mut ui, split, [_, main, _, _, col, show, _], w) = with_inspector();
        // The keys come up from the focused view in the window.
        let field = ui.create(crate::amalfi::fields::TextField::new(Default::default()));
        ui.add_child(main, field);
        ui.prepare_frame(w);
        ui.set_focus(field, true);
        let key = crate::core::event::KeyEvent::press(Key::Char('i'), Mods::ALT | Mods::LOGO);
        ui.key(w, key);
        ui.flush();
        settle(&mut ui, w);
        assert_eq!(ui.get(split, Prop::InspectorOpen), Some(Value::Bool(false)));
        assert_eq!(ui.tree.window_rect(main).width, 760.0);
        assert_eq!(ui.get(col, Prop::Hidden), Some(Value::Bool(true)));
        assert_eq!(ui.get(show, Prop::Hidden), Some(Value::Bool(false)));
        // The content's section makes room for the button at its end.
        let bar = ui.tree.children(main)[0];
        assert_eq!(ui.widget::<Toolbar>(bar).map(|t| t.end), Some(40.0));
        let b = ui.tree.window_rect(show);
        assert!(
            b.right() <= 1000.0 - 8.0 && b.x >= 1000.0 - 8.0 - 44.0,
            "{b:?}"
        );
    }

    #[test]
    fn the_inspector_grip_resizes_and_folds_it() {
        let (mut ui, split, _, w) = with_inspector();
        let at = |x: f32| crate::geom::Point::new(x, 300.0);
        ui.pointer_move(w, at(744.0), Mods::empty());
        ui.pointer_down(w, at(744.0), Button::Left, Mods::empty());
        ui.pointer_move(w, at(644.0), Mods::empty());
        assert_eq!(
            ui.get(split, Prop::InspectorWidth),
            Some(Value::from(360.0))
        );
        ui.pointer_move(w, at(444.0), Mods::empty());
        assert_eq!(
            ui.get(split, Prop::InspectorWidth),
            Some(Value::from(400.0))
        );
        // Pulled 56 under the minimum it folds.
        ui.pointer_move(w, at(950.0), Mods::empty());
        assert_eq!(ui.get(split, Prop::InspectorOpen), Some(Value::Bool(false)));
        ui.set(split, Prop::InspectorWidth, Value::from(1000.0))
            .unwrap();
        assert_eq!(
            ui.get(split, Prop::InspectorWidth),
            Some(Value::from(400.0))
        );
    }
}
