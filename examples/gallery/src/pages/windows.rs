//! Windows: tabs, an alert, a popover; the window keeps its state.

use appkit_ui::prelude::*;
use appkit_ui::{AlertSpec, tabs};

use super::section;

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let titles = cx.state(|| vec!["Welcome".to_string(), "Notes".to_string()]);
        let selected = cx.state(|| 0usize);
        let alert = cx.state(|| false);
        let popover = cx.state(|| false);
        let answer = cx.state(String::new);
        let t = titles.get();
        let pages: Vec<Element> = t
            .iter()
            .enumerate()
            .map(|(i, title)| {
                column((
                    label(title.clone()).text_style("Title 3"),
                    label(format!(
                        "Tab {} of {}. ⌃Tab and ⌃⇧Tab move between tabs, ⌘1…⌘9 pick one.",
                        i + 1,
                        t.len()
                    ))
                    .color("Text/Secondary"),
                ))
                .spacing(8.0)
                .alignment(0)
                .padding(16.0)
                .fill()
                .align(0)
                .key(title.clone())
            })
            .collect();
        let (t2, t3, s2, s3) = (
            titles.clone(),
            titles.clone(),
            selected.clone(),
            selected.clone(),
        );
        let a2 = alert.clone();
        let p2 = popover.clone();
        let ans = answer.clone();
        column((
            section(
                "Window tabs",
                tabs(
                    &t.iter().map(String::as_str).collect::<Vec<_>>(),
                    &selected.binding(),
                    pages,
                )
                .on_new_tab(move || {
                    let n = t2.get().len() + 1;
                    t2.update(|v| v.push(format!("Tab {n}")));
                    s2.set(n - 1);
                })
                .on_close_tab(move |i| {
                    t3.update(|v| {
                        if v.len() > 1 && i < v.len() {
                            v.remove(i);
                        }
                    });
                    let len = t3.get().len();
                    s3.update(|s| *s = (*s).min(len.saturating_sub(1)));
                })
                .fill_width()
                .height(160.0),
            ),
            section(
                "Alerts and popovers",
                row((
                    button("Show Alert…").on_activate(move || a2.set(true)),
                    button("Show Popover")
                        .on_activate(move || p2.set(true))
                        .popover(&popover.binding(), (220.0, 60.0), || {
                            label("A popover with the kit's arrow and material.").padding(8.0)
                        }),
                    label(answer.get()).color("Text/Secondary"),
                ))
                .spacing(12.0)
                .alert(
                    &alert.binding(),
                    AlertSpec::new(
                        "Move “Notes” to the Trash?",
                        &["Cancel", "Move to Trash"],
                        move |r| {
                            ans.set(match r {
                                1 => "Moved to the Trash.".to_string(),
                                0 => "Cancelled.".to_string(),
                                _ => "Dismissed.".to_string(),
                            })
                        },
                    )
                    .message("You can put it back from the Trash.")
                    .destructive()
                    .icon("trash"),
                ),
            ),
            section(
                "State restoration",
                label(
                    "The window keeps its size, the table's column widths and the outline's \
                     expanded folders between runs (autosave name “gallery”).",
                )
                .color("Text/Secondary"),
            ),
        ))
        .spacing(18.0)
        .alignment(0)
        .fill()
        .align(0)
    }
}
