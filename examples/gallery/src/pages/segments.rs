//! SegmentedDoc: the playground, controls, icon and label, action segments, text alignment.

use appkit_ui::prelude::*;

use super::{card, code, field, h2, lead, page, stage};

/// The page.
pub struct Page;

const SIZES: [&str; 4] = ["mini", "small", "regular", "large"];

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let size = cx.state(|| 2usize);
        let count = cx.state(|| 1usize);
        let value = cx.state(|| 0usize);
        let tsize = cx.state(|| 2usize);
        let (a, b, c) = (
            cx.state(|| 0usize),
            cx.state(|| 1usize),
            cx.state(|| 1usize),
        );
        let multi = cx.state(|| vec![0usize]);
        let icons = cx.state(|| 0usize);
        let panes = cx.state(|| 0usize);
        let all = ["Day", "Week", "Month", "Year", "All", "Custom"];
        let n = count.get() + 2;
        let items: Vec<&str> = all.iter().take(n).copied().collect();
        let live = column((
            segmented(&items, &value.binding()).control_size(size.get() as i64),
            label(format!(
                "selected: {}",
                items.get(value.get()).copied().unwrap_or("-")
            ))
            .color("Text/Secondary"),
        ))
        .spacing(12.0)
        .alignment(1);
        let controls = column((
            field("Size", segmented(&SIZES, &size.binding())),
            field(
                "Count",
                segmented(&["2", "3", "4", "5", "6"], &count.binding()),
            ),
            field(
                "Multiple",
                segmented_multiple(&["Day", "Week", "Month"], &multi.binding()),
            ),
            label(format!("on: {:?}", multi.get()))
                .text_style("Caption")
                .color("Text/Tertiary"),
        ))
        .spacing(10.0)
        .alignment(0);
        let t = tsize.get() as i64;
        let sized = |i: i64, bind: &Binding<usize>| {
            segmented(&["Day", "Week", "Month"], bind).control_size(i)
        };
        let _ = sized;
        page(vec![
            lead(
                "A glass capsule with a thumb that slides to the selected item. Single or multiple selection, or an action group that only fires clicks. Arrow keys move through the items.",
            ),
            h2("Playground"),
            row((stage(live), card(controls)))
                .spacing(16.0)
                .alignment(0),
            code("segmented(&[\"Day\", \"Week\", \"Month\"], &selected).control_size(2)"),
            h2("Controls"),
            segmented(&SIZES, &tsize.binding()),
            column((
                segmented(&["Day", "Week", "Month"], &a.binding()).control_size(t),
                segmented(
                    &["Inbox", "Sent", "Drafts", "Archive", "Trash"],
                    &b.binding(),
                )
                .control_size(t),
                segmented(&["Day", "Week", "Month"], &c.binding())
                    .control_size(t)
                    .pressed(true),
                segmented(&["On", "Off"], &a.binding())
                    .control_size(t)
                    .disabled(true),
            ))
            .spacing(12.0)
            .alignment(0),
            label("Rest, five items, held (the thumb is the lens), disabled.")
                .text_style("Caption")
                .color("Text/Tertiary"),
            h2("Icon and label"),
            segmented_icons(
                &[
                    ("list-bullet", "List"),
                    ("square-grid-2x2", "Grid"),
                    ("gallery", "Gallery"),
                ],
                &icons.binding(),
            ),
            pane_control(
                &[
                    ("gear", "General"),
                    ("folder", "Files"),
                    ("gallery", "Display"),
                ],
                &panes.binding(),
            ),
            h2("Action segments"),
            toolbar_group(
                &[("chevron-left", "Back"), ("chevron-right", "Forward")],
                |_| {},
            ),
            h2("Text alignment"),
            segmented(&["Left", "Center", "Right", "Justified"], &b.binding()).control_size(1),
        ])
    }
}
