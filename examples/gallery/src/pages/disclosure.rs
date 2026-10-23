//! Disclosure groups.

use appkit_ui::prelude::*;

use super::{page, section};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let (a, b) = (cx.state(|| false), cx.state(|| true));
        let c = cx.state(|| true);
        let g = column((
            disclosure_group(
                "Collapsed",
                &a.binding(),
                column((
                    label("Hidden until expanded"),
                    checkbox("An option", &c.binding()),
                ))
                .spacing(8.0),
            ),
            disclosure_group("Expanded", &b.binding(), label("Content shows.")),
        ))
        .spacing(12.0)
        .alignment(0);
        page(vec![section("Disclosure groups", g)])
    }
}
