//! Pop-up and pull-down buttons.

use appkit_ui::prelude::*;

use super::{captioned, page, section};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let pick = cx.state(|| 1usize);
        let last = cx.state(String::new);
        let l2 = last.clone();
        let items = ["Small", "Medium", "Large"];
        let r = row((
            captioned("Pop-up", pop_up(&items, &pick.binding())),
            captioned("Disabled", pop_up(&items, &pick.binding()).disabled(true)),
            captioned(
                "Pull-down",
                pull_down(
                    "Actions",
                    &["Rename", "Duplicate", "Move to Trash"],
                    move |i| l2.set(format!("Chose item {}", i + 1)),
                ),
            ),
            label(last.get()).color("Text/Secondary"),
        ))
        .spacing(24.0)
        .alignment(1);
        page(vec![section("Pop-up and pull-down", r)])
    }
}
