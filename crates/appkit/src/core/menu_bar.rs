//! The compositor's menu bar (`silica-menu-bar-v1`), as on macOS: the app draws its titles on
//! a menu-titles surface that the compositor shows while the app is frontmost, opens their
//! menus as its own popups, and draws its status items, each on a surface of its own. A hung
//! app's titles stay on screen; its menus need the app.

use crate::amalfi::menubar::{self, TitleKind};
use crate::amalfi::menus;
use crate::amalfi::statusbar::{self, ItemContent, StatusItem};
use crate::core::event::{Event, Key, KeyEvent, Shortcut};
use crate::core::menu::{MenuAction, MenuItem, PopupKind, id_at, rows_of};
use crate::core::tree::ViewId;
use crate::core::ui::{AppAction, Request, Ui};
use crate::core::value::{Prop, Signal, Value};
use crate::core::window::{WindowId, WindowKind};
use crate::geom::{Rect, Size};

/// Decides whether a menu item can be chosen now, right before its menu shows or its key
/// equivalent runs (`validateMenuItem`).
pub type MenuValidator = std::rc::Rc<dyn Fn(&mut Ui, &MenuItem) -> bool>;

/// The titles' surface.
pub(crate) struct Titles {
    window: WindowId,
    views: Vec<ViewId>,
}

/// The open menu of a title.
pub(crate) struct Open {
    title: usize,
    popup: WindowId,
    /// The window that had the keyboard when it opened (where the Edit items act).
    target: Option<WindowId>,
}

/// A status item of the app.
pub(crate) struct Status {
    window: WindowId,
    view: ViewId,
    menu: Option<Vec<MenuItem>>,
    action: Option<MenuAction>,
}

/// The menu bar's state of the runtime.
#[derive(Default)]
pub(crate) struct StripState {
    /// The compositor has a menu bar.
    available: bool,
    /// The compositor runs the App menu's Hide, Hide Others and Show All, and Bring All to
    /// Front (`silica_menu_titles_v1.app_action`).
    app_actions: bool,
    titles: Option<Titles>,
    /// The room the compositor gives the titles.
    room: Option<f32>,
    open: Option<Open>,
    items: Vec<Status>,
    pub(crate) validator: Option<MenuValidator>,
}

/// The key of a key equivalent, for the Edit items that act through it.
fn key_of(shortcut: &Shortcut) -> Option<KeyEvent> {
    let mut chars = shortcut.key.chars();
    let (Some(c), None) = (chars.next(), chars.next()) else {
        return None;
    };
    Some(KeyEvent::press(Key::Char(c), shortcut.mods))
}

/// The item of the app menu the compositor runs for a standard role.
fn app_action(role: &str) -> Option<AppAction> {
    match role {
        "hide" => Some(AppAction::Hide),
        "hide_others" => Some(AppAction::HideOthers),
        "show_all" => Some(AppAction::ShowAll),
        "bring_all_to_front" => Some(AppAction::BringAllToFront),
        _ => None,
    }
}

/// The standard items that act through their key equivalent in the key window.
const EDIT_ROLES: [&str; 6] = ["undo", "redo", "cut", "copy", "paste", "select_all"];

impl Ui {
    /// The compositor has a menu bar (the platform found `silica-menu-bar-v1`): the app's
    /// titles and status items show there.
    pub fn set_menu_bar_available(&mut self, available: bool) {
        self.menus.strip.available = available;
        self.sync_menu_titles();
    }

    /// The compositor runs the items that act on other apps or on the stacking (Hide, Hide
    /// Others, Show All, Bring All to Front): they show enabled and send
    /// [`Request::AppAction`].
    pub fn set_app_actions_available(&mut self, available: bool) {
        self.menus.strip.app_actions = available;
    }

    /// Sets what decides, right before a menu shows, which of its items are enabled.
    pub fn set_menu_validator(&mut self, validator: Option<MenuValidator>) {
        self.menus.strip.validator = validator;
    }

    /// The compositor gave the titles `room` (configure): their surface's size.
    pub fn menu_titles_configured(&mut self, room: f32, height: f32) -> Size {
        self.menus.strip.room = Some(room);
        self.titles_size(height)
    }

    /// The titles' window.
    pub fn menu_titles_window(&self) -> Option<WindowId> {
        self.menus.strip.titles.as_ref().map(|t| t.window)
    }

