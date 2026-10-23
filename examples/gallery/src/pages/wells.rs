//! Wells: colour wells in three styles, and the image well.

use appkit_ui::kit::Kind;
use appkit_ui::prelude::*;

use super::{captioned, page, section};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let c = cx.state(|| rgba(0x0E77E6FF));
        let styles = row(["Minimal", "Standard", "Expanded"]
            .iter()
            .enumerate()
            .map(|(i, n)| captioned(n, color_well(&c.binding()).style(i as i64)))
            .collect::<Vec<_>>())
        .spacing(24.0)
        .alignment(1);
        page(vec![
            section("Colour wells", styles),
            section("Image well", view(Kind::IMAGE_WELL).frame(96.0, 96.0)),
        ])
    }
}
