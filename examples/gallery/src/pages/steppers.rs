//! ControlsDoc (Stepper) and FormsDoc (Number field).

use appkit_ui::prelude::*;

use super::{h2, lead, matrix, page};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let qty = cx.state(|| 3.0f64);
        let text = cx.state(|| "3".to_string());
        let b = qty.binding();
        page(vec![
            lead(
                "Value controls. The slider is a role=slider you can drag or drive with the keyboard, the stepper a spin button with hold-to-repeat, and progress a bar, ring or spinner.",
            ),
            h2("Stepper"),
            row((
                text_field("0", &text.binding()).width(56.0),
                stepper(&b, 0.0, 10.0, 1.0),
                label(format!("quantity {}", qty.get())).color("Text/Secondary"),
            ))
            .spacing(8.0)
            .alignment(1),
            label("Click or hold either half; focus the stepper and use ↑ ↓.")
                .color("Text/Tertiary"),
            matrix(
                &["Rest", "Disabled"],
                vec![(
                    "Stepper",
                    vec![
                        stepper(&b, 0.0, 10.0, 1.0),
                        stepper(&b, 0.0, 10.0, 1.0).disabled(true),
                    ],
                )],
            )
            .spacing(12.0),
            h2("Number field"),
            number_field(cx, &b, 0.0, 99.0, 1.0),
            label("Type a number, press ↑ ↓ or use the stepper.").color("Text/Tertiary"),
        ])
    }
}
