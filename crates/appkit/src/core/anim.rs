//! Implicit animations of views: a view with an animation moves to its new place smoothly
//! when its layout changes (the layout is final at once; the drawing slides from the old
//! position, FLIP-style), and a view with a transition fades (or fades and scales) in when
//! added and out when removed (it stays drawn, out of layout and hit testing, until the
//! fade ends). Reduce Motion turns springs into the quick cross-fade and removes the scale.

use crate::amalfi::tokens::motion;
use crate::core::tree::{NodeFlags, ViewId};
use crate::core::ui::Ui;
use crate::geom::{Point, Rect};
use crate::motion::{Animated, Timing};

/// A window's content motion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Presentation {
    Popover,
    Sheet,
}

/// How a view enters and leaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Transition {
    /// Appears and disappears at once.
    #[default]
    None,
    /// Fades.
    Opacity,
    /// Fades and scales from 95 %.
    Scale,
}

impl Transition {
    /// The transition of an ABI number.
    pub fn from_int(n: i64) -> Option<Transition> {
        Some(match n {
            0 => Transition::None,
            1 => Transition::Opacity,
            2 => Transition::Scale,
            _ => return None,
        })
    }
}

/// The timing of an ABI number (`AK_PROP_ANIMATION`): 0 none, 1–5 the motion springs
/// (snappy, panel, window, morph, bouncy), 6–9 the fades (instant, quick, standard, slow).
pub fn timing_from_int(n: i64) -> Option<Option<Timing>> {
    use crate::motion::Timing as T;
    Some(match n {
        0 => None,
        1 => Some(T::Spring(motion::SPRING_SNAPPY)),
        2 => Some(T::Spring(motion::SPRING_PANEL)),
        3 => Some(T::Spring(motion::SPRING_WINDOW)),
        4 => Some(T::Spring(motion::SPRING_MORPH)),
        5 => Some(T::Spring(motion::SPRING_BOUNCY)),
        6 => Some(T::curve(motion::FADE_INSTANT_CURVE, motion::FADE_INSTANT)),
        7 => Some(T::curve(motion::FADE_QUICK_CURVE, motion::FADE_QUICK)),
        8 => Some(T::curve(motion::FADE_STANDARD_CURVE, motion::FADE_STANDARD)),
        9 => Some(T::curve(motion::FADE_SLOW_CURVE, motion::FADE_SLOW)),
        _ => return None,
    })
}

/// The animated drawing state of a view.
#[derive(Debug, Clone, Copy)]
pub struct NodeAnim {
    /// Offset from the laid-out position.
    pub offset: Animated<Point>,
    /// Opacity multiplier.
    pub opacity: Animated<f32>,
    /// Scale multiplier.
    pub scale: Animated<f32>,
}

impl Default for NodeAnim {
    fn default() -> Self {
        NodeAnim {
            offset: Animated::new(Point::ZERO),
            opacity: Animated::new(1.0),
            scale: Animated::new(1.0),
        }
    }
}

impl NodeAnim {
    fn tick(&mut self, dt: f32) -> bool {
        let a = self.offset.tick(dt);
        let b = self.opacity.tick(dt);
        let c = self.scale.tick(dt);
        a || b || c
    }
}

impl Ui {
    fn motion_timing(&self, t: Timing) -> Timing {
        if self.appearance.reduce_motion {
            match t {
                Timing::Spring(_) => Timing::curve(motion::FADE_QUICK_CURVE, motion::FADE_QUICK),
                other => other,
            }
        } else {
            t
        }
    }

    fn shown(&self, id: ViewId) -> bool {
        self.window_of(id)
            .and_then(|w| self.window(w))
            .is_some_and(|w| w.visible)
    }

    fn anim_mut(&mut self, id: ViewId) -> Option<&mut NodeAnim> {
        let n = self.tree.get_mut(id)?;
        Some(n.anim.get_or_insert_with(Box::default))
    }

