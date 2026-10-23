//! The C ABI of AppKit (`libappkit.so`, header `appkit.h`, symbol version `APPKIT_1.0`).
//!
//! One list, [`for_each_function!`], declares every function: this crate expands it into
//! `extern "C"` declarations for Rust users (appkit-rs), and appkit expands it into a
//! compile-time check that its definitions have exactly these signatures. Types are
//! `#[repr(C)]` and shared by both. Identifiers (properties, signals, view kinds) are stable
//! numbers: new ones are only appended.
//!
//! Rules of the ABI:
//! - every handle is opaque; views are reference counted (`ak_view_retain` / `release`),
//!   a view added to a parent is also held by the parent;
//! - strings are UTF-8, NUL-terminated on input; output strings are written into a caller
//!   buffer, or borrowed for the duration of a callback;
//! - functions return a status (`AK_OK` or a negative error), or a null handle on error
//!   with the status in `ak_last_status()`;
//! - callbacks run on the UI thread, take a `void *user`, and may call back into the ABI;
//!   a `destroy` callback, when given, is called once when the callback is dropped;
//! - nothing panics across the boundary.

#![allow(non_camel_case_types)]
// The declarations of the ABI are an `extern` block: the only unsafe of this crate.
#![allow(unsafe_code)]

use std::ffi::{c_char, c_void};

/// Defines `pub const` statuses and a name table.
macro_rules! ids_i32 {
    ($table:ident, $($name:ident = $value:expr,)*) => {
        $(
            #[allow(missing_docs)]
            pub const $name: AKStatus = $value;
        )*
        /// Every identifier of the group: (name, value).
        pub static $table: &[(&str, i32)] = &[$((stringify!($name), $value),)*];
    };
}

/// Defines `pub const` identifiers and a name table.
macro_rules! ids {
    ($table:ident, $($name:ident = $value:expr,)*) => {
        $(
            #[allow(missing_docs)]
            pub const $name: u32 = $value;
        )*
        /// Every identifier of the group: (name, value).
        pub static $table: &[(&str, u32)] = &[$((stringify!($name), $value),)*];
    };
}

/// A view (opaque).
#[repr(C)]
pub struct AKView {
    _private: [u8; 0],
}

/// A window (opaque).
#[repr(C)]
pub struct AKWindow {
    _private: [u8; 0],
}

/// A drawing surface handed to a custom view's draw callback (opaque).
#[repr(C)]
pub struct AKCanvas {
    _private: [u8; 0],
}

/// A menu being built (opaque).
#[repr(C)]
pub struct AKMenu {
    _private: [u8; 0],
}

/// A status: `AK_OK` or a negative error.
pub type AKStatus = i32;

// Statuses.
ids_i32! { STATUSES,
    AK_OK = 0, AK_ERROR_NO_SUCH_OBJECT = -1, AK_ERROR_UNKNOWN_PROPERTY = -2,
    AK_ERROR_BAD_VALUE = -3, AK_ERROR_BAD_HIERARCHY = -4, AK_ERROR_WRONG_THREAD = -5,
    AK_ERROR_INVALID_ARGUMENT = -6, AK_ERROR_PLATFORM = -7, AK_ERROR_NO_RUNTIME = -8,
    AK_ERROR_BUFFER_TOO_SMALL = -9,
}

/// The ABI's version: major << 16 | minor << 8 | patch.
pub const AK_ABI_VERSION: u32 = 1 << 16;

/// A colour, straight alpha.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AKColor {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
    /// Alpha.
    pub a: u8,
}

/// A rectangle in points.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AKRect {
    /// Left.
    pub x: f32,
    /// Top.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

/// Insets in points.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AKInsets {
    /// Top.
    pub top: f32,
    /// Left.
    pub left: f32,
    /// Bottom.
    pub bottom: f32,
    /// Right.
    pub right: f32,
}

/// The kind of a [`AKValue`].
pub type AKValueKind = u32;
ids! { VALUE_KINDS,
    AK_VALUE_NONE = 0, AK_VALUE_BOOL = 1, AK_VALUE_INT = 2, AK_VALUE_FLOAT = 3,
    AK_VALUE_STRING = 4, AK_VALUE_COLOR = 5, AK_VALUE_POINT = 6, AK_VALUE_SIZE = 7,
    AK_VALUE_RECT = 8, AK_VALUE_RANGE = 9, AK_VALUE_STRING_LIST = 10,
    AK_VALUE_CELL = 11,
}

/// A value handed to a signal callback (borrowed for the callback's duration).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AKValue {
    /// Which fields are meaningful.
    pub kind: AKValueKind,
    /// Boolean.
    pub b: u8,
    /// Integer.
    pub i: i64,
    /// Number.
    pub f: f64,
    /// UTF-8 text (not NUL-terminated; `text_len` bytes).
    pub text: *const c_char,
    /// Length of `text`.
    pub text_len: usize,
    /// Point, size or rectangle.
    pub rect: AKRect,
    /// Colour.
    pub color: AKColor,
}

