//! Segmented controls: text segments, several sizes, disabled.

use appkit_ui::prelude::*;

use super::{captioned, page, section};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let (a, b, c) = (
            cx.state(|| 0usize),
            cx.state(|| 1usize),
            cx.state(|| 2usize),
        );
        let sizes = column((
            captioned(
                "Small",
                segmented(&["Day", "Week", "Month"], &a.binding()).control_size(1),
            ),
            captioned(
                "Regular",
                segmented(&["Day", "Week", "Month"], &b.binding()),
            ),
            captioned(
                "Large",
                segmented(&["Day", "Week", "Month"], &c.binding()).control_size(3),
            ),
        ))
        .spacing(12.0)
        .alignment(0);
        let many = segmented(
            &["Inbox", "Sent", "Drafts", "Archive", "Trash"],
            &b.binding(),
        );
        let off = segmented(&["On", "Off"], &a.binding()).disabled(true);
        page(vec![
            section("Sizes", sizes),
            section("Five segments", many),
            section("Disabled", off),
        ])
    }
}
