//! FormsDoc: the playground fields and the form layout.

use appkit_ui::prelude::*;

use super::{code, h2, h3, lead, page, stage};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let name = cx.state(String::new);
        let mail = cx.state(String::new);
        let region = cx.state(|| 0usize);
        let sign = cx.state(|| true);
        let remind = cx.state(|| false);
        let display = cx.state(String::new);
        let valid = mail.get().is_empty() || mail.get().contains('@');
        let f = form(
            Some("Account details"),
            (
                form_section(
                    Some("Profile"),
                    (
                        form_row("Name", text_field("Full name", &name.binding())),
                        form_row("Email", text_field("name@example.com", &mail.binding())),
                        form_row(
                            "Region",
                            pop_up(&["Campania", "Liguria", "Sicilia"], &region.binding()),
                        ),
                    ),
                ),
                form_section(
                    Some("Preferences"),
                    (
                        form_row("Sign in", checkbox("Keep me signed in", &sign.binding())),
                        form_row("Reminders", switch(&remind.binding())),
                        form_row(
                            "Display name",
                            text_field("Display name", &display.binding()),
                        ),
                    ),
                ),
                form_footer((button("Cancel").style(1), button("Save").style(0))),
            ),
        )
        .width(460.0);
        page(vec![
            lead(
                "The remaining fields (text area, secure field, token field, inline button, combo button), the number field (a text field with a stepper) and the form layout pieces: a form card, titled sections, rows with a label column, and a footer.",
            ),
            h2("Form"),
            stage(f),
            if valid {
                label("This name is shown to people you share with.").color("Text/Tertiary")
            } else {
                label("Enter a valid email address.").color("Text/Secondary")
            },
            code(
                "form(Some(\"Account details\"), (form_section(Some(\"Profile\"), rows), form_footer(buttons)))",
            ),
            h3("Sections and rows"),
            label("The fields of this Doc are on the Text Fields page; the form is above.")
                .color("Text/Secondary"),
        ])
    }
}
