//! Status bars.

use appkit_ui::PValue;
use appkit_ui::kit::{Kind, Prop};
use appkit_ui::prelude::*;

use super::{page, section};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, _cx: &mut Cx) -> Element {
        let bar = view(Kind::STATUS_BAR)
            .children((
                view(Kind::STATUS_ITEM).prop(Prop::TITLE, PValue::Str("12 items".into())),
                view(Kind::STATUS_ITEM).prop(Prop::TITLE, PValue::Str("3 selected".into())),
            ))
            .width(420.0);
        page(vec![section("Status bar", bar)])
    }
}
