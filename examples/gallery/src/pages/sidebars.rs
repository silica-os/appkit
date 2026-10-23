//! Sidebars: rows with a symbol, selected and not.

use appkit_ui::prelude::*;

use super::{page, section};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let sel = cx.state(|| 0usize);
        let items = [
            ("folder", "Documents"),
            ("doc", "Recents"),
            ("trash", "Trash"),
        ];
        let rows: Vec<Element> = items
            .iter()
            .enumerate()
            .map(|(i, (s, t))| {
                let sel = sel.clone();
                sidebar_row(s, t, sel.get() == i, move || sel.set(i)).key(i as i64)
            })
            .collect();
        page(vec![section("Sidebar", sidebar(rows).frame(220.0, 140.0))])
    }
}
