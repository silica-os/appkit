//! Fields: text, secure, search, text area and combo box.

use appkit_ui::prelude::*;

use super::{captioned, page, pick, section};

/// The page.
pub struct Page(pub &'static [usize]);

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let (t, p, s, a) = (
            cx.state(String::new),
            cx.state(String::new),
            cx.state(String::new),
            cx.state(String::new),
        );
        let text = column((
            captioned("Text", text_field("Name", &t.binding()).width(240.0)),
            captioned(
                "Disabled",
                text_field("Name", &t.binding()).width(240.0).disabled(true),
            ),
        ))
        .spacing(12.0)
        .alignment(0);
        let secure = secure_field("Password", &p.binding()).width(240.0);
        let search = search_field("Search", &s.binding()).width(240.0);
        let area = text_area("Notes", &a.binding()).frame(320.0, 96.0);
        let fruit = cx.state(|| "Pear".to_string());
        let combo = column((
            combo_box("Fruit", &["Apple", "Pear", "Plum"], &fruit.binding()).width(200.0),
            label(format!("Chosen: {}", fruit.get())).color("Text/Secondary"),
        ))
        .spacing(8.0)
        .alignment(0);
        page(pick(
            self.0,
            vec![
                section("Text field", text),
                section("Secure field", secure),
                section("Search field", search),
                section("Text area", area),
                section("Combo box", combo),
            ],
        ))
    }
}
