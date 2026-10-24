//! Windows: the state the core keeps per window (backend-independent). The platform layer
//! maps each to a Wayland surface (xdg toplevel, popup, layer-shell or lock surface); headless
//! windows render into raster images for tests.

use crate::core::tree::ViewId;
use crate::geom::{Point, Rect, Size};

/// A window's id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WindowId(pub(crate) u64);

impl WindowId {
    /// The id as a number (never 0).
    pub fn to_raw(self) -> u64 {
        self.0
    }

    /// The id from a number.
    pub fn from_raw(raw: u64) -> Option<WindowId> {
        (raw != 0).then_some(WindowId(raw))
    }
}

/// The layer of a shell surface (wlr-layer-shell).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    /// Under everything (wallpaper, desktop icons).
    Background,
    /// Under windows.
    Bottom,
    /// Over windows (Dock, menu bar).
    Top,
    /// Over everything (notifications, HUDs, Faro).
    Overlay,
}

bitflags::bitflags! {
    /// Screen edges a shell surface is anchored to.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct Edges: u32 {
        /// Top.
        const TOP = 1;
        /// Bottom.
        const BOTTOM = 2;
        /// Left.
        const LEFT = 4;
        /// Right.
        const RIGHT = 8;
    }
}

/// Keyboard interactivity of a shell surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardMode {
    /// Never takes the keyboard.
    None,
    /// Takes it while open (Faro, menus).
    Exclusive,
    /// Takes it when clicked.
    OnDemand,
}

/// The side of its target a popup goes on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PopupSide {
    /// Under it (menus, popovers, tooltips).
    #[default]
    Below,
    /// Over it.
    Above,
    /// Trailing (submenus).
    After,
    /// Leading.
    Before,
    /// On it (a pop-up button's menu, its selected item over the button).
    Over,
}

/// How a popup hangs off its target. A Wayland client does not know where it is on screen:
/// the compositor keeps the popup on screen with these adjustments (`xdg_positioner`), as
/// macOS AppKit does itself, and the popup lays out from where it landed.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PopupPlacement {
    /// What it hangs off (a control, a menu title, the pointer), in the parent's coordinates.
    pub target: Rect,
    /// The side of `target` it goes on.
    pub side: PopupSide,
    /// May move to the opposite side across (context menus).
    pub flip_x: bool,
    /// May move to the opposite side down (popovers, context menus).
    pub flip_y: bool,
    /// May slide across along the screen's edge.
    pub slide_x: bool,
    /// May slide down or up along the screen's edge.
    pub slide_y: bool,
    /// May get shorter (long menus, which then scroll).
    pub resize: bool,
}

impl PopupPlacement {
    /// A menu (from the menu bar, a pop-up button, a pull-down): slid across, never down,
    /// and shortened to fit, keeping its rows aligned by scrolling (docs/spec/menus.md).
    pub fn menu(target: Rect, side: PopupSide) -> Self {
        Self {
            target,
            side,
            flip_x: false,
            flip_y: false,
            slide_x: true,
            slide_y: false,
            resize: true,
        }
    }

    /// A context menu at `point`: down and to the right of it; past the right edge it opens
    /// to the left, past the bottom it slides up (docs/spec/menus.md).
    pub fn context(point: Point) -> Self {
        Self {
            target: Rect::from_origin_size(point, Size::ZERO),
            side: PopupSide::Over,
            flip_x: true,
            flip_y: false,
            slide_x: false,
            slide_y: true,
            resize: true,
        }
    }

    /// A popover, submenu or tooltip: flipped to the other side when it does not fit, then
    /// slid.
    pub fn flipping(target: Rect, side: PopupSide, resize: bool) -> Self {
        let across = matches!(side, PopupSide::After | PopupSide::Before);
        Self {
            target,
            side,
            flip_x: across,
            flip_y: !across && side != PopupSide::Over,
            slide_x: !across,
            slide_y: across,
            resize,
        }
    }

    /// The side `frame` ended up on: flipped by the compositor, or the asked one.
    pub fn landed(&self, frame: Rect) -> PopupSide {
        let t = self.target;
        let eps = 0.5;
        match self.side {
            PopupSide::Below if frame.bottom() <= t.y + eps => PopupSide::Above,
            PopupSide::Above if frame.y >= t.bottom() - eps => PopupSide::Below,
            PopupSide::After if frame.right() <= t.x + eps => PopupSide::Before,
            PopupSide::Before if frame.x >= t.right() - eps => PopupSide::After,
            s => s,
        }
    }
}

