//! ControlsDoc (Playground, Slider) and ExtraControlsDoc (Dial).

use appkit_ui::kit::Kind;
use appkit_ui::prelude::*;

use super::{h2, h3, lead, matrix, page};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let volume = cx.state(|| 0.5f64);
        let level = cx.state(|| 0.3f64);
        let b = volume.binding();
        let vertical = view(Kind::SLIDER).prop(
            appkit_ui::kit::Prop::VERTICAL,
            appkit_ui::PValue::Bool(true),
        );
        let states = matrix(
            &["Rest", "Lens (held)", "Disabled"],
            vec![(
                "Horizontal",
                vec![
                    slider(&b, 0.0, 1.0).width(180.0),
                    slider(&b, 0.0, 1.0).width(180.0).dragging(true),
                    slider(&b, 0.0, 1.0).width(180.0).disabled(true),
                ],
            )],
        )
        .spacing(14.0);
        page(vec![
            lead(
                "Value controls. The slider is a role=slider you can drag or drive with the keyboard, the stepper a spin button with hold-to-repeat, and progress a bar, ring or spinner.",
            ),
            h2("Playground"),
            row((
                column((
                    slider(&volume.binding(), 0.0, 1.0).width(260.0),
                    row((
                        slider(&volume.binding(), 0.0, 1.0).width(200.0),
                        label(format!("{:.0}", volume.get() * 100.0)).color("Text/Secondary"),
                    ))
                    .spacing(8.0)
                    .alignment(1),
                    label("Drag the slider, or focus it and use the arrow keys, Home and End.")
                        .color("Text/Tertiary"),
                ))
                .spacing(12.0)
                .alignment(0),
                vertical.frame(24.0, 140.0),
            ))
            .spacing(32.0)
            .alignment(0),
            h2("Slider"),
            states,
            h3("Vertical"),
            label(format!("level {:.0}", level.get() * 100.0)).color("Text/Secondary"),
            h2("Dial"),
            view(Kind::DIAL).frame(64.0, 64.0),
            super::wrapped(
                "Drag around the ring, or focus it and use the arrows, PageUp/PageDown, Home and End.",
                "Callout",
                "Text/Tertiary",
                680.0,
            ),
        ])
    }
}
