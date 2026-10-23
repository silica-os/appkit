//! Extras: the families the ui-kit storybook has no page for (collections, panels, the status
//! bar), kept as a test app with a small sidebar.
//!
//! - `appkit-extras`: the window.
//! - `appkit-extras --snapshot DIR [--dark]`: every page headless to `DIR/<nn>-<page>.png`.

mod pages;

use appkit_ui::kit;
use appkit_ui::prelude::*;
use pages::PAGES;

const SIZE: (f32, f32) = (1000.0, 800.0);

/// The window's content; `start` is the page shown first.
struct Extras {
    start: usize,
}

impl Component for Extras {
    fn body(&self, cx: &mut Cx) -> Element {
        let start = self.start.min(PAGES.len().saturating_sub(1));
        let page = cx.state(move || start);
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
            column((sidebar(rows).width(200.0).fill(),))
                .width(200.0)
                .max_height(f32::INFINITY),
            scroll_view(content).fill(),
        ))
        .spacing(0.0)
        .alignment(0)
        .fill()
    }
}

/// `--snapshot DIR [--dark]`.
fn snapshot(args: &[String]) -> Option<i32> {
    let i = args.iter().position(|a| a == "--snapshot")?;
    let Some(dir) = args.get(i + 1).map(std::path::PathBuf::from) else {
        eprintln!("appkit-extras: --snapshot needs a directory");
        return Some(2);
    };
    let dark = args.iter().any(|a| a == "--dark");
    let run = || -> Result<(), String> {
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let _rt = kit::Headless::new().map_err(|e| e.to_string())?;
        for (n, p) in PAGES.iter().enumerate() {
            let png = appkit_ui::preview(SIZE, 2.0, dark, move || Extras { start: n })
                .map_err(|e| format!("{}: {e}", p.name))?;
            let slug = p.name.to_lowercase().replace(' ', "-");
            std::fs::write(dir.join(format!("{:02}-{slug}.png", n + 1)), png)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    };
    match run() {
        Ok(()) => Some(0),
        Err(e) => {
            eprintln!("appkit-extras: {e}");
            Some(1)
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(code) = snapshot(&args) {
        std::process::exit(code);
    }
    let r = kit::run("dev.silica.KitExtras", move || {
        let spec = appkit_ui::WindowSpec::new("AppKit Extras", SIZE).min_size(700.0, 500.0);
        if let Ok(root) = spec.open(|| Extras { start: 0 }) {
            std::mem::forget(root);
        }
    });
    if let Err(e) = r {
        eprintln!("appkit-extras: {e}");
        std::process::exit(1);
    }
}
