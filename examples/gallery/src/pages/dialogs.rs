//! Dialogs: a sheet with a form and a footer.

use appkit_ui::prelude::*;

use super::{page, section};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let open = cx.state(|| false);
        let o2 = open.clone();
        let b = button("Share Recipe…")
            .style(1)
            .on_activate(move || o2.set(true))
            .sheet(&open.binding(), (400.0, 180.0), {
                let o3 = open.clone();
                move || {
                    let (c, s) = (o3.clone(), o3.clone());
                    column((
                        label("Share recipe").text_style("Title 3"),
                        label("Anyone with the link can view it."),
                        row((
                            spacer(),
                            button("Cancel")
                                .style(1)
                                .cancel_action()
                                .on_activate(move || c.set(false)),
                            button("Share")
                                .style(0)
                                .default_action()
                                .on_activate(move || s.set(false)),
                        ))
                        .spacing(8.0),
                    ))
                    .spacing(16.0)
                    .padding(24.0)
                }
            });
        page(vec![section("Dialog sheet", b)])
    }
}
