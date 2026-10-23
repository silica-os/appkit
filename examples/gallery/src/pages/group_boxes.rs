//! ButtonsMoreDoc, Group box: titled, untitled, nested.

use appkit_ui::prelude::*;

use super::{h2, lead, page};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let (a, b, c, sync) = (
            cx.state(|| true),
            cx.state(|| true),
            cx.state(|| false),
            cx.state(|| true),
        );
        let name = cx.state(|| "Amalfi coast".to_string());
        let remember = cx.state(|| true);
        let titled = group_box(
            Some("Notifications"),
            column((
                checkbox("Show banners", &a.binding()),
                checkbox("Play sound", &b.binding()),
                checkbox("Show on lock screen", &c.binding()),
            ))
            .spacing(8.0)
            .alignment(0)
            .padding(12.0),
        )
        .width(220.0);
        let untitled = group_box(
            None,
            column((
                text_field("Name", &name.binding()),
                checkbox("Remember this name", &remember.binding()),
            ))
            .spacing(8.0)
            .alignment(0)
            .padding(12.0),
        )
        .width(240.0);
        let nested = group_box(
            Some("Recipe sharing"),
            column((
                row((label("Enable sync"), spacer(), switch(&sync.binding()))),
                group_box(
                    None,
                    column((
                        text_field("Name", &name.binding()),
                        checkbox("Remember this name", &remember.binding()),
                    ))
                    .spacing(8.0)
                    .alignment(0)
                    .padding(12.0),
                )
                .width(240.0),
            ))
            .spacing(8.0)
            .alignment(0)
            .padding(12.0),
        )
        .width(300.0);
        page(vec![
            lead(
                "Small controls that other components build on: a glyph-only button, a text link, the expand chevron and the box that groups controls.",
            ),
            h2("Group box"),
            row((
                column((
                    label("Titled").text_style("Caption").color("Text/Tertiary"),
                    titled,
                ))
                .spacing(4.0)
                .alignment(0),
                column((
                    label("Titled, disabled")
                        .text_style("Caption")
                        .color("Text/Tertiary"),
                    group_box(Some("Notifications"), label("Show banners").padding(12.0))
                        .width(220.0)
                        .disabled(true),
                ))
                .spacing(4.0)
                .alignment(0),
                column((
                    label("Untitled")
                        .text_style("Caption")
                        .color("Text/Tertiary"),
                    untitled,
                ))
                .spacing(4.0)
                .alignment(0),
                column((
                    label("Nested").text_style("Caption").color("Text/Tertiary"),
                    nested,
                ))
                .spacing(4.0)
                .alignment(0),
            ))
            .spacing(16.0)
            .alignment(0),
        ])
    }
}
