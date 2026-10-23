//! Buttons: the five push variants at every size and state, icon buttons, links, help.

use appkit_ui::prelude::*;

use super::{captioned, page, section};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, _cx: &mut Cx) -> Element {
        let variants = ["Primary", "Secondary", "Destructive", "Borderless", "Glass"];
        let by_variant = row(variants
            .iter()
            .enumerate()
            .map(|(i, v)| captioned(v, button("Button").style(i as i64)))
            .collect::<Vec<_>>())
        .spacing(16.0)
        .alignment(1);
        let by_size = row(["Mini", "Small", "Regular", "Large"]
            .iter()
            .enumerate()
            .map(|(i, v)| captioned(v, button("Button").style(0).control_size(i as i64)))
            .collect::<Vec<_>>())
        .spacing(16.0)
        .alignment(1);
        let states = row((
            captioned("Rest", button("Button").style(1)),
            captioned("Disabled", button("Button").style(1).disabled(true)),
            captioned("Default", button("Button").style(0).default_action()),
            captioned("With symbol", button("Share").style(1).symbol("plus")),
        ))
        .spacing(16.0)
        .alignment(1);
        let icons = row((
            captioned("Round", icon_button("gear")),
            captioned("Square", icon_button("gear").style(1)),
            captioned("Disabled", icon_button("gear").disabled(true)),
            captioned("Link", link("A link")),
            captioned("Help", help_button()),
        ))
        .spacing(16.0)
        .alignment(1);
        page(vec![
            section("Variants", by_variant),
            section("Sizes", by_size),
            section("States", states),
            section("Icon buttons, links, help", icons),
        ])
    }
}