    fn titles_size(&mut self, height: f32) -> Size {
        let Some(window) = self.menu_titles_window() else {
            return Size::ZERO;
        };
        let natural = self.fitting_size(window).width;
        let room = self.menus.strip.room.unwrap_or(f32::INFINITY);
        Size::new(natural.min(room).max(1.0), height)
    }

    /// The titles follow the menu bar: the Silica mark, then the app's menus (the App menu
    /// as its name, in bold).
    pub(crate) fn sync_menu_titles(&mut self) {
        self.close_title_menu();
        let mut titles = Vec::new();
        if self.menus.strip.available && !self.menus.bar.is_empty() {
            titles.push((TitleKind::Mark, String::new()));
            for m in &self.menus.bar {
                let kind = if m.role.as_deref() == Some("app") {
                    TitleKind::Name
                } else {
                    TitleKind::Menu
                };
                titles.push((kind, m.title.clone()));
            }
        }
        if titles.is_empty() {
            if let Some(t) = self.menus.strip.titles.take() {
                self.close_window(t.window);
            }
            return;
        }
        let (bar, views) = menubar::menu_bar(self, &titles);
        // The system menu is not there yet (silica-os/appkit#25): the mark shows, disabled.
        if let Some(mark) = views.first() {
            let _ = self.set(*mark, Prop::Disabled, Value::Bool(true));
        }
        for (i, v) in views.iter().enumerate() {
            self.connect(*v, Signal::Activated, move |ui, _, _| {
                if ui.menus.strip.open.as_ref().is_some_and(|o| o.title == i) {
                    ui.close_title_menu();
                } else {
                    ui.open_title_menu(i);
                }
            });
            self.connect(*v, Signal::PointerEntered, move |ui, _, _| {
                if ui.menus.strip.open.as_ref().is_some_and(|o| o.title != i) {
                    ui.open_title_menu(i);
                }
            });
        }
        let window = match self.menus.strip.titles.take() {
            Some(t) => t.window,
            None => {
                let w = self.create_window(
                    WindowKind::MenuTitles,
                    Size::new(1.0, menubar::metrics::BAR_HEIGHT),
                );
                self.set_content(w, bar);
                self.menus.strip.titles = Some(Titles {
                    window: w,
                    views: views.clone(),
                });
                let size = self.titles_size(menubar::metrics::BAR_HEIGHT);
                self.resize_window(w, size, 1.0);
                self.show_window(w);
                return;
            }
        };
        self.set_content(window, bar);
        self.menus.strip.titles = Some(Titles { window, views });
        let scale = self.window(window).map_or(1.0, |w| w.scale);
        let size = self.titles_size(menubar::metrics::BAR_HEIGHT);
        self.resize_window(window, size, scale);
        self.request(Request::Resize(window, size));
    }

    /// Opens the menu of title `index` (the open one closes).
    pub fn open_title_menu(&mut self, index: usize) {
        let Some((window, view)) = self
            .menus
            .strip
            .titles
            .as_ref()
            .and_then(|t| Some((t.window, *t.views.get(index)?)))
        else {
            return;
        };
        if self.is_disabled(view) {
            return;
        }
        let Some(menu) = index
            .checked_sub(1)
            .and_then(|i| self.menus.bar.get(i))
            .cloned()
        else {
            return;
        };
        let target = self
            .menus
            .strip
            .open
            .as_ref()
            .map(|o| o.target)
            .unwrap_or_else(|| self.key_window());
        self.close_title_menu();
        let items = self.validated(&menu.items, target);
        let (m, _) = menus::menu(self, &rows_of(&items));
        let anchor = self.tree.window_rect(view);
        let popup = menus::present(self, window, anchor, m, menus::Placement::MenuBar);
        let _ = self.set(view, Prop::Expanded, Value::Bool(true));
        self.menus.strip.open = Some(Open {
            title: index,
            popup,
            target,
        });
        self.connect(m, Signal::Activated, move |ui, _, v| {
            let path = match v {
                Value::Int(i) => vec![*i],
                Value::IntList(p) => p.clone(),
                _ => Vec::new(),
            };
            let chosen = id_at(&items, &path);
            ui.close_title_menu();
            if let Some(id) = chosen {
                ui.choose_bar_item(id, target);
            }
        });
        self.connect(m, Signal::CloseRequested, |ui, _, _| ui.close_title_menu());
        if let Some(root) = self.window(popup).map(|w| w.root) {
            // However it closes (Escape, a click outside, the compositor), the title's capsule
            // goes.
            self.connect(root, Signal::Closed, move |ui, _, _| {
                if ui
                    .menus
                    .strip
                    .open
                    .as_ref()
                    .is_some_and(|o| o.popup == popup)
                {
                    ui.menus.strip.open = None;
                    let _ = ui.set(view, Prop::Expanded, Value::Bool(false));
                }
            });
            // ← and → move to the neighbouring menu.
            self.connect(root, Signal::KeyUnhandled, move |ui, _, v| {
                let step: isize = match v.as_str() {
                    Some("left") => -1,
                    Some("right") => 1,
                    _ => return,
                };
                ui.step_title_menu(index, step);
            });
        }
        self.set_focus(m, true);
    }

