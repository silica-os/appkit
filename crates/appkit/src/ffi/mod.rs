//! The C ABI (`appkit.h`): the definitions of every function of `appkit_sys::for_each_function!`,
//! checked against that list at compile time (see the bottom of this file).
//!
//! The runtime is reached through [`with_ui`]: inside `ak_app_run` (and every callback it
//! makes) the current `Ui` is the platform's; after `ak_headless_init` it is an owned,
//! headless one. Callbacks run with the `Ui` that invoked them made current, so C code can
//! call back into the ABI. Every entry point catches panics (none are expected: a panic is a
//! bug, reported as an error instead of crossing the boundary).

#![allow(
    unsafe_code,
    clippy::missing_safety_doc,
    clippy::not_unsafe_ptr_arg_deref
)]

mod canvas;
mod collections;
mod kinds;
mod menu;
mod panels;

use std::cell::{Cell, RefCell};
use std::ffi::{CStr, c_char, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::OnceLock;
use std::thread::ThreadId;

use appkit_sys::*;

use crate::color::Color;
use crate::core::ViewId;
use crate::core::event::{Button, Event, Handled, Key, Mods};
use crate::core::layout::Proposal;
use crate::core::ui::{Error, Ui};
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, FocusKind, LayoutCx, PaintCx, Widget};
use crate::core::window::{Edges, KeyboardMode, Layer, WindowId, WindowKind};
use crate::geom::{Insets, Point, Rect, Size};

thread_local! {
    static CURRENT: Cell<*mut Ui> = const { Cell::new(std::ptr::null_mut()) };
    static OWNED: RefCell<Option<Box<Ui>>> = const { RefCell::new(None) };
    static LAST: Cell<AKStatus> = const { Cell::new(AK_OK) };
}

static MAIN: OnceLock<ThreadId> = OnceLock::new();

fn status_of(e: Error) -> AKStatus {
    e as i32
}

fn set_last(s: AKStatus) -> AKStatus {
    LAST.with(|c| c.set(s));
    s
}

/// Runs `f` with the current runtime.
pub(crate) fn with_ui<R>(f: impl FnOnce(&mut Ui) -> R) -> Result<R, AKStatus> {
    let p = CURRENT.with(Cell::get);
    if !p.is_null() {
        // SAFETY: `p` was set by `enter` from a live `&mut Ui` whose owner does not use it
        // until `enter` returns; nested scopes rebind it to a reborrow of the same `Ui`.
        return Ok(f(unsafe { &mut *p }));
    }
    let wrong_thread = MAIN
        .get()
        .is_some_and(|m| *m != std::thread::current().id());
    OWNED.with(|o| match o.try_borrow_mut() {
        Ok(mut guard) => match guard.as_deref_mut() {
            Some(ui) => Ok(f(ui)),
            // The app's runtime lives on the thread that called ak_app_run.
            None if wrong_thread => Err(AK_ERROR_WRONG_THREAD),
            None => Err(AK_ERROR_NO_RUNTIME),
        },
        Err(_) => Err(AK_ERROR_NO_RUNTIME),
    })
}

/// Makes `ui` the current runtime while `f` runs (callbacks into C).
pub(crate) fn enter<R>(ui: &mut Ui, f: impl FnOnce() -> R) -> R {
    let prev = CURRENT.with(|c| c.replace(ui as *mut Ui));
    let r = f();
    CURRENT.with(|c| c.set(prev));
    r
}

/// Runs an entry point's body, catching panics.
fn guard<R>(fallback: R, f: impl FnOnce() -> R) -> R {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(r) => r,
        Err(_) => {
            tracing::error!("appkit: a panic was caught at the C boundary (a bug)");
            fallback
        }
    }
}

fn status(r: Result<(), AKStatus>) -> AKStatus {
    set_last(match r {
        Ok(()) => AK_OK,
        Err(s) => s,
    })
}

/// A view handle.
pub(crate) fn view_ptr(id: ViewId) -> *mut AKView {
    id.to_raw() as usize as *mut AKView
}

/// The id of a view handle.
pub(crate) fn view_id(p: *mut AKView) -> Result<ViewId, AKStatus> {
    ViewId::from_raw(p as usize as u64).ok_or(AK_ERROR_INVALID_ARGUMENT)
}

fn window_ptr(id: WindowId) -> *mut AKWindow {
    id.to_raw() as usize as *mut AKWindow
}

fn window_id(p: *mut AKWindow) -> Result<WindowId, AKStatus> {
    WindowId::from_raw(p as usize as u64).ok_or(AK_ERROR_INVALID_ARGUMENT)
}

/// A borrowed UTF-8 string from C (`None` for null).
pub(crate) fn opt_str<'a>(p: *const c_char) -> Result<Option<&'a str>, AKStatus> {
    if p.is_null() {
        return Ok(None);
    }
    // SAFETY: the caller passes a NUL-terminated string valid for the call.
    unsafe { CStr::from_ptr(p) }
        .to_str()
        .map(Some)
        .map_err(|_| AK_ERROR_INVALID_ARGUMENT)
}

pub(crate) fn req_str<'a>(p: *const c_char) -> Result<&'a str, AKStatus> {
    opt_str(p)?.ok_or(AK_ERROR_INVALID_ARGUMENT)
}

pub(crate) fn color_of(c: AKColor) -> Color {
    Color::rgba(c.r, c.g, c.b, c.a)
}

fn rect_of(r: AKRect) -> Rect {
    Rect::new(r.x, r.y, r.width, r.height)
}

/// User data of a callback, dropped with its destroy function.
pub(crate) struct UserData {
    pub(crate) user: *mut c_void,
    destroy: AKDestroy,
}

impl UserData {
    pub(crate) fn new(user: *mut c_void, destroy: AKDestroy) -> Self {
        UserData { user, destroy }
    }
}

impl Drop for UserData {
    fn drop(&mut self) {
        if let Some(d) = self.destroy {
            // SAFETY: the caller's destroy function for its own data, called once.
            unsafe { d(self.user) };
        }
    }
}

