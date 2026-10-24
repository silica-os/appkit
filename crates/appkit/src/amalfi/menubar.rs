//! Menu Bar (kit `Menu Bar`): the app's titles in the compositor's menu bar (the Silica mark,
//! the app's name in bold, its menus) and the capsule of the title whose menu is open. The
//! status items are the Status Bar's (`statusbar.rs`), each on its own surface.

use crate::amalfi::foundation::{Label, whole};
use crate::amalfi::tokens::typography;
use crate::core::a11y::{A11y, Action, Role};
use crate::core::event::{Button, Event, Handled};
use crate::core::layout::Proposal;
use crate::core::tree::ViewId;
use crate::core::ui::Ui;
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, FocusKind, Interaction, LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Rect, Size};
use crate::render::path::PathBuilder;
use crate::shape::Shape;
use crate::style::Style;
use crate::text::Align;

/// Fixed measures of the kit.
pub mod metrics {
    /// The bar's height.
    pub const BAR_HEIGHT: f32 = crate::amalfi::tokens::control_heights::LARGE;
    /// A title's height (2 pt above and below it).
    pub const ITEM_HEIGHT: f32 = crate::amalfi::tokens::control_heights::REGULAR;
    /// The bar's padding before the first title.
    pub const LEADING: f32 = crate::amalfi::tokens::spacing::S;
    /// A title's padding at its sides.
    pub const PADDING: f32 = crate::amalfi::tokens::spacing::S;
    /// The Silica mark's box.
    pub const MARK_WIDTH: f32 = 36.0;
    /// The open title's capsule (`SMenuBarItem`: half the item).
    pub const RADIUS: f32 = 14.0;
    /// The label grid.
    pub const STEP: f32 = crate::amalfi::tokens::grid::BASE;
    /// A menu's body starts 5 pt left of its title (docs/spec/menubar.md) …
    pub const MENU_INSET: f32 = 5.0;
    /// … with its top 8 pt below the bar (kit `Menu Bar/Bar/Menu Open`).
    pub const MENU_GAP: f32 = 8.0;
}

/// What a title is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleKind {
    /// The Silica mark (the system menu).
    Mark,
    /// The app's name, in bold (the App menu).
    Name,
    /// One of the app's menus.
    Menu,
}

/// A title of the menu bar.
#[derive(Debug)]
pub struct MenuTitle {
    /// What it is.
    pub kind: TitleKind,
    /// Its label (the mark has none).
    pub label: String,
    /// Its menu is open: the capsule shows.
    pub active: bool,
    text: Label,
}

impl MenuTitle {
    /// A title.
    pub fn new(kind: TitleKind, label: &str) -> MenuTitle {
        MenuTitle {
            kind,
            label: label.to_string(),
            active: false,
            text: Label::default(),
        }
    }

    fn token(&self) -> &'static crate::amalfi::tokens::TypeStyle {
        match self.kind {
            TitleKind::Name => &typography::HEADLINE,
            _ => &typography::BODY,
        }
    }
}

/// A face of the Silica mark, a prism seen end-on (kit `Face Left`, `Face Right`): its frame in
/// the mark's 36 × 28 box and its shape.
fn face(left: bool) -> (Rect, Shape) {
    let (w, h) = (7.708_333, 15.416_667);
    let mut b = PathBuilder::new();
    let x = if left {
        b.move_to((w, 0.0)).line_to((0.0, h / 2.0)).line_to((w, h));
        9.666_667
    } else {
        b.move_to((0.0, 0.0))
            .line_to((w, h / 2.0))
            .line_to((0.0, h));
        18.625
    };
    b.close();
    (Rect::new(x, 6.291_667, w, h), Shape::Path(b.detach()))
}