/// What kind of surface a window is.
#[derive(Debug, Clone, PartialEq)]
pub enum WindowKind {
    /// An app window (xdg toplevel).
    Normal,
    /// A utility panel (floats over its app's windows, no Dock presence).
    Panel,
    /// A sheet attached to a parent window.
    Sheet {
        /// The window it is attached to.
        parent: WindowId,
    },
    /// A transient surface anchored to a rectangle of a parent (menus, popovers, tooltips,
    /// pop-up lists): an xdg popup.
    Popup {
        /// The parent.
        parent: WindowId,
        /// Where the popup goes: its own frame in the parent's coordinates, as the widget that
        /// opens it wants it; once the compositor has kept it on screen, where it landed.
        anchor: Rect,
        /// Takes a grab (closes on a click outside).
        grab: bool,
        /// What it hangs off and how the compositor may move it (`xdg_positioner`).
        place: PopupPlacement,
    },
    /// A shell surface (wlr-layer-shell).
    Shell {
        /// Layer.
        layer: Layer,
        /// Anchored edges.
        anchor: Edges,
        /// Exclusive zone (space reserved from windows), -1 to ignore others' zones.
        exclusive: i32,
        /// Margins: top, right, bottom, left.
        margin: [i32; 4],
        /// Keyboard.
        keyboard: KeyboardMode,
        /// Namespace (the shell's component name).
        namespace: String,
        /// Output name (`None`: the compositor picks).
        output: Option<String>,
    },
    /// A lock-screen surface (ext-session-lock), one per output.
    Lock {
        /// Output name.
        output: Option<String>,
    },
    /// The app's menu titles in the compositor's menu bar (silica-menu-bar-v1), shown while
    /// the app is frontmost. The compositor gives it the room before the status items.
    MenuTitles,
    /// A status item in the compositor's menu bar (silica-menu-bar-v1).
    StatusItem {
        /// A system item (Control Center's), right of the apps' items.
        system: bool,
        /// Among the system items, from the right.
        order: u32,
    },
}

impl Window {
    /// `size` within the window's minimum and maximum sizes (silica-desktop#245): what the
    /// window is laid out at whatever the compositor or the app asked for.
    pub fn clamp_size(&self, size: Size) -> Size {
        let mut s = size;
        if let Some(min) = self.min_size {
            s.width = s.width.max(min.width);
            s.height = s.height.max(min.height);
        }
        if let Some(max) = self.max_size {
            if max.width > 0.0 {
                s.width = s.width.min(max.width);
            }
            if max.height > 0.0 {
                s.height = s.height.min(max.height);
            }
        }
        s
    }

    /// Whether the user can resize it (it has no fixed size: minimum = maximum).
    pub fn is_resizable(&self) -> bool {
        self.resizable && !matches!((self.min_size, self.max_size), (Some(a), Some(b)) if a == b)
    }
}

impl WindowKind {
    /// The parent of a transient window.
    pub fn parent(&self) -> Option<WindowId> {
        match self {
            WindowKind::Sheet { parent } | WindowKind::Popup { parent, .. } => Some(*parent),
            _ => None,
        }
    }
}