    /// Opens the next enabled title's menu from `index`, wrapping around.
    fn step_title_menu(&mut self, index: usize, step: isize) {
        let views = match &self.menus.strip.titles {
            Some(t) => t.views.clone(),
            None => return,
        };
        let n = views.len() as isize;
        let mut i = index as isize;
        for _ in 0..n {
            i = (i + step).rem_euclid(n.max(1));
            if views.get(i as usize).is_some_and(|v| !self.is_disabled(*v)) {
                self.open_title_menu(i as usize);
                return;
            }
        }
    }

    /// Closes the open menu of a title.
    pub fn close_title_menu(&mut self) {
        let Some(open) = self.menus.strip.open.take() else {
            return;
        };
        if let Some(v) = self
            .menus
            .strip
            .titles
            .as_ref()
            .and_then(|t| t.views.get(open.title).copied())
        {
            let _ = self.set(v, Prop::Expanded, Value::Bool(false));
        }
        self.close_window(open.popup);
    }

    /// The open title's index.
    pub fn open_title(&self) -> Option<usize> {
        self.menus.strip.open.as_ref().map(|o| o.title)
    }

    /// The window with the keyboard among the app's windows.
    pub(crate) fn key_window(&self) -> Option<WindowId> {
        self.windows()
            .find(|w| {
                w.active
                    && matches!(
                        w.kind,
                        WindowKind::Normal | WindowKind::Panel | WindowKind::Sheet { .. }
                    )
            })
            .map(|w| w.id)
    }

    /// `items` as they show now: an item is enabled when it is, when it does something (an
    /// action, a submenu, a standard role AppKit runs) and when the validator agrees; without
    /// that it shows disabled, as AppKit's autoenabling menus do (the standard items AppKit
    /// does not run yet, Services: silica-os/appkit#26).
    pub(crate) fn validated(
        &mut self,
        items: &[MenuItem],
        target: Option<WindowId>,
    ) -> Vec<MenuItem> {
        let mut out = items.to_vec();
        for it in &mut out {
            if it.separator || it.heading {
                continue;
            }
            if let Some(sub) = &it.submenu {
                it.submenu = Some(self.validated(sub, target));
            }
            if it.role.as_deref() == Some("fullscreen")
                && it.action.is_none()
                && target
                    .and_then(|w| self.window(w))
                    .is_some_and(|w| w.fullscreen)
            {
                it.title = "Exit Full Screen".into();
            }
            it.enabled = self.can_choose(it, target);
        }
        out
    }

    /// Whether `item` can be chosen now.
    fn can_choose(&mut self, item: &MenuItem, target: Option<WindowId>) -> bool {
        if !item.enabled || item.separator || item.heading {
            return false;
        }
        let acts = item.action.is_some()
            || item.submenu.is_some()
            || match item.role.as_deref() {
                Some("close" | "minimize") => target.is_some(),
                Some("about") => true,
                // A window that can be resized can fill the screen.
                Some("fullscreen") => target
                    .and_then(|w| self.window(w))
                    .is_some_and(|w| w.resizable && w.kind == WindowKind::Normal),
                Some(r) if app_action(r).is_some() => self.menus.strip.app_actions,
                Some(r) if EDIT_ROLES.contains(&r) => {
                    target.is_some_and(|w| self.focus(w).is_some())
                        && item
                            .shortcut
                            .as_deref()
                            .and_then(Shortcut::parse)
                            .and_then(|s| key_of(&s))
                            .is_some()
                }
                _ => false,
            };
        let validator = self.menus.strip.validator.clone();
        acts && validator.is_none_or(|v| v(self, item))
    }