/// An input event handed to a custom view.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AKEvent {
    /// `AK_EVENT_*`.
    pub kind: u32,
    /// Position in the view (pointer events).
    pub x: f32,
    /// Position in the view (pointer events).
    pub y: f32,
    /// Scroll delta.
    pub dx: f32,
    /// Scroll delta.
    pub dy: f32,
    /// Button (1 left, 2 right, 3 middle).
    pub button: u32,
    /// Click count.
    pub clicks: u32,
    /// Modifiers (`AK_MOD_*`).
    pub mods: u32,
    /// Key: a character's code point, or `AK_KEY_*`.
    pub key: u32,
    /// Pressed (keys).
    pub pressed: u8,
    /// Text typed or committed (UTF-8, `text_len` bytes).
    pub text: *const c_char,
    /// Length of `text`.
    pub text_len: usize,
}

// Events, modifiers and keys (keys above the Unicode range; characters are code points).
ids! { EVENT_IDS,
    AK_EVENT_POINTER_DOWN = 1, AK_EVENT_POINTER_UP = 2, AK_EVENT_POINTER_MOVE = 3,
    AK_EVENT_POINTER_ENTER = 4, AK_EVENT_POINTER_LEAVE = 5, AK_EVENT_SCROLL = 6,
    AK_EVENT_KEY = 7, AK_EVENT_TEXT = 8, AK_EVENT_FOCUS_IN = 9, AK_EVENT_FOCUS_OUT = 10,
    AK_EVENT_ACTIVATE = 11,
    AK_MOD_SHIFT = 1, AK_MOD_CTRL = 2, AK_MOD_ALT = 4, AK_MOD_LOGO = 8,
    AK_KEY_SPACE = 0x20, AK_KEY_ENTER = 0x0011_0001, AK_KEY_TAB = 0x0011_0002,
    AK_KEY_ESCAPE = 0x0011_0003, AK_KEY_BACKSPACE = 0x0011_0004, AK_KEY_DELETE = 0x0011_0005,
    AK_KEY_LEFT = 0x0011_0006, AK_KEY_RIGHT = 0x0011_0007, AK_KEY_UP = 0x0011_0008,
    AK_KEY_DOWN = 0x0011_0009, AK_KEY_HOME = 0x0011_000A, AK_KEY_END = 0x0011_000B,
    AK_KEY_PAGE_UP = 0x0011_000C, AK_KEY_PAGE_DOWN = 0x0011_000D,
}

/// A plain callback.
pub type AKCallback = Option<unsafe extern "C" fn(user: *mut c_void)>;
/// Called once when a callback's `user` data is dropped.
pub type AKDestroy = Option<unsafe extern "C" fn(user: *mut c_void)>;
/// A signal callback.
pub type AKSignalFn = Option<
    unsafe extern "C" fn(view: *mut AKView, signal: u32, value: *const AKValue, user: *mut c_void),
>;
/// A custom view's drawing, in its own coordinates.
pub type AKDrawFn =
    Option<unsafe extern "C" fn(canvas: *mut AKCanvas, width: f32, height: f32, user: *mut c_void)>;
/// A custom view's size for a proposal (NaN: unspecified, infinity: maximum).
pub type AKMeasureFn = Option<
    unsafe extern "C" fn(
        proposed_width: f32,
        proposed_height: f32,
        out_width: *mut f32,
        out_height: *mut f32,
        user: *mut c_void,
    ),
>;
/// A custom view's event handler; returns 1 when it handled the event.
pub type AKEventFn =
    Option<unsafe extern "C" fn(view: *mut AKView, event: *const AKEvent, user: *mut c_void) -> u8>;
/// Receives bytes (a PNG …), borrowed for the call.
pub type AKBytesFn = Option<unsafe extern "C" fn(data: *const u8, len: usize, user: *mut c_void)>;
/// A menu item was chosen (id), or a pop-up menu ended (0: dismissed).
pub type AKMenuFn = Option<unsafe extern "C" fn(id: u32, user: *mut c_void)>;
/// Makes the view of row `index` of a list (a new reference the list takes over), or null.
pub type AKRowFn = Option<unsafe extern "C" fn(index: u64, user: *mut c_void) -> *mut AKView>;
/// Makes the view of a table cell: the row's key (its index, or its node in an outline) and
/// the column (a new reference the table takes over), or null.
pub type AKCellFn =
    Option<unsafe extern "C" fn(key: u64, column: u32, user: *mut c_void) -> *mut AKView>;
/// The number of children of an outline node (`AK_OUTLINE_ROOT`: the roots).
pub type AKTreeCountFn = Option<unsafe extern "C" fn(parent: u64, user: *mut c_void) -> u64>;
/// The id of child `index` of an outline node (`AK_OUTLINE_ROOT`: the roots).
pub type AKTreeChildFn =
    Option<unsafe extern "C" fn(parent: u64, index: u64, user: *mut c_void) -> u64>;
