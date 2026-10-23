//! Scroll views and their scrollbars.

use appkit_ui::prelude::*;

use super::{page, section};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, _cx: &mut Cx) -> Element {
        let rows = column(
            (1..=40)
                .map(|i| label(format!("Row {i}")).key(i as i64))
                .collect::<Vec<_>>(),
        )
        .spacing(6.0)
        .alignment(0)
        .padding(12.0);
        let sv = scroll_view(rows).frame(320.0, 200.0);
        page(vec![section("Scroll the list", sv)])
    }
}
