//! Notifications (kit `Notifications`, ui-kit `SNotification`, `SNotificationStack` and
//! `SNotificationCentre`): the glass banner of the screen corner, the raised card inside the
//! notification centre, the grouped stack of one app's banners, and the centre panel.
//!
//! A notification is a 36 pt app icon tile, a title and a message of two lines at most, the
//! time, and a row of small buttons under the text. A close button shows while the pointer is
//! over it (or it has the focus). Entering fades in and the others glide (`spring.panel`).

use crate::amalfi::button::{PushButton, PushVariant};
use crate::amalfi::buttons::LinkButton;
use crate::amalfi::foundation::{ControlSize, Interaction as Transitions, focus_halo_named};
use crate::amalfi::materials::{Tier, glass_style};
use crate::amalfi::text::{Text, TextRole};
use crate::amalfi::tokens::typography;
use crate::color::Color;
use crate::core::a11y::{A11y, Action, Role};
use crate::core::event::{Event, Handled, Key};
use crate::core::layout::{Alignment, Proposal, Stack};
use crate::core::tree::{NodeFlags, ViewId};
use crate::core::ui::Ui;
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, FocusKind, Interaction, LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Insets, Point, Rect, Size};
use crate::shape::Shape;
use crate::style::{Border, Style};

/// Fixed measures of the kit.
pub mod metrics {
    /// A banner's width.
    pub const BANNER_WIDTH: f32 = 360.0;
    /// A card's width in the centre.
    pub const CARD_WIDTH: f32 = 340.0;
    /// The space between the edge and the content.
    pub const PADDING: f32 = 12.0;
    /// The space between the content and the actions.
    pub const GAP: f32 = 8.0;
    /// The banner's corner radius.
    pub const BANNER_RADIUS: f32 = 18.0;
    /// The card's corner radius.
    pub const CARD_RADIUS: f32 = 16.0;
    /// The app icon's size.
    pub const ICON: f32 = 36.0;
    /// The app icon placeholder's corner radius.
    pub const ICON_RADIUS: f32 = 9.0;
    /// The content block is this tall even with a one-line text.
    pub const CONTENT_MIN_HEIGHT: f32 = 52.0;
    /// The actions start under the text.
    pub const ACTIONS_INDENT: f32 = 44.0;
    /// The close button's diameter.
    pub const CLOSE: f32 = 20.0;
    /// How far the close button hangs out of the corner.
    pub const CLOSE_OUT: f32 = 8.0;
    /// How far the plates of a stack peek out under the front banner.
    pub const PEEK: f32 = 20.0;
    /// The plates' inset per level.
    pub const PLATE_INSET: f32 = 16.0;
    /// The plate's and banner's height of the collapsed stack.
    pub const PLATE_HEIGHT: f32 = 76.0;
    /// The notification centre's width.
    pub const CENTRE_WIDTH: f32 = 380.0;
    /// The centre's minimum height.
    pub const CENTRE_MIN_HEIGHT: f32 = 416.0;
    /// The centre's corner radius.
    pub const CENTRE_RADIUS: f32 = 24.0;
    /// The centre's padding.
    pub const CENTRE_PADDING: f32 = 16.0;
    /// The height of the centre's header row.
    pub const HEADER: f32 = 24.0;
    /// The height of a section title.
    pub const SECTION: f32 = 16.0;
}

/// Which surface a notification has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Variant {
    /// The glass banner of the screen corner.
    #[default]
    Banner,
    /// The raised card inside the notification centre.
    Panel,
}

/// What a notification says.
#[derive(Debug, Clone, PartialEq)]
pub struct NotificationSpec {
    /// Identifies it in the signals of a stack or the centre.
    pub id: String,
    /// The title.
    pub title: String,
    /// The message (two lines at most).
    pub message: String,
    /// The time ("now").
    pub time: String,
    /// The app icon tile's colour (`Text/Tertiary` when `None`).
    pub color: Option<Color>,
    /// The app icon tile's symbol.
    pub glyph: Option<String>,
    /// The labels of the action buttons.
    pub actions: Vec<String>,
    /// Whether it has a close button.
    pub closable: bool,
}

impl NotificationSpec {
    /// A notification with a title and a message, shown "now".
    pub fn new(id: &str, title: &str, message: &str) -> NotificationSpec {
        NotificationSpec {
            id: id.to_string(),
            title: title.to_string(),
            message: message.to_string(),
            time: "now".to_string(),
            color: None,
            glyph: None,
            actions: Vec::new(),
            closable: true,
        }
    }