/// Whether an outline node can be expanded (1) or not (0).
pub type AKTreeExpandableFn = Option<unsafe extern "C" fn(node: u64, user: *mut c_void) -> u8>;

/// An alert's response: the index of the button pressed, or -1 (Escape).
pub type AKAlertFn = Option<unsafe extern "C" fn(response: i64, user: *mut c_void)>;

/// The invisible root of an outline's tree.
pub const AK_OUTLINE_ROOT: u64 = u64::MAX;

// Properties: the numbers of appkit's `core::value::Prop` (checked by its tests).
ids! { PROPERTIES,
    AK_PROP_PADDING = 1, AK_PROP_WIDTH = 2, AK_PROP_HEIGHT = 3, AK_PROP_MIN_WIDTH = 4,
    AK_PROP_MAX_WIDTH = 5, AK_PROP_MIN_HEIGHT = 6, AK_PROP_MAX_HEIGHT = 7,
    AK_PROP_FRAME_ALIGNMENT = 8, AK_PROP_LAYOUT_PRIORITY = 9, AK_PROP_HIDDEN = 10,
    AK_PROP_DISABLED = 11, AK_PROP_OPACITY = 12, AK_PROP_IDENTIFIER = 13,
    AK_PROP_TOOLTIP = 14, AK_PROP_ACCESSIBILITY_LABEL = 15, AK_PROP_ACCESSIBILITY_HELP = 16,
    AK_PROP_CLIPS_CHILDREN = 17, AK_PROP_APPEARANCE_OVERRIDE = 18, AK_PROP_BACKGROUND = 19,
    AK_PROP_CORNER_RADIUS = 20, AK_PROP_ASPECT_RATIO = 21, AK_PROP_OFFSET = 22,
    AK_PROP_SCALE = 23,
    AK_PROP_TITLE = 100, AK_PROP_SUBTITLE = 101, AK_PROP_SYMBOL = 102, AK_PROP_IMAGE = 103,
    AK_PROP_PLACEHOLDER = 104, AK_PROP_TEXT = 105, AK_PROP_VALUE = 106,
    AK_PROP_MIN_VALUE = 107, AK_PROP_MAX_VALUE = 108, AK_PROP_STEP = 109,
    AK_PROP_STATE = 110, AK_PROP_SELECTED_INDEX = 111, AK_PROP_ITEMS = 112,
    AK_PROP_ITEM_SYMBOLS = 113, AK_PROP_CONTROL_SIZE = 114, AK_PROP_STYLE = 115,
    AK_PROP_IS_DEFAULT = 116, AK_PROP_IS_CANCEL = 117, AK_PROP_SHORTCUT = 118,
    AK_PROP_EDITABLE = 119, AK_PROP_SELECTABLE = 120, AK_PROP_SECURE = 121,
    AK_PROP_MAX_LINES = 122, AK_PROP_TEXT_ALIGNMENT = 123, AK_PROP_TEXT_STYLE = 124,
    AK_PROP_TEXT_COLOR = 125, AK_PROP_SELECTED_RANGE = 126, AK_PROP_TICK_MARKS = 127,
    AK_PROP_VERTICAL = 128, AK_PROP_INDETERMINATE = 129, AK_PROP_EXPANDED = 130,
    AK_PROP_SPACING = 131, AK_PROP_ALIGNMENT = 132, AK_PROP_COLUMNS = 133,
    AK_PROP_MATERIAL = 134, AK_PROP_BADGE = 135, AK_PROP_COLOR_VALUE = 136,
    AK_PROP_DATE_VALUE = 137, AK_PROP_SELECTED_ROWS = 138, AK_PROP_ROW_COUNT = 139,
    AK_PROP_COLUMN_TITLES = 140, AK_PROP_TOKENS = 141, AK_PROP_SCROLL_OFFSET = 142,
    AK_PROP_CONTENT_SIZE = 143, AK_PROP_SPLIT_POSITION = 144, AK_PROP_COLLAPSED = 145,
    AK_PROP_SHOWS_CLEAR = 146, AK_PROP_ICON_POSITION = 147, AK_PROP_HELP_ANCHOR = 148,
    AK_PROP_PULLS_DOWN = 149, AK_PROP_ALERT_KIND = 150, AK_PROP_BUTTONS = 151,
    AK_PROP_EDGE = 152, AK_PROP_BORDERED = 153, AK_PROP_FORMAT = 154, AK_PROP_TINT = 155,
    AK_PROP_WRAPS = 156, AK_PROP_FONT_FAMILY = 157, AK_PROP_FONT_SIZE = 158,
    AK_PROP_FONT_WEIGHT = 159, AK_PROP_ANIMATION = 160, AK_PROP_TRANSITION = 161,
    AK_PROP_SORT_DESCRIPTOR = 162, AK_PROP_COLUMN_WIDTHS = 163,
    AK_PROP_ALTERNATING_ROWS = 164, AK_PROP_HEADER_VISIBLE = 165,
    AK_PROP_ALLOWS_MULTIPLE_SELECTION = 166, AK_PROP_EXPANDED_ITEMS = 167,
    AK_PROP_TILE_SIZE = 168, AK_PROP_PRESSED = 169,
    AK_PROP_DRAGGING = 170,
}

