//! WindowsDoc: the playground (toolbar styles, sidebar, inspector, sheet) and the window
//! kinds.

use appkit_ui::AlertSpec;
use appkit_ui::prelude::*;

use super::{card, code, field, h2, h3, lead, page};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let mode = cx.state(|| 0usize);
        let (side, insp) = (cx.state(|| true), cx.state(|| false));
        let sheet = cx.state(|| false);
        let nav = cx.state(|| 0usize);
        let q = cx.state(String::new);
        let view = cx.state(|| 0usize);
        let msg = cx.state(String::new);
        let m2 = msg.clone();
        let bar = match mode.get() {
            0 => Some(toolbar((
                toolbar_button("sidebar").accessibility_label("Sidebar"),
                toolbar_group(
                    &[("chevron-left", "Back"), ("chevron-right", "Forward")],
                    |_| {},
                ),
                toolbar_title("Lemon cake"),
                spacer(),
                segmented(&["List", "Grid"], &view.binding()),
                toolbar_search("Search", &q.binding()),
            ))),
            1 => Some(toolbar((
                toolbar_button("chevron-left").accessibility_label("Back"),
                toolbar_title("Lemon cake"),
                spacer(),
            ))),
            _ => None,
        };
        let items = [
            ("clock", "Recents"),
            ("folder", "Documents"),
            ("photo", "Photos"),
        ];
        let rows: Vec<Element> = items
            .iter()
            .enumerate()
            .map(|(i, (s, t))| {
                let nav = nav.clone();
                sidebar_row(s, t, nav.get() == i, move || nav.set(i)).key(i as i64)
            })
            .collect();
        let s2 = sheet.clone();
        let body = column((
            label("Lemon cake").text_style("Title 1"),
            super::wrapped(
                "Whisk the eggs and sugar until pale, fold in the flour, then add the lemon zest.",
                "Callout",
                "Text/Secondary",
                680.0,
            ),
            button("Done…").style(0).on_activate(move || s2.set(true)),
        ))
        .spacing(10.0)
        .alignment(0)
        .padding(24.0)
        .fill()
        .align(0);
        let mut parts = Vec::new();
        if side.get() {
            parts.push(sidebar(rows).frame(180.0, 340.0).key(1i64));
        }
        parts.push(body.key(2i64));
        if insp.get() {
            parts.push(
                column((
                    label("Inspector").text_style("Headline"),
                    checkbox("Show ruler", &Binding::new(|| true, |_| {})),
                ))
                .spacing(8.0)
                .alignment(0)
                .padding(14.0)
                .width(160.0)
                .key(3i64),
            );
        }
        let content = row(parts).spacing(0.0).alignment(0).height(340.0);
        let window = match bar {
            Some(b) => column((b.fill_width().height(52.0), content)),
            None => column((content,)),
        }
        .spacing(0.0)
        .alignment(0)
        .width(560.0)
        .background(rgba(0x80808018))
        .corner_radius(12.0)
        .alert(
            &sheet.binding(),
            AlertSpec::new(
                "Save changes to “Lemon cake”?",
                &["Cancel", "Don’t Save", "Save"],
                move |r| m2.set(format!("sheet: button {r}")),
            )
            .message("Your edits will be lost if you close without saving.")
            .icon("note"),
        );
        let controls = column((
            field("Toolbar", segmented(&["unified", "compact", "none"], &mode.binding())),
            checkbox("sidebar", &side.binding()),
            checkbox("inspector", &insp.binding()),
            label("Inactive window and the traffic lights belong to the real window: see the window chrome around this gallery.")
                .text_style("Caption").color("Text/Tertiary").width(200.0),
        ))
        .spacing(10.0)
        .alignment(0);
        page(vec![
            lead(
                "SWindow assembles a title bar or toolbar (STitlebar with STitlebarPiece, STitlebarTitle, STrafficLights), a sidebar, content and an inspector. Regions are split by 1 pt lines; only the window has a shadow.",
            ),
            h2("Playground"),
            column((window, card(controls))).spacing(16.0).alignment(0),
            label(if msg.get().is_empty() {
                "click the toolbar buttons".to_string()
            } else {
                format!("event: {}", msg.get())
            })
            .color("Text/Secondary"),
            code("column((toolbar(…), row((sidebar(rows), content, inspector))))"),
            h2("Windows"),
            h3("Document, panel, utility, sheet"),
            super::wrapped(
                "Document, panel and utility windows are real windows of the system: the gallery window is a document window, its sheets and popovers are the attached ones.",
                "Callout",
                "Text/Secondary",
                680.0,
            ),
        ])
    }
}
