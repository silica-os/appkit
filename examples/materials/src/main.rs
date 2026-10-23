//! Materials: a stand-alone example.
//!
//! - `appkit-materials`: the window.
//! - `appkit-materials --preview PATH [dark]`: renders it headless to a PNG at 2× and exits.

mod page;

use appkit_ui::kit;
use appkit_ui::prelude::*;

const SIZE: (f32, f32) = (720.0, 420.0);

/// The window's content.
struct Example;

impl Component for Example {
    fn body(&self, cx: &mut Cx) -> Element {
        scroll_view(page::Page.body(cx)).fill()
    }
}

/// `--preview PATH [dark]`.
fn preview(args: &[String]) -> Option<i32> {
    let i = args.iter().position(|a| a == "--preview")?;
    let path = args.get(i + 1)?.clone();
    let dark = args.iter().any(|a| a == "dark");
    let _rt = match kit::Headless::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("appkit-materials: {e}");
            return Some(1);
        }
    };
    let png = appkit_ui::preview(SIZE, 2.0, dark, || Example).map_err(|e| e.to_string());
    match png.and_then(|png| std::fs::write(&path, png).map_err(|e| e.to_string())) {
        Ok(()) => Some(0),
        Err(e) => {
            eprintln!("appkit-materials: {path}: {e}");
            Some(1)
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(code) = preview(&args) {
        std::process::exit(code);
    }
    let r = kit::run("dev.silica.Materials", move || {
        if let Ok(root) = appkit_ui::WindowSpec::new("Materials", SIZE).open(|| Example) {
            // The window lives as long as the process.
            std::mem::forget(root);
        }
    });
    if let Err(e) = r {
        eprintln!("appkit-materials: {e}");
        std::process::exit(1);
    }
}