// ----- Runtime -----------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn ak_abi_version() -> u32 {
    AK_ABI_VERSION
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_last_status() -> AKStatus {
    LAST.with(Cell::get)
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_status_message(status: AKStatus) -> *const c_char {
    let s: &'static CStr = match status {
        AK_OK => c"ok",
        AK_ERROR_NO_SUCH_OBJECT => c"no such object",
        AK_ERROR_UNKNOWN_PROPERTY => c"unknown property",
        AK_ERROR_BAD_VALUE => c"bad value",
        AK_ERROR_BAD_HIERARCHY => c"bad hierarchy",
        AK_ERROR_WRONG_THREAD => c"wrong thread",
        AK_ERROR_INVALID_ARGUMENT => c"invalid argument",
        AK_ERROR_PLATFORM => c"platform error",
        AK_ERROR_NO_RUNTIME => c"no runtime",
        AK_ERROR_BUFFER_TOO_SMALL => c"buffer too small",
        _ => c"unknown status",
    };
    s.as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_headless_init() -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        OWNED.with(|o| match o.try_borrow_mut() {
            Ok(mut g) => {
                if g.is_none() {
                    *g = Some(Box::new(Ui::new_headless()));
                }
                set_last(AK_OK)
            }
            Err(_) => set_last(AK_ERROR_NO_RUNTIME),
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_headless_shutdown() {
    guard((), || {
        let ui = OWNED.with(|o| o.try_borrow_mut().ok().and_then(|mut g| g.take()));
        drop(ui);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_app_run(
    app_id: *const c_char,
    setup: AKCallback,
    user: *mut c_void,
) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        let Ok(app_id) = req_str(app_id) else {
            return set_last(AK_ERROR_INVALID_ARGUMENT);
        };
        if MAIN.set(std::thread::current().id()).is_err() {
            // One app loop per process.
            return set_last(AK_ERROR_PLATFORM);
        }
        let r = crate::platform::run(app_id, |ui| {
            if let Some(f) = setup {
                // SAFETY: the caller's callback with its own data.
                enter(ui, || unsafe { f(user) });
            }
        });
        set_last(match r {
            Ok(()) => AK_OK,
            Err(e) => {
                tracing::error!("ak_app_run: {e}");
                AK_ERROR_PLATFORM
            }
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_quit() {
    guard((), || {
        let _ = with_ui(|ui| ui.request(crate::core::ui::Request::Quit));
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_advance(seconds: f64) {
    guard((), || {
        let _ = with_ui(|ui| ui.advance(seconds));
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_after(
    seconds: f64,
    callback: AKCallback,
    user: *mut c_void,
    destroy: AKDestroy,
) -> u64 {
    guard(0, || {
        let Some(f) = callback else {
            set_last(AK_ERROR_INVALID_ARGUMENT);
            return 0;
        };
        let data = UserData::new(user, destroy);
        match with_ui(move |ui| {
            ui.after(seconds, move |ui| {
                let d = data;
                // SAFETY: the caller's callback with its own data.
                enter(ui, || unsafe { f(d.user) });
            })
        }) {
            Ok(t) => {
                set_last(AK_OK);
                timer_raw(t)
            }
            Err(s) => {
                set_last(s);
                0
            }
        }
    })
}

fn timer_raw(t: crate::core::ui::TimerId) -> u64 {
    t.raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_cancel(timer: u64) {
    guard((), || {
        let _ = with_ui(|ui| ui.cancel_timer(crate::core::ui::TimerId::from_raw(timer)));
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_set_dark(dark: u8) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        status(with_ui(|ui| {
            let mut a = ui.appearance;
            a.dark = dark != 0;
            ui.set_appearance(a);
        }))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_is_dark() -> u8 {
    guard(0, || {
        with_ui(|ui| u8::from(ui.appearance.dark)).unwrap_or(0)
    })
}

/// Sets an appearance option for every window: Reduce Motion, Increase Contrast and Reduce
/// Transparency take 0 or 1; the accent takes 0xRRGGBBAA, and 0 goes back to Amalfi's.
#[unsafe(no_mangle)]
pub extern "C" fn ak_set_appearance_option(option: u32, value: u32) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        if !(AK_APPEARANCE_REDUCE_MOTION..=AK_APPEARANCE_ACCENT).contains(&option) {
            return set_last(AK_ERROR_INVALID_ARGUMENT);
        }
        status(with_ui(|ui| {
            let mut a = ui.appearance;
            match option {
                AK_APPEARANCE_REDUCE_MOTION => a.reduce_motion = value != 0,
                AK_APPEARANCE_INCREASE_CONTRAST => a.increase_contrast = value != 0,
                AK_APPEARANCE_REDUCE_TRANSPARENCY => a.reduce_transparency = value != 0,
                _ => {
                    a.accent = (value != 0).then(|| {
                        crate::color::Color::rgba(
                            (value >> 24) as u8,
                            (value >> 16) as u8,
                            (value >> 8) as u8,
                            value as u8,
                        )
                    })
                }
            }
            ui.set_appearance(a);
        }))
    })
}

/// The appearance option `option` (see `ak_set_appearance_option`); 0 when unknown.
#[unsafe(no_mangle)]
pub extern "C" fn ak_appearance_option(option: u32) -> u32 {
    guard(0, || {
        with_ui(|ui| {
            let a = ui.appearance;
            match option {
                AK_APPEARANCE_REDUCE_MOTION => u32::from(a.reduce_motion),
                AK_APPEARANCE_INCREASE_CONTRAST => u32::from(a.increase_contrast),
                AK_APPEARANCE_REDUCE_TRANSPARENCY => u32::from(a.reduce_transparency),
                AK_APPEARANCE_ACCENT => a.accent.map_or(0, |c| {
                    (u32::from(c.r) << 24)
                        | (u32::from(c.g) << 16)
                        | (u32::from(c.b) << 8)
                        | u32::from(c.a)
                }),
                _ => 0,
            }
        })
        .unwrap_or(0)
    })
}

// ----- Views -------------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_new(kind: u32) -> *mut AKView {
    guard(std::ptr::null_mut(), || {
        let Some(widget) = kinds::make(kind) else {
            set_last(AK_ERROR_INVALID_ARGUMENT);
            return std::ptr::null_mut();
        };
        match with_ui(|ui| ui.create_boxed(widget)) {
            Ok(id) => {
                set_last(AK_OK);
                view_ptr(id)
            }
            Err(s) => {
                set_last(s);
                std::ptr::null_mut()
            }
        }
    })
}

/// A view drawn and driven by C callbacks.
struct CustomView {
    draw: AKDrawFn,
    measure: AKMeasureFn,
    event: AKEventFn,
    data: std::rc::Rc<UserData>,
}

impl Widget for CustomView {
    fn kind(&self) -> &'static str {
        "Custom"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let Some(f) = self.measure else {
            return p.replacing_unspecified(Size::ZERO);
        };
        let (mut w, mut h) = (0.0f32, 0.0f32);
        // SAFETY: the caller's callback; the out pointers are valid locals.
        unsafe {
            f(
                p.width.unwrap_or(f32::NAN),
                p.height.unwrap_or(f32::NAN),
                &mut w,
                &mut h,
                self.data.user,
            );
        }
        Size::new(w, h)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let Some(f) = self.draw else { return };
        let data = self.data.clone();
        let draw: crate::core::display::CustomDraw = std::rc::Rc::new(move |canvas, rect| {
            canvas.translate((rect.x, rect.y));
            let c = canvas as *mut crate::render::canvas::Canvas as *mut AKCanvas;
            // SAFETY: the canvas lives for the call; the callback is the caller's.
            unsafe { f(c, rect.width, rect.height, data.user) };
        });
        let b = cx.bounds();
        cx.rec.custom("Canvas", b, draw);
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let Some(f) = self.event else {
            return Handled::No;
        };
        let mut text = String::new();
        let mut e = AKEvent {
            kind: 0,
            x: 0.0,
            y: 0.0,
            dx: 0.0,
            dy: 0.0,
            button: 0,
            clicks: 0,
            mods: 0,
            key: 0,
            pressed: 0,
            text: std::ptr::null(),
            text_len: 0,
        };
        let mods_of = |m: Mods| -> u32 {
            let mut v = 0;
            if m.contains(Mods::SHIFT) {
                v |= AK_MOD_SHIFT;
            }
            if m.contains(Mods::CTRL) {
                v |= AK_MOD_CTRL;
            }
            if m.contains(Mods::ALT) {
                v |= AK_MOD_ALT;
            }
            if m.contains(Mods::LOGO) {
                v |= AK_MOD_LOGO;
            }
            v
        };
        let button_of = |b: Button| -> u32 {
            match b {
                Button::Left => 1,
                Button::Right => 2,
                Button::Middle => 3,
                Button::Other(n) => n,
            }
        };
        match event {
            Event::PointerDown {
                pos,
                button,
                clicks,
                mods,
            } => {
                e.kind = AK_EVENT_POINTER_DOWN;
                e.x = pos.x;
                e.y = pos.y;
                e.button = button_of(*button);
                e.clicks = *clicks;
                e.mods = mods_of(*mods);
            }
            Event::PointerUp { pos, button, mods } => {
                e.kind = AK_EVENT_POINTER_UP;
                e.x = pos.x;
                e.y = pos.y;
                e.button = button_of(*button);
                e.mods = mods_of(*mods);
            }
            Event::PointerMove { pos, mods, .. } => {
                e.kind = AK_EVENT_POINTER_MOVE;
                e.x = pos.x;
                e.y = pos.y;
                e.mods = mods_of(*mods);
            }
            Event::PointerEnter => e.kind = AK_EVENT_POINTER_ENTER,
            Event::PointerLeave => e.kind = AK_EVENT_POINTER_LEAVE,
            Event::Scroll { pos, dx, dy, .. } => {
                e.kind = AK_EVENT_SCROLL;
                e.x = pos.x;
                e.y = pos.y;
                e.dx = *dx;
                e.dy = *dy;
            }
            Event::Key(k) => {
                e.kind = AK_EVENT_KEY;
                e.mods = mods_of(k.mods);
                e.pressed = u8::from(k.pressed);
                e.key = match &k.key {
                    Key::Char(c) => u32::from(*c),
                    Key::Space => AK_KEY_SPACE,
                    Key::Enter => AK_KEY_ENTER,
                    Key::Tab => AK_KEY_TAB,
                    Key::Escape => AK_KEY_ESCAPE,
                    Key::Backspace => AK_KEY_BACKSPACE,
                    Key::Delete => AK_KEY_DELETE,
                    Key::Left => AK_KEY_LEFT,
                    Key::Right => AK_KEY_RIGHT,
                    Key::Up => AK_KEY_UP,
                    Key::Down => AK_KEY_DOWN,
                    Key::Home => AK_KEY_HOME,
                    Key::End => AK_KEY_END,
                    Key::PageUp => AK_KEY_PAGE_UP,
                    Key::PageDown => AK_KEY_PAGE_DOWN,
                    _ => 0,
                };
                if let Some(t) = &k.text {
                    text = t.clone();
                }
            }
            Event::Text(t) => {
                e.kind = AK_EVENT_TEXT;
                text = t.clone();
            }
            Event::FocusIn => e.kind = AK_EVENT_FOCUS_IN,
            Event::FocusOut => e.kind = AK_EVENT_FOCUS_OUT,
            Event::Activate => e.kind = AK_EVENT_ACTIVATE,
            _ => return Handled::No,
        }
        e.text = text.as_ptr().cast();
        e.text_len = text.len();
        let view = view_ptr(cx.id());
        let user = self.data.user;
        let ui = cx.ui();
        // SAFETY: the caller's callback; `e` and its text live for the call.
        let handled = enter(ui, || unsafe { f(view, &e, user) });
        if handled != 0 {
            Handled::Yes
        } else {
            Handled::No
        }
    }

    fn focus(&self) -> FocusKind {
        if self.event.is_some() {
            FocusKind::Always
        } else {
            FocusKind::None
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_custom_view_new(
    draw: AKDrawFn,
    measure: AKMeasureFn,
    event: AKEventFn,
    user: *mut c_void,
    destroy: AKDestroy,
) -> *mut AKView {
    guard(std::ptr::null_mut(), || {
        let w = CustomView {
            draw,
            measure,
            event,
            data: std::rc::Rc::new(UserData::new(user, destroy)),
        };
        match with_ui(|ui| ui.create(w)) {
            Ok(id) => {
                set_last(AK_OK);
                view_ptr(id)
            }
            Err(s) => {
                set_last(s);
                std::ptr::null_mut()
            }
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_retain(view: *mut AKView) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        status(view_id(view).and_then(|id| {
            with_ui(|ui| {
                if ui.retain(id) {
                    Ok(())
                } else {
                    Err(AK_ERROR_NO_SUCH_OBJECT)
                }
            })?
        }))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_release(view: *mut AKView) {
    guard((), || {
        if let Ok(id) = view_id(view) {
            let _ = with_ui(|ui| ui.release(id));
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_add_child(parent: *mut AKView, child: *mut AKView) -> AKStatus {
    ak_view_insert_child(parent, usize::MAX, child)
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_insert_child(
    parent: *mut AKView,
    index: usize,
    child: *mut AKView,
) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        status((|| {
            let p = view_id(parent)?;
            let c = view_id(child)?;
            with_ui(|ui| ui.insert_child(p, index, c).map_err(status_of))?
        })())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_remove(child: *mut AKView) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        status(view_id(child).and_then(|c| {
            with_ui(|ui| {
                if ui.tree.contains(c) {
                    ui.detach(c);
                    Ok(())
                } else {
                    Err(AK_ERROR_NO_SUCH_OBJECT)
                }
            })?
        }))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_clear_children(parent: *mut AKView) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        status(view_id(parent).and_then(|p| with_ui(|ui| ui.clear_children(p))))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_child_count(view: *mut AKView) -> i64 {
    guard(i64::from(AK_ERROR_PLATFORM), || {
        match view_id(view).and_then(|v| {
            with_ui(|ui| {
                if ui.tree.contains(v) {
                    Ok(ui.tree.children(v).len() as i64)
                } else {
                    Err(AK_ERROR_NO_SUCH_OBJECT)
                }
            })?
        }) {
            Ok(n) => n,
            Err(s) => i64::from(set_last(s)),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_child_at(view: *mut AKView, index: usize) -> *mut AKView {
    guard(std::ptr::null_mut(), || {
        view_id(view)
            .and_then(|v| with_ui(|ui| ui.tree.children(v).get(index).copied()))
            .ok()
            .flatten()
            .map_or(std::ptr::null_mut(), view_ptr)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_parent(view: *mut AKView) -> *mut AKView {
    guard(std::ptr::null_mut(), || {
        view_id(view)
            .and_then(|v| with_ui(|ui| ui.tree.parent(v)))
            .ok()
            .flatten()
            .map_or(std::ptr::null_mut(), view_ptr)
    })
}

fn set_value(view: *mut AKView, prop: u32, value: Value) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        status((|| {
            let v = view_id(view)?;
            let p = Prop::from_u32(prop).ok_or(AK_ERROR_UNKNOWN_PROPERTY)?;
            with_ui(|ui| ui.set(v, p, value).map_err(status_of))?
        })())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_set_none(view: *mut AKView, prop: u32) -> AKStatus {
    set_value(view, prop, Value::None)
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_set_bool(view: *mut AKView, prop: u32, value: u8) -> AKStatus {
    set_value(view, prop, Value::Bool(value != 0))
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_set_int(view: *mut AKView, prop: u32, value: i64) -> AKStatus {
    set_value(view, prop, Value::Int(value))
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_set_float(view: *mut AKView, prop: u32, value: f64) -> AKStatus {
    set_value(view, prop, Value::Float(value))
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_set_string(
    view: *mut AKView,
    prop: u32,
    value: *const c_char,
) -> AKStatus {
    match opt_str(value) {
        Ok(Some(s)) => set_value(view, prop, Value::Str(s.to_string())),
        Ok(None) => set_value(view, prop, Value::None),
        Err(e) => set_last(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_set_color(view: *mut AKView, prop: u32, value: AKColor) -> AKStatus {
    set_value(view, prop, Value::Color(color_of(value)))
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_set_insets(view: *mut AKView, prop: u32, value: AKInsets) -> AKStatus {
    set_value(
        view,
        prop,
        Value::Insets(Insets::new(
            value.top,
            value.left,
            value.bottom,
            value.right,
        )),
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_set_size(
    view: *mut AKView,
    prop: u32,
    width: f32,
    height: f32,
) -> AKStatus {
    set_value(view, prop, Value::Size(Size::new(width, height)))
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_set_point(view: *mut AKView, prop: u32, x: f32, y: f32) -> AKStatus {
    set_value(view, prop, Value::Point(Point::new(x, y)))
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_set_strings(
    view: *mut AKView,
    prop: u32,
    items: *const *const c_char,
    count: usize,
) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        if items.is_null() && count > 0 {
            return set_last(AK_ERROR_INVALID_ARGUMENT);
        }
        let mut list = Vec::with_capacity(count.min(4096));
        for i in 0..count {
            // SAFETY: the caller passes `count` string pointers.
            let p = unsafe { *items.add(i) };
            match req_str(p) {
                Ok(s) => list.push(s.to_string()),
                Err(e) => return set_last(e),
            }
        }
        set_value(view, prop, Value::StrList(list))
    })
}

fn get_value(view: *mut AKView, prop: u32) -> Result<Value, AKStatus> {
    let v = view_id(view)?;
    let p = Prop::from_u32(prop).ok_or(AK_ERROR_UNKNOWN_PROPERTY)?;
    with_ui(|ui| {
        if !ui.tree.contains(v) {
            return Err(AK_ERROR_NO_SUCH_OBJECT);
        }
        ui.get(v, p).ok_or(AK_ERROR_UNKNOWN_PROPERTY)
    })?
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_get_bool(view: *mut AKView, prop: u32, out: *mut u8) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        if out.is_null() {
            return set_last(AK_ERROR_INVALID_ARGUMENT);
        }
        status(get_value(view, prop).and_then(|v| {
            let b = v.as_bool().ok_or(AK_ERROR_BAD_VALUE)?;
            // SAFETY: checked non-null; the caller's out pointer.
            unsafe { *out = u8::from(b) };
            Ok(())
        }))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_get_int(view: *mut AKView, prop: u32, out: *mut i64) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        if out.is_null() {
            return set_last(AK_ERROR_INVALID_ARGUMENT);
        }
        status(get_value(view, prop).and_then(|v| {
            let i = v.as_int().ok_or(AK_ERROR_BAD_VALUE)?;
            // SAFETY: checked non-null.
            unsafe { *out = i };
            Ok(())
        }))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_get_float(view: *mut AKView, prop: u32, out: *mut f64) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        if out.is_null() {
            return set_last(AK_ERROR_INVALID_ARGUMENT);
        }
        status(get_value(view, prop).and_then(|v| {
            let f = v.as_f64().ok_or(AK_ERROR_BAD_VALUE)?;
            // SAFETY: checked non-null.
            unsafe { *out = f };
            Ok(())
        }))
    })
}

/// Copies `s` into a caller buffer (NUL-terminated), reporting the length.
pub(crate) fn copy_out(
    s: &str,
    buffer: *mut c_char,
    capacity: usize,
    out_len: *mut usize,
) -> AKStatus {
    if !out_len.is_null() {
        // SAFETY: the caller's out pointer, checked non-null.
        unsafe { *out_len = s.len() };
    }
    if buffer.is_null() || capacity < s.len() + 1 {
        return AK_ERROR_BUFFER_TOO_SMALL;
    }
    // SAFETY: the buffer holds at least `s.len() + 1` bytes.
    unsafe {
        std::ptr::copy_nonoverlapping(s.as_ptr(), buffer.cast::<u8>(), s.len());
        *buffer.add(s.len()) = 0;
    }
    AK_OK
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_get_string(
    view: *mut AKView,
    prop: u32,
    buffer: *mut c_char,
    capacity: usize,
    out_len: *mut usize,
) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || match get_value(view, prop) {
        Ok(Value::Str(s)) => set_last(copy_out(&s, buffer, capacity, out_len)),
        Ok(Value::None) => set_last(copy_out("", buffer, capacity, out_len)),
        // Lists: one item per line.
        Ok(Value::StrList(l)) => set_last(copy_out(&l.join("\n"), buffer, capacity, out_len)),
        Ok(Value::IntList(l)) => {
            let s = l.iter().map(i64::to_string).collect::<Vec<_>>().join("\n");
            set_last(copy_out(&s, buffer, capacity, out_len))
        }
        Ok(_) => set_last(AK_ERROR_BAD_VALUE),
        Err(e) => set_last(e),
    })
}

/// Fills a C value for a signal's payload; `text` keeps the string alive.
fn c_value(v: &Value, text: &mut String) -> AKValue {
    let mut out = AKValue {
        kind: AK_VALUE_NONE,
        b: 0,
        i: 0,
        f: 0.0,
        text: std::ptr::null(),
        text_len: 0,
        rect: AKRect::default(),
        color: AKColor::default(),
    };
    match v {
        Value::None => {}
        Value::Bool(b) => {
            out.kind = AK_VALUE_BOOL;
            out.b = u8::from(*b);
        }
        Value::Int(i) => {
            out.kind = AK_VALUE_INT;
            out.i = *i;
            out.f = *i as f64;
        }
        Value::Float(f) => {
            out.kind = AK_VALUE_FLOAT;
            out.f = *f;
            out.i = *f as i64;
        }
        Value::Str(s) => {
            out.kind = AK_VALUE_STRING;
            *text = s.clone();
        }
        Value::Color(c) => {
            out.kind = AK_VALUE_COLOR;
            out.color = AKColor {
                r: c.r,
                g: c.g,
                b: c.b,
                a: c.a,
            };
        }
        Value::Point(p) => {
            out.kind = AK_VALUE_POINT;
            out.rect.x = p.x;
            out.rect.y = p.y;
        }
        Value::Size(s) => {
            out.kind = AK_VALUE_SIZE;
            out.rect.width = s.width;
            out.rect.height = s.height;
        }
        Value::Rect(r) => {
            out.kind = AK_VALUE_RECT;
            out.rect = AKRect {
                x: r.x,
                y: r.y,
                width: r.width,
                height: r.height,
            };
        }
        Value::Insets(i) => {
            out.kind = AK_VALUE_RECT;
            out.rect = AKRect {
                x: i.left,
                y: i.top,
                width: i.right,
                height: i.bottom,
            };
        }
        Value::StrList(l) => {
            out.kind = AK_VALUE_STRING_LIST;
            out.i = l.len() as i64;
            *text = l.join("\0");
        }
        Value::IntList(l) => {
            out.kind = AK_VALUE_STRING_LIST;
            out.i = l.len() as i64;
            *text = l.iter().map(i64::to_string).collect::<Vec<_>>().join("\0");
        }
        Value::Range(a, b) => {
            out.kind = AK_VALUE_RANGE;
            out.i = *a as i64;
            out.f = *b as f64;
        }
        Value::Cell {
            row,
            column,
            text: t,
        } => {
            out.kind = AK_VALUE_CELL;
            out.i = *row as i64;
            out.f = f64::from(*column);
            *text = t.clone();
        }
    }
    if !text.is_empty() {
        out.text = text.as_ptr().cast();
        out.text_len = text.len();
    }
    out
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_connect(
    view: *mut AKView,
    signal: u32,
    callback: AKSignalFn,
    user: *mut c_void,
    destroy: AKDestroy,
) -> u64 {
    guard(0, || {
        let data = UserData::new(user, destroy);
        let r = (|| {
            let v = view_id(view)?;
            let s = Signal::from_u32(signal).ok_or(AK_ERROR_INVALID_ARGUMENT)?;
            let f = callback.ok_or(AK_ERROR_INVALID_ARGUMENT)?;
            with_ui(move |ui| {
                if !ui.tree.contains(v) {
                    return Err(AK_ERROR_NO_SUCH_OBJECT);
                }
                let h = ui.connect(v, s, move |ui, view, value| {
                    let mut text = String::new();
                    let cv = c_value(value, &mut text);
                    let vp = view_ptr(view);
                    // `&data` makes the closure own all of `data` (a field access alone
                    // would capture only the pointer, and drop the destroy guard early).
                    let d = &data;
                    let user = d.user;
                    // SAFETY: the caller's callback; `cv` and its text live for the call.
                    enter(ui, || unsafe { f(vp, signal, &cv, user) });
                });
                Ok(h.raw())
            })?
        })();
        match r {
            Ok(h) => {
                set_last(AK_OK);
                h
            }
            Err(s) => {
                set_last(s);
                0
            }
        }
    })
}

/// Sets the rows of a list: count, height and the provider that makes the visible ones.
#[unsafe(no_mangle)]
pub extern "C" fn ak_list_set_rows(
    view: *mut AKView,
    rows: u64,
    row_height: f32,
    provider: AKRowFn,
    user: *mut c_void,
    destroy: AKDestroy,
) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        let data = UserData::new(user, destroy);
        let r = (|| {
            let v = view_id(view)?;
            let f = provider.ok_or(AK_ERROR_INVALID_ARGUMENT)?;
            let rows = usize::try_from(rows).map_err(|_| AK_ERROR_INVALID_ARGUMENT)?;
            let make: crate::amalfi::collections::RowProvider = Box::new(move |ui, i| {
                // `&data` keeps the whole destroy guard in the closure.
                let d = &data;
                let user = d.user;
                // SAFETY: the caller's provider with its own data; it returns a new reference.
                let p = enter(ui, || unsafe { f(i as u64, user) });
                if p.is_null() {
                    None
                } else {
                    view_id(p).ok().filter(|id| ui.tree.contains(*id))
                }
            });
            with_ui(move |ui| {
                ui.with_widget::<crate::amalfi::collections::ListView, _>(v, move |l| {
                    l.reset(rows, row_height, make)
                })
                .ok_or(AK_ERROR_INVALID_ARGUMENT)
            })?
        })();
        let s = r.err().unwrap_or(AK_OK);
        set_last(s);
        s
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_disconnect(handler: u64) {
    guard((), || {
        let _ = with_ui(|ui| ui.disconnect(crate::core::ui::HandlerId::from_raw(handler)));
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_request_paint(view: *mut AKView) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        status(view_id(view).and_then(|v| with_ui(|ui| ui.request_paint(v))))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_request_layout(view: *mut AKView) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        status(view_id(view).and_then(|v| with_ui(|ui| ui.request_layout(v))))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_focus(view: *mut AKView) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        status(view_id(view).and_then(|v| with_ui(|ui| ui.set_focus(v, false))))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_frame(view: *mut AKView, out: *mut AKRect) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        if out.is_null() {
            return set_last(AK_ERROR_INVALID_ARGUMENT);
        }
        status(view_id(view).and_then(|v| {
            let r = with_ui(|ui| ui.tree.window_rect(v))?;
            // SAFETY: checked non-null.
            unsafe {
                *out = AKRect {
                    x: r.x,
                    y: r.y,
                    width: r.width,
                    height: r.height,
                };
            }
            Ok(())
        }))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_find(root: *mut AKView, identifier: *const c_char) -> *mut AKView {
    guard(std::ptr::null_mut(), || {
        let r = (|| {
            let v = view_id(root)?;
            let ident = req_str(identifier)?;
            with_ui(|ui| ui.find(v, ident))
        })();
        r.ok().flatten().map_or(std::ptr::null_mut(), view_ptr)
    })
}

// ----- Windows -----------------------------------------------------------------------------

fn new_window(kind: WindowKind, size: Size) -> *mut AKWindow {
    match with_ui(|ui| ui.create_window(kind, size)) {
        Ok(w) => {
            set_last(AK_OK);
            window_ptr(w)
        }
        Err(s) => {
            set_last(s);
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_window_new(kind: u32, width: f32, height: f32) -> *mut AKWindow {
    guard(std::ptr::null_mut(), || {
        let k = match kind {
            AK_WINDOW_NORMAL => WindowKind::Normal,
            AK_WINDOW_PANEL => WindowKind::Panel,
            _ => {
                set_last(AK_ERROR_INVALID_ARGUMENT);
                return std::ptr::null_mut();
            }
        };
        new_window(k, Size::new(width, height))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_window_new_popup(
    parent: *mut AKWindow,
    anchor: AKRect,
    width: f32,
    height: f32,
) -> *mut AKWindow {
    guard(std::ptr::null_mut(), || match window_id(parent) {
        Ok(p) => new_window(
            WindowKind::Popup {
                parent: p,
                anchor: rect_of(anchor),
                grab: true,
            },
            Size::new(width, height),
        ),
        Err(s) => {
            set_last(s);
            std::ptr::null_mut()
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_window_new_shell(
    layer: u32,
    edges: u32,
    exclusive: i32,
    keyboard: u32,
    namespace: *const c_char,
    width: f32,
    height: f32,
) -> *mut AKWindow {
    guard(std::ptr::null_mut(), || {
        let ns = match req_str(namespace) {
            Ok(s) => s.to_string(),
            Err(e) => {
                set_last(e);
                return std::ptr::null_mut();
            }
        };
        let layer = match layer {
            AK_LAYER_BACKGROUND => Layer::Background,
            AK_LAYER_BOTTOM => Layer::Bottom,
            AK_LAYER_TOP => Layer::Top,
            _ => Layer::Overlay,
        };
        let keyboard = match keyboard {
            AK_KEYBOARD_EXCLUSIVE => KeyboardMode::Exclusive,
            AK_KEYBOARD_ON_DEMAND => KeyboardMode::OnDemand,
            _ => KeyboardMode::None,
        };
        new_window(
            WindowKind::Shell {
                layer,
                anchor: Edges::from_bits_truncate(edges),
                exclusive,
                margin: [0; 4],
                keyboard,
                namespace: ns,
                output: None,
            },
            Size::new(width, height),
        )
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_window_new_lock() -> *mut AKWindow {
    guard(std::ptr::null_mut(), || {
        new_window(WindowKind::Lock { output: None }, Size::new(1.0, 1.0))
    })
}

fn with_window(window: *mut AKWindow, f: impl FnOnce(&mut Ui, WindowId)) -> AKStatus {
    status(window_id(window).and_then(|w| {
        with_ui(|ui| {
            if ui.window(w).is_none() {
                return Err(AK_ERROR_NO_SUCH_OBJECT);
            }
            f(ui, w);
            Ok(())
        })?
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_window_set_title(window: *mut AKWindow, title: *const c_char) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || match req_str(title) {
        Ok(t) => with_window(window, |ui, w| ui.set_title(w, t)),
        Err(e) => set_last(e),
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_window_set_content(window: *mut AKWindow, view: *mut AKView) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || match view_id(view) {
        Ok(v) => with_window(window, |ui, w| ui.set_content(w, v)),
        Err(e) => set_last(e),
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_window_root(window: *mut AKWindow) -> *mut AKView {
    guard(std::ptr::null_mut(), || {
        window_id(window)
            .and_then(|w| with_ui(|ui| ui.window(w).map(|x| x.root)))
            .ok()
            .flatten()
            .map_or(std::ptr::null_mut(), view_ptr)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_window_show(window: *mut AKWindow) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        with_window(window, |ui, w| ui.show_window(w))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_window_close(window: *mut AKWindow) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        with_window(window, |ui, w| ui.close_window(w))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_window_set_size(window: *mut AKWindow, width: f32, height: f32) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        with_window(window, |ui, w| {
            let scale = ui.window(w).map(|x| x.scale).unwrap_or(1.0);
            ui.resize_window(w, Size::new(width, height), scale);
            ui.request(crate::core::ui::Request::Resize(
                w,
                Size::new(width, height),
            ));
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_window_set_size_limits(
    window: *mut AKWindow,
    min_width: f32,
    min_height: f32,
    max_width: f32,
    max_height: f32,
) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        let opt = |w: f32, h: f32| (w > 0.0 && h > 0.0).then(|| Size::new(w, h));
        with_window(window, |ui, w| {
            ui.set_size_limits(w, opt(min_width, min_height), opt(max_width, max_height))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_window_set_appearance(window: *mut AKWindow, appearance: u32) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        let o = match appearance {
            0 => None,
            1 => Some(false),
            2 => Some(true),
            _ => return set_last(AK_ERROR_INVALID_ARGUMENT),
        };
        with_window(window, |ui, w| {
            if let Some(win) = ui.window_mut(w) {
                win.dark_override = o;
            }
            if let Some(root) = ui.window(w).map(|x| x.root) {
                ui.request_paint(root);
                let _ = ui.set(
                    root,
                    Prop::AppearanceOverride,
                    Value::Int(i64::from(appearance)),
                );
            }
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_window_set_toolbar_height(window: *mut AKWindow, height: f32) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        with_window(window, |ui, w| ui.set_toolbar_height(w, height))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_window_set_keep_on_close_request(window: *mut AKWindow, keep: u8) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        with_window(window, |ui, w| {
            if let Some(win) = ui.window_mut(w) {
                win.keep_on_close_request = keep != 0;
            }
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_window_snapshot_png(
    window: *mut AKWindow,
    scale: f32,
    callback: AKBytesFn,
    user: *mut c_void,
) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        let Some(f) = callback else {
            return set_last(AK_ERROR_INVALID_ARGUMENT);
        };
        let png = match window_id(window).and_then(|w| with_ui(|ui| ui.snapshot(w, scale))) {
            Ok(Some(img)) => img.encode_png(),
            Ok(None) => return set_last(AK_ERROR_NO_SUCH_OBJECT),
            Err(e) => return set_last(e),
        };
        let Some(png) = png else {
            return set_last(AK_ERROR_PLATFORM);
        };
        let bytes = png.as_slice();
        // SAFETY: the caller's callback; the bytes live for the call.
        unsafe { f(bytes.as_ptr(), bytes.len(), user) };
        set_last(AK_OK)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_perform(view: *mut AKView, action: u32) -> AKStatus {
    use crate::core::a11y::Action;
    guard(AK_ERROR_PLATFORM, || {
        let a = match action {
            AK_ACTION_CLICK => Action::Click,
            AK_ACTION_FOCUS => Action::Focus,
            AK_ACTION_INCREMENT => Action::Increment,
            AK_ACTION_DECREMENT => Action::Decrement,
            AK_ACTION_EXPAND => Action::Expand,
            AK_ACTION_COLLAPSE => Action::Collapse,
            AK_ACTION_SHOW_MENU => Action::ShowMenu,
            _ => return set_last(AK_ERROR_INVALID_ARGUMENT),
        };
        status(view_id(view).and_then(|v| {
            with_ui(|ui| {
                if !ui.tree.contains(v) {
                    return Err(AK_ERROR_NO_SUCH_OBJECT);
                }
                ui.perform(v, a);
                Ok(())
            })?
        }))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_view_window(view: *mut AKView) -> *mut AKWindow {
    guard(std::ptr::null_mut(), || {
        view_id(view)
            .and_then(|v| with_ui(|ui| ui.window_of(v)))
            .ok()
            .flatten()
            .map_or(std::ptr::null_mut(), window_ptr)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_window_new_sheet(
    parent: *mut AKWindow,
    width: f32,
    height: f32,
) -> *mut AKWindow {
    guard(std::ptr::null_mut(), || match window_id(parent) {
        Ok(p) => new_window(WindowKind::Sheet { parent: p }, Size::new(width, height)),
        Err(s) => {
            set_last(s);
            std::ptr::null_mut()
        }
    })
}

/// Compile-time check: every function of the ABI list is defined here with exactly its
/// signature.
macro_rules! check_signatures {
    ($(fn $name:ident($($arg:ident: $ty:ty),* $(,)?) $(-> $ret:ty)?;)*) => {
        $(
            const _: unsafe extern "C" fn($($ty),*) $(-> $ret)? = $name;
        )*
    };
}

#[allow(unused_imports)]
use canvas::*;
#[allow(unused_imports)]
use collections::*;
use menu::*;
use panels::*;

appkit_sys::for_each_function!(check_signatures);

#[cfg(test)]
mod tests {
    use super::*;

    fn snake_upper(name: &str) -> String {
        let mut s = String::new();
        for (i, c) in name.chars().enumerate() {
            if c.is_ascii_uppercase() && i > 0 {
                s.push('_');
            }
            s.push(c.to_ascii_uppercase());
        }
        s
    }

    #[test]
    fn identifiers_match_the_core() {
        for p in Prop::ALL {
            let name = format!("AK_PROP_{}", snake_upper(p.name()));
            assert!(
                appkit_sys::PROPERTIES.contains(&(name.as_str(), *p as u32)),
                "{name} = {} missing from appkit-sys",
                *p as u32
            );
        }
        assert_eq!(appkit_sys::PROPERTIES.len(), Prop::ALL.len());
        for s in Signal::ALL {
            let name = format!("AK_SIGNAL_{}", snake_upper(s.name()));
            assert!(
                appkit_sys::SIGNALS.contains(&(name.as_str(), *s as u32)),
                "{name}"
            );
        }
        assert_eq!(appkit_sys::SIGNALS.len(), Signal::ALL.len());
        for (_, k) in appkit_sys::KINDS {
            assert!(kinds::make(*k).is_some(), "kind {k}");
        }
    }

    unsafe extern "C" fn count(_: *mut AKView, _: u32, v: *const AKValue, user: *mut c_void) {
        // SAFETY: test data.
        unsafe {
            *(user as *mut i64) += 1;
            assert_eq!((*v).kind, AK_VALUE_NONE);
        }
        // Calling back into the ABI from a callback works.
        assert_eq!(ak_is_dark(), 0);
    }

    static DESTROYED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    unsafe extern "C" fn noop(_: *mut AKView, _: u32, _: *const AKValue, _: *mut c_void) {}

    unsafe extern "C" fn mark(_: *mut c_void) {
        DESTROYED.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    #[test]
    fn callback_data_lives_until_disconnect() {
        assert_eq!(ak_headless_init(), AK_OK);
        let v = ak_view_new(AK_KIND_VIEW);
        let h = ak_view_connect(
            v,
            AK_SIGNAL_RESIZED,
            Some(noop),
            std::ptr::null_mut(),
            Some(mark),
        );
        assert_ne!(h, 0);
        assert_eq!(DESTROYED.load(std::sync::atomic::Ordering::SeqCst), 0);
        ak_disconnect(h);
        assert_eq!(DESTROYED.load(std::sync::atomic::Ordering::SeqCst), 1);
        ak_view_release(v);
        ak_headless_shutdown();
    }

    #[test]
    fn headless_round_trip() {
        assert_eq!(ak_view_new(AK_KIND_VIEW), std::ptr::null_mut());
        assert_eq!(ak_last_status(), AK_ERROR_NO_RUNTIME);
        assert_eq!(ak_headless_init(), AK_OK);
        let win = ak_window_new(AK_WINDOW_NORMAL, 200.0, 100.0);
        assert!(!win.is_null());
        let col = ak_view_new(AK_KIND_COLUMN);
        let b = ak_view_new(AK_KIND_PUSH_BUTTON);
        assert_eq!(ak_view_set_string(b, AK_PROP_TITLE, c"OK".as_ptr()), AK_OK);
        assert_eq!(
            ak_view_set_string(b, 9999, c"x".as_ptr()),
            AK_ERROR_UNKNOWN_PROPERTY
        );
        assert_eq!(ak_view_add_child(col, b), AK_OK);
        assert_eq!(ak_view_add_child(b, col), AK_ERROR_BAD_HIERARCHY);
        assert_eq!(ak_window_set_content(win, col), AK_OK);
        assert_eq!(ak_view_child_count(col), 1);
        let mut buf = [0 as c_char; 16];
        let mut len = 0usize;
        assert_eq!(
            ak_view_get_string(b, AK_PROP_TITLE, buf.as_mut_ptr(), buf.len(), &mut len),
            AK_OK
        );
        assert_eq!(len, 2);
        let mut hits: i64 = 0;
        let h = ak_view_connect(
            b,
            AK_SIGNAL_ACTIVATED,
            Some(count),
            (&mut hits as *mut i64).cast(),
            None,
        );
        assert_ne!(h, 0);
        with_ui(|ui| {
            let id = view_id(b).unwrap();
            ui.send(id, &Event::Activate);
            ui.flush();
        })
        .unwrap();
        assert_eq!(hits, 1);
        assert_eq!(ak_window_show(win), AK_OK);
        unsafe extern "C" fn keep(data: *const u8, len: usize, user: *mut c_void) {
            // SAFETY: test data.
            unsafe {
                let out = &mut *(user as *mut Vec<u8>);
                out.extend_from_slice(std::slice::from_raw_parts(data, len));
            }
        }
        let mut png: Vec<u8> = Vec::new();
        assert_eq!(
            ak_window_snapshot_png(win, 1.0, Some(keep), (&mut png as *mut Vec<u8>).cast()),
            AK_OK
        );
        assert_eq!(png.get(1..4), Some(&b"PNG"[..]));
        ak_view_release(b);
        ak_view_release(col);
        assert_eq!(ak_window_close(win), AK_OK);
        assert_eq!(
            ak_view_set_bool(b, AK_PROP_HIDDEN, 1),
            AK_ERROR_NO_SUCH_OBJECT
        );
        ak_headless_shutdown();
        // Each thread has its own headless runtime.
        let t = std::thread::spawn(|| {
            let s = ak_headless_init();
            ak_headless_shutdown();
            s
        })
        .join()
        .unwrap();
        assert_eq!(t, AK_OK);
    }
}
