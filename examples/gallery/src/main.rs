//! The Kit Gallery: every family of AppKit's Amalfi kit, live, in one window, mirroring the
//! ui-kit Library (a sidebar of families; a content pane with each family's variants, sizes and
//! states). Built with the declarative layer, so it runs the whole stack: appkit-ui →
//! appkit-rs → the C ABI → appkit.
//!
//! - `appkit-gallery`: the window.
//! - `appkit-gallery --snapshot DIR [--dark]`: renders every page headless to
//!   `DIR/<nn>-<page>.png` at 2× and exits.
//! - `appkit-gallery --preview PATH [PAGE] [dark]`: renders one page.

mod foundations;
mod pages;

use std::cell::RefCell;
use std::rc::Rc;

use appkit_ui::kit::{self, AppearanceOption, Kind};
use appkit_ui::prelude::*;
use pages::{PAGES, field};

const SIZE: (f32, f32) = (1240.0, 1000.0);

/// Accent choices of the toolbar: the name and the colour (`None`: Amalfi's).
const ACCENTS: &[(&str, Option<u32>)] = &[
    ("Amalfi", None),
    ("Red", Some(0xE5484DFF)),
    ("Orange", Some(0xF76B15FF)),
    ("Green", Some(0x30A46CFF)),
    ("Purple", Some(0x8E4EC6FF)),
];

/// The Foundations pages: their names.
const FOUNDATIONS: &[&str] = &[
    "Colors",
    "Typography",
    "Glass",
    "Spacing, radii, shadows",
    "Motion",
    "Animation Lab",
];

/// The menu items the gallery runs (the others have roles AppKit runs).
const QUIT: u32 = 1;
const SIDEBAR: u32 = 2;
const LIGHT: u32 = 3;
const DARK: u32 = 4;
const HOME: u32 = 5;
const PREVIOUS: u32 = 6;
const NEXT: u32 = 7;
const ROLE: u32 = 100;

/// What the menus act on: the window's page, sidebar and appearance, and what the bar shows.
struct Nav {
    page: State<usize>,
    sidebar: State<bool>,
    tick: State<u32>,
    shown: Option<(usize, bool, bool)>,
}

thread_local! {
    static NAV: RefCell<Option<Nav>> = const { RefCell::new(None) };
}

/// The window rendered: the menus follow its page, sidebar and appearance.
fn follow(page: &State<usize>, sidebar: &State<bool>, tick: &State<u32>) {
    let now = (page.get(), sidebar.get(), kit::is_dark());
    let changed = NAV.with(|nav| {
        let mut nav = nav.borrow_mut();
        let changed = nav.as_ref().is_none_or(|n| n.shown != Some(now));
        *nav = Some(Nav {
            page: page.clone(),
            sidebar: sidebar.clone(),
            tick: tick.clone(),
            shown: Some(now),
        });
        changed
    });
    if changed {
        let _ = menus(now.0, now.1, now.2);
    }
}

/// A menu item AppKit runs: `role`, with its standard shortcut.
fn standard(
    menu: kit::Menu,
    title: &str,
    shortcut: Option<&str>,
    role: &str,
) -> kit::Result<kit::Menu> {
    menu.item(title, shortcut, ROLE, 0)?.role(role)
}

