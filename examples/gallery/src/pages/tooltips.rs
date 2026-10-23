//! MenusDoc, Tooltips.

use appkit_ui::prelude::*;

use super::{h2, lead, page, stage};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, _cx: &mut Cx) -> Element {
        page(vec![
            lead(
                "A tooltip waits 700 ms on the first hover and shows at once when you move between tooltips.",
            ),
            h2("Tooltips"),
            stage(
                row((
                    icon_button("plus").tooltip("Add to favourites  ⌘D"),
                    icon_button("gear").tooltip("Settings"),
                    button("Two lines")
                        .style(1)
                        .tooltip("Shows the recipe in a new window so you can keep this one open."),
                    label("Hover the controls").color("Text/Secondary"),
                ))
                .spacing(16.0)
                .alignment(1),
            ),
        ])
    }
}