    fn track(&mut self, id: ViewId) {
        if !self.node_anims.contains(&id) {
            self.node_anims.push(id);
        }
        if let Some(w) = self.window_of(id) {
            self.request_frame(w);
        }
    }

    /// Scales the view about its centre to `target` with `spring.snappy` (held-down growth:
    /// icon buttons 1.05, segmented controls 1.035, back to 1 on release). Reduce Motion: no
    /// scale.
    pub fn grow(&mut self, id: ViewId, target: f32) {
        if self.appearance.reduce_motion && target != 1.0 {
            return;
        }
        let t = self.motion_timing(Timing::Spring(motion::SPRING_SNAPPY));
        let Some(a) = self.anim_mut(id) else { return };
        if (a.scale.target() - target).abs() <= f32::EPSILON {
            return;
        }
        a.scale.animate_to(target, t);
        self.track(id);
    }

    /// Ends every held-down growth at once (the kit's pressed masters show none: the parity
    /// tests call this before they compare).
    pub fn clear_growth(&mut self) {
        let mut ids = self.node_anims.clone();
        let roots: Vec<ViewId> = self.windows().map(|w| w.root).collect();
        for r in roots {
            ids.push(r);
            ids.extend(self.tree.descendants(r));
        }
        for id in ids {
            if let Some(a) = self.tree.get_mut(id).and_then(|n| n.anim.as_mut()) {
                a.scale.jump(1.0);
            }
            self.request_paint(id);
        }
    }

    /// The view moved from `old` to `new` (parent coordinates).
    pub(crate) fn animate_move(&mut self, id: ViewId, old: Rect, new: Rect) {
        let Some(t) = self.tree.get(id).and_then(|n| n.modifiers.animation) else {
            return;
        };
        if old.is_empty() || !self.shown(id) || self.appearance.reduce_motion {
            return;
        }
        let t = self.motion_timing(t);
        let Some(a) = self.anim_mut(id) else { return };
        let d = old.origin() - new.origin();
        let current = a.offset.get();
        a.offset.jump(current + d);
        a.offset.animate_to(Point::ZERO, t);
        self.track(id);
    }

    /// The view was added: its transition plays in.
    pub(crate) fn animate_insert(&mut self, id: ViewId) {
        let Some(n) = self.tree.get(id) else { return };
        let tr = n.modifiers.transition;
        if tr == Transition::None || !self.shown(id) {
            return;
        }
        let t = self.motion_timing(n.modifiers.animation.unwrap_or(Timing::curve(
            motion::FADE_STANDARD_CURVE,
            motion::FADE_STANDARD,
        )));
        let scale = tr == Transition::Scale && !self.appearance.reduce_motion;
        let Some(a) = self.anim_mut(id) else { return };
        a.opacity.jump(0.0);
        a.opacity.animate_to(1.0, t);
        if scale {
            a.scale.jump(0.95);
            a.scale.animate_to(1.0, t);
        }
        self.track(id);
    }

    /// The view is being removed: true when its transition plays out first (the caller then
    /// leaves it in the tree; [`Ui::advance`] removes it at the end).
    pub(crate) fn begin_leave(&mut self, id: ViewId) -> bool {
        let Some(n) = self.tree.get(id) else {
            return false;
        };
        let tr = n.modifiers.transition;
        if tr == Transition::None || n.flags.contains(NodeFlags::LEAVING) || !self.shown(id) {
            return false;
        }
        let t = self.motion_timing(n.modifiers.animation.unwrap_or(Timing::curve(
            motion::FADE_INSTANT_CURVE,
            motion::FADE_INSTANT,
        )));
        let scale = tr == Transition::Scale && !self.appearance.reduce_motion;
        if let Some(n) = self.tree.get_mut(id) {
            n.flags.insert(NodeFlags::LEAVING);
        }
        if let Some(a) = self.anim_mut(id) {
            a.opacity.animate_to(0.0, t);
            if scale {
                a.scale.animate_to(0.95, t);
            }
        }
        self.track(id);
        if let Some(p) = self.tree.parent(id) {
            self.request_layout(p);
        }
        true
    }