// Signals: appkit's `core::value::Signal`.
ids! { SIGNALS,
    AK_SIGNAL_ACTIVATED = 1, AK_SIGNAL_VALUE_CHANGED = 2, AK_SIGNAL_TEXT_CHANGED = 3,
    AK_SIGNAL_SUBMITTED = 4, AK_SIGNAL_SELECTION_CHANGED = 5, AK_SIGNAL_EDITING_BEGAN = 6,
    AK_SIGNAL_EDITING_ENDED = 7, AK_SIGNAL_CLOSE_REQUESTED = 8, AK_SIGNAL_CLOSED = 9,
    AK_SIGNAL_POINTER_ENTERED = 10, AK_SIGNAL_POINTER_EXITED = 11,
    AK_SIGNAL_EXPANDED_CHANGED = 12, AK_SIGNAL_DOUBLE_CLICKED = 13,
    AK_SIGNAL_CONTEXT_MENU = 14, AK_SIGNAL_DROPPED = 15, AK_SIGNAL_LINK_ACTIVATED = 16,
    AK_SIGNAL_SCROLLED = 17, AK_SIGNAL_RESIZED = 18, AK_SIGNAL_FOCUS_IN = 19,
    AK_SIGNAL_FOCUS_OUT = 20, AK_SIGNAL_KEY_UNHANDLED = 21, AK_SIGNAL_DRAG_BEGAN = 22,
    AK_SIGNAL_DRAW = 23, AK_SIGNAL_SORT_CHANGED = 24, AK_SIGNAL_CELL_EDITED = 25,
    AK_SIGNAL_COLUMN_RESIZED = 26,
}

// View kinds for `ak_view_new` (appkit's `ffi::kinds`).
ids! { KINDS,
    AK_KIND_VIEW = 1, AK_KIND_ROW = 2, AK_KIND_COLUMN = 3, AK_KIND_OVERLAY = 4,
    AK_KIND_GRID = 5, AK_KIND_SPACER = 6, AK_KIND_LABEL = 7, AK_KIND_SYMBOL = 8,
    AK_KIND_PUSH_BUTTON = 9, AK_KIND_TEXT_VIEW = 10,
    AK_KIND_CHECKBOX = 11, AK_KIND_RADIO = 12, AK_KIND_SWITCH = 13, AK_KIND_SLIDER = 14,
    AK_KIND_STEPPER = 15, AK_KIND_PROGRESS_BAR = 16, AK_KIND_CIRCULAR_PROGRESS = 17,
    AK_KIND_SPINNER = 18, AK_KIND_SEGMENTED = 19, AK_KIND_TEXT_FIELD = 20,
    AK_KIND_SECURE_FIELD = 21, AK_KIND_SEARCH_FIELD = 22, AK_KIND_TOKEN_FIELD = 23,
    AK_KIND_TEXT_AREA = 24, AK_KIND_COMBO_BOX = 25, AK_KIND_POP_UP_BUTTON = 26,
    AK_KIND_PULL_DOWN_BUTTON = 27, AK_KIND_ICON_BUTTON = 28, AK_KIND_HELP_BUTTON = 29,
    AK_KIND_LINK_BUTTON = 30, AK_KIND_TOOLBAR_BUTTON = 31, AK_KIND_DISCLOSURE = 32,
    AK_KIND_DISCLOSURE_GROUP = 33, AK_KIND_GROUP_BOX = 34, AK_KIND_COLOR_WELL = 35,
    AK_KIND_IMAGE_WELL = 36, AK_KIND_SCROLL_VIEW = 37, AK_KIND_DIAL = 38,
    AK_KIND_LIST = 39, AK_KIND_TABLE_HEADER = 40, AK_KIND_SIDEBAR_ROW = 41,
    AK_KIND_SIDEBAR_HEADER = 42, AK_KIND_SIDEBAR_GROUP = 43, AK_KIND_SIDEBAR_PANEL = 44,
    AK_KIND_FORM = 45, AK_KIND_FORM_ROW = 46, AK_KIND_TITLE_BAR = 47,
    AK_KIND_TAB_BAR = 48, AK_KIND_TAB = 49, AK_KIND_STATUS_BAR = 50, AK_KIND_TEXT = 51,
    AK_KIND_TABLE = 52, AK_KIND_OUTLINE = 53, AK_KIND_GRID_VIEW = 54,
    AK_KIND_TAB_VIEW = 55, AK_KIND_STATUS_ITEM = 56, AK_KIND_TOOLBAR = 57,
    AK_KIND_TOOLBAR_GROUP = 58, AK_KIND_FORM_VALUE = 59, AK_KIND_FORM_FOOTER = 60,
    AK_KIND_BOX_SURFACE = 61, AK_KIND_GLASS_PANEL = 62, AK_KIND_CALLOUT = 63, AK_KIND_TOUCH_PROBE = 64, AK_KIND_PANE_CONTROL = 65, AK_KIND_SIDEBAR_BADGE = 66, AK_KIND_SIDEBAR_EJECT = 67,
    AK_KIND_SIDEBAR_FOOTER = 68, AK_KIND_COMBO_BUTTON = 69,
}

