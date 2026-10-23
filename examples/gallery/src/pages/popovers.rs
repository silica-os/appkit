//! Popovers on each side of their anchor.

use appkit_ui::prelude::*;

use super::{page, section};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let open = cx.state(|| false);
        let o2 = open.clone();
        let anchor = button("Show popover")
            .on_activate(move || o2.set(true))
            .popover(&open.binding(), (240.0, 72.0), || {
                label("A popover with the kit's arrow and glass.").padding(12.0)
            });
        page(vec![section("Popover", anchor)])
    }
}
