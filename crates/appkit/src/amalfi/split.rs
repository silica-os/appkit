//! A window split into a full-height sidebar and its content (ui-kit `SWindow` with a split
//! toolbar): each column has its own section of the toolbar, the window controls stay put at
//! the leading edge, and the sidebar folds away by its width on `spring.panel` (Reduce Motion:
//! at once). Folded, the content's section makes room for the controls and a reveal button
//! fades in (`fade.quick`); the sidebar's inner edge is a grip that resizes it (200–360) and
//! folds it when pulled 56 under the minimum. Control-Command-S toggles it.
//!
//! The children, in order: the sidebar column (its toolbar section first), the content column
//! (its toolbar section first), the reveal button and the grip.

use crate::amalfi::tokens;
use crate::amalfi::toolbar::{self, Toolbar};
use crate::core::a11y::{A11y, Role};
use crate::core::event::{Button, Event, Handled, Key, Mods};
use crate::core::layout::Proposal;
use crate::core::tree::ViewId;
use crate::core::ui::{Cursor, Ui};
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, LayoutCx, PropChange, Widget};
use crate::geom::{Rect, Size};
use crate::motion::{Animated, Timing};

/// Measures of the split window (ui-kit `SWindow`).
pub mod metrics {
    /// The sidebar's width when nothing sets one.
    pub const SIDEBAR_WIDTH: f32 = 240.0;
    /// The narrowest the grip makes the sidebar.
    pub const MIN_WIDTH: f32 = 200.0;
    /// The widest the grip makes the sidebar.
    pub const MAX_WIDTH: f32 = 360.0;
    /// How far under the minimum the grip is pulled before the sidebar folds.
    pub const FOLD_PULL: f32 = 56.0;
    /// The grip's width, on the sidebar's inner edge.
    pub const GRIP: f32 = 8.0;
    /// The reveal button's slot in the content's section.
    pub const REVEAL: f32 = 44.0;
}

/// The split window.
#[derive(Debug)]
pub struct SplitWindow {
    /// The sidebar is shown.
    pub open: bool,
    /// The sidebar's width when shown.
    pub width: f32,
    fold: Animated<f32>,
    reveal: Animated<f32>,
    /// The grip's drag: the pointer's x and the width when it started.
    drag: Option<(f32, f32)>,
    /// The grip set the resize cursor.
    cursor: bool,
}

impl Default for SplitWindow {
    fn default() -> Self {
        SplitWindow::new(true)
    }
}

impl SplitWindow {
    /// A split window with its sidebar shown or folded.
    pub fn new(open: bool) -> SplitWindow {
        let f = if open { 1.0 } else { 0.0 };
        SplitWindow {
            open,
            width: metrics::SIDEBAR_WIDTH,
            fold: Animated::new(f),
            reveal: Animated::new(1.0 - f),
            drag: None,
            cursor: false,
        }
    }

    /// Shows or folds the sidebar, on `spring.panel`; the reveal button fades on `fade.quick`.
    pub fn set_open(&mut self, open: bool) {
        self.open = open;
        let f = if open { 1.0 } else { 0.0 };
        self.fold
            .animate_to(f, Timing::Spring(tokens::motion::SPRING_PANEL));
        self.reveal.animate_to(
            1.0 - f,
            Timing::curve(tokens::motion::FADE_QUICK_CURVE, tokens::motion::FADE_QUICK),
        );
    }

    /// The sidebar's visible width.
    fn shown_width(&self) -> f32 {
        (self.width * self.fold.get()).max(0.0)
    }

    fn grip_hit(&self, x: f32) -> bool {
        let s = self.shown_width();
        self.open && x >= s - metrics::GRIP && x <= s
    }

    fn toggle(&mut self, cx: &mut EventCx<'_>) {
        let open = !self.open;
        self.set_open(open);
        cx.emit(Signal::ExpandedChanged, Value::Bool(open));
        cx.animate();
        cx.request_layout();
    }
}