// Alert flags and popover sides.
ids! { PANEL_IDS,
    AK_ALERT_CRITICAL = 1, AK_ALERT_GLASS = 2, AK_ALERT_STACKED = 4,
    AK_ALERT_DESTRUCTIVE = 8,
    AK_APPEARANCE_REDUCE_MOTION = 1, AK_APPEARANCE_INCREASE_CONTRAST = 2,
    AK_APPEARANCE_REDUCE_TRANSPARENCY = 3, AK_APPEARANCE_ACCENT = 4,
    AK_PANEL_SWATCHES_ONLY = 1, AK_PANEL_EXPANDED = 1, AK_PANEL_OPEN = 2,
    AK_SIDE_BELOW = 0, AK_SIDE_ABOVE = 1, AK_SIDE_AFTER = 2, AK_SIDE_BEFORE = 3,
}

// Table column flags.
ids! { COLUMN_FLAGS,
    AK_COLUMN_EDITABLE = 1, AK_COLUMN_NOT_SORTABLE = 2, AK_COLUMN_FIXED_WIDTH = 4,
    AK_COLUMN_ALIGN_CENTER = 8, AK_COLUMN_ALIGN_TRAILING = 16,
}

// Window kinds.
ids! { WINDOW_KINDS,
    AK_WINDOW_NORMAL = 1, AK_WINDOW_PANEL = 2,
}

// Shell layers and edges.
ids! { LAYERS,
    AK_LAYER_BACKGROUND = 0, AK_LAYER_BOTTOM = 1, AK_LAYER_TOP = 2, AK_LAYER_OVERLAY = 3,
    AK_EDGE_TOP = 1, AK_EDGE_BOTTOM = 2, AK_EDGE_LEFT = 4, AK_EDGE_RIGHT = 8,
    AK_KEYBOARD_NONE = 0, AK_KEYBOARD_EXCLUSIVE = 1, AK_KEYBOARD_ON_DEMAND = 2,
}

// Accessibility actions.
ids! { ACTIONS,
    AK_ACTION_CLICK = 1, AK_ACTION_FOCUS = 2, AK_ACTION_INCREMENT = 3,
    AK_ACTION_DECREMENT = 4, AK_ACTION_EXPAND = 5, AK_ACTION_COLLAPSE = 6,
    AK_ACTION_SHOW_MENU = 7,
}

// Menu item flags and pop-up kinds.
ids! { MENU_FLAGS,
    AK_MENU_DISABLED = 1, AK_MENU_CHECKED = 2, AK_MENU_MIXED = 4,
    AK_MENU_DESTRUCTIVE = 8, AK_MENU_HEADING = 16,
    AK_POPUP_CONTEXT = 0, AK_POPUP_POP_UP = 1,
}