    /// The bar item `id` was chosen (from its menu or by its key equivalent).
    pub(crate) fn choose_bar_item(&mut self, id: u32, target: Option<WindowId>) {
        let Some(item) = crate::core::menu::find(&self.menus.bar, id).cloned() else {
            return;
        };
        if !self.can_choose(&item, target) {
            return;
        }
        if let Some(a) = &item.action {
            a(self);
        } else if let Some(action) = item.role.as_deref().and_then(app_action) {
            self.request(Request::AppAction(action));
        } else if item.role.as_deref() == Some("about") {
            self.show_about();
        } else if let Some(w) = target {
            match item.role.as_deref() {
                Some("close") => self.request_window_close(w),
                Some("minimize") => self.request(Request::Minimize(w)),
                Some("fullscreen") => {
                    let on = self.window(w).is_some_and(|w| !w.fullscreen);
                    self.request(Request::SetFullscreen(w, on));
                }
                Some(r) if EDIT_ROLES.contains(&r) => {
                    let key = item
                        .shortcut
                        .as_deref()
                        .and_then(Shortcut::parse)
                        .and_then(|s| key_of(&s));
                    if let (Some(k), Some(f)) = (key, self.focus(w)) {
                        self.send_bubbling(f, &Event::Key(k));
                    }
                }
                _ => {}
            }
        }
        self.flush();
    }

    /// Asks a window to close, as its close button does: its [`Signal::CloseRequested`]
    /// handlers run, and without [`crate::core::window::Window::keep_on_close_request`] it
    /// closes.
    pub fn request_window_close(&mut self, window: WindowId) {
        if let Some(root) = self.window(window).map(|w| w.root) {
            self.emit(root, Signal::CloseRequested, Value::None);
            self.flush();
        }
        if !self.window(window).is_some_and(|w| w.keep_on_close_request) {
            self.close_window(window);
        }
    }

    // ----- Status items --------------------------------------------------------------------

    /// Adds a status item of the app (`NSStatusItem`), left of the system's items, the newest
    /// leftmost. Returns its window, which identifies it.
    pub fn add_status_item(&mut self, content: ItemContent) -> WindowId {
        self.add_status_item_placed(content, false, 0)
    }

    /// Adds a system status item (Control Center's), `order` 0 rightmost.
    pub fn add_system_status_item(&mut self, content: ItemContent, order: u32) -> WindowId {
        self.add_status_item_placed(content, true, order)
    }

    fn add_status_item_placed(
        &mut self,
        content: ItemContent,
        system: bool,
        order: u32,
    ) -> WindowId {
        let item = self.create(StatusItem::new(content));
        let bar = statusbar::status_bar(self, &[item]);
        let window = self.create_window(
            WindowKind::StatusItem { system, order },
            Size::new(1.0, statusbar::metrics::BAR_HEIGHT),
        );
        self.set_content(window, bar);
        let size = self.fitting_size(window);
        self.resize_window(window, size, 1.0);
        self.connect(item, Signal::Activated, move |ui, _, _| {
            ui.status_item_clicked(window);
        });
        self.menus.strip.items.push(Status {
            window,
            view: item,
            menu: None,
            action: None,
        });
        self.show_window(window);
        window
    }

    fn status(&mut self, item: WindowId) -> Option<&mut Status> {
        self.menus.strip.items.iter_mut().find(|s| s.window == item)
    }

    /// The status item's view (the anchor of its popover).
    pub fn status_item_view(&self, item: WindowId) -> Option<ViewId> {
        self.menus
            .strip
            .items
            .iter()
            .find(|s| s.window == item)
            .map(|s| s.view)
    }

    /// What the item shows: a symbol, a title, or both kinds of the system's items.
    pub fn set_status_item_content(&mut self, item: WindowId, content: ItemContent) {
        let Some(view) = self.status_item_view(item) else {
            return;
        };
        let _ = self.with_widget::<StatusItem, _>(view, |w| w.content = content);
        self.request_layout(view);
        self.fit_status_item(item);
    }

    /// Its width: `None` as wide as its content, or a fixed length.
    pub fn set_status_item_length(&mut self, item: WindowId, length: Option<f32>) {
        let Some(view) = self.status_item_view(item) else {
            return;
        };
        let _ = self.set(
            view,
            Prop::MinWidth,
            length.map_or(Value::None, |l| Value::from(l.max(0.0))),
        );
        self.fit_status_item(item);
    }

