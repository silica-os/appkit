//! Titlebars and toolbars: the title, buttons, a group and the search field.

use appkit_ui::prelude::*;

use super::{page, section};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let q = cx.state(String::new);
        let bar = toolbar((
            toolbar_title("Recipes"),
            spacer(),
            toolbar_group(
                &[("chevron-left", "Back"), ("chevron-right", "Forward")],
                |_| {},
            ),
            toolbar_button("doc-plus").accessibility_label("New"),
            toolbar_search("Search", &q.binding()),
        ))
        .fill_width()
        .height(52.0);
        page(vec![section("Toolbar", bar)])
    }
}