fn set_if(ui: &mut Ui, id: ViewId, prop: Prop, value: Value) {
    if ui.get(id, prop) != Some(value.clone()) {
        let _ = ui.set(id, prop, value);
    }
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
        // By position, hidden ones too (`cx.children()` skips them).
        let kids = cx.ui.tree.children(cx.id()).to_vec();
        let f = self.fold.get();
        let s = self.shown_width();
        if let Some(&side) = kids.first() {
            // Folded away it leaves the key loop and the hit tests (the kit's `inert`).
            let gone = !self.open && !self.fold.is_animating();
            set_if(cx.ui, side, Prop::Hidden, Value::Bool(gone));
            // The column keeps its width and slides under the content's edge: nothing in it
            // reflows while it goes.
            cx.measure(side, Proposal::new(Some(self.width), Some(size.height)));
            cx.place(
                side,
                Rect::new(s - self.width, 0.0, self.width, size.height),
            );
        }
        if let Some(&main) = kids.get(1) {
            let bar = cx.ui.tree.children(main).first().copied();
            if let Some(t) = bar
                .and_then(|b| cx.ui.tree.widget_mut(b))
                .and_then(|t| t.as_any_mut().downcast_mut::<Toolbar>())
            {
                // The content's section moves its pieces past the controls and the reveal slot
                // as the sidebar goes.
                let room =
                    toolbar::metrics::REGION_CONTROLS + metrics::REVEAL - toolbar::metrics::LEADING;
                t.controls = false;
                t.inset = (room * (1.0 - f)).max(0.0);
            }
            if let Some(b) = bar {
                cx.ui.request_layout(b);
            }
            let w = (size.width - s).max(0.0);
            cx.measure(main, Proposal::new(Some(w), Some(size.height)));
            cx.place(main, Rect::new(s, 0.0, w, size.height));
        }
        if let Some(&reveal) = kids.get(2) {
            let o = self.reveal.get().clamp(0.0, 1.0);
            set_if(cx.ui, reveal, Prop::Hidden, Value::Bool(o <= 0.0));
            set_if(cx.ui, reveal, Prop::Opacity, Value::from(o));
            let b = cx.measure(reveal, Proposal::new(None, Some(toolbar::metrics::HEIGHT)));
            let x = toolbar::metrics::REGION_CONTROLS + ((metrics::REVEAL - b.width) / 2.0).round();
            let y = ((toolbar::metrics::HEIGHT - b.height) / 2.0).round();
            cx.place(reveal, Rect::new(x, y, b.width, b.height));
        }
        if let Some(&grip) = kids.get(3) {
            let r = if self.open {
                Rect::new(s - metrics::GRIP, 0.0, metrics::GRIP, size.height)
            } else {
                Rect::new(0.0, 0.0, 0.0, 0.0)
            };
            cx.measure(grip, Proposal::new(Some(r.width), Some(r.height)));
            cx.place(grip, r);
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        match event {
            Event::Key(k)
                if k.pressed
                    && k.key == Key::Char('s')
                    && (k.mods - Mods::CAPS) == Mods::CTRL | Mods::LOGO =>
            {
                self.toggle(cx);
                Handled::Yes
            }
            Event::PointerDown {
                pos,
                button: Button::Left,
                ..
            } if self.grip_hit(pos.x) => {
                self.drag = Some((pos.x, self.width));
                cx.set_cursor(Cursor::ColResize);
                Handled::Yes
            }
            Event::PointerMove { pos, dragging, .. } => {
                if let (true, Some((x0, w0))) = (*dragging, self.drag) {
                    let raw = w0 + pos.x - x0;
                    if raw < metrics::MIN_WIDTH - metrics::FOLD_PULL {
                        self.drag = None;
                        self.toggle(cx);
                    } else {
                        let w = raw.clamp(metrics::MIN_WIDTH, metrics::MAX_WIDTH).round();
                        if w != self.width {
                            self.width = w;
                            cx.emit(Signal::ValueChanged, Value::from(w));
                            cx.request_layout();
                        }
                    }
                    return Handled::Yes;
                }
                let over = self.grip_hit(pos.x);
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
            let f = if self.open { 1.0 } else { 0.0 };
            self.fold.jump(f);
        }
        let a = self.fold.tick(dt);
        let b = self.reveal.tick(dt);
        cx.request_layout();
        cx.request_paint();
        a || b
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
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Expanded => Some(Value::Bool(self.open)),
            Prop::Value => Some(Value::from(self.width)),
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
}