/// The menu bar: the App, File, Edit, View, Go and Window menus.
fn menus(page: usize, sidebar: bool, dark: bool) -> kit::Result<()> {
    use kit::menu_flags::{CHECKED, DISABLED};
    let when = |on: bool| if on { 0 } else { DISABLED };
    let check = |on: bool| if on { CHECKED } else { 0 };
    let mut app = kit::Menu::new("Kit Gallery", Some("app"))?;
    app = standard(app, "About Kit Gallery", None, "about")?.separator()?;
    app = standard(app, "Hide Kit Gallery", Some("cmd+h"), "hide")?;
    app = standard(app, "Hide Others", Some("cmd+alt+h"), "hide_others")?;
    app = standard(app, "Show All", None, "show_all")?
        .separator()?
        .item("Quit Kit Gallery", Some("cmd+q"), QUIT, 0)?;
    let file = standard(
        kit::Menu::new("File", None)?,
        "Close Window",
        Some("cmd+w"),
        "close",
    )?;
    let mut edit = kit::Menu::new("Edit", Some("edit"))?;
    edit = standard(edit, "Undo", Some("cmd+z"), "undo")?;
    edit = standard(edit, "Redo", Some("cmd+shift+z"), "redo")?.separator()?;
    for (title, key, role) in [
        ("Cut", "cmd+x", "cut"),
        ("Copy", "cmd+c", "copy"),
        ("Paste", "cmd+v", "paste"),
        ("Select All", "cmd+a", "select_all"),
    ] {
        edit = standard(edit, title, Some(key), role)?;
    }
    let sidebar_title = if sidebar {
        "Hide Sidebar"
    } else {
        "Show Sidebar"
    };
    let mut view = kit::Menu::new("View", None)?
        .item(sidebar_title, Some("ctrl+cmd+s"), SIDEBAR, 0)?
        .separator()?
        .item("Light", None, LIGHT, check(!dark))?
        .item("Dark", None, DARK, check(dark))?
        .separator()?;
    view = standard(view, "Enter Full Screen", Some("ctrl+cmd+f"), "fullscreen")?;
    let last = count().saturating_sub(1);
    let go = kit::Menu::new("Go", None)?
        .item("Home", Some("shift+cmd+h"), HOME, when(page != 0))?
        .item("Previous Page", Some("cmd+["), PREVIOUS, when(page > 0))?
        .item("Next Page", Some("cmd+]"), NEXT, when(page < last))?;
    let mut window = kit::Menu::new("Window", Some("window"))?;
    window = standard(window, "Minimize", Some("cmd+m"), "minimize")?;
    window = standard(window, "Zoom", None, "zoom")?.separator()?;
    window = standard(window, "Bring All to Front", None, "bring_all_to_front")?;
    kit::set_menu_bar(vec![app, file, edit, view, go, window], chosen)
}

/// A gallery menu item was chosen.
fn chosen(id: u32) {
    if id == QUIT {
        kit::quit();
        return;
    }
    let Some((page, sidebar, tick)) = NAV.with(|nav| {
        nav.borrow()
            .as_ref()
            .map(|n| (n.page.clone(), n.sidebar.clone(), n.tick.clone()))
    }) else {
        return;
    };
    let now = page.get();
    match id {
        SIDEBAR => sidebar.set(!sidebar.get()),
        LIGHT | DARK => {
            let _ = kit::set_dark(id == DARK);
            tick.update(|n| *n += 1);
        }
        HOME => page.set(0),
        PREVIOUS => page.set(now.saturating_sub(1)),
        NEXT => page.set((now + 1).min(count().saturating_sub(1))),
        _ => {}
    }
}

/// What the sidebar and the snapshots index: 0 is Home, then the Library pages, then the
/// Foundations.
fn count() -> usize {
    1 + PAGES.len() + FOUNDATIONS.len()
}

fn name_of(index: usize) -> String {
    match index {
        0 => "Home".to_string(),
        i if i <= PAGES.len() => PAGES
            .get(i - 1)
            .map_or(String::new(), |p| p.name.to_string()),
        i => FOUNDATIONS
            .get(i - 1 - PAGES.len())
            .map_or(String::new(), |n| (*n).to_string()),
    }
}

/// The content of page `index`.
fn content_of(index: usize, go: Rc<dyn Fn(usize)>) -> Element {
    match index {
        0 => component(foundations::Home {
            go,
            library: PAGES.len(),
        }),
        i if i <= PAGES.len() => match PAGES.get(i - 1) {
            Some(p) => (p.build)(),
            None => label("No page."),
        },
        i => match i - 1 - PAGES.len() {
            0 => component(foundations::Colors),
            1 => component(foundations::Typography),
            2 => component(foundations::Glass),
            3 => component(foundations::Layout),
            4 => component(foundations::Motion),
            _ => component(foundations::Lab),
        },
    }
}

/// The window's content; `start` is the page shown first, `dark` the preview of a Library page.
struct Gallery {
    start: usize,
}

/// A binding to one of the appearance options, which live in the runtime.
fn option(tick: &State<u32>, o: AppearanceOption) -> Binding<bool> {
    let t = tick.clone();
    Binding::new(
        move || kit::appearance_option(o),
        move |v| {
            let _ = kit::set_appearance_option(o, v);
            t.update(|n| *n += 1);
        },
    )
}

/// A section label of the sidebar (`.label`: 11, semibold, upper case, muted; the subtitle
/// in plain case).
fn group_header(title: &str, subtitle: Option<&str>) -> Element {
    let mut parts = vec![
        label(title.to_uppercase())
            .text_style("Label")
            .color("Text/Secondary"),
    ];
    if let Some(sub) = subtitle {
        parts.push(label(sub).text_style("Caption").color("Text/Secondary"));
    }
    // Rows inset their label by the panel's padding; the label lines up with them.
    row(parts)
        .spacing(4.0)
        .alignment(10)
        .padding_edges(14.0, 12.0, 4.0, 12.0)
}

