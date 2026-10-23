//! Materials: the three glass tiers over a coloured backdrop.

use appkit_ui::prelude::*;

use super::{page, section};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, _cx: &mut Cx) -> Element {
        let tiers = ["Thin", "Regular", "Thick"];
        let panels: Vec<Element> = tiers
            .iter()
            .enumerate()
            .map(|(i, t)| glass_panel(i as i64, 16.0, label(*t).padding(24.0)).key(i as i64))
            .collect();
        let backdrop = row(panels)
            .spacing(16.0)
            .padding(32.0)
            .background(rgba(0x2B7FD9FF))
            .corner_radius(20.0);
        page(vec![section("Glass: Thin, Regular, Thick", backdrop)])
    }
}
