//! Menus: a pull-down, and a context menu (secondary click on the panel).

use appkit_ui::MenuEntry;
use appkit_ui::prelude::*;

use super::{page, section};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let last = cx.state(|| "Nothing chosen yet.".to_string());
        let (a, b, c) = (last.clone(), last.clone(), last.clone());
        let pull = pull_down("File", &["New", "Open…", "Close"], move |i| {
            a.set(format!("File menu: item {}", i + 1))
        });
        let area = label("Secondary-click here for a context menu.")
            .padding(24.0)
            .background(rgba(0x80808022))
            .corner_radius(12.0)
            .context_menu(vec![
                MenuEntry::item("Copy", move || b.set("Copy".into())).shortcut("cmd+c"),
                MenuEntry::item("Paste", move || c.set("Paste".into())).shortcut("cmd+v"),
                MenuEntry::separator(),
                MenuEntry::item("Quit", appkit_ui::kit::quit).shortcut("cmd+q"),
            ]);
        page(vec![
            section("Pull-down menu", pull),
            section("Context menu", area),
            label(last.get()).color("Text/Secondary"),
        ])
    }
}
