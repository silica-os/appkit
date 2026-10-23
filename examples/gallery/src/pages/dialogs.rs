//! DialogsDoc: the dialog playground and the generic dialog. The file panels are in the
//! extras app, the notifications are shell-only.

use appkit_ui::prelude::*;

use super::{code, h2, lead, page, stage};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let open = cx.state(|| false);
        let name = cx.state(|| "Marta Esposito".to_string());
        let (edit, notify) = (cx.state(|| true), cx.state(|| false));
        let chosen = cx.state(String::new);
        let (o1, c1) = (open.clone(), chosen.clone());
        let n1 = name.clone();
        let live = column((
            button("Share recipe…")
                .style(1)
                .on_activate(move || o1.set(true))
                .sheet(&open.binding(), (420.0, 230.0), {
                    let (o, c, n, e, no) = (
                        open.clone(),
                        c1.clone(),
                        n1.clone(),
                        edit.clone(),
                        notify.clone(),
                    );
                    move || {
                        let (o2, o3, c2, c3, n2) =
                            (o.clone(), o.clone(), c.clone(), c.clone(), n.clone());
                        form(
                            Some("Share recipe"),
                            (
                                form_section(
                                    None,
                                    (
                                        form_row("Name:", text_field("Recipe name", &n.binding())),
                                        form_row(
                                            "Options:",
                                            checkbox("Allow editing", &e.binding()),
                                        ),
                                        form_row("", checkbox("Notify when opened", &no.binding())),
                                    ),
                                ),
                                form_footer((
                                    button("Cancel").style(1).cancel_action().on_activate(
                                        move || {
                                            c2.set("cancelled".into());
                                            o2.set(false)
                                        },
                                    ),
                                    button("Share").style(0).default_action().on_activate(
                                        move || {
                                            c3.set(format!("share {}", n2.get()));
                                            o3.set(false)
                                        },
                                    ),
                                )),
                            ),
                        )
                        .padding(8.0)
                    }
                }),
            label(chosen.get()).color("Text/Secondary"),
        ))
        .spacing(10.0)
        .alignment(1);
        let (gn, gm, gr) = (
            cx.state(|| "Marta Esposito".to_string()),
            cx.state(|| "marta@example.it".to_string()),
            cx.state(|| 0usize),
        );
        let (up, keep) = (cx.state(|| true), cx.state(|| true));
        let generic = form(
            Some("Share recipe"),
            (
                form_section(
                    None,
                    (
                        form_row("Name:", text_field("Name", &gn.binding())),
                        form_row("Email:", text_field("Email", &gm.binding())),
                        form_row(
                            "Region:",
                            pop_up(&["Lemon", "Orange", "Olive"], &gr.binding()),
                        ),
                        form_row("Options:", checkbox("Send me updates", &up.binding())),
                        form_row("", checkbox("Keep me signed in", &keep.binding())),
                    ),
                ),
                form_footer((button("Cancel").style(1), button("Save").style(0))),
            ),
        )
        .width(460.0);
        page(vec![
            lead(
                "SDialog is the raised 20 pt dialog (title, form rows, SDialogFooter with the primary button last) that also serves as a sheet in an SWindow; SFilePanel is the Finder-style open and save panel (full-height sidebar, toolbar, striped SFileBrowser, buttons) and collapses to the compact save panel. SNotification is the glass banner (or the card inside SNotificationCentre), SNotificationStack groups them.",
            ),
            h2("Playground: dialog"),
            stage(live),
            code("button(\"Share recipe…\").sheet(&open, size, || form(…))"),
            h2("Dialogs"),
            label("Generic")
                .text_style("Caption")
                .color("Text/Tertiary"),
            stage(generic),
            super::wrapped(
                "The open and save panels are in the extras app (appkit-extras); the notifications belong to the shell.",
                "Callout",
                "Text/Tertiary",
                680.0,
            ),
        ])
    }
}
