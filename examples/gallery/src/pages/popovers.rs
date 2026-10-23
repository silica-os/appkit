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
        let mut sides = Vec::new();
        for (i, name) in ["Below", "Above", "After", "Before"]
            .into_iter()
            .enumerate()
        {
            let st = cx.state(|| false);
            let s2 = st.clone();
            sides.push(
                button(name)
                    .style(1)
                    .key(i as i64)
                    .on_activate(move || s2.set(true))
                    .popover(&st.binding(), (220.0, 56.0), move || {
                        label(format!(
                            "Opens {} the trigger when it fits.",
                            name.to_lowercase()
                        ))
                        .padding(14.0)
                    })
                    .popover_side(i as u32),
            );
        }
        page(vec![
            lead("A popover anchors any content (or a menu) to a trigger."),
            h2("Popovers"),
            stage(trigger),
            h3("Arrow on every edge"),
            row(sides).spacing(16.0),
        ])
    }
}
