//! Group boxes.

use appkit_ui::prelude::*;

use super::{page, section};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let on = cx.state(|| true);
        let titled = group_box(
            Some("Sharing"),
            column((
                checkbox("Allow guests", &on.binding()),
                checkbox("Show previews", &on.binding()),
            ))
            .spacing(8.0)
            .alignment(0)
            .padding(12.0),
        )
        .width(280.0);
        let plain = group_box(None, label("A box without a title").padding(12.0)).width(280.0);
        page(vec![section("Titled", titled), section("Untitled", plain)])
    }
}