    fn fit_status_item(&mut self, item: WindowId) {
        let size = self.fitting_size(item);
        let scale = self.window(item).map_or(1.0, |w| w.scale);
        if self.window(item).is_some_and(|w| w.size != size) {
            self.resize_window(item, size, scale);
            self.request(Request::Resize(item, size));
        }
    }

    /// Its menu, opened by a click (`None`: the action runs instead).
    pub fn set_status_item_menu(&mut self, item: WindowId, menu: Option<Vec<MenuItem>>) {
        if let Some(s) = self.status(item) {
            s.menu = menu;
        }
    }

    /// What a click does when it has no menu.
    pub fn set_status_item_action(&mut self, item: WindowId, action: Option<MenuAction>) {
        if let Some(s) = self.status(item) {
            s.action = action;
        }
    }

    /// Shows or hides it; a hidden item leaves no gap.
    pub fn set_status_item_visible(&mut self, item: WindowId, visible: bool) {
        if self.status_item_view(item).is_some() {
            self.request(Request::SetStatusItemVisible(item, visible));
        }
    }

    /// Removes it.
    pub fn remove_status_item(&mut self, item: WindowId) {
        self.menus.strip.items.retain(|s| s.window != item);
        self.close_window(item);
    }

    fn status_item_clicked(&mut self, item: WindowId) {
        let Some((view, menu, action)) = self
            .menus
            .strip
            .items
            .iter()
            .find(|s| s.window == item)
            .map(|s| (s.view, s.menu.clone(), s.action.clone()))
        else {
            return;
        };
        match (menu, action) {
            (Some(items), _) => {
                let target = self.key_window();
                let items = self.validated(&items, target);
                let size = self.tree.window_rect(view).size();
                let _ = self.set(view, Prop::Expanded, Value::Bool(true));
                let _ = self.popup_menu(
                    view,
                    Rect::from_size(size),
                    PopupKind::MenuBar,
                    0,
                    items,
                    Some(Box::new(move |ui, _| {
                        let _ = ui.set(view, Prop::Expanded, Value::Bool(false));
                    })),
                );
            }
            (None, Some(a)) => a(self),
            (None, None) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

    use super::*;
    use crate::core::event::{Key, KeyEvent, Mods};
    use crate::core::menu::Menu;
    use std::cell::Cell;
    use std::rc::Rc;

    fn menus(hits: &Rc<Cell<u32>>) -> Vec<Menu> {
        let h = hits.clone();
        vec![
            Menu {
                title: "Recipes".into(),
                role: Some("app".into()),
                items: vec![MenuItem::new("About Recipes").role("about")],
            },
            Menu {
                title: "File".into(),
                role: None,
                items: vec![
                    MenuItem::new("New")
                        .shortcut("cmd+n")
                        .action(move |_| h.set(h.get() + 1)),
                    MenuItem::new("Nothing"),
                ],
            },
            Menu {
                title: "Edit".into(),
                role: None,
                items: vec![MenuItem::new("Copy").role("copy").shortcut("cmd+c")],
            },
        ]
    }

    fn popups(ui: &Ui) -> Vec<WindowId> {
        ui.windows()
            .filter(|w| matches!(w.kind, WindowKind::Popup { .. }))
            .map(|w| w.id)
            .collect()
    }

    fn titles(ui: &Ui) -> Vec<ViewId> {
        ui.menus.strip.titles.as_ref().unwrap().views.clone()
    }

    #[test]
    fn the_titles_are_the_mark_the_name_and_the_menus() {
        let mut ui = Ui::new_headless();
        ui.set_menu_bar(menus(&Rc::new(Cell::new(0))));
        assert!(ui.menu_titles_window().is_none(), "no menu bar, no titles");
        ui.set_menu_bar_available(true);
        let window = ui.menu_titles_window().unwrap();
        assert_eq!(ui.window(window).unwrap().kind, WindowKind::MenuTitles);
        let views = titles(&ui);
        assert_eq!(views.len(), 4);
        // The mark waits for the system menu (#25).
        assert!(ui.is_disabled(views[0]));
        // 8 before the mark (36), the bold name and the menus, each its text on the grid + 16.
        let full = ui.window(window).unwrap().size;
        assert_eq!(full.height, 32.0);
        assert!(full.width > 8.0 + 36.0 + 3.0 * 16.0, "{full:?}");
        // The compositor gives less room: the surface shrinks to it.
        let size = ui.menu_titles_configured(100.0, 32.0);
        assert_eq!(size, Size::new(100.0, 32.0));
        ui.set_menu_bar(Vec::new());
        assert!(ui.menu_titles_window().is_none());
    }

    #[test]
    fn a_press_opens_a_menu_and_the_pointer_switches_titles() {
        let mut ui = Ui::new_headless();
        ui.set_menu_bar_available(true);
        ui.set_menu_bar(menus(&Rc::new(Cell::new(0))));
        let views = titles(&ui);
        let window = ui.menu_titles_window().unwrap();
        ui.layout_window(window);
        ui.emit(views[2], Signal::Activated, Value::None);
        ui.flush();
        let open = popups(&ui);
        assert_eq!(open.len(), 1);
        let popup = ui.window(open[0]).unwrap().kind.clone();
        let WindowKind::Popup { parent, anchor, .. } = popup else {
            panic!("{popup:?}");
        };
        assert_eq!(parent, window);
        // 5 left of the title, 8 below the bar.
        let title = ui.tree.window_rect(views[2]);
        assert_eq!((anchor.x, anchor.y), (title.x - 5.0, 40.0));
        assert_eq!(ui.open_title(), Some(2));
        assert_eq!(ui.get(views[2], Prop::Expanded), Some(Value::Bool(true)));

        // Onto another title: its menu replaces the open one.
        ui.emit(views[3], Signal::PointerEntered, Value::None);
        ui.flush();
        assert_eq!(ui.open_title(), Some(3));
        assert_eq!(popups(&ui).len(), 1);
        assert_eq!(ui.get(views[2], Prop::Expanded), Some(Value::Bool(false)));
        // Not onto the disabled mark.
        ui.emit(views[0], Signal::PointerEntered, Value::None);
        ui.flush();
        assert_eq!(ui.open_title(), Some(3));

        // ← moves to the menu before it.
        let popup = popups(&ui)[0];
        ui.key(popup, KeyEvent::press(Key::Left, Mods::empty()));
        assert_eq!(ui.open_title(), Some(2));

        // A press on the open title closes it; the pointer alone opens nothing then.
        ui.emit(views[2], Signal::Activated, Value::None);
        ui.flush();
        assert!(popups(&ui).is_empty());
        assert_eq!(ui.open_title(), None);
        ui.emit(views[3], Signal::PointerEntered, Value::None);
        ui.flush();
        assert!(popups(&ui).is_empty());
    }

    #[test]
    fn escape_closes_the_menu() {
        let mut ui = Ui::new_headless();
        ui.set_menu_bar_available(true);
        ui.set_menu_bar(menus(&Rc::new(Cell::new(0))));
        let views = titles(&ui);
        ui.emit(views[2], Signal::Activated, Value::None);
        ui.flush();
        let popup = popups(&ui)[0];
        ui.key(popup, KeyEvent::press(Key::Escape, Mods::empty()));
        assert!(popups(&ui).is_empty());
        assert_eq!(ui.get(views[2], Prop::Expanded), Some(Value::Bool(false)));
    }

    #[test]
    fn items_validate_before_showing_and_before_their_key_equivalent() {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(100.0, 100.0));
        let hits = Rc::new(Cell::new(0));
        ui.set_menu_bar(menus(&hits));
        let file = ui.menu_bar()[1].items.clone();
        // An item that does nothing shows disabled.
        let shown = ui.validated(&file, Some(win));
        assert!(shown[0].enabled);
        assert!(!shown[1].enabled);
        // The validator turns New off: it is disabled, and ⌘N does nothing.
        let allow = Rc::new(Cell::new(false));
        let a = allow.clone();
        ui.set_menu_validator(Some(Rc::new(move |_, it| it.title != "New" || a.get())));
        assert!(!ui.validated(&file, Some(win))[0].enabled);
        ui.key(win, KeyEvent::press(Key::Char('n'), Mods::LOGO));
        assert_eq!(hits.get(), 0);
        allow.set(true);
        ui.key(win, KeyEvent::press(Key::Char('n'), Mods::LOGO));
        assert_eq!(hits.get(), 1);
    }

    #[test]
    fn the_compositor_runs_the_app_items_and_full_screen_toggles() {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(100.0, 100.0));
        ui.set_window_active(win, true);
        let info = crate::core::app::AppInfo {
            name: "Recipes".into(),
            ..Default::default()
        };
        let window = Menu {
            title: "Window".into(),
            role: Some("window".into()),
            items: vec![
                crate::core::app::system_item("fullscreen"),
                crate::core::app::system_item("bring_all_to_front"),
            ],
        };
        ui.set_menu_bar(vec![
            crate::core::app::app_menu(&info, None, Vec::new()),
            window,
        ]);
        let app = ui.menu_bar()[0].items.clone();
        let enabled = |ui: &mut Ui, items: &[MenuItem], role: &str| {
            ui.validated(items, Some(win))
                .iter()
                .find(|i| i.role.as_deref() == Some(role))
                .unwrap()
                .enabled
        };
        // An older compositor runs none of them; Services stays off.
        assert!(!enabled(&mut ui, &app, "hide"));
        ui.set_app_actions_available(true);
        for role in ["hide", "hide_others", "show_all"] {
            assert!(enabled(&mut ui, &app, role), "{role}");
        }
        assert!(!enabled(&mut ui, &app, "services"));
        // ⌘H asks the compositor to hide the app.
        ui.take_requests();
        ui.key(win, KeyEvent::press(Key::Char('h'), Mods::LOGO));
        assert!(
            ui.take_requests()
                .contains(&Request::AppAction(AppAction::Hide))
        );

        // ⌃⌘F fills the screen; once it does, the item reads Exit Full Screen.
        let items = ui.menu_bar()[1].items.clone();
        assert!(enabled(&mut ui, &items, "bring_all_to_front"));
        ui.key(
            win,
            KeyEvent::press(Key::Char('f'), Mods::LOGO | Mods::CTRL),
        );
        assert!(
            ui.take_requests()
                .contains(&Request::SetFullscreen(win, true))
        );
        ui.set_window_fullscreen(win, true);
        assert_eq!(ui.validated(&items, Some(win))[0].title, "Exit Full Screen");
        ui.key(
            win,
            KeyEvent::press(Key::Char('f'), Mods::LOGO | Mods::CTRL),
        );
        assert!(
            ui.take_requests()
                .contains(&Request::SetFullscreen(win, false))
        );
        // A window of fixed size cannot.
        ui.window_mut(win).unwrap().resizable = false;
        assert!(!enabled(&mut ui, &items, "fullscreen"));
    }

