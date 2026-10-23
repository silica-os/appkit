//! Forms: rows with a label column, sections and a footer.

use appkit_ui::prelude::*;

use super::{page, section};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let (name, mail) = (cx.state(String::new), cx.state(String::new));
        let size = cx.state(|| 1usize);
        let news = cx.state(|| true);
        let f = form(
            Some("Account"),
            (
                form_section(
                    None,
                    (
                        form_row("Name:", text_field("Full name", &name.binding())),
                        form_row("Email:", text_field("name@example.com", &mail.binding())),
                        form_row(
                            "Size:",
                            pop_up(&["Small", "Medium", "Large"], &size.binding()),
                        ),
                        form_row("", checkbox("Send me news", &news.binding())),
                    ),
                ),
                form_footer((button("Cancel").style(1), button("Save").style(0))),
            ),
        )
        .width(440.0);
        page(vec![section("Form", f)])
    }
}
