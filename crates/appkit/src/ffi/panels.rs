//! Alerts, popovers, menus as views, window tabs' and windows' state through the C ABI.

use std::ffi::{c_char, c_void};
use std::rc::Rc;

use appkit_sys::*;

use super::{
    UserData, enter, guard, opt_str, req_str, set_last, view_id, view_ptr, window_id, window_ptr,
    with_ui,
};
use crate::amalfi::alerts::{AlertButton, AlertSpec, ButtonKind, alert};
use crate::amalfi::label::SymbolImage;
use crate::amalfi::menus::{Row, RowKind, menu};
use crate::amalfi::panels::{
    FileEntry, FilePanelSpec, Mode as PanelMode, color_picker, file_panel,
};
use crate::amalfi::popovers::{self, Side};
use crate::color::Color;
use crate::core::event::Shortcut;
use crate::core::layout::Proposal;
use crate::core::menu::MenuItem;
use crate::core::ui::Ui;
use crate::core::value::Signal;
use crate::core::window::WindowKind;

fn null_with<T>(s: AKStatus) -> *mut T {
    set_last(s);
    std::ptr::null_mut()
}

/// Makes an alert: a title, an optional message, buttons in visual order (the last one, or
/// the first when stacked, is the default; with `AK_ALERT_DESTRUCTIVE` it is
/// destructive) and an optional symbol as its icon. The view emits `ACTIVATED` with the
/// index of the button pressed and `CLOSE_REQUESTED` on Escape.
#[unsafe(no_mangle)]
pub extern "C" fn ak_alert_new(
    title: *const c_char,
    message: *const c_char,
    buttons: *const *const c_char,
    count: usize,
    flags: u32,
    icon: *const c_char,
) -> *mut AKView {
    guard(std::ptr::null_mut(), || {
        let (Ok(title), Ok(message), Ok(icon)) = (req_str(title), opt_str(message), opt_str(icon))
        else {
            return null_with(AK_ERROR_INVALID_ARGUMENT);
        };
        if buttons.is_null() && count > 0 {
            return null_with(AK_ERROR_INVALID_ARGUMENT);
        }
        let mut titles = Vec::with_capacity(count.min(16));
        for i in 0..count.min(16) {
            // SAFETY: the caller passes `count` string pointers.
            let p = unsafe { *buttons.add(i) };
            match req_str(p) {
                Ok(s) => titles.push(s.to_string()),
                Err(e) => return null_with(e),
            }
        }
        let stacked = flags & AK_ALERT_STACKED != 0;
        let default = if stacked {
            0
        } else {
            titles.len().saturating_sub(1)
        };
        let default_kind = if flags & AK_ALERT_DESTRUCTIVE != 0 {
            ButtonKind::Destructive
        } else {
            ButtonKind::Primary
        };
        let r = with_ui(|ui| {
            let icon = icon.filter(|s| !s.is_empty()).map(|s| {
                let mut img = SymbolImage::new(s);
                img.size = 56.0;
                ui.create(img)
            });
            let spec = AlertSpec {
                icon,
                title: title.to_string(),
                message: message.map(str::to_string),
                buttons: titles
                    .iter()
                    .enumerate()
                    .map(|(i, t)| {
                        AlertButton::new(
                            t,
                            if i == default {
                                default_kind
                            } else {
                                ButtonKind::Secondary
                            },
                        )
                    })
                    .collect(),
                stacked,
                wide_buttons: stacked,
                glass: flags & AK_ALERT_GLASS != 0,
                badge: flags & AK_ALERT_CRITICAL != 0,
                ..AlertSpec::default()
            };
            alert(ui, spec)
        });
        match r {
            Ok(v) => {
                set_last(AK_OK);
                view_ptr(v)
            }
            Err(s) => null_with(s),
        }
    })
}

