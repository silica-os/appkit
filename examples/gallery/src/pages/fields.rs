//! FieldsDoc and the field parts of FormsDoc: text, secure, area, token, number, search and
//! combo.

use appkit_ui::kit::Kind;
use appkit_ui::prelude::*;

use super::{card, code, field, h2, h3, lead, matrix, page, pick, stage};

/// The page: `only` picks the sections (1 text field, 2 the others, 3 search, 5 combo).
pub struct Page(pub &'static [usize]);

const SIZES: [&str; 4] = ["mini", "small", "regular", "large"];

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let size = cx.state(|| 2usize);
        let name = cx.state(String::new);
        let query = cx.state(String::new);
        let fruit = cx.state(|| "Pear".to_string());
        let pw = cx.state(String::new);
        let note = cx.state(String::new);
        let qty = cx.state(|| 3.0f64);
        let disabled = cx.state(|| false);
        let s = size.get() as i64;
        let d = disabled.get();
        let fruits = ["Apple", "Pear", "Plum", "Fig"];
        let live = column((
            text_field("Placeholder", &name.binding())
                .width(220.0)
                .control_size(s)
                .disabled(d),
            search_field("Search", &query.binding())
                .width(220.0)
                .control_size(s)
                .disabled(d),
            combo_box("Fruit", &fruits, &fruit.binding())
                .width(220.0)
                .control_size(s)
                .disabled(d),
            label(format!(
                "text “{}” · search “{}” · combo “{}”",
                name.get(),
                query.get(),
                fruit.get()
            ))
            .color("Text/Secondary"),
        ))
        .spacing(10.0)
        .alignment(0);
        let controls = column((
            field("Size", segmented(&SIZES, &size.binding())),
            checkbox("disabled", &disabled.binding()),
            label("Clearable and error are not options of AppKit's fields.")
                .text_style("Caption")
                .color("Text/Tertiary"),
        ))
        .spacing(10.0)
        .alignment(0);
        let states = |mk: &dyn Fn(i64, bool) -> Element| {
            matrix(
                &["Rest", "Disabled"],
                SIZES
                    .iter()
                    .enumerate()
                    .map(|(i, n)| (*n, vec![mk(i as i64, false), mk(i as i64, true)]))
                    .collect(),
            )
            .spacing(10.0)
        };
        let b_name = name.binding();
        let text_matrix = states(&|i, d| {
            text_field("Placeholder", &b_name)
                .width(200.0)
                .control_size(i)
                .disabled(d)
        });
        let b_query = query.binding();
        let search_matrix = states(&|i, d| {
            search_field("Search", &b_query)
                .width(200.0)
                .control_size(i)
                .disabled(d)
        });
        let b_fruit = fruit.binding();
        let combo_matrix = states(&|i, d| {
            combo_box("Fruit", &fruits, &b_fruit)
                .width(200.0)
                .control_size(i)
                .disabled(d)
        });
        let sec =
            |title: &str, content: Element| column((h2(title), content)).spacing(10.0).alignment(0);
        let others = column((
            sec(
                "Text area",
                text_area("Write a note", &note.binding()).frame(320.0, 96.0),
            ),
            sec(
                "Secure field",
                secure_field("Password", &pw.binding()).width(240.0),
            ),
            column((
                h2("Token field and token"),
                view(Kind::TOKEN_FIELD).width(300.0),
                label("Enter or comma adds a token, Backspace twice removes the last.")
                    .color("Text/Secondary"),
            ))
            .spacing(10.0)
            .alignment(0),
            column((
                h2("Inline button"),
                super::wrapped(
                    "The text field with a Browse button inside is not in AppKit: not shown.",
                    "Callout",
                    "Text/Tertiary",
                    680.0,
                ),
            ))
            .spacing(10.0)
            .alignment(0),
            column((
                h2("Number field"),
                row((
                    text_field("0", &cx.state(|| qty.get().to_string()).binding()).width(56.0),
                    stepper(&qty.binding(), 0.0, 99.0, 1.0),
                    label(format!("number {}", qty.get())).color("Text/Secondary"),
                ))
                .spacing(8.0),
            ))
            .spacing(10.0)
            .alignment(0),
        ))
        .spacing(18.0)
        .alignment(0);
        page(pick(
            self.0,
            vec![
                column((
                    lead("Single-line fields built on a native input: text field, search field and combo box. They type, focus, clear and, for the combo box, open a list with the keyboard."),
                    h2("Playground"),
                    row((stage(live), card(controls))).spacing(16.0).alignment(0),
                    code("text_field(\"Placeholder\", &text).control_size(2)"),
                    h2("Text field"),
                    text_matrix,
                ))
                .spacing(14.0)
                .alignment(0),
                column((
                    lead("The remaining fields (text area, secure field, token field, inline button, combo button), the number field (a text field with a stepper) and the form layout pieces: a form card, titled sections, rows with a label column, and a footer."),
                    others,
                ))
                .spacing(14.0)
                .alignment(0),
                column((h2("Search field"), search_matrix)).spacing(14.0).alignment(0),
                spacer(),
                column((
                    h2("Combo box"),
                    combo_matrix,
                    h3("Open"),
                    label("Click the arrow of a combo box to open its list.").color("Text/Secondary"),
                    h2("Combo button"),
                    super::wrapped("The combo button (a push button joined to a menu) is not in AppKit: not shown.", "Callout", "Text/Tertiary", 680.0),
                ))
                .spacing(14.0)
                .alignment(0),
            ],
        ))
    }
}
