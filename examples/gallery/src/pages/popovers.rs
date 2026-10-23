//! MenusDoc, Popovers.

use appkit_ui::prelude::*;

use super::{h2, h3, lead, page, stage};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let o = cx.state(|| false);
        let o3 = o.clone();
        let trigger = button("Popover")
            .style(0)
            .on_activate(move || o3.set(true))
            .popover(&o.binding(), (260.0, 84.0), || {
                column((
                    label("About this ingredient").text_style("Headline"),
                    label("Olive oil keeps the crust tender and adds a peppery note.")
                        .color("Text/Secondary"),
                ))
                .spacing(4.0)
                .alignment(0)
                .padding(14.0)
            });
        page(vec![
            lead("A popover anchors any content (or a menu) to a trigger."),
            h2("Popovers"),
            stage(trigger),
            h3("Arrow on every edge"),
            super::wrapped(
                "The popover opens on the side of its trigger where it fits and its arrow points at the trigger; near a window edge it flips to the other side.",
                "Callout",
                "Text/Secondary",
                680.0,
            ),
        ])
    }
}
