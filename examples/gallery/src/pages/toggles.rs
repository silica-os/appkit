//! Toggles: checkboxes (on, off, mixed-free), radio buttons and switches, every state.

use appkit_ui::prelude::*;

use super::{captioned, page, section};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let (a, b, c) = (cx.state(|| true), cx.state(|| false), cx.state(|| true));
        let choice = cx.state(|| 1usize);
        let (s1, s2) = (cx.state(|| true), cx.state(|| false));
        let checks = row((
            captioned("On", checkbox("Checkbox", &a.binding())),
            captioned("Off", checkbox("Checkbox", &b.binding())),
            captioned(
                "Disabled",
                checkbox("Checkbox", &c.binding()).disabled(true),
            ),
        ))
        .spacing(24.0)
        .alignment(1);
        let radios = column(
            ["Small", "Medium", "Large"]
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let ch = choice.clone();
                    radio(t, choice.get() == i, move || ch.set(i))
                })
                .collect::<Vec<_>>(),
        )
        .spacing(8.0)
        .alignment(0);
        let switches = row((
            captioned("On", switch(&s1.binding())),
            captioned("Off", switch(&s2.binding())),
            captioned("Disabled", switch(&s1.binding()).disabled(true)),
            captioned("Pressed", switch(&s1.binding()).pressed(true)),
            captioned("Dragging: the lens", switch(&s2.binding()).dragging(true)),
        ))
        .spacing(24.0)
        .alignment(1);
        page(vec![
            section("Checkboxes", checks),
            section("Radio buttons", radios),
            section("Switches", switches),
        ])
    }
}
