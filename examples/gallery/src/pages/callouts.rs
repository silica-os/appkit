//! Callouts with the arrow on each edge.

use appkit_ui::prelude::*;

use super::{page, section};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, _cx: &mut Cx) -> Element {
        let edges = ["Top", "Bottom", "Leading", "Trailing", "None"];
        let items: Vec<Element> = edges
            .iter()
            .enumerate()
            .map(|(i, e)| callout(i as i64, 2, label(*e).padding(16.0)).key(i as i64))
            .collect();
        let backdrop = row(items)
            .spacing(24.0)
            .padding(32.0)
            .background(rgba(0xD98F2BFF))
            .corner_radius(20.0);
        page(vec![section("Arrow edge", backdrop)])
    }
}
