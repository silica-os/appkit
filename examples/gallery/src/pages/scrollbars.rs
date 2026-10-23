//! ScrollDoc: the scroll area playground and the two bar styles.

use appkit_ui::prelude::*;

use super::{card, code, field, h2, lead, page, stage};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let disabled = cx.state(|| false);
        let mode = cx.state(|| 0usize);
        let axis = cx.state(|| 0usize);
        let lines = |n: usize| {
            column(
                (1..=n)
                    .map(|i| {
                        label(format!(
                            "Line {i}. The terrace looks over the bay and the lemon groves climb the hill behind the house."
                        ))
                        .key(i as i64)
                    })
                    .collect::<Vec<_>>(),
            )
            .spacing(8.0)
            .alignment(0)
            .padding(12.0)
        };
        let area = scroll_view(lines(14))
            .style(mode.get() as i64)
            .scroll_axis(axis.get() as i64)
            .frame(264.0, 180.0)
            .disabled(disabled.get());
        let controls = column((
            field("Mode", segmented(&["overlay", "legacy"], &mode.binding())),
            field("Axis", segmented(&["both", "y", "x"], &axis.binding())),
            checkbox("disabled", &disabled.binding()),
        ))
        .spacing(10.0)
        .alignment(0);
        page(vec![
            lead(
                "Content scrolls natively; the scrollbars are drawn by the component and stay in sync. Overlay bars float over the content, appear while scrolling or hovering and grow when pointed at; legacy bars keep an always visible 16 pt track. Drag a thumb, click the track to move a page, use the wheel or the keyboard.",
            ),
            h2("Playground"),
            row((stage(area), card(controls)))
                .spacing(16.0)
                .alignment(0),
            code("scroll_view(content).frame(264.0, 180.0)"),
            h2("Overlay"),
            row((
                scroll_view(lines(14)).scroll_hold(1).frame(264.0, 140.0),
                scroll_view(lines(14)).scroll_hold(2).frame(264.0, 140.0),
            ))
            .spacing(16.0),
            label("Thin, then expanded: the states an overlay bar goes through.")
                .text_style("Caption")
                .color("Text/Tertiary"),
            h2("Legacy"),
            scroll_view(lines(14)).style(1).frame(264.0, 140.0),
        ])
    }
}
