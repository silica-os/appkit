//! PopUpDoc: pop-up, pull-down, gradient and help buttons.

use appkit_ui::prelude::*;

use super::{card, code, field, h2, lead, matrix, page, stage};

/// The page.
pub struct Page;

const SIZES: [&str; 4] = ["mini", "small", "regular", "large"];

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let size = cx.state(|| 2usize);
        let disabled = cx.state(|| false);
        let fruit = cx.state(|| 1usize);
        let last = cx.state(String::new);
        let items = cx.state(|| 0u32);
        let fruits = ["Apple", "Pear", "Plum", "Fig"];
        let actions = ["Rename", "Duplicate", "Move to Trash"];
        let (s, d) = (size.get() as i64, disabled.get());
        let (l1, i1, i2) = (last.clone(), items.clone(), items.clone());
        let live = column((
            row((
                pop_up(&fruits, &fruit.binding())
                    .control_size(s)
                    .disabled(d),
                pull_down("Actions", &actions, move |i| {
                    l1.set(actions.get(i).copied().unwrap_or_default().to_string())
                })
                .control_size(s)
                .disabled(d),
                help_button().disabled(d),
            ))
            .spacing(12.0)
            .alignment(1),
            row((
                button("Add")
                    .style(1)
                    .control_size(1)
                    .on_activate(move || i1.update(|n| *n += 1)),
                button("Remove")
                    .style(1)
                    .control_size(1)
                    .disabled(items.get() == 0)
                    .on_activate(move || i2.update(|n| *n = n.saturating_sub(1))),
                label(format!("{} items", items.get())).color("Text/Secondary"),
            ))
            .spacing(8.0)
            .alignment(1),
            label(format!(
                "pop-up “{}” · action “{}”",
                fruits.get(fruit.get()).copied().unwrap_or_default(),
                last.get()
            ))
            .color("Text/Secondary"),
        ))
        .spacing(12.0)
        .alignment(0);
        let controls = column((
            field("Size", segmented(&SIZES, &size.binding())),
            checkbox("disabled", &disabled.binding()),
        ))
        .spacing(10.0)
        .alignment(0);
        let b_fruit = fruit.binding();
        let states = |name: &str, mk: &dyn Fn(i64, bool) -> Element| {
            column((
                h2(name),
                matrix(
                    &["Rest", "Disabled"],
                    SIZES
                        .iter()
                        .enumerate()
                        .map(|(i, n)| (*n, vec![mk(i as i64, false), mk(i as i64, true)]))
                        .collect(),
                )
                .spacing(10.0),
            ))
            .spacing(10.0)
            .alignment(0)
        };
        page(vec![
            lead(
                "A pop-up button shows the current choice and opens a menu to change it; a pull-down button has a fixed label and its menu holds actions. Both reuse the push button and the menu. The gradient button is the add/remove pair under a list, the help button the round question mark.",
            ),
            h2("Playground"),
            row((stage(live), card(controls)))
                .spacing(16.0)
                .alignment(0),
            code("pop_up(&items, &selected)  pull_down(\"Actions\", &items, |i| …)"),
            states("Pop-up button", &|i, d| {
                pop_up(&fruits, &b_fruit).control_size(i).disabled(d)
            }),
            states("Pull-down button", &|i, d| {
                pull_down("Actions", &actions, |_| {})
                    .control_size(i)
                    .disabled(d)
            }),
            h2("Open"),
            label("Click a pop-up or pull-down button to open its menu.").color("Text/Secondary"),
            h2("Gradient and help buttons"),
            row((help_button(), help_button().disabled(true))).spacing(16.0),
        ])
    }
}
