//! Steppers.

use appkit_ui::prelude::*;

use super::{captioned, page, section};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let n = cx.state(|| 3.0f64);
        let row1 = row((
            captioned("Stepper", stepper(&n.binding(), 0.0, 10.0, 1.0)),
            captioned(
                "Disabled",
                stepper(&n.binding(), 0.0, 10.0, 1.0).disabled(true),
            ),
            label(format!("{}", n.get())).color("Text/Secondary"),
        ))
        .spacing(24.0)
        .alignment(1);
        page(vec![section("Steppers", row1)])
    }
}
