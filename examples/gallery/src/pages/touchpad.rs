//! Touchpad: the events a touchpad sends, shown live, and a list to try kinetic scrolling and
//! the rubber band on.

use appkit_ui::prelude::*;

use super::{page, section};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, _cx: &mut Cx) -> Element {
        let rows = column(
            (1..=60)
                .map(|i| label(format!("Row {i}")).key(i as i64))
                .collect::<Vec<_>>(),
        )
        .spacing(6.0)
        .alignment(0)
        .padding(12.0);
        page(vec![
            section(
                "Events: scroll with two fingers, pinch, swipe with three, rest your fingers",
                touch_probe(),
            ),
            section(
                "Scroll the list: it coasts after you lift your fingers and stretches at the ends",
                scroll_view(rows).frame(320.0, 200.0),
            ),
        ])
    }
}