/// Adds an accessory view (a suppression checkbox, a field) to an alert, above its buttons.
#[unsafe(no_mangle)]
pub extern "C" fn ak_alert_add_accessory(alert: *mut AKView, view: *mut AKView) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        let r = (|| {
            let a = view_id(alert)?;
            let v = view_id(view)?;
            with_ui(|ui| {
                if ui.tree.get(a).map(|n| n.kind()) != Some("Alert") {
                    return Err(AK_ERROR_INVALID_ARGUMENT);
                }
                let kids = ui.tree.children(a).to_vec();
                // The buttons' row is the last child when there are buttons.
                let at = match kids.last() {
                    Some(last)
                        if ui.tree.get(*last).and_then(|n| n.identifier.as_deref())
                            == Some("Buttons") =>
                    {
                        kids.len() - 1
                    }
                    _ => kids.len(),
                };
                ui.insert_child(a, at, v).map_err(super::status_of)
            })?
        })();
        set_last(r.err().unwrap_or(AK_OK))
    })
}

/// Shows an alert: as a sheet of `parent`, or in a panel of its own when `parent` is null.
/// `callback` gets the index of the button pressed, or -1 for Escape; the window then
/// closes. Returns the window.
#[unsafe(no_mangle)]
pub extern "C" fn ak_window_present_alert(
    parent: *mut AKWindow,
    alert: *mut AKView,
    callback: AKAlertFn,
    user: *mut c_void,
    destroy: AKDestroy,
) -> *mut AKWindow {
    guard(std::ptr::null_mut(), || {
        let data = Rc::new(UserData::new(user, destroy));
        let r = (|| {
            let a = view_id(alert)?;
            let parent = if parent.is_null() {
                None
            } else {
                Some(window_id(parent)?)
            };
            with_ui(move |ui| {
                if ui.tree.get(a).map(|n| n.kind()) != Some("Alert") {
                    return Err(AK_ERROR_INVALID_ARGUMENT);
                }
                let size = ui.measure(a, Proposal::new(None, None));
                let kind = match parent {
                    Some(p) => WindowKind::Sheet { parent: p },
                    None => WindowKind::Panel,
                };
                let w = ui.create_window(kind, size);
                if let Some(win) = ui.window_mut(w) {
                    // The alert draws its own panel, with no title bar.
                    win.opaque = false;
                    win.decorated = false;
                }
                ui.set_content(w, a);
                ui.show_window(w);
                let respond = move |ui: &mut Ui, response: i64| {
                    if let Some(f) = callback {
                        let user = data.user;
                        // SAFETY: the caller's callback with its own data.
                        enter(ui, || unsafe { f(response, user) });
                    }
                    ui.dismiss_window(w);
                };
                let r1 = Rc::new(respond);
                let r2 = r1.clone();
                ui.connect(a, Signal::Activated, move |ui, _, v| {
                    r1(ui, v.as_int().unwrap_or(-1));
                });
                ui.connect(a, Signal::CloseRequested, move |ui, _, _| r2(ui, -1));
                Ok(w)
            })?
        })();
        match r {
            Ok(w) => {
                set_last(AK_OK);
                window_ptr(w)
            }
            Err(s) => null_with(s),
        }
    })
}

/// Shows `content` in a popover against `anchor` (on `side`: `AK_SIDE_*`, flipped when it
/// does not fit). `closed` runs once when it goes (a click outside, Escape, or
/// `ak_window_close`). Returns the popover's window.
#[unsafe(no_mangle)]
pub extern "C" fn ak_view_present_popover(
    anchor: *mut AKView,
    content: *mut AKView,
    side: u32,
    closed: AKCallback,
    user: *mut c_void,
    destroy: AKDestroy,
) -> *mut AKWindow {
    guard(std::ptr::null_mut(), || {
        let data = UserData::new(user, destroy);
        let r = (|| {
            let at = view_id(anchor)?;
            let c = view_id(content)?;
            let side = match side {
                AK_SIDE_BELOW => Side::Below,
                AK_SIDE_ABOVE => Side::Above,
                AK_SIDE_AFTER => Side::After,
                AK_SIDE_BEFORE => Side::Before,
                _ => return Err(AK_ERROR_INVALID_ARGUMENT),
            };
            with_ui(move |ui| {
                let parent = ui.window_of(at).ok_or(AK_ERROR_BAD_HIERARCHY)?;
                let rect = ui.tree.window_rect(at);
                let w = popovers::present(ui, parent, rect, c, side, 12.0);
                let root = ui.window(w).map(|w| w.root);
                if let Some(root) = root {
                    let mut once = Some(data);
                    ui.connect(root, Signal::Closed, move |ui, _, _| {
                        if let Some(d) = once.take()
                            && let Some(f) = closed
                        {
                            let user = d.user;
                            // SAFETY: the caller's callback with its own data.
                            enter(ui, || unsafe { f(user) });
                        }
                    });
                }
                Ok(w)
            })?
        })();
        match r {
            Ok(w) => {
                set_last(AK_OK);
                window_ptr(w)
            }
            Err(s) => null_with(s),
        }
    })
}

