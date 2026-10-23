//! ScrollDoc: the scroll area playground and the two bar styles.

use appkit_ui::prelude::*;

use super::{code, field, h2, lead, page, playground};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let disabled = cx.state(|| false);
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
            .frame(264.0, 180.0)
            .disabled(disabled.get());
        let controls = column((
            field(
                "Mode",
                label("overlay (legacy follows the system setting)").color("Text/Secondary"),
            ),
            field(
                "Axis",
                label("both (a wide line scrolls sideways)").color("Text/Secondary"),
            ),
            checkbox("disabled", &disabled.binding()),
        ))
        .spacing(10.0)
        .alignment(0);
        page(vec![
            lead(
                "Content scrolls natively; the scrollbars are drawn by the component and stay in sync. Overlay bars float over the content, appear while scrolling or hovering and grow when pointed at; legacy bars keep an always visible 16 pt track. Drag a thumb, click the track to move a page, use the wheel or the keyboard.",
            ),
            h2("Playground"),
            playground(area, controls).spacing(16.0).alignment(0),
            code("scroll_view(content).frame(264.0, 180.0)"),
            h2("Overlay"),
            super::wrapped(
                "The bars above are the overlay ones: they appear while you scroll or point at them.",
                "Callout",
                "Text/Secondary",
                680.0,
            ),
            h2("Legacy"),
            super::wrapped(
                "Legacy bars (an always visible 16 pt track) are chosen by the system setting, not per view.",
                "Callout",
                "Text/Tertiary",
                680.0,
            ),
        ])
    }
}