impl Component for Gallery {
    fn body(&self, cx: &mut Cx) -> Element {
        let start = self.start.min(count().saturating_sub(1));
        let page = cx.state(move || start);
        let tick = cx.state(|| 0u32);
        let query = cx.state(String::new);
        let t_dark = tick.clone();
        let appearance = Binding::new(
            || usize::from(kit::is_dark()),
            move |i: usize| {
                let _ = kit::set_dark(i == 1);
                t_dark.update(|n| *n += 1);
            },
        );
        let accent_open = cx.state(|| false);
        let access_open = cx.state(|| false);
        let t_acc = tick.clone();
        let accent = Binding::new(
            || {
                let now = kit::accent().map(|c| {
                    (u32::from(c.r) << 24) | (u32::from(c.g) << 16) | (u32::from(c.b) << 8) | 0xFF
                });
                ACCENTS.iter().position(|(_, c)| *c == now).unwrap_or(0)
            },
            move |i: usize| {
                let c = ACCENTS.get(i).and_then(|(_, c)| *c).map(rgba);
                let _ = kit::set_accent(c);
                t_acc.update(|n| *n += 1);
            },
        );
        let names: Vec<&str> = ACCENTS.iter().map(|(n, _)| *n).collect();
        let accent_menu = {
            let accent = accent.clone();
            move || {
                column((field("Accent", pop_up(&names, &accent).id("accent")),))
                    .alignment(0)
                    .padding(16.0)
            }
        };
        let access_menu = {
            let motion = option(&tick, AppearanceOption::ReduceMotion);
            let contrast = option(&tick, AppearanceOption::IncreaseContrast);
            move || {
                column((
                    row((
                        label("Reduce Motion"),
                        spacer(),
                        switch(&motion).id("reduce-motion"),
                    ))
                    .alignment(4),
                    row((
                        label("Increase Contrast"),
                        spacer(),
                        switch(&contrast).id("increase-contrast"),
                    ))
                    .alignment(4),
                ))
                .spacing(12.0)
                .alignment(0)
                .padding(16.0)
            }
        };
        let current = page.get().min(count().saturating_sub(1));
        let sidebar_open = cx.state(|| true);
        follow(&page, &sidebar_open, &tick);
        let bar = (
            toolbar_title(&name_of(current)),
            spacer(),
            toolbar_views(&[("sun", "Light"), ("moon", "Dark")], &appearance).id("appearance"),
            {
                let open = accent_open.clone();
                toolbar_button("palette")
                    .id("accent-button")
                    .tooltip("Accent colour")
                    .on_activate(move || open.set(true))
                    .popover(&accent_open.binding(), (240.0, 92.0), accent_menu)
            },
            {
                let open = access_open.clone();
                toolbar_button("accessibility")
                    .id("accessibility-button")
                    .tooltip("Accessibility")
                    .on_activate(move || open.set(true))
                    .popover(&access_open.binding(), (260.0, 108.0), access_menu)
            },
        );
        let go: Rc<dyn Fn(usize)> = {
            let page = page.clone();
            Rc::new(move |i| page.set(i))
        };
        // Sidebar.vue: the brand, the search, the Library (filtered), the Foundations.
        let q = query.get().trim().to_lowercase();
        let row_for = |i: usize| {
            let page = page.clone();
            sidebar_row("", &name_of(i), i == current, move || page.set(i)).key(i as i64)
        };
        let mut rows: Vec<Element> = Vec::new();
        // The brand goes home, as the storybook's does.
        let brand = {
            let page = page.clone();
            button("Silica UI Kit")
                .style(3)
                .id("brand")
                .on_activate(move || page.set(0))
        };
        rows.push(group_header("Library", Some("pages of the Sketch kit")).key(-1i64));
        let mut shown = 0;
        for (n, p) in PAGES.iter().enumerate() {
            if q.is_empty() || p.name.to_lowercase().contains(&q) {
                rows.push(row_for(n + 1));
                shown += 1;
            }
        }
        if shown == 0 {
            rows.push(
                label("No library page matches.")
                    .text_style("Caption")
                    .color("Text/Tertiary")
                    .padding_xy(12.0, 4.0)
                    .key(-3i64),
            );
        }
        rows.push(group_header("Foundations", None).key(-2i64));
        for (n, _) in FOUNDATIONS.iter().enumerate() {
            rows.push(row_for(1 + PAGES.len() + n));
        }
        // DocPage: the title, then the page, 36 in and at most 1100 wide.
        // Home carries its own heading.
        let title = if current == 0 {
            view(Kind::VIEW).hidden(true)
        } else {
            label(name_of(current)).text_style("Title 1")
        };
        let body = column((title, content_of(current, go)))
            .spacing(12.0)
            .alignment(0)
            .padding(36.0)
            .max_width(1100.0)
            .fill_width();
        // Sidebar.vue in SWindow's split: the brand and the search across the sidebar (in by
        // the panel's padding, as the rows are), then the pages.
        let side = column((
            column((
                row((brand, label("Amalfi 1.0").color("Text/Secondary")))
                    .spacing(4.0)
                    .alignment(10),
                search_field("Search pages and components", &query.binding())
                    .id("search")
                    .fill_width(),
            ))
            .spacing(12.0)
            .alignment(0)
            .padding_edges(4.0, 12.0, 0.0, 12.0)
            .fill_width(),
            scroll_view(sidebar(rows).style(1)).fill(),
        ))
        .spacing(0.0)
        .alignment(0);
        split_window(
            &sidebar_open.binding(),
            264.0,
            Vec::new(),
            side,
            bar,
            scroll_view(body).fill().background_role("Surface/Window"),
        )
        .id("window")
    }
}