    /// The motion a window's content plays, if it has one (the ui-kit's `SPopover`, `SDialog`
    /// and `SWindow` sheet transitions): popovers fade in with `fade.instant` while scaling
    /// from 0.96 with `spring.panel`; sheets fade in with `fade.standard` while moving down 20 pt
    /// with `spring.panel`. Both leave with a `fade.instant` fade. Reduce Motion: a
    /// `fade.quick` fade, no scale or move. Menus, pop-up lists and tooltips have their own.
    fn presentation(
        &self,
        window: crate::core::window::WindowId,
    ) -> Option<(ViewId, Presentation)> {
        use crate::core::window::WindowKind;
        let w = self.window(window)?;
        let root = w.root;
        match w.kind {
            WindowKind::Sheet { .. } => Some((root, Presentation::Sheet)),
            WindowKind::Popup { .. } => {
                let first = self.tree.children(root).first().copied()?;
                let popover = self
                    .tree
                    .widget(first)
                    .is_some_and(|w| w.kind() == "Popover");
                popover.then_some((root, Presentation::Popover))
            }
            _ => None,
        }
    }

    /// Plays the enter motion of a window that was just shown.
    pub(crate) fn animate_present(&mut self, window: crate::core::window::WindowId) {
        let Some((root, kind)) = self.presentation(window) else {
            return;
        };
        let reduce = self.appearance.reduce_motion;
        let fade = match (reduce, kind) {
            (true, _) => Timing::curve(motion::FADE_QUICK_CURVE, motion::FADE_QUICK),
            (false, Presentation::Popover) => {
                Timing::curve(motion::FADE_INSTANT_CURVE, motion::FADE_INSTANT)
            }
            (false, Presentation::Sheet) => {
                Timing::curve(motion::FADE_STANDARD_CURVE, motion::FADE_STANDARD)
            }
        };
        let Some(a) = self.anim_mut(root) else { return };
        a.opacity.jump(0.0);
        a.opacity.animate_to(1.0, fade);
        if !reduce {
            let spring = Timing::Spring(motion::SPRING_PANEL);
            match kind {
                Presentation::Popover => {
                    a.scale.jump(0.96);
                    a.scale.animate_to(1.0, spring);
                }
                Presentation::Sheet => {
                    a.offset.jump(Point::new(0.0, -20.0));
                    a.offset.animate_to(Point::ZERO, spring);
                }
            }
        }
        self.track(root);
    }

    /// Closes a window the way the ui-kit leaves: a popover or sheet fades out first (the
    /// platform's own dismissals, which have no surface left to animate, use
    /// [`Ui::close_window`]). Other windows close at once.
    pub fn dismiss_window(&mut self, window: crate::core::window::WindowId) {
        let Some((root, _)) = self.presentation(window) else {
            self.close_window(window);
            return;
        };
        let reduce = self.appearance.reduce_motion;
        let (curve, dur) = if reduce {
            (motion::FADE_QUICK_CURVE, motion::FADE_QUICK)
        } else {
            (motion::FADE_INSTANT_CURVE, motion::FADE_INSTANT)
        };
        if let Some(a) = self.anim_mut(root) {
            a.opacity.animate_to(0.0, Timing::curve(curve, dur));
        }
        self.track(root);
        self.after(f64::from(dur) + 0.02, move |ui| ui.close_window(window));
    }

