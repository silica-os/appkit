//! ButtonDoc and the Icon button part of ButtonsMoreDoc.

use appkit_ui::prelude::*;

use super::{card, code, field, h2, lead, matrix, page, stage};

/// The page.
pub struct Page;

const VARIANTS: [&str; 5] = ["primary", "secondary", "destructive", "borderless", "glass"];
const SIZES: [&str; 4] = ["mini", "small", "regular", "large"];

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let variant = cx.state(|| 0usize);
        let size = cx.state(|| 2usize);
        let text = cx.state(|| "Button".to_string());
        let disabled = cx.state(|| false);
        let clicks = cx.state(|| 0u32);
        let table_size = cx.state(|| 2usize);
        let shape = cx.state(|| 0usize);
        let c2 = clicks.clone();
        let live = column((
            button(text.get())
                .style(variant.get() as i64)
                .control_size(size.get() as i64)
                .disabled(disabled.get())
                .on_activate(move || c2.update(|n| *n += 1)),
            label(format!("clicked {}×", clicks.get())).color("Text/Secondary"),
        ))
        .spacing(12.0)
        .alignment(1);
        let controls = column((
            field("Variant", segmented(&VARIANTS, &variant.binding())),
            field("Size", segmented(&SIZES, &size.binding())),
            field("Label", text_field("Label", &text.binding()).width(200.0)),
            checkbox("disabled", &disabled.binding()),
        ))
        .spacing(10.0)
        .alignment(0);
        let states = matrix(
            &["Rest", "Pressed", "Disabled"],
            VARIANTS
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    let s = table_size.get() as i64;
                    (
                        *v,
                        vec![
                            button("Button").style(i as i64).control_size(s),
                            button("Button")
                                .style(i as i64)
                                .control_size(s)
                                .pressed(true),
                            button("Button")
                                .style(i as i64)
                                .control_size(s)
                                .disabled(true),
                        ],
                    )
                })
                .collect(),
        )
        .spacing(12.0);
        let icons = matrix(
            &["Rest", "Pressed", "Disabled"],
            SIZES
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let st = shape.get() as i64;
                    (
                        *s,
                        vec![
                            icon_button("plus").style(st).control_size(i as i64),
                            icon_button("plus")
                                .style(st)
                                .control_size(i as i64)
                                .pressed(true),
                            icon_button("plus")
                                .style(st)
                                .control_size(i as i64)
                                .disabled(true),
                        ],
                    )
                })
                .collect(),
        )
        .spacing(12.0);
        page(vec![
            lead(
                "Push button in five variants and four sizes. A real <button>: it takes focus, fires click, reacts to hover and press.",
            ),
            h2("Playground"),
            row((stage(live), card(controls)))
                .spacing(16.0)
                .alignment(0),
            code("button(\"Button\").style(0).control_size(2).on_activate(|| …)"),
            h2("States"),
            segmented(&SIZES, &table_size.binding()),
            states,
            h2("Icon button"),
            segmented(&["round", "square"], &shape.binding()),
            icons,
        ])
    }
}
