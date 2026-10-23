//! Sliders and dials.

use appkit_ui::kit::Kind;
use appkit_ui::prelude::*;

use super::{captioned, page, section};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let (a, b) = (cx.state(|| 0.0f64), cx.state(|| 0.6f64));
        let sliders = column((
            captioned("Minimum", slider(&a.binding(), 0.0, 1.0).width(240.0)),
            captioned("Value", slider(&b.binding(), 0.0, 1.0).width(240.0)),
            captioned(
                "Disabled",
                slider(&b.binding(), 0.0, 1.0).width(240.0).disabled(true),
            ),
            label(format!("{:.0} %", b.get() * 100.0)).color("Text/Secondary"),
        ))
        .spacing(12.0)
        .alignment(0);
        page(vec![
            section("Sliders", sliders),
            section("Dial", view(Kind::DIAL).frame(64.0, 64.0)),
        ])
    }
}
