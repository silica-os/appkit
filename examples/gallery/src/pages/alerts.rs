//! Alerts: standard, destructive.

use appkit_ui::AlertSpec;
use appkit_ui::prelude::*;

use super::{page, section};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let (plain, danger) = (cx.state(|| false), cx.state(|| false));
        let said = cx.state(String::new);
        let (p2, d2) = (plain.clone(), danger.clone());
        let (s1, s2) = (said.clone(), said.clone());
        let buttons = row((
            button("Show Alert…")
                .style(1)
                .on_activate(move || p2.set(true)),
            button("Show Destructive Alert…")
                .style(1)
                .on_activate(move || d2.set(true)),
            label(said.get()).color("Text/Secondary"),
        ))
        .spacing(12.0)
        .alert(
            &plain.binding(),
            AlertSpec::new("Quit without saving?", &["Cancel", "Quit"], move |r| {
                s1.set(format!("Plain alert: button {r}"))
            })
            .message("Your changes will be lost.")
            .icon("info"),
        )
        .alert(
            &danger.binding(),
            AlertSpec::new(
                "Move to the Trash?",
                &["Cancel", "Move to Trash"],
                move |r| s2.set(format!("Destructive alert: button {r}")),
            )
            .destructive()
            .icon("trash"),
        );
        page(vec![section("Alerts", buttons)])
    }
}