/// The rows of `items` and their ids: each row's own and its submenu's.
fn rows_of(items: &[MenuItem]) -> (Vec<Row>, Vec<(u32, Vec<u32>)>) {
    let mut rows = Vec::new();
    let mut ids = Vec::new();
    for it in items {
        let kind = if it.separator {
            RowKind::Separator
        } else if it.heading {
            RowKind::Header
        } else {
            RowKind::Item
        };
        rows.push(Row {
            kind,
            label: it.title.clone(),
            shortcut: it
                .shortcut
                .as_deref()
                .and_then(Shortcut::parse)
                .map(|s| s.display()),
            icon: it.icon.clone(),
            checked: it.checked == Some(true),
            submenu: it.submenu.is_some(),
            sub: it
                .submenu
                .as_ref()
                .map(|s| rows_of(s).0)
                .unwrap_or_default(),
            destructive: it.destructive,
            disabled: !it.enabled,
        });
        ids.push((
            it.id,
            it.submenu
                .as_ref()
                .map(|s| s.iter().map(|i| i.id).collect())
                .unwrap_or_default(),
        ));
    }
    (rows, ids)
}

/// Makes a menu panel view (the kit's `Menus/Menu`) of `menu`'s items, to put in a window
/// of the app's own (a shell's drop-down, a popover); the menu is consumed. Choosing an
/// item calls `callback` with its id.
#[unsafe(no_mangle)]
pub extern "C" fn ak_menu_view_new(
    menu_ptr: *mut AKMenu,
    callback: AKMenuFn,
    user: *mut c_void,
    destroy: AKDestroy,
) -> *mut AKView {
    guard(std::ptr::null_mut(), || {
        let data = UserData::new(user, destroy);
        let Some(m) = super::menu::take_menu(menu_ptr) else {
            return null_with(AK_ERROR_INVALID_ARGUMENT);
        };
        let (rows, ids) = rows_of(&m.items);
        let r = with_ui(move |ui| {
            let (v, _) = menu(ui, &rows);
            ui.connect(v, Signal::Activated, move |ui, _, val| {
                let id = match val {
                    crate::core::Value::IntList(p) => match p.as_slice() {
                        [i, j] => usize::try_from(*i)
                            .ok()
                            .and_then(|i| ids.get(i))
                            .and_then(|(_, sub)| sub.get(usize::try_from(*j).unwrap_or(usize::MAX)))
                            .copied()
                            .unwrap_or(0),
                        _ => 0,
                    },
                    _ => val
                        .as_int()
                        .and_then(|i| usize::try_from(i).ok())
                        .and_then(|i| ids.get(i).map(|(id, _)| *id))
                        .unwrap_or(0),
                };
                if id != 0
                    && let Some(f) = callback
                {
                    let d = &data;
                    let user = d.user;
                    // SAFETY: the caller's callback with its own data.
                    enter(ui, || unsafe { f(id, user) });
                }
            });
            v
        });
        match r {
            Ok(v) => {
                set_last(AK_OK);
                view_ptr(v)
            }
            Err(s) => null_with(s),
        }
    })
}

