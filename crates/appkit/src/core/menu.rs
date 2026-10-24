//! Menus: the app's menu bar, whose titles the app draws in the compositor's menu bar
//! (`core::menu_bar`), and pop-up menus (context menus, pop-up and pull-down buttons). All are
//! the app's own popups drawn by AppKit, as on macOS. Items carry ids; choosing one runs its action.
//! Shortcuts of the menu bar's items become app key equivalents.

use crate::core::tree::ViewId;
use crate::core::ui::Ui;
use crate::core::value::{Prop, Signal, Value};
use crate::geom::Rect;

/// An item's action.
pub type MenuAction = std::rc::Rc<dyn Fn(&mut Ui)>;

/// One row of a menu.
#[derive(Clone, Default)]
pub struct MenuItem {
    /// Id (assigned by [`Ui::set_menu_bar`] or [`Ui::popup_menu`] when 0).
    pub id: u32,
    /// Label.
    pub title: String,
    /// Symbol name (Silica Symbols) or absolute icon path.
    pub icon: Option<String>,
    /// Key equivalent (`cmd+s` …).
    pub shortcut: Option<String>,
    /// Enabled.
    pub enabled: bool,
    /// Checkmark: `Some(true)` checked, `Some(false)` mixed, `None` none.
    pub checked: Option<bool>,
    /// Destructive (drawn in red).
    pub destructive: bool,
    /// A system role (`about`, `quit`, `hide`, `preferences` …) the shell may treat specially.
    pub role: Option<String>,
    /// A separator row (the other fields are ignored).
    pub separator: bool,
    /// A section heading row.
    pub heading: bool,
    /// Submenu rows.
    pub submenu: Option<Vec<MenuItem>>,
    /// What choosing it does.
    pub action: Option<MenuAction>,
}

impl std::fmt::Debug for MenuItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MenuItem")
            .field("id", &self.id)
            .field("title", &self.title)
            .field("shortcut", &self.shortcut)
            .field("separator", &self.separator)
            .field("submenu", &self.submenu)
            .finish()
    }
}

impl MenuItem {
    /// An enabled item.
    pub fn new(title: &str) -> Self {
        MenuItem {
            title: title.to_string(),
            enabled: true,
            ..MenuItem::default()
        }
    }

    /// A separator.
    pub fn separator() -> Self {
        MenuItem {
            separator: true,
            ..MenuItem::default()
        }
    }

    /// A section heading.
    pub fn heading(title: &str) -> Self {
        MenuItem {
            title: title.to_string(),
            heading: true,
            ..MenuItem::default()
        }
    }

    /// A submenu.
    pub fn submenu(title: &str, items: Vec<MenuItem>) -> Self {
        MenuItem {
            submenu: Some(items),
            ..MenuItem::new(title)
        }
    }

    /// With a key equivalent.
    pub fn shortcut(mut self, s: &str) -> Self {
        self.shortcut = Some(s.to_string());
        self
    }

    /// With an action.
    pub fn action(mut self, f: impl Fn(&mut Ui) + 'static) -> Self {
        self.action = Some(std::rc::Rc::new(f));
        self
    }

    /// With a role.
    pub fn role(mut self, r: &str) -> Self {
        self.role = Some(r.to_string());
        self
    }

    /// Enabled or not.
    pub fn enabled(mut self, e: bool) -> Self {
        self.enabled = e;
        self
    }

    /// Checked state.
    pub fn checked(mut self, c: Option<bool>) -> Self {
        self.checked = c;
        self
    }

    /// With an icon.
    pub fn icon(mut self, name: &str) -> Self {
        self.icon = Some(name.to_string());
        self
    }
}

/// A top-level menu of the bar.
#[derive(Debug, Clone, Default)]
pub struct Menu {
    /// Title in the bar.
    pub title: String,
    /// `app` (the bold menu named after the app), `window`, `help`, or none.
    pub role: Option<String>,
    /// Rows.
    pub items: Vec<MenuItem>,
}

/// The kind of a pop-up menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupKind {
    /// A context menu at the pointer, or a pull-down below a control.
    Context,
    /// A pop-up button's menu, the selected item over the control.
    PopUp,
    /// A status item's menu, under the menu bar.
    MenuBar,
}

/// The menu state of the runtime.
#[derive(Default)]
pub struct MenuState {
    pub(crate) bar: Vec<Menu>,
    pub(crate) next_id: u32,
    pub(crate) popups: Vec<PendingPopup>,
    pub(crate) next_token: u64,
    pub(crate) strip: crate::core::menu_bar::StripState,
}

/// A pop-up menu shown and not yet answered: token, item actions, completion.
pub(crate) type PendingPopup = (u64, Vec<(u32, MenuAction)>, Option<PopupDone>);

/// Called when a pop-up menu ends: the chosen item's id, or `None` when dismissed.
pub type PopupDone = Box<dyn FnOnce(&mut Ui, Option<u32>)>;