impl Widget for MenuTitle {
    fn kind(&self) -> &'static str {
        "MenuTitle"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        let width = match self.kind {
            TitleKind::Mark => metrics::MARK_WIDTH,
            _ => {
                let theme = cx.theme();
                let style = theme.text(self.token(), theme.colors.text_primary);
                let label = self.label.clone();
                whole(self.text.width(&label, &style), metrics::STEP) + 2.0 * metrics::PADDING
            }
        };
        Size::new(width, metrics::ITEM_HEIGHT)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let r = cx.bounds();
        let ink = theme.colors.text_primary;
        if self.active {
            let color = theme
                .color("Menu Bar/Selection")
                .unwrap_or(theme.colors.state_hover_overlay);
            cx.rec.layer(
                "Active fill",
                r,
                Shape::rounded(metrics::RADIUS),
                Style::fill(color),
            );
        }
        match self.kind {
            TitleKind::Mark => {
                for (name, left) in [("Face Left", true), ("Face Right", false)] {
                    let (at, shape) = face(left);
                    cx.rec
                        .layer(name, at.translate(r.origin()), shape, Style::fill(ink));
                }
            }
            _ => {
                let token = self.token();
                let style = theme.text(token, ink);
                let w = (r.width - 2.0 * metrics::PADDING).max(0.0);
                let label = self.label.clone();
                let layout = self.text.layout(&label, &style, Align::Start, w, Some(1));
                cx.rec.text(
                    "Label",
                    Rect::new(r.x + metrics::PADDING, r.y + 5.0, w, 18.0),
                    layout,
                    Some(token.name),
                    ink,
                );
            }
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        match event {
            // The menu opens on the press (docs/spec/menubar.md, M-01).
            Event::PointerDown {
                button: Button::Left,
                ..
            }
            | Event::Activate => {
                cx.emit(Signal::Activated, Value::None);
                Handled::Yes
            }
            // While a menu is open, the pointer onto another title switches menus.
            Event::PointerEnter => {
                cx.emit(Signal::PointerEntered, Value::None);
                Handled::No
            }
            _ => Handled::No,
        }
    }

    // No press state: the title opens on the press, and the release keeps the menu open.
    fn interaction(&self) -> Interaction {
        Interaction::None
    }

    fn focus(&self) -> FocusKind {
        FocusKind::None
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::Expanded => value.as_bool().map_or(PropChange::Unknown, |b| {
                self.active = b;
                PropChange::Paint
            }),
            Prop::Title => value.as_str().map_or(PropChange::Unknown, |s| {
                self.label = s.to_string();
                PropChange::Layout
            }),
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Expanded => Some(Value::Bool(self.active)),
            Prop::Title => Some(Value::Str(self.label.clone())),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::MenuItem;
        a.label = Some(match self.kind {
            TitleKind::Mark => "Silica".to_string(),
            _ => self.label.clone(),
        });
        a.expanded = Some(self.active);
        a.actions = vec![Action::Click];
    }
}

/// The titles side by side, 8 pt from the start and 2 pt below the top.
#[derive(Debug, Default)]
pub struct MenuBar;

impl Widget for MenuBar {
    fn kind(&self) -> &'static str {
        "MenuBar"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        let mut w = metrics::LEADING;
        for c in cx.children() {
            w += cx.measure(c, Proposal::new(None, None)).width;
        }
        Size::new(w, metrics::BAR_HEIGHT)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        let mut x = metrics::LEADING;
        for c in cx.children() {
            let s = cx.measure(c, Proposal::new(None, None));
            // Titles past the room the compositor gave are left out, as on macOS.
            let r = if x + s.width <= size.width + 0.5 {
                Rect::new(x, 2.0, s.width, s.height)
            } else {
                Rect::new(x, -1.0e4, s.width, s.height)
            };
            cx.place(c, r);
            x += s.width;
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Group;
        a.label = Some("Menu bar".to_string());
    }
}

/// Makes a menu bar over `titles`.
pub fn menu_bar(ui: &mut Ui, titles: &[(TitleKind, String)]) -> (ViewId, Vec<ViewId>) {
    let bar = ui.create(MenuBar);
    let views = titles
        .iter()
        .map(|(kind, label)| {
            let v = ui.create(MenuTitle::new(*kind, label));
            ui.add_child(bar, v);
            v
        })
        .collect();
    (bar, views)
}