/// The state of a window.
#[derive(Debug, Clone)]
pub struct Window {
    /// Id.
    pub id: WindowId,
    /// The root view.
    pub root: ViewId,
    /// Title.
    pub title: String,
    /// Kind.
    pub kind: WindowKind,
    /// Size in points.
    pub size: Size,
    /// Device pixels per point (fractional scale).
    pub scale: f32,
    /// Minimum size (Ossido respects it, silica-desktop#245).
    pub min_size: Option<Size>,
    /// Maximum size.
    pub max_size: Option<Size>,
    /// Resizable.
    pub resizable: bool,
    /// Full-size content (Ossido's buttons over the content, silica-window-v1).
    pub full_size_content: bool,
    /// The view with the keyboard focus.
    pub focus: Option<ViewId>,
    /// The autosave name: the window's state is kept under it (`core::restore`).
    pub autosave: Option<String>,
    /// Saved state of views not restored yet (they have not appeared).
    pub restore: std::collections::HashMap<
        String,
        Vec<(crate::core::value::Prop, crate::core::value::Value)>,
    >,
    /// Where keys go without a focus: the parent of a focused view that was hidden (a tab
    /// switched away), so its containers still get the keys.
    pub key_anchor: Option<ViewId>,
    /// Views under the pointer, outermost first.
    pub hover: Vec<ViewId>,
    /// The view that took the press.
    pub capture: Option<ViewId>,
    /// The last pointer position.
    pub pointer: Option<Point>,
    /// Damage since the last frame (points).
    pub damage: Rect,
    /// The input region last asked for (`None`: never), so an unchanged one is not sent again.
    pub input_region: Option<Option<Vec<Rect>>>,
    /// The layout must run.
    pub needs_layout: bool,
    /// A frame is wanted.
    pub needs_frame: bool,
    /// Shown.
    pub visible: bool,
    /// The key window (has the keyboard).
    pub active: bool,
    /// Full screen (as the compositor configured it).
    pub fullscreen: bool,
    /// Zoomed to the usable area (as the compositor configured it).
    pub maximized: bool,
    /// A left press on the toolbar's background, where the window moves from if the pointer
    /// leaves the slop while the button is down.
    pub(crate) move_press: Option<Point>,
    /// Appearance override (`Some(true)`: always dark, the Clock and Calculator).
    pub dark_override: Option<bool>,
    /// Last click: time, position, count (double-click detection).
    pub last_click: (f64, Point, u32),
    /// Whether the last focus change came from the keyboard.
    pub keyboard_navigation: bool,
    /// The app handles [`crate::core::value::Signal::CloseRequested`] itself (a document
    /// that asks to save); otherwise the close button closes the window.
    pub keep_on_close_request: bool,
    /// Opaque: painted with `Surface/Window` under the content (Amalfi windows are flat and
    /// opaque; popups and shell surfaces are transparent).
    pub opaque: bool,
    /// The compositor draws a title bar (false: an alert's panel, which has none).
    pub decorated: bool,
    /// The compositor draws the glass regions (popups, shell surfaces, transparent panels).
    pub compositor_glass: bool,
    /// Height of the toolbar the app draws under Ossido's window buttons (full-size
    /// content; 0: Ossido's title bar).
    pub toolbar_height: f32,
    /// The opacity of the sheet's veil as last painted (0: none).
    pub(crate) veil: f32,
}

impl Window {
    pub(crate) fn new(id: WindowId, root: ViewId, kind: WindowKind, size: Size) -> Self {
        let kind_ref = kind.clone();
        Window {
            id,
            root,
            title: String::new(),
            kind,
            size,
            scale: 1.0,
            min_size: None,
            max_size: None,
            resizable: true,
            full_size_content: false,
            focus: None,
            key_anchor: None,
            autosave: None,
            restore: std::collections::HashMap::new(),
            hover: Vec::new(),
            capture: None,
            pointer: None,
            damage: Rect::from_size(size),
            input_region: None,
            needs_layout: true,
            needs_frame: true,
            visible: false,
            active: false,
            fullscreen: false,
            dark_override: None,
            last_click: (f64::NEG_INFINITY, Point::ZERO, 0),
            maximized: false,
            move_press: None,
            keyboard_navigation: false,
            keep_on_close_request: false,
            compositor_glass: kind_glass(&kind_ref),
            opaque: !kind_glass(&kind_ref) && !matches!(kind_ref, WindowKind::Lock { .. }),
            decorated: true,
            toolbar_height: 0.0,
            veil: 0.0,
        }
    }
}

/// Whether windows of `kind` float over something they do not draw (their glass is the
/// compositor's).
fn kind_glass(kind: &WindowKind) -> bool {
    matches!(
        kind,
        WindowKind::Popup { .. }
            | WindowKind::Shell { .. }
            | WindowKind::MenuTitles
            | WindowKind::StatusItem { .. }
    )
}

impl WindowKind {
    /// A surface of the compositor's menu bar.
    pub fn in_menu_bar(&self) -> bool {
        matches!(self, WindowKind::MenuTitles | WindowKind::StatusItem { .. })
    }
}