    /// With action buttons.
    pub fn actions(mut self, labels: &[&str]) -> NotificationSpec {
        self.actions = labels.iter().map(|l| (*l).to_string()).collect();
        self
    }

    /// With an app icon tile.
    pub fn icon(mut self, color: Color, glyph: &str) -> NotificationSpec {
        self.color = Some(color);
        self.glyph = Some(glyph.to_string());
        self
    }

    /// With another time.
    pub fn time(mut self, time: &str) -> NotificationSpec {
        self.time = time.to_string();
        self
    }

    /// Without a close button.
    pub fn fixed(mut self) -> NotificationSpec {
        self.closable = false;
        self
    }
}

/// The app icon's placeholder (the kit's are bitmaps): a rounded square of a colour with a
/// white glyph.
#[derive(Debug, Clone)]
pub struct AppIcon {
    /// The colour (`Text/Tertiary` when `None`).
    pub color: Option<Color>,
    /// The symbol.
    pub glyph: Option<String>,
}

impl Widget for AppIcon {
    fn kind(&self) -> &'static str {
        "AppIcon"
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        Size::new(metrics::ICON, metrics::ICON)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let r = cx.bounds();
        let fill = self.color.unwrap_or(c.text_tertiary);
        let mut style =
            Style::fill(fill).with_border(Border::inside(Color::rgba(0, 0, 0, 0x14), 1.0));
        style.inner_shadows.push(crate::style::Shadow::new(
            Color::rgba(255, 255, 255, 0x40),
            0.0,
            1.0,
            0.0,
        ));
        cx.rec
            .layer("Tile", r, Shape::rounded(metrics::ICON_RADIUS), style);
        if let Some(g) = &self.glyph {
            let s = 20.0;
            cx.rec.glyph(
                "Glyph",
                Rect::new(r.x + (r.width - s) / 2.0, r.y + (r.height - s) / 2.0, s, s),
                g,
                400,
                c.text_on_accent,
            );
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.hidden = true;
    }
}

/// The invisible hit area of a notification's close button; the notification draws it.
#[derive(Debug)]
struct CloseHit {
    owner: ViewId,
}