    #[test]
    fn a_status_item_opens_its_menu_under_it() {
        let mut ui = Ui::new_headless();
        ui.set_menu_bar_available(true);
        let item = ui.add_status_item(ItemContent::Title("Timer".into()));
        assert_eq!(
            ui.window(item).unwrap().kind,
            WindowKind::StatusItem {
                system: false,
                order: 0
            }
        );
        let size = ui.window(item).unwrap().size;
        assert_eq!(size.height, 32.0);
        assert!(size.width > 16.0, "{size:?}");
        // A fixed length.
        ui.set_status_item_length(item, Some(80.0));
        assert_eq!(ui.window(item).unwrap().size.width, 80.0);

        let hits = Rc::new(Cell::new(0));
        let h = hits.clone();
        ui.set_status_item_menu(
            item,
            Some(vec![
                MenuItem::new("Start").action(move |_| h.set(h.get() + 1)),
            ]),
        );
        let view = ui.status_item_view(item).unwrap();
        ui.emit(view, Signal::Activated, Value::None);
        ui.flush();
        let open = popups(&ui);
        assert_eq!(open.len(), 1);
        assert!(
            matches!(ui.window(open[0]).unwrap().kind, WindowKind::Popup { parent, .. } if parent == item)
        );
        assert_eq!(ui.get(view, Prop::Expanded), Some(Value::Bool(true)));
        let root = ui.window(open[0]).unwrap().root;
        let menu = ui.tree.children(root)[0];
        ui.emit(menu, Signal::Activated, Value::Int(0));
        ui.flush();
        assert_eq!(hits.get(), 1);
        assert!(popups(&ui).is_empty());
        assert_eq!(ui.get(view, Prop::Expanded), Some(Value::Bool(false)));

        ui.take_requests();
        ui.set_status_item_visible(item, false);
        assert!(
            ui.take_requests()
                .contains(&Request::SetStatusItemVisible(item, false))
        );
        ui.remove_status_item(item);
        assert!(ui.window(item).is_none());
    }
}
