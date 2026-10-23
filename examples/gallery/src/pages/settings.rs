//! Example: Settings, built only from the kit's controls (a form with sections, switches,
//! pop-ups, a slider, a colour well, a stepper and a footer).

use appkit_ui::prelude::*;

use super::page;

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let name = cx.state(|| "Emanuele".to_string());
        let wifi = cx.state(|| true);
        let bt = cx.state(|| false);
        let sleep = cx.state(|| 1usize);
        let vol = cx.state(|| 0.5f64);
        let accent = cx.state(|| rgba(0x0E77E6FF));
        let size = cx.state(|| 14.0f64);
        let dark = cx.state(appkit_ui::kit::is_dark);
        let saved = cx.state(String::new);
        let (d2, s2) = (dark.clone(), saved.clone());
        let f = form(
            Some("Settings"),
            (
                form_section(
                    Some("General"),
                    (
                        form_row("Name:", text_field("Your name", &name.binding())),
                        form_row("Dark mode:", switch(&dark.binding())),
                        form_row("Accent:", color_well(&accent.binding())),
                        form_row(
                            "Text size:",
                            row((
                                stepper(&size.binding(), 9.0, 24.0, 1.0),
                                label(format!("{} pt", size.get())),
                            ))
                            .spacing(8.0),
                        ),
                    ),
                ),
                form_section(
                    Some("Connectivity"),
                    (
                        form_row("Wi-Fi:", switch(&wifi.binding())),
                        form_row("Bluetooth:", switch(&bt.binding())),
                    ),
                ),
                form_section(
                    Some("Sound and power"),
                    (
                        form_row("Volume:", slider(&vol.binding(), 0.0, 1.0).width(200.0)),
                        form_row(
                            "Sleep after:",
                            pop_up(
                                &["5 minutes", "15 minutes", "1 hour", "Never"],
                                &sleep.binding(),
                            ),
                        ),
                    ),
                ),
                form_footer((
                    label(saved.get()).color("Text/Secondary"),
                    button("Revert").style(1).on_activate(move || {
                        d2.set(appkit_ui::kit::is_dark());
                        s2.set("Reverted.".into());
                    }),
                    button("Apply").style(0).default_action().on_activate({
                        let (d, s) = (dark.clone(), saved.clone());
                        move || {
                            let _ = appkit_ui::kit::set_dark(d.get());
                            s.set("Applied.".into());
                        }
                    }),
                )),
            ),
        )
        .width(520.0);
        page(vec![f])
    }
}
