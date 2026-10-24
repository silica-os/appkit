# AppKit: status

Living document: what is done, the decisions taken, and what comes next. A successor picks up
from "Next" below. Architecture: [architecture.md](architecture.md).

Gate (real exit codes): `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.
ABI: `cargo xtask header` (regenerates `include/appkit.h`, `abi/appkit.map`), `cargo xtask abi-check [--debug]`.
VM: in `silica-os/system`, `tools/dev-sysext appkit-dev` builds the library and the Kit
Gallery from this checkout and merges them into the running dev VM (`tools/run-vm --gpu venus
--resolution 1920x1200 --fullscreen`); `tools/test-vm --desktop --appkit` checks the gallery.

## Crates

| Crate | What | State |
|---|---|---|
| `appkit` (lib `appkit`, cdylib + rlib + staticlib for packaging) | the toolkit | see below |
| `appkit-sys` | the C ABI: types, ids, one function list (`for_each_function!`) | done (94 functions, APPKIT_1.0) |
| `appkit-rs` | safe Rust objects over the ABI | done for the current ABI |
| `appkit-ui` | declarative layer | done (v1), see below |
| `appkit-test` | kit-parity harness | done; families added as components land |
| `examples/gallery` | demo app for the VM | push buttons, menus, context menu; grows with components |
| `xtask` | sync-kit, check-sources, header, package, abi-check, vm | done |
| `appkit-inspector` | dumps view trees, layers and the a11y tree over the inspector socket | done |
| `examples/bench` | frame-time budgets (exits non-zero over budget) | done |
| `appkit-terminal` | planned | not started |

## appkit: done

- **Foundations**: `geom`, `color`, `shape` (smooth corners), `style` (kit layer styles:
  fills/gradients, inside/centre/outside/dashed borders, CSS-blur shadows, inner shadows,
  backdrop blur), glass refraction on the CPU (`render::backdrop`), `motion` (Bézier, springs
  from response/ζ, interruptible `Animated<T>`).
- **Tokens** from `silica-os/ui-kit` (`data/tokens.json`, `data/silica_motion.rs`) and the
  Silica Symbols codepoints from `silica-os/symbols`; `cargo xtask sync-sources` copies them,
  `cargo xtask check-sources` compares. `amalfi::theme`: palette, Increase Contrast, accent,
  materials as styles, Reduce Transparency fallback.
- **Text**: embedded Inter / JetBrains Mono / Silica Symbols, fontconfig fallback (off in
  tests: `APPKIT_DETERMINISTIC`), parley layout (even leading as the kit), attributed
  strings, attachments, UTF-8 offsets; `text::edit::Editor` (selection, grapheme/word/line/
  paragraph/document movement, macOS bindings, undo groups, IME preedit, surrounding delete).
- **Core**: generational view arena; generic modifiers (padding, frame, priority, opacity,
  background, corner radius, offset, scale, hidden, disabled, appearance override); proposal
  layout (Stack with SwiftUI space sharing and first-baseline alignment, Spacer, Overlay,
  Grid); display lists of named kit layers (render + inspection) incl. compositor glass
  items; events with responder chain, hover/press/focus and activation (silica_ui
  AmalfiInteraction semantics), `state_changed` notifications, key view loop with Full
  Keyboard Access, default/cancel buttons, menu key equivalents, app shortcuts; signals with
  deferred re-entrant-safe handlers; timers; animations; damage tracking; snapshots;
  accessibility descriptions/tree and actions (`Ui::perform`); menus (`core::menu`: pop-up
  menus; `core::menu_bar`: the app's menu titles drawn and tracked in the app, autoenabling
  with a validator before showing and before key equivalents, status items).
- **Components**: every kit family (table under "Components"), plus `TextView` (documents:
  line index with Fenwick sums, layout per visible line, 0.8 ms per key on a 10 MB document in
  release) and the collections (`amalfi::collections`, one provider model: items are made
  when they come into sight and dropped when they leave; a million rows cost what is shown):
  `ListView` (inset, plain and bordered list-box looks), `TableView` (columns, header with
  sort and column resizing, cells per row and column, in-place editing of editable columns,
  alternating rows; outline mode over a `TreeSource` with chevrons, indentation, ←/→ and ⌥
  deep expansion), `GridView` (reflowing tiles, 2-D keyboard), `TableHeader`; shared
  selection (⌘/⇧, anchor and cursor), keyboard, double click/Return, overlay scroller;
  selected rows are emphasized (`NodeFlags::EMPHASIZED`, `Theme::emphasize`) while focused
  in an active window. `TabView` (window tabs on the kit's tab bar), `Toolbar` (kit unified
  bar pieces after the window controls), form pieces as kinds (value, footer, box surface).
- **Windows**: size limits honoured (#245: sizes clamped from the compositor, the app and
  restoration); sheets are xdg-dialog modal dialogs with client-side decorations (Ossido on
  the silica-desktop `appkit` branch centres them, veils the parent, keeps the focus and
  moves them with the parent) and their parent takes no input; state restoration
  (`core::restore`: autosave name → size and identified views' column widths, sort,
  expanded nodes, split positions, selected tabs, collapsed panes, scroll offsets in
  `$XDG_STATE_HOME/<app id>/windows/<name>.state`); full-size content with a toolbar
  (`Ui::set_toolbar_height`, also on shown windows); frame alignment applied
  (`.align()`: content at its own size aligned in a larger frame); input regions
  (`Ui::set_input_region`, for shell surfaces with transparent margins); undecorated
  windows (`Window::decorated`: alert panels of their own have no title bar and are modal
  xdg dialogs without a parent, which Ossido centres on the screen).
- **App**: lifecycle (`core::app`), single instance and reopen through silica-desktop's app
  registry socket (`platform::registry`), drag and drop, xdg-activation, pinch gestures, idle
  inhibition, the AccessKit AT-SPI bridge (`platform::a11y`, verified in the VM when
  `IsEnabled`), the inspector socket (`appkit-inspector` CLI dumps trees and display lists).
- **Platform** (`platform::wayland`, SCTK 0.20): xdg toplevels, popups, layer-shell, session
  lock, fractional scale + viewporter, cursor-shape, keyboard (xkb, client-side repeat with
  Settings1 rate/delay), pointer + axis (value120), text-input-v3 (preedit/commit/delete,
  surrounding text clipped to 4000 bytes), clipboard (copy/paste text), silica-window-v1
  (full-size content, appearance), silica-glass-v1 (compositor glass regions with profiles),
  silica-menu-bar-v1 (the menu-title surface and status-item surfaces, menus as popups of
  them), frame pacing by frame callbacks.
- **Renderer** (`render`): `Canvas` on vello_cpu (single-threaded) with curves flattened at
  0.1 device px, region rendering (`begin_region`: a frame rasterizes only its damage rows),
  whole-pixel copies of cached images, a glyph-outline cache (`render::glyphs`, skrifa) and
  backdrops (blur, saturation, refraction) filtered in bands on threads. Presented through
  wl_shm with `damage_buffer`. Raster caches of static subtrees (`core::raster`: stable for 3
  frames, ≥ 24 items, no backdrop/glass/custom drawing; drawn at whole device pixels; 96 MB
  cap; `APPKIT_RASTER_CACHE=0` turns it off; off in headless tests). The GPU backend
  (`render::gpu`, wgpu: Vulkan, GLES fallback) has the CPU canvas's formulas, a retained frame
  texture, atlases for glyphs and paths, layer textures and presents through a wgpu surface;
  agreement tests against the CPU canvas (docs/architecture.md).
- **Start-up**: bundled fonts registered at once; the system fonts (fontconfig) are loaded on a
  thread, the collection is warmed there and joins the fallback when ready (text laid out
  again); `APPKIT_STARTUP_TIMING` prints the start-up stages and the first frame.
- **Services** (`services`, zbus): typed proxies for every daemon interface and the portals,
  `AppearanceWatcher` (System1 look → `Appearance`, live), `SettingsWatcher` (key repeat,
  renderer key), `Observable`, `Services` facade. The platform runs them on the calloop
  executor; `platform::spawn` / `platform::services` for apps. Details: `docs/services.md`.
- **C ABI** (`ffi`): every function of `appkit_sys::for_each_function!` defined and
  signature-checked at compile time; runtime reached through `with_ui` (the platform's `Ui`
  inside `ak_app_run` and its callbacks, or a per-thread headless one); panics caught at
  the boundary; callback user data freed through `destroy` exactly once.

## Declarative layer (`appkit-ui`)

Components (`body(&self, cx) -> Element`), positional `cx.state`, `State` / `Binding` /
`Model` (observable, `cx.watch`), builders (`column`, `row`, `overlay`, `grid`, `spacer`,
`group`, `label`, `symbol`, `button`, `view(kind)`, `custom`, `host`, `component`,
`for_each`), modifiers (padding, frame, min/max, fill, align, priority, background, corner
radius, clip, opacity, hidden, disabled, id, tooltip, accessibility label, dark, spacing,
alignment, style, control size, text style/colour, symbol, default/cancel, handlers). Keyed
reconciler with property diffing and handler swapping; one batched re-render per loop turn;
`mount` (declarative inside imperative), `host` (imperative inside declarative), `preview`
(headless PNG). Implicit animations (`.animation`, `.transition` with FLIP moves and leaving
views), `.sheet`, `.popover`, `.context_menu`; control builders bound to state
(`controls.rs`); `list(count, row_height, |i| element)` mounts a tree per visible row and
renders the live rows again with their component; `table`, `outline` and `collection` on the
same model (`on_sort`, `sorted`, `on_edit`, `on_select`, `selection`, `on_open`, `on_expand`,
`alternating`); `tabs` (`on_new_tab`, `on_close_tab`); `.alert(&Binding, AlertSpec)`; the
kit's popover for `.popover`; `toolbar` (`toolbar_button`, `toolbar_group`, `toolbar_title`,
`toolbar_search`) with `Element::toolbar` (full-size content); `form`, `form_section`,
`form_row`, `form_row_with`, `form_footer`; `WindowSpec` (size limits, autosave).

## Decisions

- **Library name.** The crate `appkit` builds `appkit` (the crate `appkit` is the
  declarative layer). Distribution: `cargo xtask package` links `libappkit.so.1` (soname) from
  the staticlib with `abi/appkit.map` (`APPKIT_1.0`), plus `libappkit.so` and `appkit.pc`.
- **Header generator instead of cbindgen.** The ABI is one macro list in appkit-sys (so that
  appkit's definitions are checked against the declarations Rust users link); cbindgen does
  not expand macros on stable, so `xtask header` generates `appkit.h` from the same list. The
  ABI check compiles a C layout probe and a C smoke test against the header and the packaged
  library, and verifies the exported symbol set and versions.
- **Property-based ABI.** Views are made by kind (`AK_KIND_*`) and configured by property
  ids (typed setters) and signals: new components and properties are new numbers, never new
  signatures, so the ABI stays stable while the kit grows.
- **Kit layers as display lists.** Components draw named layers (`Surface`, `Label`, `Focus
  Halo` …) with kit styles; the same list renders and is read back by kit-parity tests and the
  inspector.
- **Kit defects** go to `docs/kit-issues.md` and become named deviations in the tests (the
  design docs win over the kit).
- **Pixel tolerances**: `Tolerance::SHAPES` (mean 1, max 48), `CONTROLS` (mean 1.5, max 84:
  Sketch antialiases small controls ~¼ pt wider, measured by silica_ui on the same masters),
  `WITH_TEXT` (mean 2.5); glass refraction bands are masked when a comparison fails (as
  silica_ui).
- **Fonts embedded**; **no runtime JSON**.
- **Core never calls the platform**: it queues `Request`s; signal handlers run after dispatch.
- **Window content** is offered the window size and centred at the size it takes; fill with
  an infinite max width/height. Normal windows are opaque (`Surface/Window`), popups and shell
  surfaces transparent with compositor glass.
- **One app loop per process** (`ak_app_run`); headless runtimes are per thread.
- **Precise closure capture pitfall**: a `move` closure that reads only `data.user` captures
  only that field; the FFI forces whole capture (`let d = &data;`) where a guard must live.

## Testing

- `cargo test`: unit tests, kit parity (`crates/appkit-test/tests/*.rs`), FFI round trips,
  appkit-rs, declarative reconciler.
- `examples/gallery/tests/e2e.rs` runs a headless compositor when its binary is found, starts
  the gallery, clicks Quit through the control socket and checks the exit.
  `examples/bench`: frame budgets, see below.
- VM: the Kit Gallery runs in the Silica VM at 1920×1200 (Venus GPU, wl_shm buffers).

## Components

Each family: widgets in `crates/appkit/src/amalfi/<family>.rs`, parity test in
`crates/appkit-test/tests/<family>.rs` (every master of the family, geometry exact, pixels with
a named tolerance, text masked), behaviour unit tests in the module. Kit defects and the
deviations they cause are in [kit-issues.md](kit-issues.md).

| Family | Widgets | Masters | State |
| --- | --- | --- | --- |
| Toggles | `Toggle` (checkbox, radio), `Switch`, `toggle_group` | 296 | done |
| Tooltips | `Tooltip`, `place`, hover presenter in `Ui` | 14 | done (placement by position; pixels of `Placement` wait for the round icon button) |
| Segments | `SegmentedControl`, `ActionSegments`, `PaneControl`, `SegmentItem` | 530 | done (Dark Icon + Label: geometry only) |
| Sliders | `Slider`, `Dial`, `SliderThumb`, `LensThumb` | 64 | done |
| Steppers | `Stepper`, `stepper_with_field` | 68 | done |
| Progress | `ProgressBar`, `CircularProgress`, `Spinner` | 84 | done |
| Disclosure | `Disclosure`, `DisclosureGroup` | 44 | done |
| Fields | `TextField` (text, secure, area, search, tokens, combo, value), `Token`, `ClearButton`, `ComboButton` | 128 | done (the combo list is a menu popup, `TextField::choose_in`; fields send `TextInput` and `TextInputSurrounding`) |
| Buttons | `PushButton`, `IconButton`, `HelpButton`, `LinkButton`, `GradientButtons`, `ToolbarButton`, `MenuButton` | 214 of 220 | partial: pop-up/pull-down `Open` masters geometry only (the kit's glass hides the button), `App` masters need app icons |
| Group Boxes | `GroupBox` | 8 | done |
| Wells | `ColorWell`, `ImageWell`, swatch grid, colour panel popup | 74 | done |
| Forms | `form_row`, `form_section`, `form_footer`, `form_layout` | 22 | done |
| Scrollbars | `Scrollbar`, `ScrollbarCorner`, `ScrollView` | 34 | done |
| Menus | `MenuItem`, `Menu`, `menus::present` (pop-up, pull-down, list, at a point) | 48 of 52 (+4 open buttons) | partial: `Item/App` needs app icons; the chosen-row blink is not timed |
| Sidebars | `Row`, `Badge`, `EjectButton`, `Header`, `group`, `Footer`, `Panel` | 44 | done |
| Titlebars | `Dot`, `WindowControls`, `TitleBlock`, `SearchPiece`, `SwitcherPiece`, `SendPiece`, `Strip`, `Spacer`, `TitleBar`, `Tab`, `TabBar` | 181 | done |
| Popovers | `PopoverShape`, `Popover`, `popovers::present` (flips sides) | 84 | done; in the C ABI (`ak_view_present_popover`) |
| Alerts | `Alert`, `alert()` (title, message, accessories, buttons, badge) | 26 | done (icons are bitmaps: masked); in the C ABI (`ak_alert_*`, presented as a sheet or a panel) |
| Dialogs | `Dialog`, `Pane`, `browser`, `dialog_footer` | 12 | done |
| Status Bar | `StatusItem`, `StatusBar` (plates of Increase Contrast and Reduce Transparency) | 22 | done; `STATUS_ITEM` kind (content kind, glyph, text); status items in the C ABI (`ak_status_item_*`) |
| Menu Bar | `MenuTitle` (mark, bold app name, menus), `MenuBar` | 12 | done (Menu Open: the menu is compared by `Menus`); the Silica mark's system menu is silica-os/appkit#25 |
| Windows | `WindowFrame`, `Block` | 40 | done for the window's own layers; nested components of other families (group boxes, segments, checkbox group, search field, menu bar) are placeholders there |
| Collections (`Apps/Mustache` file list) | `TableHeader`, `TableView` rows, `ListView` rows, `GridView` tile | 16 | done (two kit quirks in kit-issues.md) |

Shared changes made on the way: the harness expands nested symbol masters (path-addressed
layers, instance text overrides applied in order) and walks groups; `Scene` has
`auto_geometry_except`, `extra_mask`, `png_margin`, `hover_at`, `press_at`; `Recorder` has
`glyph_sized`; `Widget::attached` lets a view start animations on its own; `Ui::set` restarts
ticking; the tooltip presenter is part of `Ui`; `Text` has a wrapping paragraph mode. The harness
also has `ignore_aspects` (a kit defect on every layer, e.g. the dark glass blur) and
`skip_trees` (nested components replaced by placeholders); kit-driven builders shared by the
tests live in `tests/common/mod.rs`. Open problems: app icons (Item/App, Pop-up/App, alert
icons), the `Open` masters of buttons, Windows with
the real nested components. Window buttons are at the 23 pt pitch (windows.md and every kit
render since silica-design fb5f9f10, compared unmasked); alerts follow the binding
anatomy of components.md (280 wide, centred text, buttons one per row 240 × 28, default
first, Escape last).

## Performance (#254 budgets)

`cargo run --release -p appkit-bench` (headless, CPU raster, development machine); the table is
in the README. Cold start 89 ms (Vetro on Skia: 125 ms), hover 0.16 ms, typing 0.62 ms per key
in a 10 MB document, 500 controls with a spinner 0.13 ms (damage only) and 4.4 ms (whole
window from the raster cache), Dock with 20 magnifying tiles on glass at 1920×1200 3.8 ms, 975
symbols at 2× 9.6 ms. Where the time goes: vello_cpu's `fill_path` costs about 5 µs per glyph
(its glyph atlas has 4096² pages, too costly for a first frame, so it is not used); a flush
costs the rows it covers; backdrops dominate glass scenes.

## Next

1. Behaviour with no ui-kit source yet: collections (selection, keyboard, type-select),
   tooltips' enter fade.
2. Touchpad tuning on real hardware (kinetic decay, navigation swipe distance); the VM only
   has a mouse.

Scope: AppKit is the public toolkit only. The Dock, Control Centre, Notification Centre,
menu bar, Faro and the lock screen are system apps built on AppKit in their own processes,
not part of this crate. Values come from the ui-kit tokens (`crates/appkit/data/tokens.json`),
or from the ui-kit `.vue`/CSS where no token exists; never from Vetro.
