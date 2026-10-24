//! Menus through the C ABI: built with `ak_menu_*`, handed to the menu bar or a pop-up.

use std::ffi::{c_char, c_void};
use std::rc::Rc;

use appkit_sys::*;

use super::{
    UserData, enter, guard, opt_str, req_str, set_last, view_id, window_id, window_ptr, with_ui,
};
use crate::amalfi::statusbar::ItemContent;
use crate::core::Ui;
use crate::core::menu::{Menu, MenuAction, MenuItem, PopupKind};
use crate::core::menu_bar::MenuValidator;
use crate::core::window::WindowId;
use crate::geom::Rect;

/// A menu being built (behind `AKMenu *`).
struct Building {
    menu: Menu,
}

fn building<'a>(m: *mut AKMenu) -> Option<&'a mut Building> {
    if m.is_null() {
        None
    } else {
        // SAFETY: the pointer came from ak_menu_new and was not freed or consumed.
        Some(unsafe { &mut *(m as *mut Building) })
    }
}

/// Takes a built menu back (consumes the handle).
pub(super) fn take_menu(m: *mut AKMenu) -> Option<Menu> {
    take(m).map(|b| b.menu)
}

fn take(m: *mut AKMenu) -> Option<Box<Building>> {
    if m.is_null() {
        None
    } else {
        // SAFETY: as above; ownership moves back to Rust exactly once.
        Some(unsafe { Box::from_raw(m as *mut Building) })
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_menu_new(title: *const c_char, role: *const c_char) -> *mut AKMenu {
    guard(std::ptr::null_mut(), || {
        let (Ok(title), Ok(role)) = (opt_str(title), opt_str(role)) else {
            set_last(AK_ERROR_INVALID_ARGUMENT);
            return std::ptr::null_mut();
        };
        let b = Box::new(Building {
            menu: Menu {
                title: title.unwrap_or_default().to_string(),
                role: role.map(str::to_string),
                items: Vec::new(),
            },
        });
        set_last(AK_OK);
        Box::into_raw(b) as *mut AKMenu
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_menu_free(menu: *mut AKMenu) {
    guard((), || drop(take(menu)));
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_menu_add_item(
    menu: *mut AKMenu,
    title: *const c_char,
    shortcut: *const c_char,
    id: u32,
    flags: u32,
) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        let (Some(b), Ok(title), Ok(shortcut)) =
            (building(menu), req_str(title), opt_str(shortcut))
        else {
            return set_last(AK_ERROR_INVALID_ARGUMENT);
        };
        if id == 0 && flags & AK_MENU_HEADING == 0 {
            return set_last(AK_ERROR_INVALID_ARGUMENT);
        }
        let mut item = if flags & AK_MENU_HEADING != 0 {
            MenuItem::heading(title)
        } else {
            MenuItem::new(title)
        };
        item.id = id;
        item.shortcut = shortcut.map(str::to_string);
        item.enabled = flags & AK_MENU_DISABLED == 0;
        item.checked = if flags & AK_MENU_CHECKED != 0 {
            Some(true)
        } else if flags & AK_MENU_MIXED != 0 {
            Some(false)
        } else {
            None
        };
        item.destructive = flags & AK_MENU_DESTRUCTIVE != 0;
        b.menu.items.push(item);
        set_last(AK_OK)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_menu_add_separator(menu: *mut AKMenu) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || match building(menu) {
        Some(b) => {
            b.menu.items.push(MenuItem::separator());
            set_last(AK_OK)
        }
        None => set_last(AK_ERROR_INVALID_ARGUMENT),
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_menu_set_icon(menu: *mut AKMenu, symbol: *const c_char) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        let (Some(b), Ok(symbol)) = (building(menu), req_str(symbol)) else {
            return set_last(AK_ERROR_INVALID_ARGUMENT);
        };
        match b.menu.items.last_mut() {
            Some(item) if !item.separator => {
                item.icon = Some(symbol.to_string());
                set_last(AK_OK)
            }
            _ => set_last(AK_ERROR_INVALID_ARGUMENT),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_menu_add_submenu(menu: *mut AKMenu, submenu: *mut AKMenu) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        if menu == submenu {
            return set_last(AK_ERROR_INVALID_ARGUMENT);
        }
        let Some(b) = building(menu) else {
            return set_last(AK_ERROR_INVALID_ARGUMENT);
        };
        let Some(sub) = take(submenu) else {
            return set_last(AK_ERROR_INVALID_ARGUMENT);
        };
        b.menu
            .items
            .push(MenuItem::submenu(&sub.menu.title, sub.menu.items));
        set_last(AK_OK)
    })
}

/// Gives every item with an id an action calling `f(id)`.
fn wire(items: &mut [MenuItem], f: unsafe extern "C" fn(u32, *mut c_void), data: &Rc<UserData>) {
    for it in items {
        if let Some(sub) = &mut it.submenu {
            wire(sub, f, data);
        } else if it.id != 0 && !it.separator {
            let id = it.id;
            let d = data.clone();
            it.action = Some(Rc::new(move |ui| {
                let user = d.user;
                // SAFETY: the caller's callback with its own data.
                enter(ui, || unsafe { f(id, user) });
            }));
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_app_set_menu_bar(
    menus: *const *mut AKMenu,
    count: usize,
    callback: AKMenuFn,
    user: *mut c_void,
    destroy: AKDestroy,
) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        if menus.is_null() && count > 0 {
            return set_last(AK_ERROR_INVALID_ARGUMENT);
        }
        let data = Rc::new(UserData::new(user, destroy));
        let mut list = Vec::new();
        for i in 0..count {
            // SAFETY: the caller passes `count` menu pointers; each is consumed.
            let m = unsafe { *menus.add(i) };
            if let Some(mut b) = take(m) {
                if let Some(f) = callback {
                    wire(&mut b.menu.items, f, &data);
                }
                list.push(b.menu);
            }
        }
        match with_ui(|ui| ui.set_menu_bar(list)) {
            Ok(()) => set_last(AK_OK),
            Err(e) => set_last(e),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_popup_menu(
    view: *mut AKView,
    menu: *mut AKMenu,
    anchor: AKRect,
    kind: u32,
    selected: u32,
    callback: AKMenuFn,
    user: *mut c_void,
    destroy: AKDestroy,
) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        let Some(b) = take(menu) else {
            return set_last(AK_ERROR_INVALID_ARGUMENT);
        };
        let v = match view_id(view) {
            Ok(v) => v,
            Err(e) => return set_last(e),
        };
        let kind = if kind == AK_POPUP_POP_UP {
            PopupKind::PopUp
        } else {
            PopupKind::Context
        };
        let data = UserData::new(user, destroy);
        let done: Option<crate::core::menu::PopupDone> = callback.map(|f| {
            Box::new(move |ui: &mut crate::core::Ui, chosen: Option<u32>| {
                let d = data;
                // SAFETY: the caller's callback with its own data.
                enter(ui, || unsafe { f(chosen.unwrap_or(0), d.user) });
            }) as crate::core::menu::PopupDone
        });
        let rect = Rect::new(anchor.x, anchor.y, anchor.width, anchor.height);
        match with_ui(|ui| ui.popup_menu(v, rect, kind, selected, b.menu.items, done)) {
            Ok(Some(_)) => set_last(AK_OK),
            Ok(None) => set_last(AK_ERROR_NO_SUCH_OBJECT),
            Err(e) => set_last(e),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_app_set_menu_validator(
    validate: AKMenuValidateFn,
    user: *mut c_void,
    destroy: AKDestroy,
) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        let data = UserData::new(user, destroy);
        let validator: Option<MenuValidator> = validate.map(|f| {
            Rc::new(move |ui: &mut Ui, item: &MenuItem| {
                let (id, user) = (item.id, data.user);
                // SAFETY: the caller's callback with its own data.
                id == 0 || enter(ui, || unsafe { f(id, user) }) != 0
            }) as MenuValidator
        });
        match with_ui(|ui| ui.set_menu_validator(validator)) {
            Ok(()) => set_last(AK_OK),
            Err(e) => set_last(e),
        }
    })
}

/// A status item's content: its symbol, else its title.
fn item_content(symbol: *const c_char, title: *const c_char) -> Result<ItemContent, AKStatus> {
    Ok(match opt_str(symbol)? {
        Some(s) => ItemContent::Glyph(s.to_string()),
        None => ItemContent::Title(opt_str(title)?.unwrap_or_default().to_string()),
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_status_item_new(
    symbol: *const c_char,
    title: *const c_char,
    placement: u32,
    order: u32,
) -> *mut AKWindow {
    guard(std::ptr::null_mut(), || {
        let content = match item_content(symbol, title) {
            Ok(c) => c,
            Err(e) => {
                set_last(e);
                return std::ptr::null_mut();
            }
        };
        match with_ui(|ui| {
            if placement == AK_STATUS_ITEM_SYSTEM {
                ui.add_system_status_item(content, order)
            } else {
                ui.add_status_item(content)
            }
        }) {
            Ok(w) => {
                set_last(AK_OK);
                window_ptr(w)
            }
            Err(e) => {
                set_last(e);
                std::ptr::null_mut()
            }
        }
    })
}

/// Runs `f` on a status item: `AK_ERROR_NO_SUCH_OBJECT` for any other window.
fn with_item(item: *mut AKWindow, f: impl FnOnce(&mut Ui, WindowId)) -> AKStatus {
    let r = window_id(item).and_then(|w| {
        with_ui(|ui| {
            if ui.status_item_view(w).is_none() {
                return Err(AK_ERROR_NO_SUCH_OBJECT);
            }
            f(ui, w);
            Ok(())
        })?
    });
    set_last(r.err().unwrap_or(AK_OK))
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_status_item_set_content(
    item: *mut AKWindow,
    symbol: *const c_char,
    title: *const c_char,
) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || match item_content(symbol, title) {
        Ok(c) => with_item(item, |ui, w| ui.set_status_item_content(w, c)),
        Err(e) => set_last(e),
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_status_item_set_length(item: *mut AKWindow, length: f32) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        let length = (length.is_finite() && length >= 0.0).then_some(length);
        with_item(item, |ui, w| ui.set_status_item_length(w, length))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_status_item_set_menu(
    item: *mut AKWindow,
    menu: *mut AKMenu,
    callback: AKMenuFn,
    user: *mut c_void,
    destroy: AKDestroy,
) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        let data = Rc::new(UserData::new(user, destroy));
        let items = take(menu).map(|mut b| {
            if let Some(f) = callback {
                wire(&mut b.menu.items, f, &data);
            }
            b.menu.items
        });
        with_item(item, |ui, w| ui.set_status_item_menu(w, items))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_status_item_set_action(
    item: *mut AKWindow,
    callback: AKCallback,
    user: *mut c_void,
    destroy: AKDestroy,
) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        let data = UserData::new(user, destroy);
        let action: Option<MenuAction> = callback.map(|f| {
            Rc::new(move |ui: &mut Ui| {
                let user = data.user;
                // SAFETY: the caller's callback with its own data.
                enter(ui, || unsafe { f(user) });
            }) as MenuAction
        });
        with_item(item, |ui, w| ui.set_status_item_action(w, action))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_status_item_set_visible(item: *mut AKWindow, visible: u8) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        with_item(item, |ui, w| ui.set_status_item_visible(w, visible != 0))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_status_item_remove(item: *mut AKWindow) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        with_item(item, |ui, w| ui.remove_status_item(w))
    })
}