impl Widget for CloseHit {
    fn kind(&self) -> &'static str {
        "NotificationClose"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        Size::new(metrics::CLOSE, metrics::CLOSE)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        if cx.focus_visible() {
            let theme = *cx.theme();
            let r = cx.bounds();
            focus_halo_named(cx.rec, "Close Focus Halo", r, Shape::Ellipse, &theme);
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        match event {
            Event::Activate => {
                cx.ui()
                    .emit(self.owner, Signal::CloseRequested, Value::None);
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn state_changed(&mut self, cx: &mut EventCx<'_>, old: NodeFlags, new: NodeFlags) {
        if old.contains(NodeFlags::FOCUSED) != new.contains(NodeFlags::FOCUSED) {
            let on = new.contains(NodeFlags::FOCUSED);
            let owner = self.owner;
            let _ = cx
                .ui()
                .with_widget::<Notification, _>(owner, |n| n.close_focused = on);
        }
    }

    fn interaction(&self) -> Interaction {
        Interaction::Button
    }

    fn focus(&self) -> FocusKind {
        FocusKind::Control
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Button;
        a.label = Some("Close".to_string());
        a.actions = vec![Action::Click, Action::Focus];
    }
}

/// A notification: the surface (glass banner or raised card), laid out as a padded column of
/// its children (the content row, the actions row), with a close button hanging out of its
/// top left corner when it has one. A click on it emits `Activated`; the close button emits
/// `CloseRequested`.
#[derive(Debug)]
pub struct Notification {
    /// The surface.
    pub variant: Variant,
    /// The title, for assistive technologies.
    pub title: String,
    close: Option<ViewId>,
    close_focused: bool,
    transitions: Transitions,
}

impl Notification {
    /// A notification surface.
    pub fn new(variant: Variant) -> Notification {
        Notification {
            variant,
            title: String::new(),
            close: None,
            close_focused: false,
            transitions: Transitions::default(),
        }
    }

    fn radius(&self) -> f32 {
        match self.variant {
            Variant::Banner => metrics::BANNER_RADIUS,
            Variant::Panel => metrics::CARD_RADIUS,
        }
    }

    fn default_width(&self) -> f32 {
        match self.variant {
            Variant::Banner => metrics::BANNER_WIDTH,
            Variant::Panel => metrics::CARD_WIDTH,
        }
    }

    /// The children without the close button.
    fn body(&self, cx: &LayoutCx<'_>) -> Vec<ViewId> {
        cx.children()
            .into_iter()
            .filter(|c| Some(*c) != self.close)
            .collect()
    }
}

impl Widget for Notification {
    fn kind(&self) -> &'static str {
        "Notification"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let w = match p.width {
            Some(w) if w.is_finite() => w.max(0.0),
            _ => self.default_width(),
        };
        let inner = (w - 2.0 * metrics::PADDING).max(0.0);
        let body = self.body(cx);
        let mut h = 2.0 * metrics::PADDING;
        for (i, c) in body.iter().enumerate() {
            if i > 0 {
                h += metrics::GAP;
            }
            h += cx.measure(*c, Proposal::new(Some(inner), None)).height;
        }
        if let Some(c) = self.close {
            let _ = cx.measure(c, Proposal::UNSPECIFIED);
        }
        Size::new(w, h)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        let inner = (size.width - 2.0 * metrics::PADDING).max(0.0);
        let mut y = metrics::PADDING;
        for c in self.body(cx) {
            let s = cx.measure(c, Proposal::new(Some(inner), None));
            cx.place(c, Rect::new(metrics::PADDING, y, inner, s.height));
            y += s.height + metrics::GAP;
        }
        if let Some(c) = self.close {
            let _ = cx.measure(c, Proposal::UNSPECIFIED);
            cx.place(
                c,
                Rect::new(
                    -metrics::CLOSE_OUT,
                    -metrics::CLOSE_OUT,
                    metrics::CLOSE,
                    metrics::CLOSE,
                ),
            );
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let r = cx.bounds();
        match self.variant {
            Variant::Banner => {
                let style = glass_style(&theme, Tier::Regular);
                cx.rec
                    .layer("Surface", r, Shape::rounded(self.radius()), style);
            }
            Variant::Panel => {
                let mut style = Style::fill(theme.colors.surface_raised);
                if let Some(s) = theme.shadow(1) {
                    style.shadows.push(s);
                }
                cx.rec
                    .layer("Surface", r, Shape::rounded(self.radius()), style);
            }
        }
    }

    fn paint_over(&mut self, cx: &mut PaintCx<'_>) {
        if self.close.is_none() {
            return;
        }
        let theme = *cx.theme();
        let c = theme.colors;
        let r = cx.bounds();
        let shown = if self.close_focused {
            1.0
        } else {
            self.transitions.hover()
        };
        if shown <= 0.0 {
            return;
        }
        let rect = Rect::new(
            r.x - metrics::CLOSE_OUT,
            r.y - metrics::CLOSE_OUT,
            metrics::CLOSE,
            metrics::CLOSE,
        );
        let mut style = Style::fill(c.surface_control)
            .with_border(Border::inside(c.line_border, 1.0))
            .with_opacity(shown);
        if let Some(s) = theme.shadow(1) {
            style.shadows.push(s);
        }
        cx.rec.layer("Close", rect, Shape::Ellipse, style);
        cx.rec.push_opacity(shown);
        cx.rec.glyph(
            "Close Glyph",
            Rect::new(rect.x + 4.0, rect.y + 4.0, 12.0, 12.0),
            "xmark",
            400,
            c.text_secondary,
        );
        cx.rec.pop();
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        match event {
            Event::Activate => {
                cx.emit(Signal::Activated, Value::None);
                Handled::Yes
            }
            Event::Key(k) if k.pressed && matches!(k.key, Key::Escape) && self.close.is_some() => {
                cx.emit(Signal::CloseRequested, Value::None);
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
        Interaction::Button
    }

    fn focus(&self) -> FocusKind {
        FocusKind::Control
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match (prop, value.as_str()) {
            (Prop::Title, Some(t)) => {
                self.title = t.to_string();
                PropChange::Nothing
            }
            _ => PropChange::Unknown,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Group;
        a.label = Some(self.title.clone());
        a.live = true;
        a.actions = vec![Action::Click, Action::Focus];
    }
}

/// The views of a built notification.
#[derive(Debug, Clone)]
pub struct NotificationParts {
    /// The notification.
    pub root: ViewId,
    /// Its close button.
    pub close: Option<ViewId>,
    /// Its action buttons, in order.
    pub actions: Vec<ViewId>,
}

fn id_of(ui: &mut Ui, v: ViewId, name: &str) {
    let _ = ui.set(v, Prop::Identifier, Value::from(name));
}

/// Makes the notification described by `spec` (a card of `variant`).
pub fn notification(ui: &mut Ui, spec: &NotificationSpec, variant: Variant) -> NotificationParts {
    let icon = ui.create(AppIcon {
        color: spec.color,
        glyph: spec.glyph.clone(),
    });
    id_of(ui, icon, "App icon");
    let title = ui.create(
        Text::new(&spec.title)
            .token(&typography::HEADLINE)
            .role(TextRole::Primary)
            .layer("Title")
            .whole(1.0, 0.0),
    );
    let body = ui.create(
        Text::new(&spec.message)
            .token(&typography::CALLOUT)
            .role(TextRole::Secondary)
            .layer("Body")
            .lines(2),
    );
    let text = ui.create(Stack::column(0.0).aligned(Alignment::LEADING));
    id_of(ui, text, "Text");
    let _ = ui.set(
        text,
        Prop::MinHeight,
        Value::from(metrics::CONTENT_MIN_HEIGHT),
    );
    ui.add_child(text, title);
    ui.add_child(text, body);
    let time = ui.create(
        Text::new(&spec.time)
            .token(&typography::CAPTION)
            .role(TextRole::Tertiary)
            .layer("Time")
            .whole(1.0, 0.0),
    );
    let content = ui.create(Stack::row(metrics::GAP).aligned(Alignment::TOP_LEADING));
    id_of(ui, content, "Content");
    ui.add_child(content, icon);
    ui.add_child(content, text);
    ui.add_child(content, time);

    let mut n = Notification::new(variant);
    n.title.clone_from(&spec.title);
    let root = ui.create(n);
    ui.add_child(root, content);
    let mut actions = Vec::new();
    if !spec.actions.is_empty() {
        let row = ui.create(Stack::row(metrics::GAP).aligned(Alignment::TOP_LEADING));
        id_of(ui, row, "Actions");
        let _ = ui.set(
            row,
            Prop::Padding,
            Value::Insets(Insets::new(0.0, metrics::ACTIONS_INDENT, 0.0, 0.0)),
        );
        for label in &spec.actions {
            let b = ui.create(
                PushButton::new(label)
                    .variant(PushVariant::Secondary)
                    .size(ControlSize::Small),
            );
            id_of(ui, b, label);
            ui.add_child(row, b);
            actions.push(b);
        }
        ui.add_child(root, row);
    }
    let close = spec.closable.then(|| {
        let c = ui.create(CloseHit { owner: root });
        ui.add_child(root, c);
        let _ = ui.with_widget::<Notification, _>(root, |n| n.close = Some(c));
        c
    });
    NotificationParts {
        root,
        close,
        actions,
    }
}

/// Fades a view in when it is added and glides it when its neighbours come and go
/// (`spring.panel`).
fn glide(ui: &mut Ui, v: ViewId) {
    let _ = ui.set(v, Prop::Transition, Value::Int(1));
    let _ = ui.set(v, Prop::Animation, Value::Int(2));
}

// ---------------------------------------------------------------------------------------------
// Stack
// ---------------------------------------------------------------------------------------------

/// One app's notifications grouped: the newest banner in front and a glass plate behind it for
/// each of the next two. A click on the front banner unfolds the list; "Show less" folds it.
/// Emits `ExpandedChanged` (the new state), `CloseRequested` (the id of a notification that
/// was closed; it is removed) and `Activated` (`"<id>/<action>"`).
#[derive(Debug)]
pub struct NotificationStack {
    /// The notifications, newest first.
    pub items: Vec<NotificationSpec>,
    /// Whether the list is unfolded.
    pub expanded: bool,
    plates: usize,
}

impl NotificationStack {
    fn collapsed(&self) -> bool {
        !self.expanded
    }
}

impl Widget for NotificationStack {
    fn kind(&self) -> &'static str {
        "NotificationStack"
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let w = match p.width {
            Some(w) if w.is_finite() => w.max(0.0),
            _ => metrics::BANNER_WIDTH,
        };
        let kids = cx.children();
        if self.collapsed() {
            let front = kids
                .last()
                .map_or(0.0, |c| cx.measure(*c, Proposal::new(Some(w), None)).height);
            return Size::new(w, front + metrics::PEEK);
        }
        let mut h = 0.0;
        for (i, c) in kids.iter().enumerate() {
            if i > 0 {
                h += metrics::GAP;
            }
            h += cx.measure(*c, Proposal::new(Some(w), None)).height;
        }
        Size::new(w, h)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        let kids = cx.children();
        if self.collapsed() {
            let n = kids.len();
            let front_h = kids.last().map_or(0.0, |c| {
                cx.measure(*c, Proposal::new(Some(size.width), None)).height
            });
            for (i, c) in kids.iter().enumerate() {
                if i + 1 == n {
                    cx.place(*c, Rect::new(0.0, 0.0, size.width, front_h));
                } else {
                    // Plates: the one nearest the front is the first level.
                    let level = (n - 1 - i) as f32;
                    let inset = metrics::PLATE_INSET * level;
                    let _ = cx.measure(*c, Proposal::UNSPECIFIED);
                    cx.place(
                        *c,
                        Rect::new(
                            inset,
                            metrics::PEEK / 2.0 * level,
                            (size.width - 2.0 * inset).max(0.0),
                            front_h.max(metrics::PLATE_HEIGHT.min(front_h)),
                        ),
                    );
                }
            }
            return;
        }
        let mut y = 0.0;
        for c in kids {
            let s = cx.measure(c, Proposal::new(Some(size.width), None));
            let x = if s.width < size.width {
                (size.width - s.width) / 2.0
            } else {
                0.0
            };
            cx.place(c, Rect::new(x, y, s.width.min(size.width), s.height));
            y += s.height + metrics::GAP;
        }
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match (prop, value.as_bool()) {
            (Prop::Expanded, Some(e)) => {
                self.expanded = e;
                PropChange::Layout
            }
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Expanded => Some(Value::Bool(self.expanded)),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Group;
        a.expanded = Some(self.expanded);
        a.label = Some("Notifications".to_string());
    }
}

/// Makes a stack of `items` (newest first), folded.
pub fn notification_stack(ui: &mut Ui, items: Vec<NotificationSpec>) -> ViewId {
    let s = ui.create(NotificationStack {
        items,
        expanded: false,
        plates: 0,
    });
    rebuild_stack(ui, s);
    s
}

/// Unfolds or folds the stack (what a click on the front banner, or "Show less", does).
pub fn set_expanded(ui: &mut Ui, stack: ViewId, expanded: bool) {
    let changed = ui
        .with_widget::<NotificationStack, _>(stack, |s| {
            let changed = s.expanded != expanded && (expanded || s.items.len() > 1 || s.expanded);
            if changed && (!expanded || s.items.len() > 1) {
                s.expanded = expanded;
                true
            } else {
                false
            }
        })
        .unwrap_or(false);
    if changed {
        rebuild_stack(ui, stack);
        ui.emit(stack, Signal::ExpandedChanged, Value::Bool(expanded));
    }
}

/// Removes the notification `id` from the stack and emits `CloseRequested` with it.
pub fn remove_item(ui: &mut Ui, stack: ViewId, id: &str) {
    let removed = ui
        .with_widget::<NotificationStack, _>(stack, |s| {
            let before = s.items.len();
            s.items.retain(|i| i.id != id);
            if s.items.len() < 2 {
                s.expanded = false;
            }
            s.items.len() != before
        })
        .unwrap_or(false);
    if removed {
        rebuild_stack(ui, stack);
        ui.emit(stack, Signal::CloseRequested, Value::from(id));
    }
}

fn wire_card(ui: &mut Ui, stack: ViewId, parts: &NotificationParts, spec: &NotificationSpec) {
    for (b, label) in parts.actions.iter().zip(&spec.actions) {
        let payload = format!("{}/{}", spec.id, label);
        ui.connect(*b, Signal::Activated, move |ui, _, _| {
            ui.emit(stack, Signal::Activated, Value::from(payload.as_str()));
        });
    }
    let id = spec.id.clone();
    ui.connect(parts.root, Signal::CloseRequested, move |ui, _, _| {
        remove_item(ui, stack, &id);
    });
}

fn rebuild_stack(ui: &mut Ui, stack: ViewId) {
    let Some((items, expanded)) =
        ui.with_widget::<NotificationStack, _>(stack, |s| (s.items.clone(), s.expanded))
    else {
        return;
    };
    ui.clear_children(stack);
    if items.is_empty() {
        return;
    }
    if expanded {
        for spec in &items {
            let parts = notification(ui, spec, Variant::Banner);
            wire_card(ui, stack, &parts, spec);
            glide(ui, parts.root);
            ui.add_child(stack, parts.root);
        }
        let fold = ui.create(
            PushButton::new("Show less")
                .variant(PushVariant::Borderless)
                .size(ControlSize::Small),
        );
        id_of(ui, fold, "Show less");
        ui.connect(fold, Signal::Activated, move |ui, _, _| {
            set_expanded(ui, stack, false);
        });
        ui.add_child(stack, fold);
        return;
    }
    // Folded: plates behind the front banner, farthest first.
    let plates = items.len().saturating_sub(1).min(2);
    for level in (1..=plates).rev() {
        let plate = ui.create(
            crate::amalfi::materials::GlassPanel::new(Tier::Regular, metrics::BANNER_RADIUS)
                .layer(&format!("Plate {level}")),
        );
        ui.add_child(stack, plate);
    }
    let Some(front) = items.first() else {
        return;
    };
    let parts = notification(ui, front, Variant::Banner);
    id_of(ui, parts.root, "Front banner");
    wire_card(ui, stack, &parts, front);
    if items.len() > 1 {
        ui.connect(parts.root, Signal::Activated, move |ui, _, _| {
            set_expanded(ui, stack, true);
        });
    }
    ui.add_child(stack, parts.root);
    let _ = ui.with_widget::<NotificationStack, _>(stack, |s| s.plates = plates);
}

// ---------------------------------------------------------------------------------------------
// Centre
// ---------------------------------------------------------------------------------------------

/// A titled group of notifications of the centre ("Today", "Earlier").
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    /// The title.
    pub title: String,
    /// The notifications.
    pub items: Vec<NotificationSpec>,
}

impl Section {
    /// A section.
    pub fn new(title: &str, items: Vec<NotificationSpec>) -> Section {
        Section {
            title: title.to_string(),
            items,
        }
    }
}

/// The notification centre: a Thick glass column with the header (title and the Clear link)
/// and sections of cards. "Clear all" empties it (and emits `Activated` with `"clear"`); the
/// cards' actions emit `Activated` with `"<id>/<action>"`.
#[derive(Debug)]
pub struct NotificationCentre {
    /// The title.
    pub title: String,
    /// The Clear link's label (none: no link).
    pub clear: String,
    /// The sections.
    pub sections: Vec<Section>,
}

impl Widget for NotificationCentre {
    fn kind(&self) -> &'static str {
        "NotificationCentre"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let w = match p.width {
            Some(w) if w.is_finite() => w.max(0.0),
            _ => metrics::CENTRE_WIDTH,
        };
        let inner = (w - 2.0 * metrics::CENTRE_PADDING).max(0.0);
        let mut h = 2.0 * metrics::CENTRE_PADDING;
        for (i, c) in cx.children().iter().enumerate() {
            if i > 0 {
                h += metrics::GAP;
            }
            h += cx.measure(*c, Proposal::new(Some(inner), None)).height;
        }
        Size::new(w, h.max(metrics::CENTRE_MIN_HEIGHT))
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        let inner = (size.width - 2.0 * metrics::CENTRE_PADDING).max(0.0);
        let mut y = metrics::CENTRE_PADDING;
        for c in cx.children() {
            let s = cx.measure(c, Proposal::new(Some(inner), None));
            cx.place(c, Rect::new(metrics::CENTRE_PADDING, y, inner, s.height));
            y += s.height + metrics::GAP;
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let r = cx.bounds();
        cx.rec.layer(
            "Surface",
            r,
            Shape::rounded(metrics::CENTRE_RADIUS),
            glass_style(&theme, Tier::Thick),
        );
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Group;
        a.label = Some(self.title.clone());
    }
}

/// Makes the centre.
pub fn notification_centre(
    ui: &mut Ui,
    title: &str,
    clear: &str,
    sections: Vec<Section>,
) -> ViewId {
    let c = ui.create(NotificationCentre {
        title: title.to_string(),
        clear: clear.to_string(),
        sections,
    });
    rebuild_centre(ui, c);
    c
}

/// Empties the centre ("Clear all").
pub fn clear_all(ui: &mut Ui, centre: ViewId) {
    let _ = ui.with_widget::<NotificationCentre, _>(centre, |c| c.sections.clear());
    rebuild_centre(ui, centre);
    ui.emit(centre, Signal::Activated, Value::from("clear"));
}

fn rebuild_centre(ui: &mut Ui, centre: ViewId) {
    let Some((title, clear, sections)) = ui.with_widget::<NotificationCentre, _>(centre, |c| {
        (c.title.clone(), c.clear.clone(), c.sections.clone())
    }) else {
        return;
    };
    ui.clear_children(centre);
    let header = ui.create(Stack::row(8.0));
    id_of(ui, header, "Header");
    let _ = ui.set(
        header,
        Prop::Padding,
        Value::Insets(Insets::new(0.0, 4.0, 0.0, 0.0)),
    );
    let _ = ui.set(header, Prop::MinHeight, Value::from(metrics::HEADER));
    let t = ui.create(
        Text::new(&title)
            .token(&typography::TITLE_3)
            .role(TextRole::Primary)
            .layer("Title")
            .whole(1.0, 0.0),
    );
    ui.add_child(header, t);
    let spacer = ui.create(crate::core::layout::Spacer::default());
    ui.add_child(header, spacer);
    if !clear.is_empty() {
        let link = ui.create(LinkButton::new(&clear));
        id_of(ui, link, "Clear");
        ui.connect(link, Signal::Activated, move |ui, _, _| {
            clear_all(ui, centre)
        });
        ui.add_child(header, link);
    }
    ui.add_child(centre, header);
    for section in &sections {
        let s = ui.create(
            Text::new(&section.title)
                .token(&typography::LABEL)
                .role(TextRole::Tertiary)
                .layer(&format!("Section {}", section.title)),
        );
        let _ = ui.set(s, Prop::Height, Value::from(metrics::SECTION));
        ui.add_child(centre, s);
        for spec in &section.items {
            let mut spec = spec.clone();
            spec.closable = false;
            let parts = notification(ui, &spec, Variant::Panel);
            id_of(ui, parts.root, &spec.id);
            for (b, label) in parts.actions.iter().zip(&spec.actions) {
                let payload = format!("{}/{}", spec.id, label);
                ui.connect(*b, Signal::Activated, move |ui, _, _| {
                    ui.emit(centre, Signal::Activated, Value::from(payload.as_str()));
                });
            }
            glide(ui, parts.root);
            ui.add_child(centre, parts.root);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::window::WindowKind;

    fn spec(id: &str) -> NotificationSpec {
        NotificationSpec::new(id, "Recipes", "Two new recipes were added to Lemon cake.")
    }

    #[test]
    fn a_banner_is_76_tall_and_108_with_actions() {
        let mut ui = Ui::new_headless();
        let a = notification(&mut ui, &spec("a"), Variant::Banner);
        assert_eq!(
            ui.measure(a.root, Proposal::UNSPECIFIED),
            Size::new(360.0, 76.0)
        );
        let b = notification(
            &mut ui,
            &spec("b").actions(&["Reply", "Mute"]),
            Variant::Banner,
        );
        assert_eq!(
            ui.measure(b.root, Proposal::UNSPECIFIED),
            Size::new(360.0, 108.0)
        );
        let c = notification(&mut ui, &spec("c"), Variant::Panel);
        assert_eq!(ui.measure(c.root, Proposal::UNSPECIFIED).width, 340.0);
    }

    #[test]
    fn a_folded_stack_peeks_20_under_the_front_banner() {
        let mut ui = Ui::new_headless();
        let s = notification_stack(&mut ui, vec![spec("a"), spec("b"), spec("c")]);
        assert_eq!(ui.measure(s, Proposal::UNSPECIFIED), Size::new(360.0, 96.0));
        assert_eq!(
            ui.tree.children(s).len(),
            3,
            "two plates and the front banner"
        );
    }

    #[test]
    fn the_stack_unfolds_folds_and_closes() {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(400.0, 600.0));
        let s = notification_stack(&mut ui, vec![spec("a"), spec("b")]);
        ui.set_content(win, s);
        ui.show_window(win);
        ui.prepare_frame(win);
        let log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let l = log.clone();
        ui.connect(s, Signal::ExpandedChanged, move |_, _, v| {
            l.borrow_mut().push(v.as_bool().unwrap_or(false));
        });
        let front = *ui.tree.children(s).last().expect("front");
        ui.emit(front, Signal::Activated, Value::None);
        ui.flush();
        assert_eq!(*log.borrow(), vec![true]);
        // Unfolded: both banners and the fold button.
        assert_eq!(ui.tree.children(s).len(), 3);
        let fold = *ui.tree.children(s).last().expect("fold");
        ui.emit(fold, Signal::Activated, Value::None);
        ui.flush();
        assert_eq!(*log.borrow(), vec![true, false]);
        // Closing the front one leaves a single banner and no plate.
        let front = *ui.tree.children(s).last().expect("front");
        ui.emit(front, Signal::CloseRequested, Value::None);
        ui.flush();
        assert_eq!(ui.tree.children(s).len(), 1);
    }

    #[test]
    fn a_single_notification_does_not_unfold() {
        let mut ui = Ui::new_headless();
        let s = notification_stack(&mut ui, vec![spec("a")]);
        set_expanded(&mut ui, s, true);
        assert_eq!(ui.get(s, Prop::Expanded), Some(Value::Bool(false)));
    }

    #[test]
    fn clear_all_empties_the_centre() {
        let mut ui = Ui::new_headless();
        let c = notification_centre(
            &mut ui,
            "Notifications",
            "Clear all",
            vec![Section::new("Today", vec![spec("a"), spec("b")])],
        );
        assert_eq!(ui.measure(c, Proposal::UNSPECIFIED).width, 380.0);
        assert!(ui.measure(c, Proposal::UNSPECIFIED).height >= 416.0);
        let n = ui.tree.children(c).len();
        assert_eq!(n, 4, "header, section title, two cards");
        clear_all(&mut ui, c);
        assert_eq!(ui.tree.children(c).len(), 1, "only the header is left");
    }

    #[test]
    fn the_close_button_hangs_out_of_the_corner_and_closes() {
        use crate::core::event::{Button, Mods};
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(420.0, 200.0));
        let parts = notification(&mut ui, &spec("a"), Variant::Banner);
        let holder = ui.create(Stack::column(0.0));
        let _ = ui.set(holder, Prop::Padding, Value::Insets(Insets::all(20.0)));
        ui.add_child(holder, parts.root);
        ui.set_content(win, holder);
        ui.show_window(win);
        ui.prepare_frame(win);
        let closed = std::rc::Rc::new(std::cell::Cell::new(0));
        let c = closed.clone();
        ui.connect(parts.root, Signal::CloseRequested, move |_, _, _| {
            c.set(c.get() + 1)
        });
        // Its 20 pt box starts 8 pt outside the corner: a click 4 pt outside is on it.
        let origin = ui.tree.window_rect(parts.root).origin();
        let at = Point::new(origin.x - 4.0, origin.y - 4.0);
        ui.pointer_move(win, at, Mods::empty());
        ui.pointer_down(win, at, Button::Left, Mods::empty());
        ui.pointer_up(win, at, Button::Left, Mods::empty());
        ui.flush();
        assert_eq!(closed.get(), 1);
        // The pointer on the banner itself shows the button and activates the banner.
        let on = std::rc::Rc::new(std::cell::Cell::new(0));
        let o = on.clone();
        ui.connect(parts.root, Signal::Activated, move |_, _, _| {
            o.set(o.get() + 1)
        });
        let inside = Point::new(origin.x + 200.0, origin.y + 40.0);
        ui.pointer_move(win, inside, Mods::empty());
        ui.pointer_down(win, inside, Button::Left, Mods::empty());
        ui.pointer_up(win, inside, Button::Left, Mods::empty());
        ui.flush();
        assert_eq!(on.get(), 1);
        let none = notification(&mut ui, &spec("b").fixed(), Variant::Banner);
        assert!(none.close.is_none());
    }

    #[test]
    fn it_is_a_live_region_and_survives_odd_widths() {
        let mut ui = Ui::new_headless();
        let a = notification(&mut ui, &spec("a"), Variant::Banner);
        let info = ui.accessibility_of(a.root).expect("a11y");
        assert!(info.live);
        for w in [0.0, 1.0, 1e6] {
            let s = ui.measure(a.root, Proposal::new(Some(w), None));
            assert!(s.width.is_finite() && s.height.is_finite());
        }
    }
}