fn assign(items: &mut [MenuItem], next: &mut u32, actions: &mut Vec<(u32, MenuAction)>) {
    for it in items {
        if it.separator || it.heading {
            continue;
        }
        if it.id == 0 {
            *next = next.wrapping_add(1).max(1);
            it.id = *next;
        }
        if let Some(a) = &it.action {
            actions.push((it.id, a.clone()));
        }
        if let Some(sub) = &mut it.submenu {
            assign(sub, next, actions);
        }
    }
}

fn shortcuts(items: &[MenuItem], out: &mut Vec<(String, u32)>) {
    for it in items {
        if let (Some(s), true) = (&it.shortcut, it.enabled) {
            out.push((s.clone(), it.id));
        }
        if let Some(sub) = &it.submenu {
            shortcuts(sub, out);
        }
    }
}

/// The rows AppKit draws for `items`.
pub(crate) fn rows_of(items: &[MenuItem]) -> Vec<crate::amalfi::menus::Row> {
    use crate::amalfi::menus::{Row, RowKind};
    items
        .iter()
        .map(|it| Row {
            kind: if it.separator {
                RowKind::Separator
            } else if it.heading {
                RowKind::Header
            } else {
                RowKind::Item
            },
            label: it.title.clone(),
            // `cmd+w` shows as `⌘W`.
            shortcut: it.shortcut.as_deref().map(|s| {
                crate::core::event::Shortcut::parse(s)
                    .map_or_else(|| s.to_string(), |k| k.display())
            }),
            icon: it.icon.clone(),
            checked: it.checked == Some(true),
            submenu: it.submenu.is_some(),
            sub: it.submenu.as_deref().map(rows_of).unwrap_or_default(),
            destructive: it.destructive,
            disabled: !it.enabled,
        })
        .collect()
}

/// The id of the item at `path` (row indices, into submenus).
pub(crate) fn id_at(items: &[MenuItem], path: &[i64]) -> Option<u32> {
    let (first, rest) = path.split_first()?;
    let it = items.get(usize::try_from(*first).ok()?)?;
    match it.submenu.as_deref() {
        Some(sub) if !rest.is_empty() => id_at(sub, rest),
        _ if rest.is_empty() && !it.separator && !it.heading => Some(it.id),
        _ => None,
    }
}

/// The item `id` of the menu bar.
pub(crate) fn find(menus: &[Menu], id: u32) -> Option<&MenuItem> {
    fn walk(items: &[MenuItem], id: u32) -> Option<&MenuItem> {
        items.iter().find_map(|it| {
            if it.id == id && !it.separator && !it.heading {
                return Some(it);
            }
            it.submenu.as_deref().and_then(|s| walk(s, id))
        })
    }
    menus.iter().find_map(|m| walk(&m.items, id))
}

impl Ui {
    /// Sets the app's menu bar (its titles in the compositor's menu bar; key equivalents
    /// registered).
    pub fn set_menu_bar(&mut self, mut menus: Vec<Menu>) {
        let mut next = self.menus.next_id;
        for m in &mut menus {
            assign(&mut m.items, &mut next, &mut Vec::new());
        }
        self.menus.next_id = next;
        let mut keys = Vec::new();
        for m in &menus {
            shortcuts(&m.items, &mut keys);
        }
        self.clear_menu_shortcuts();
        for (s, id) in keys {
            self.add_menu_shortcut(&s, id);
        }
        self.menus.bar = menus;
        self.sync_menu_titles();
    }

    /// The menu bar.
    pub fn menu_bar(&self) -> &[Menu] {
        &self.menus.bar
    }

    /// A menu-bar item was chosen (by its key equivalent): it runs if it validates.
    pub fn menu_activated(&mut self, id: u32) {
        let target = self.key_window();
        self.choose_bar_item(id, target);
    }

