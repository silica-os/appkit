//! The Kit Gallery: every family of AppKit's Amalfi kit, live, in one window, mirroring the
//! ui-kit Library (a sidebar of families; a content pane with each family's variants, sizes and
//! states). Built with the declarative layer, so it runs the whole stack: appkit-ui →
//! appkit-rs → the C ABI → appkit.
//!
//! - `appkit-gallery`: the window.
//! - `appkit-gallery --snapshot DIR [--dark]`: renders every page headless to
//!   `DIR/<nn>-<page>.png` at 2× and exits.
//! - `appkit-gallery --preview PATH [PAGE] [dark]`: renders one page.

mod pages;

use appkit_ui::kit::{self, AppearanceOption};
use appkit_ui::prelude::*;
use pages::PAGES;

const SIZE: (f32, f32) = (1100.0, 800.0);

/// Accent choices of the toolbar: the name and the colour (`None`: Amalfi's).
const ACCENTS: &[(&str, Option<u32>)] = &[
    ("Amalfi", None),
    ("Red", Some(0xE5484DFF)),
    ("Orange", Some(0xF76B15FF)),
    ("Green", Some(0x30A46CFF)),
    ("Purple", Some(0x8E4EC6FF)),
];

/// The window's content; `start` is the page shown first.
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

impl Component for Gallery {
    fn body(&self, cx: &mut Cx) -> Element {
        let start = self.start.min(PAGES.len().saturating_sub(1));
        let page = cx.state(move || start);
        let tick = cx.state(|| 0u32);
        let t_dark = tick.clone();
        let dark = Binding::new(kit::is_dark, move |v| {
            let _ = kit::set_dark(v);
            t_dark.update(|n| *n += 1);
        });
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
        let bar = toolbar((
            toolbar_title("Kit Gallery"),
            spacer(),
            label("Accent").color("Text/Secondary"),
            pop_up(&names, &accent).id("accent"),
            checkbox("Dark", &dark).id("dark"),
            checkbox(
                "Reduce Motion",
                &option(&tick, AppearanceOption::ReduceMotion),
            )
            .id("reduce-motion"),
            checkbox(
                "Increase Contrast",
                &option(&tick, AppearanceOption::IncreaseContrast),
            )
            .id("increase-contrast"),
        ));
        let current = page.get().min(PAGES.len().saturating_sub(1));
        let rows: Vec<Element> = PAGES
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let page = page.clone();
                sidebar_row(p.symbol, p.name, i == current, move || page.set(i)).key(i as i64)
            })
            .collect();
        let content = match PAGES.get(current) {
            Some(p) => (p.build)().key(current as i64),
            None => label("No pages."),
        };
        row((
            scroll_view(sidebar(rows).width(240.0))
                .width(240.0)
                .max_height(f32::INFINITY),
            column((
                row((
                    label(PAGES.get(current).map_or("", |p| p.name)).text_style("Title 2"),
                    spacer(),
                    button("Quit").style(1).id("quit").on_activate(kit::quit),
                ))
                .padding_xy(24.0, 12.0),
                scroll_view(content).fill(),
            ))
            .spacing(0.0)
            .alignment(0)
            .fill(),
        ))
        .spacing(0.0)
        .alignment(0)
        .fill()
        .toolbar(bar)
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
        for (n, p) in PAGES.iter().enumerate() {
            let png = render(n, dark).map_err(|e| format!("{}: {e}", p.name))?;
            let path = dir.join(snapshot_name(n, p.name));
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
    let menus = || -> kit::Result<()> {
        let app = kit::Menu::new("Kit Gallery", Some("app"))?
            .item("About Kit Gallery", None, 1, 0)?
            .separator()?
            .item("Quit Kit Gallery", Some("cmd+q"), 2, 0)?;
        let window = kit::Menu::new("Window", Some("window"))?;
        kit::set_menu_bar(vec![app, window], |id| {
            if id == 2 {
                kit::quit()
            }
        })
    };
    let r = kit::run("dev.silica.KitGallery", move || {
        let _ = menus();
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