/// Calls `$m! { … }` with every function of the ABI, as
/// `fn name(arg: Type, …) -> Ret;` items. Order and signatures are the ABI.
#[macro_export]
macro_rules! for_each_function {
    ($m:ident) => {
        $m! {
            // Runtime.
            fn ak_abi_version() -> u32;
            fn ak_last_status() -> $crate::AKStatus;
            fn ak_status_message(status: $crate::AKStatus) -> *const ::std::ffi::c_char;
            fn ak_headless_init() -> $crate::AKStatus;
            fn ak_headless_shutdown();
            fn ak_app_run(app_id: *const ::std::ffi::c_char, setup: $crate::AKCallback, user: *mut ::std::ffi::c_void) -> $crate::AKStatus;
            fn ak_quit();
            fn ak_advance(seconds: f64);
            fn ak_after(seconds: f64, callback: $crate::AKCallback, user: *mut ::std::ffi::c_void, destroy: $crate::AKDestroy) -> u64;
            fn ak_cancel(timer: u64);
            fn ak_set_dark(dark: u8) -> $crate::AKStatus;
            fn ak_is_dark() -> u8;
            // Views.
            fn ak_view_new(kind: u32) -> *mut $crate::AKView;
            fn ak_custom_view_new(draw: $crate::AKDrawFn, measure: $crate::AKMeasureFn, event: $crate::AKEventFn, user: *mut ::std::ffi::c_void, destroy: $crate::AKDestroy) -> *mut $crate::AKView;
            fn ak_view_retain(view: *mut $crate::AKView) -> $crate::AKStatus;
            fn ak_view_release(view: *mut $crate::AKView);
            fn ak_view_add_child(parent: *mut $crate::AKView, child: *mut $crate::AKView) -> $crate::AKStatus;
            fn ak_view_insert_child(parent: *mut $crate::AKView, index: usize, child: *mut $crate::AKView) -> $crate::AKStatus;
            fn ak_view_remove(child: *mut $crate::AKView) -> $crate::AKStatus;
            fn ak_view_clear_children(parent: *mut $crate::AKView) -> $crate::AKStatus;
            fn ak_view_child_count(view: *mut $crate::AKView) -> i64;
            fn ak_view_child_at(view: *mut $crate::AKView, index: usize) -> *mut $crate::AKView;
            fn ak_view_parent(view: *mut $crate::AKView) -> *mut $crate::AKView;
            fn ak_view_set_none(view: *mut $crate::AKView, prop: u32) -> $crate::AKStatus;
            fn ak_view_set_bool(view: *mut $crate::AKView, prop: u32, value: u8) -> $crate::AKStatus;
            fn ak_view_set_int(view: *mut $crate::AKView, prop: u32, value: i64) -> $crate::AKStatus;
            fn ak_view_set_float(view: *mut $crate::AKView, prop: u32, value: f64) -> $crate::AKStatus;
            fn ak_view_set_string(view: *mut $crate::AKView, prop: u32, value: *const ::std::ffi::c_char) -> $crate::AKStatus;
            fn ak_view_set_color(view: *mut $crate::AKView, prop: u32, value: $crate::AKColor) -> $crate::AKStatus;
            fn ak_view_set_insets(view: *mut $crate::AKView, prop: u32, value: $crate::AKInsets) -> $crate::AKStatus;
            fn ak_view_set_size(view: *mut $crate::AKView, prop: u32, width: f32, height: f32) -> $crate::AKStatus;
            fn ak_view_set_point(view: *mut $crate::AKView, prop: u32, x: f32, y: f32) -> $crate::AKStatus;
            fn ak_view_set_strings(view: *mut $crate::AKView, prop: u32, items: *const *const ::std::ffi::c_char, count: usize) -> $crate::AKStatus;
            fn ak_view_get_bool(view: *mut $crate::AKView, prop: u32, out: *mut u8) -> $crate::AKStatus;
            fn ak_view_get_int(view: *mut $crate::AKView, prop: u32, out: *mut i64) -> $crate::AKStatus;
            fn ak_view_get_float(view: *mut $crate::AKView, prop: u32, out: *mut f64) -> $crate::AKStatus;
            fn ak_view_get_string(view: *mut $crate::AKView, prop: u32, buffer: *mut ::std::ffi::c_char, capacity: usize, out_len: *mut usize) -> $crate::AKStatus;
            fn ak_view_connect(view: *mut $crate::AKView, signal: u32, callback: $crate::AKSignalFn, user: *mut ::std::ffi::c_void, destroy: $crate::AKDestroy) -> u64;
            fn ak_disconnect(handler: u64);
            fn ak_view_request_paint(view: *mut $crate::AKView) -> $crate::AKStatus;
            fn ak_view_request_layout(view: *mut $crate::AKView) -> $crate::AKStatus;
            fn ak_view_focus(view: *mut $crate::AKView) -> $crate::AKStatus;
            fn ak_view_frame(view: *mut $crate::AKView, out: *mut $crate::AKRect) -> $crate::AKStatus;
            fn ak_view_find(root: *mut $crate::AKView, identifier: *const ::std::ffi::c_char) -> *mut $crate::AKView;
            // Windows.
            fn ak_window_new(kind: u32, width: f32, height: f32) -> *mut $crate::AKWindow;
            fn ak_window_new_popup(parent: *mut $crate::AKWindow, anchor: $crate::AKRect, width: f32, height: f32) -> *mut $crate::AKWindow;
            fn ak_window_new_shell(layer: u32, edges: u32, exclusive: i32, keyboard: u32, namespace: *const ::std::ffi::c_char, width: f32, height: f32) -> *mut $crate::AKWindow;
            fn ak_window_new_lock() -> *mut $crate::AKWindow;
            fn ak_window_set_title(window: *mut $crate::AKWindow, title: *const ::std::ffi::c_char) -> $crate::AKStatus;
            fn ak_window_set_content(window: *mut $crate::AKWindow, view: *mut $crate::AKView) -> $crate::AKStatus;
            fn ak_window_root(window: *mut $crate::AKWindow) -> *mut $crate::AKView;
            fn ak_window_show(window: *mut $crate::AKWindow) -> $crate::AKStatus;
            fn ak_window_close(window: *mut $crate::AKWindow) -> $crate::AKStatus;
            fn ak_window_set_size(window: *mut $crate::AKWindow, width: f32, height: f32) -> $crate::AKStatus;
            fn ak_window_set_size_limits(window: *mut $crate::AKWindow, min_width: f32, min_height: f32, max_width: f32, max_height: f32) -> $crate::AKStatus;
            fn ak_window_set_appearance(window: *mut $crate::AKWindow, appearance: u32) -> $crate::AKStatus;
            fn ak_window_set_toolbar_height(window: *mut $crate::AKWindow, height: f32) -> $crate::AKStatus;
            fn ak_window_set_keep_on_close_request(window: *mut $crate::AKWindow, keep: u8) -> $crate::AKStatus;
            fn ak_window_snapshot_png(window: *mut $crate::AKWindow, scale: f32, callback: $crate::AKBytesFn, user: *mut ::std::ffi::c_void) -> $crate::AKStatus;
            // Menus.
            fn ak_menu_new(title: *const ::std::ffi::c_char, role: *const ::std::ffi::c_char) -> *mut $crate::AKMenu;
            fn ak_menu_free(menu: *mut $crate::AKMenu);
            fn ak_menu_add_item(menu: *mut $crate::AKMenu, title: *const ::std::ffi::c_char, shortcut: *const ::std::ffi::c_char, id: u32, flags: u32) -> $crate::AKStatus;
            fn ak_menu_add_separator(menu: *mut $crate::AKMenu) -> $crate::AKStatus;
            fn ak_menu_set_icon(menu: *mut $crate::AKMenu, symbol: *const ::std::ffi::c_char) -> $crate::AKStatus;
            fn ak_menu_add_submenu(menu: *mut $crate::AKMenu, submenu: *mut $crate::AKMenu) -> $crate::AKStatus;
            fn ak_app_set_menu_bar(menus: *const *mut $crate::AKMenu, count: usize, callback: $crate::AKMenuFn, user: *mut ::std::ffi::c_void, destroy: $crate::AKDestroy) -> $crate::AKStatus;
            fn ak_view_popup_menu(view: *mut $crate::AKView, menu: *mut $crate::AKMenu, anchor: $crate::AKRect, kind: u32, selected: u32, callback: $crate::AKMenuFn, user: *mut ::std::ffi::c_void, destroy: $crate::AKDestroy) -> $crate::AKStatus;
            // Canvas (inside a draw callback).
            fn ak_canvas_save(canvas: *mut $crate::AKCanvas);
            fn ak_canvas_restore(canvas: *mut $crate::AKCanvas);
            fn ak_canvas_translate(canvas: *mut $crate::AKCanvas, x: f32, y: f32);
            fn ak_canvas_clip_rect(canvas: *mut $crate::AKCanvas, rect: $crate::AKRect);
            fn ak_canvas_fill_rect(canvas: *mut $crate::AKCanvas, rect: $crate::AKRect, color: $crate::AKColor);
            fn ak_canvas_fill_rounded_rect(canvas: *mut $crate::AKCanvas, rect: $crate::AKRect, radius: f32, color: $crate::AKColor);
            fn ak_canvas_stroke_rect(canvas: *mut $crate::AKCanvas, rect: $crate::AKRect, width: f32, color: $crate::AKColor);
            fn ak_canvas_fill_ellipse(canvas: *mut $crate::AKCanvas, rect: $crate::AKRect, color: $crate::AKColor);
            fn ak_canvas_line(canvas: *mut $crate::AKCanvas, x0: f32, y0: f32, x1: f32, y1: f32, width: f32, color: $crate::AKColor);
            fn ak_canvas_text(canvas: *mut $crate::AKCanvas, x: f32, y: f32, text: *const ::std::ffi::c_char, family: *const ::std::ffi::c_char, size: f32, weight: u32, color: $crate::AKColor);
            fn ak_canvas_glyph(canvas: *mut $crate::AKCanvas, rect: $crate::AKRect, symbol: *const ::std::ffi::c_char, weight: u32, color: $crate::AKColor);
            fn ak_measure_text(text: *const ::std::ffi::c_char, family: *const ::std::ffi::c_char, size: f32, weight: u32, out_width: *mut f32, out_height: *mut f32) -> $crate::AKStatus;
            // Accessibility actions (assistive technologies, tests).
            fn ak_view_perform(view: *mut $crate::AKView, action: u32) -> $crate::AKStatus;
            // Windows of views; sheets.
            fn ak_view_window(view: *mut $crate::AKView) -> *mut $crate::AKWindow;
            fn ak_window_new_sheet(parent: *mut $crate::AKWindow, width: f32, height: f32) -> *mut $crate::AKWindow;
            // Virtualised lists.
            fn ak_list_set_rows(view: *mut $crate::AKView, rows: u64, row_height: f32, provider: $crate::AKRowFn, user: *mut ::std::ffi::c_void, destroy: $crate::AKDestroy) -> $crate::AKStatus;
            // Tables, outlines and grids (APPKIT_1.0 additions are appended below).
            fn ak_table_add_column(view: *mut $crate::AKView, title: *const ::std::ffi::c_char, width: f32, min_width: f32, max_width: f32, flags: u32) -> $crate::AKStatus;
            fn ak_table_clear_columns(view: *mut $crate::AKView) -> $crate::AKStatus;
            fn ak_table_set_cells(view: *mut $crate::AKView, rows: u64, row_height: f32, provider: $crate::AKCellFn, user: *mut ::std::ffi::c_void, destroy: $crate::AKDestroy) -> $crate::AKStatus;
            fn ak_outline_set_tree(view: *mut $crate::AKView, row_height: f32, count: $crate::AKTreeCountFn, child: $crate::AKTreeChildFn, expandable: $crate::AKTreeExpandableFn, provider: $crate::AKCellFn, user: *mut ::std::ffi::c_void, destroy: $crate::AKDestroy) -> $crate::AKStatus;
            fn ak_grid_set_items(view: *mut $crate::AKView, count: u64, tile_width: f32, tile_height: f32, provider: $crate::AKRowFn, user: *mut ::std::ffi::c_void, destroy: $crate::AKDestroy) -> $crate::AKStatus;
            fn ak_collection_reload(view: *mut $crate::AKView) -> $crate::AKStatus;
            // Alerts, popovers, menu views, window state.
            fn ak_alert_new(title: *const ::std::ffi::c_char, message: *const ::std::ffi::c_char, buttons: *const *const ::std::ffi::c_char, count: usize, flags: u32, icon: *const ::std::ffi::c_char) -> *mut $crate::AKView;
            fn ak_alert_add_accessory(alert: *mut $crate::AKView, view: *mut $crate::AKView) -> $crate::AKStatus;
            fn ak_window_present_alert(parent: *mut $crate::AKWindow, alert: *mut $crate::AKView, callback: $crate::AKAlertFn, user: *mut ::std::ffi::c_void, destroy: $crate::AKDestroy) -> *mut $crate::AKWindow;
            fn ak_view_present_popover(anchor: *mut $crate::AKView, content: *mut $crate::AKView, side: u32, closed: $crate::AKCallback, user: *mut ::std::ffi::c_void, destroy: $crate::AKDestroy) -> *mut $crate::AKWindow;
            fn ak_menu_view_new(menu: *mut $crate::AKMenu, callback: $crate::AKMenuFn, user: *mut ::std::ffi::c_void, destroy: $crate::AKDestroy) -> *mut $crate::AKView;
            fn ak_color_panel_new(colors: *const u32, count: usize, columns: usize, selected: u32, flags: u32) -> *mut $crate::AKView;
            fn ak_file_panel_new(flags: u32, name: *const ::std::ffi::c_char, files: *const *const ::std::ffi::c_char, count: usize) -> *mut $crate::AKView;
            fn ak_set_appearance_option(option: u32, value: u32) -> $crate::AKStatus;
            fn ak_appearance_option(option: u32) -> u32;
            fn ak_window_set_autosave(window: *mut $crate::AKWindow, name: *const ::std::ffi::c_char) -> $crate::AKStatus;
        }
    };
}

