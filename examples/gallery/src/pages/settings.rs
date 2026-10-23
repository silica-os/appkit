//! ExampleDoc: a Settings screen built only from library components.

use appkit_ui::AlertSpec;
use appkit_ui::prelude::*;

use super::{lead, page, stage};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let sel = cx.state(|| 0usize);
        let name = cx.state(|| "Recipes".to_string());
        let view = cx.state(|| 0usize);
        let (banners, sound) = (cx.state(|| true), cx.state(|| true));
        let (wifi, bt) = (cx.state(|| true), cx.state(|| false));
        let volume = cx.state(|| 0.6f64);
        let reset = cx.state(|| false);
        let done = cx.state(|| false);
        let items = [
            ("gear", "General"),
            ("wifi", "Network"),
            ("speaker", "Sound"),
        ];
        let rows: Vec<Element> = items
            .iter()
            .enumerate()
            .map(|(i, (s, t))| {
                let sel = sel.clone();
                sidebar_row(s, t, sel.get() == i, move || sel.set(i)).key(i as i64)
            })
            .collect();
        let pad = |e: Element| e.spacing(8.0).alignment(0).padding(12.0);
        let content = match sel.get() {
            0 => {
                let (r2, d2) = (reset.clone(), done.clone());
                column((
                    label("General").text_style("Title 3"),
                    group_box(
                        Some("Library"),
                        pad(column((
                            text_field("Name", &name.binding()).width(260.0),
                            segmented(&["List", "Grid", "Columns"], &view.binding()),
                        ))),
                    )
                    .width(300.0),
                    group_box(
                        Some("Notifications"),
                        pad(column((
                            checkbox("Show banners", &banners.binding()),
                            checkbox("Play sound", &sound.binding()),
                        ))),
                    )
                    .width(300.0),
                    row((
                        button("Reset…").style(2).on_activate(move || r2.set(true)),
                        label(if done.get() { "Settings reset" } else { "" })
                            .color("Text/Secondary"),
                    ))
                    .spacing(12.0)
                    .alignment(4),
                ))
                .spacing(12.0)
                .alignment(0)
                .alert(
                    &reset.binding(),
                    AlertSpec::new("Reset all settings?", &["Cancel", "Reset"], move |r| {
                        if r == 1 {
                            d2.set(true);
                        }
                    })
                    .message("This cannot be undone.")
                    .destructive()
                    .icon("warning"),
                )
            }
            1 => column((
                label("Network").text_style("Title 3"),
                group_box(
                    None,
                    pad(column((
                        row((label("Wi-Fi"), spacer(), switch(&wifi.binding()))).width(260.0),
                        row((label("Bluetooth"), spacer(), switch(&bt.binding()))).width(260.0),
                    ))),
                )
                .width(300.0),
                if wifi.get() {
                    progress(None).width(260.0)
                } else {
                    spacer().height(1.0)
                },
            ))
            .spacing(12.0)
            .alignment(0),
            _ => column((
                label("Sound").text_style("Title 3"),
                group_box(
                    Some("Output volume"),
                    pad(column((
                        slider(&volume.binding(), 0.0, 1.0).width(260.0),
                        label(format!("{:.0} %", volume.get() * 100.0)).color("Text/Secondary"),
                    ))),
                )
                .width(300.0),
            ))
            .spacing(12.0)
            .alignment(0),
        };
        page(vec![
            lead(
                "A small screen built only from library components: sidebar, group boxes, switches, a slider, a segmented control, a text field, buttons and an alert. Everything works.",
            ),
            stage(
                row((sidebar(rows).frame(200.0, 300.0), content.padding(16.0)))
                    .spacing(8.0)
                    .alignment(0),
            ),
        ])
    }
}
