//! Tooltips: hover a control and wait.

use appkit_ui::prelude::*;

use super::{page, section};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, _cx: &mut Cx) -> Element {
        let r = row((
            button("Hover me").style(1).tooltip("A tooltip on a button"),
            icon_button("gear").tooltip("Settings"),
            label("A label with a tooltip").tooltip("Tooltips appear after a short delay"),
        ))
        .spacing(16.0);
        page(vec![section("Tooltips", r)])
    }
}