/// The file name of a page's snapshot: `<nn>-<page>.png`.
fn snapshot_name(index: usize, name: &str) -> String {
    let slug: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let slug = slug
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    format!("{:02}-{slug}.png", index + 1)
}

/// Renders page `index` headless to a PNG at 2×.
fn render(index: usize, dark: bool) -> Result<Vec<u8>, String> {
    appkit_ui::preview(SIZE, 2.0, dark, move || Gallery { start: index }).map_err(|e| e.to_string())
}

/// `--snapshot DIR [--dark]`: every page to `DIR/<nn>-<page>.png`.
fn snapshot(args: &[String]) -> Option<i32> {
    let i = args.iter().position(|a| a == "--snapshot")?;
    let Some(dir) = args.get(i + 1).map(std::path::PathBuf::from) else {
        eprintln!("appkit-gallery: --snapshot needs a directory");
        return Some(2);
    };
    let dark = args.iter().any(|a| a == "--dark" || a == "dark");
    let run = || -> Result<(), String> {
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let _rt = kit::Headless::new().map_err(|e| e.to_string())?;
        for n in 0..count() {
            let name = name_of(n);
            let png = render(n, dark).map_err(|e| format!("{name}: {e}"))?;
            let path = dir.join(snapshot_name(n, &name));
            std::fs::write(&path, png).map_err(|e| format!("{}: {e}", path.display()))?;
        }
        Ok(())
    };
    match run() {
        Ok(()) => Some(0),
        Err(e) => {
            eprintln!("appkit-gallery: {e}");
            Some(1)
        }
    }
}

/// `--preview PATH [PAGE] [dark]`: renders the window headless to a PNG at 2× and exits.
fn preview(args: &[String]) -> Option<i32> {
    let i = args.iter().position(|a| a == "--preview")?;
    let path = args.get(i + 1)?.clone();
    let start = args.get(i + 2).and_then(|p| p.parse().ok()).unwrap_or(0);
    let dark = args.iter().any(|a| a == "dark");
    let _rt = match kit::Headless::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("appkit-gallery: {e}");
            return Some(1);
        }
    };
    match render(start, dark).and_then(|png| std::fs::write(&path, png).map_err(|e| e.to_string()))
    {
        Ok(()) => Some(0),
        Err(e) => {
            eprintln!("appkit-gallery: {path}: {e}");
            Some(1)
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(code) = snapshot(&args).or_else(|| preview(&args)) {
        std::process::exit(code);
    }
    let r = kit::run("dev.silica.KitGallery", move || {
        let _ = menus(0, true, kit::is_dark());
        let spec = appkit_ui::WindowSpec::new("Kit Gallery", SIZE)
            .min_size(900.0, 600.0)
            .autosave("gallery");
        if let Ok(root) = spec.open(|| Gallery { start: 0 }) {
            // The window lives as long as the process.
            std::mem::forget(root);
        }
    });
    if let Err(e) = r {
        eprintln!("appkit-gallery: {e}");
        std::process::exit(1);
    }
}
