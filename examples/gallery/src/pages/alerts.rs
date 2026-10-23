//! AlertDoc: the playground and the variants (one, two and three buttons, critical).

use appkit_ui::AlertSpec;
use appkit_ui::prelude::*;

use super::{code, field, h2, lead, page, para, playground};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let variant = cx.state(|| 0usize);
        let count = cx.state(|| 2usize);
        let title = cx.state(|| "Save changes to “Lemon cake”?".to_string());
        let message =
            cx.state(|| "Your edits will be lost if you close without saving.".to_string());
        let open = cx.state(|| false);
        let glass = cx.state(|| false);
        let suppress = cx.state(|| false);
        let accessory = cx.state(|| false);
        let ticked = cx.state(|| false);
        let typed = cx.state(String::new);
        let chosen = cx.state(String::new);
        let shown: [(&str, &str); 3] = [
            ("Save", "Save"),
            ("Don’t Save", "Don’t Save"),
            ("Cancel", "Cancel"),
        ];
        let n = count.get() + 1;
        let labels: Vec<&str> = shown.iter().take(n).map(|b| b.0).collect();
        let critical = variant.get() == 1;
        let (o2, c2) = (open.clone(), chosen.clone());
        let (t2, y2) = (ticked.clone(), typed.clone());
        let live = column((
            button("Show alert…")
                .style(1)
                .on_activate(move || o2.set(true)),
            label(if chosen.get().is_empty() {
                "press a button".to_string()
            } else {
                format!("chose: {}", chosen.get())
            })
            .color("Text/Secondary"),
        ))
        .spacing(10.0)
        .alignment(1)
        .alert(&open.binding(), {
            let spec = AlertSpec::new(&title.get(), &labels, move |r| {
                c2.set(format!("button {r}"));
            })
            .message(&message.get())
            .icon("note");
            let spec = if critical {
                spec.destructive().critical()
            } else {
                spec
            };
            let spec = if glass.get() { spec.glass() } else { spec };
            let spec = if suppress.get() {
                spec.suppression("Do not show this again", move |on| t2.set(on))
            } else {
                spec
            };
            if accessory.get() {
                spec.field("Reason", move |t| y2.set(t))
            } else {
                spec
            }
        });
        let controls = column((
            field(
                "Variant",
                segmented(&["default", "critical"], &variant.binding()),
            ),
            field("Buttons", segmented(&["1", "2", "3"], &count.binding())),
            field("Title", text_field("Title", &title.binding()).width(220.0)),
            field(
                "Message",
                text_field("Message", &message.binding()).width(220.0),
            ),
            field("Material", checkbox("Glass", &glass.binding())),
            field("Checkbox", checkbox("Suppression", &suppress.binding())),
            field("Accessory", checkbox("Text field", &accessory.binding())),
            label(format!(
                "suppressed: {} · field: “{}”",
                ticked.get(),
                typed.get()
            ))
            .text_style("Caption")
            .color("Text/Tertiary"),
        ))
        .spacing(10.0)
        .alignment(0);
        let rows = [
            (
                "One button",
                "Import complete",
                "Twelve pictures were added to your library.",
                vec!["OK"],
                false,
            ),
            (
                "Two buttons",
                "Empty the outbox?",
                "Messages waiting to be sent will be sent now.",
                vec!["Cancel", "Send"],
                false,
            ),
            (
                "Three buttons",
                "Save changes to “Lemon cake”?",
                "Your edits will be lost if you close without saving.",
                vec!["Cancel", "Don’t Save", "Save"],
                false,
            ),
            (
                "Critical",
                "Delete this list?",
                "All reminders in it will be removed for good.",
                vec!["Cancel", "Delete"],
                true,
            ),
        ];
        let mut kids = Vec::new();
        for (i, (name, t, m, b, crit)) in rows.into_iter().enumerate() {
            let st = cx.state(|| false);
            let s2 = st.clone();
            let spec = AlertSpec::new(t, &b, |_| {}).message(m).icon("note");
            let spec = if crit { spec.destructive() } else { spec };
            kids.push(
                button(name)
                    .style(1)
                    .key(i as i64)
                    .on_activate(move || s2.set(true))
                    .alert(&st.binding(), spec),
            );
        }
        let variants = row(kids).spacing(12.0).alignment(4);
        page(vec![
            lead(
                "The 280 pt alert: icon, title, message, up to three stacked buttons, an optional suppression checkbox or any accessory (a text field). Buttons and checkbox are the real SButton and SCheckbox.",
            ),
            h2("Playground"),
            playground(live, controls),
            code(
                "alert(&open, AlertSpec::new(title, &[\"Save\", \"Don’t Save\", \"Cancel\"], |r| …).message(text))",
            ),
            h2("Variants"),
            para(
                "The kit's alerts carry an app icon; each button here opens the alert of that variant.",
            ),
            variants,
        ])
    }
}