/// Expands the function list into `extern "C"` declarations.
macro_rules! declare {
    ($(fn $name:ident($($arg:ident: $ty:ty),* $(,)?) $(-> $ret:ty)?;)*) => {
        unsafe extern "C" {
            $(
                #[allow(missing_docs)]
                pub fn $name($($arg: $ty),*) $(-> $ret)?;
            )*
        }
        /// The names of every function, in ABI order.
        pub static FUNCTIONS: &[&str] = &[$(stringify!($name),)*];
    };
}

/// A function's signature as Rust tokens, for the header generator and the ABI check.
#[derive(Debug, Clone, Copy)]
pub struct Signature {
    /// Name.
    pub name: &'static str,
    /// (argument, type) pairs.
    pub args: &'static [(&'static str, &'static str)],
    /// Return type (empty: none).
    pub ret: &'static str,
}

macro_rules! signatures {
    ($(fn $name:ident($($arg:ident: $ty:ty),* $(,)?) $(-> $ret:ty)?;)*) => {
        /// Every function's signature, in ABI order.
        pub static SIGNATURES: &[Signature] = &[$(
            Signature {
                name: stringify!($name),
                args: &[$((stringify!($arg), stringify!($ty))),*],
                ret: concat!("" $(, stringify!($ret))?),
            },
        )*];
    };
}

for_each_function!(signatures);

/// The functions of the ABI (`use appkit_sys::functions::*`), apart from the types and
/// constants so that a library defining them can glob-import the latter.
pub mod functions {
    for_each_function!(declare);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_are_stable() {
        assert_eq!(std::mem::size_of::<AKColor>(), 4);
        assert_eq!(std::mem::size_of::<AKRect>(), 16);
        assert_eq!(std::mem::size_of::<AKInsets>(), 16);
        assert_eq!(std::mem::size_of::<AKValue>(), 64);
        assert_eq!(std::mem::size_of::<AKEvent>(), 56);
        assert!(functions::FUNCTIONS.len() > 70);
        let mut names: Vec<&str> = functions::FUNCTIONS.to_vec();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), functions::FUNCTIONS.len());
    }
}

#[cfg(test)]
mod signature_tests {
    #[test]
    fn signatures_are_listed() {
        let s = super::SIGNATURES
            .iter()
            .find(|s| s.name == "ak_view_set_string")
            .unwrap();
        assert_eq!(s.args.len(), 3);
        assert!(s.ret.contains("AKStatus"));
        println!("{:?}", s.args);
    }
}