/// Makes the colour panel's content: `count` swatches (0xRRGGBB) in rows of `columns`, with
/// `selected` (0xRRGGBB, or `u32::MAX` for none) marked; `AK_PANEL_SWATCHES_ONLY` leaves out
/// the pad, the hue slider and the hex field. The view emits `VALUE_CHANGED` with the colour.
#[unsafe(no_mangle)]
pub extern "C" fn ak_color_panel_new(
    colors: *const u32,
    count: usize,
    columns: usize,
    selected: u32,
    flags: u32,
) -> *mut AKView {
    guard(std::ptr::null_mut(), || {
        if (colors.is_null() && count > 0) || columns == 0 || count > 4096 {
            return null_with(AK_ERROR_INVALID_ARGUMENT);
        }
        let list: &[u32] = if count == 0 {
            &[]
        } else {
            // SAFETY: the caller passes `count` colours.
            unsafe { std::slice::from_raw_parts(colors, count) }
        };
        let rgb = |v: u32| Color::rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
        let rows: Vec<Vec<(String, Color)>> = list
            .chunks(columns)
            .enumerate()
            .map(|(r, row)| {
                row.iter()
                    .enumerate()
                    .map(|(c, v)| (format!("Colour {}", r * columns + c + 1), rgb(*v)))
                    .collect()
            })
            .collect();
        let current = if selected == u32::MAX {
            list.first().map_or(Color::rgb(0, 0, 0), |v| rgb(*v))
        } else {
            rgb(selected)
        };
        let r = with_ui(|ui| color_picker(ui, &rows, current, flags & AK_PANEL_SWATCHES_ONLY != 0));
        match r {
            Ok(v) => {
                set_last(AK_OK);
                view_ptr(v)
            }
            Err(s) => null_with(s),
        }
    })
}

/// Makes an open (`AK_PANEL_OPEN`) or save panel over `count` file names, expanded with
/// `AK_PANEL_EXPANDED`; `name` is the save panel's "Save As:" text. The view emits `ACTIVATED`
/// with "primary", "secondary", "new-folder", "back" or "forward".
#[unsafe(no_mangle)]
pub extern "C" fn ak_file_panel_new(
    flags: u32,
    name: *const c_char,
    files: *const *const c_char,
    count: usize,
) -> *mut AKView {
    guard(std::ptr::null_mut(), || {
        let Ok(name) = opt_str(name) else {
            return null_with(AK_ERROR_INVALID_ARGUMENT);
        };
        if files.is_null() && count > 0 {
            return null_with(AK_ERROR_INVALID_ARGUMENT);
        }
        let mut entries = Vec::with_capacity(count.min(1024));
        for i in 0..count.min(1024) {
            // SAFETY: the caller passes `count` string pointers.
            let p = unsafe { *files.add(i) };
            match req_str(p) {
                Ok(s) => entries.push(FileEntry::new(s, s, "doc", "Document", "Today")),
                Err(e) => return null_with(e),
            }
        }
        let mut spec = FilePanelSpec::new(if flags & AK_PANEL_OPEN != 0 {
            PanelMode::Open
        } else {
            PanelMode::Save
        });
        spec.expanded = flags & AK_PANEL_EXPANDED != 0;
        spec.name = name.unwrap_or_default().to_string();
        spec.locations = vec!["Documents".to_string()];
        spec.places = vec![(
            "Favourites".to_string(),
            vec![
                ("documents".into(), "Documents".into(), "folder".into()),
                ("downloads".into(), "Downloads".into(), "folder".into()),
            ],
        )];
        spec.files = entries;
        let r = with_ui(|ui| file_panel(ui, &spec).root);
        match r {
            Ok(v) => {
                set_last(AK_OK);
                view_ptr(v)
            }
            Err(s) => null_with(s),
        }
    })
}

/// Gives a window an autosave name: its size and its identified views' state (column
/// widths, sort, expanded nodes, split positions, selected tabs) are restored now and kept
/// when it closes or the app quits.
#[unsafe(no_mangle)]
pub extern "C" fn ak_window_set_autosave(window: *mut AKWindow, name: *const c_char) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        let r = (|| {
            let w = window_id(window)?;
            let name = req_str(name)?;
            if name.is_empty() {
                return Err(AK_ERROR_INVALID_ARGUMENT);
            }
            with_ui(|ui| ui.set_autosave(w, name))
        })();
        set_last(r.err().unwrap_or(AK_OK))
    })
}