    /// Advances the views' implicit animations; removes views whose leave transition ended.
    pub(crate) fn tick_node_anims(&mut self, dt: f32) {
        let list = std::mem::take(&mut self.node_anims);
        let mut still = Vec::new();
        let mut done_leaving = Vec::new();
        for id in list {
            let Some(n) = self.tree.get_mut(id) else {
                continue;
            };
            let leaving = n.flags.contains(NodeFlags::LEAVING);
            let more = n.anim.as_mut().is_some_and(|a| a.tick(dt));
            self.request_paint(id);
            if more {
                still.push(id);
            } else if leaving {
                done_leaving.push(id);
            }
        }
        for id in std::mem::take(&mut self.node_anims) {
            if !still.contains(&id) {
                still.push(id);
            }
        }
        self.node_anims = still;
        for id in done_leaving {
            self.detach_now(id);
        }
    }

    /// Whether views animate implicitly.
    pub fn has_node_anims(&self) -> bool {
        !self.node_anims.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::layout::Stack;
    use crate::core::value::{Prop, Value};
    use crate::core::widget::Empty;
    use crate::core::window::WindowKind;
    use crate::geom::Size;

    fn boxed(ui: &mut Ui) -> ViewId {
        let v = ui.create(Empty);
        ui.set(v, Prop::Width, Value::from(20.0)).unwrap();
        ui.set(v, Prop::Height, Value::from(20.0)).unwrap();
        v
    }

    fn anim_of(ui: &Ui, window: crate::core::window::WindowId) -> (f32, f32, Point) {
        let root = ui.window(window).map(|w| w.root);
        root.and_then(|r| ui.tree.get(r))
            .and_then(|n| n.anim.as_ref())
            .map_or((1.0, 1.0, Point::ZERO), |a| {
                (a.opacity.get(), a.scale.get(), a.offset.get())
            })
    }

    fn parent(ui: &mut Ui) -> crate::core::window::WindowId {
        let win = ui.create_window(WindowKind::Normal, Size::new(400.0, 300.0));
        ui.show_window(win);
        win
    }

    #[test]
    fn a_popover_fades_in_and_scales_from_96_percent() {
        let mut ui = Ui::new_headless();
        let win = parent(&mut ui);
        let content = ui.create(crate::amalfi::text::Text::new("Hello"));
        let pop = crate::amalfi::popovers::present(
            &mut ui,
            win,
            Rect::new(100.0, 40.0, 40.0, 20.0),
            content,
            crate::amalfi::popovers::Side::Below,
            16.0,
        );
        let (o, k, _) = anim_of(&ui, pop);
        assert_eq!((o, k), (0.0, 0.96));
        ui.advance(0.05);
        let (o, k, _) = anim_of(&ui, pop);
        assert!(o > 0.0 && k > 0.96, "{o} {k}");
        for _ in 0..120 {
            ui.advance(1.0 / 60.0);
        }
        assert_eq!(anim_of(&ui, pop), (1.0, 1.0, Point::ZERO));
    }

    #[test]
    fn a_sheet_fades_in_moving_down_20_points() {
        let mut ui = Ui::new_headless();
        let win = parent(&mut ui);
        let sheet = ui.create_window(WindowKind::Sheet { parent: win }, Size::new(200.0, 100.0));
        let v = boxed(&mut ui);
        ui.set_content(sheet, v);
        ui.show_window(sheet);
        assert_eq!(anim_of(&ui, sheet), (0.0, 1.0, Point::new(0.0, -20.0)));
        ui.advance(0.05);
        let (o, _, off) = anim_of(&ui, sheet);
        assert!(o > 0.0 && off.y > -20.0 && off.y < 0.0, "{o} {off:?}");
        for _ in 0..120 {
            ui.advance(1.0 / 60.0);
        }
        assert_eq!(anim_of(&ui, sheet), (1.0, 1.0, Point::ZERO));
    }

    #[test]
    fn dismissing_fades_out_in_place_and_then_closes() {
        let mut ui = Ui::new_headless();
        let win = parent(&mut ui);
        let sheet = ui.create_window(WindowKind::Sheet { parent: win }, Size::new(200.0, 100.0));
        let v = boxed(&mut ui);
        ui.set_content(sheet, v);
        ui.show_window(sheet);
        for _ in 0..120 {
            ui.advance(1.0 / 60.0);
        }
        ui.dismiss_window(sheet);
        ui.advance(0.05);
        let (o, _, off) = anim_of(&ui, sheet);
        assert!(o < 1.0 && off == Point::ZERO, "{o} {off:?}");
        assert!(ui.window(sheet).is_some(), "still there while it fades");
        ui.advance(0.2);
        assert!(ui.window(sheet).is_none());
        // A normal window closes at once.
        ui.dismiss_window(win);
        assert!(ui.window(win).is_none());
    }

    #[test]
    fn with_reduce_motion_a_sheet_only_fades() {
        let mut ui = Ui::new_headless();
        let mut a = ui.appearance;
        a.reduce_motion = true;
        ui.set_appearance(a);
        let win = parent(&mut ui);
        let sheet = ui.create_window(WindowKind::Sheet { parent: win }, Size::new(200.0, 100.0));
        let v = boxed(&mut ui);
        ui.set_content(sheet, v);
        ui.show_window(sheet);
        assert_eq!(anim_of(&ui, sheet), (0.0, 1.0, Point::ZERO));
        ui.advance(0.2);
        assert_eq!(anim_of(&ui, sheet).0, 1.0);
    }

    #[test]
    fn grow_scales_with_the_snappy_spring_and_not_with_reduce_motion() {
        let mut ui = Ui::new_headless();
        let w = ui.create_window(WindowKind::Normal, Size::new(100.0, 100.0));
        let v = boxed(&mut ui);
        ui.set_content(w, v);
        ui.show_window(w);
        ui.prepare_frame(w);
        ui.grow(v, 1.05);
        let target = |ui: &Ui| {
            ui.tree
                .get(v)
                .and_then(|n| n.anim.as_ref())
                .map(|a| a.scale.target())
        };
        assert_eq!(target(&ui), Some(1.05));
        ui.grow(v, 1.0);
        assert_eq!(target(&ui), Some(1.0));
        ui.set_appearance(crate::core::env::Appearance {
            reduce_motion: true,
            ..crate::core::env::Appearance::default()
        });
        ui.grow(v, 1.05);
        assert_eq!(target(&ui), Some(1.0));
    }

    #[test]
    fn moves_slide_and_removals_fade() {
        let mut ui = Ui::new_headless();
        let w = ui.create_window(WindowKind::Normal, Size::new(200.0, 100.0));
        let row = ui.create(Stack::row(10.0));
        let a = boxed(&mut ui);
        let b = boxed(&mut ui);
        ui.set(b, Prop::Animation, Value::Int(3)).unwrap();
        ui.set(a, Prop::Transition, Value::Int(1)).unwrap();
        ui.add_child(row, a);
        ui.add_child(row, b);
        ui.set_content(w, row);
        ui.show_window(w);
        ui.prepare_frame(w);
        let bx = ui.tree.window_rect(b).x;
        // Removing `a` moves `b` left; `a` fades out first.
        ui.detach(a);
        assert_eq!(ui.tree.parent(a), Some(row), "still drawn while it fades");
        ui.prepare_frame(w);
        assert!(ui.tree.window_rect(b).x < bx);
        let off = ui
            .tree
            .get(b)
            .and_then(|n| n.anim.as_ref())
            .map(|x| x.offset.get().x)
            .unwrap();
        assert!(off > 0.0, "b slides from where it was");
        for _ in 0..120 {
            ui.advance(1.0 / 60.0);
        }
        assert_eq!(ui.tree.parent(a), None, "a removed after its fade");
        let off = ui
            .tree
            .get(b)
            .and_then(|n| n.anim.as_ref())
            .map(|x| x.offset.get().x)
            .unwrap();
        assert_eq!(off, 0.0);
    }
}