    /// Shows a pop-up menu for `view` anchored at `anchor` (view coordinates): a popup of the
    /// app drawn by AppKit, as on macOS. `done` gets the chosen id (after the item's own
    /// action ran) or `None`.
    pub fn popup_menu(
        &mut self,
        view: ViewId,
        anchor: Rect,
        kind: PopupKind,
        selected: u32,
        mut items: Vec<MenuItem>,
        done: Option<PopupDone>,
    ) -> Option<u64> {
        use crate::amalfi::menus;
        let window = self.window_of(view)?;
        let mut actions = Vec::new();
        let mut next = 0;
        assign(&mut items, &mut next, &mut actions);
        self.menus.next_token += 1;
        let token = self.menus.next_token;
        let anchor = anchor.translate(self.tree.window_origin(view));
        self.menus.popups.push((token, actions, done));
        let (m, _) = menus::menu(self, &rows_of(&items));
        let row = items
            .iter()
            .position(|it| selected != 0 && it.id == selected);
        let placement = match kind {
            PopupKind::PopUp => menus::Placement::PopUp {
                row: row.unwrap_or(0),
            },
            // A context menu whose anchor has a height is a pull-down (docs/spec/menus.md).
            PopupKind::Context if anchor.height > 0.0 => menus::Placement::PullDown,
            PopupKind::Context => menus::Placement::At,
            PopupKind::MenuBar => menus::Placement::MenuBar,
        };
        if let (PopupKind::PopUp, Some(r)) = (kind, row) {
            let _ = self.set(m, Prop::SelectedIndex, Value::Int(r as i64));
        }
        let popup = menus::present(self, window, anchor, m, placement);
        self.connect(m, Signal::Activated, move |ui, _, v| {
            let path = match v {
                Value::Int(i) => vec![*i],
                Value::IntList(p) => p.clone(),
                _ => Vec::new(),
            };
            let chosen = id_at(&items, &path);
            ui.popup_menu_result(token, chosen);
            ui.close_window(popup);
        });
        self.connect(m, Signal::CloseRequested, move |ui, _, _| {
            ui.close_window(popup);
        });
        // However it goes (Escape, a click outside, the compositor), an unanswered menu is
        // dismissed.
        if let Some(root) = self.window(popup).map(|w| w.root) {
            self.connect(root, Signal::Closed, move |ui, _, _| {
                ui.popup_menu_result(token, None);
            });
        }
        self.set_focus(m, true);
        Some(token)
    }

    /// The platform's answer to a pop-up menu.
    pub fn popup_menu_result(&mut self, token: u64, chosen: Option<u32>) {
        let Some(pos) = self.menus.popups.iter().position(|(t, _, _)| *t == token) else {
            return;
        };
        let (_, actions, done) = self.menus.popups.remove(pos);
        if let Some(id) = chosen
            && let Some((_, a)) = actions.iter().find(|(i, _)| *i == id)
        {
            a(self);
        }
        if let Some(d) = done {
            d(self, chosen);
        }
        self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::{Key, KeyEvent, Mods};
    use crate::core::window::WindowKind;
    use crate::geom::Size;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn bar_ids_actions_and_key_equivalents() {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(100.0, 100.0));
        let hits = Rc::new(Cell::new(0));
        let h = hits.clone();
        ui.set_menu_bar(vec![Menu {
            title: "File".into(),
            role: None,
            items: vec![
                MenuItem::new("New")
                    .shortcut("cmd+n")
                    .action(move |_| h.set(h.get() + 1)),
                MenuItem::separator(),
                MenuItem::submenu("Recent", vec![MenuItem::new("a.txt")]),
            ],
        }]);
        let id = ui.menu_bar()[0].items[0].id;
        assert_ne!(id, 0);
        ui.menu_activated(id);
        assert_eq!(hits.get(), 1);
        ui.key(win, KeyEvent::press(Key::Char('n'), Mods::LOGO));
        assert_eq!(hits.get(), 2);
    }

    #[test]
    fn menus_show_shortcuts_as_glyphs() {
        let rows = rows_of(&[
            MenuItem::new("Close").shortcut("cmd+w"),
            MenuItem::new("Force Quit").shortcut("cmd+alt+escape"),
            MenuItem::new("Odd").shortcut("cmd+"),
        ]);
        let shown: Vec<_> = rows.iter().map(|r| r.shortcut.as_deref()).collect();
        assert_eq!(shown, [Some("⌘W"), Some("⌥⌘⎋"), Some("cmd+")]);
    }

    #[test]
    fn popup_result_runs_the_action_and_done() {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(100.0, 100.0));
        let root = ui.window(win).unwrap().root;
        let got = Rc::new(Cell::new(0u32));
        let g = got.clone();
        let g2 = got.clone();
        let token = ui
            .popup_menu(
                root,
                Rect::new(10.0, 10.0, 0.0, 0.0),
                PopupKind::Context,
                0,
                vec![MenuItem::new("Copy").action(move |_| g.set(g.get() + 10))],
                Some(Box::new(move |_, id| g2.set(g2.get() + id.unwrap_or(0)))),
            )
            .unwrap();
        // The menu is a popup of the app; choosing its row answers once and closes it.
        let popup = ui
            .windows()
            .find(|w| matches!(w.kind, WindowKind::Popup { .. }))
            .map(|w| (w.id, w.root));
        let Some((popup, root)) = popup else {
            panic!("no popup");
        };
        let menu = ui.tree.children(root)[0];
        ui.emit(menu, Signal::Activated, Value::Int(0));
        ui.flush();
        assert_eq!(got.get(), 10 + 1);
        assert!(ui.window(popup).is_none());
        // A second answer is ignored.
        ui.popup_menu_result(token, None);
        assert_eq!(got.get(), 10 + 1);
    }
}
