//! ControlsDoc: Progress bar, Circular, Spinner (and the progress value of its playground).

use appkit_ui::PValue;
use appkit_ui::kit::{Kind, Prop};
use appkit_ui::prelude::*;

use super::{h2, h3, lead, matrix, page};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let v = cx.state(|| 0.4f64);
        let indeterminate = cx.state(|| false);
        let value = if indeterminate.get() {
            None
        } else {
            Some(v.get())
        };
        let bars = matrix(
            &["0 %", "40 %", "100 %"],
            vec![(
                "Bar",
                vec![
                    progress(Some(0.0)).width(160.0),
                    progress(Some(0.4)).width(160.0),
                    progress(Some(1.0)).width(160.0),
                ],
            )],
        )
        .spacing(12.0);
        let ring = |x: f64| view(Kind::CIRCULAR_PROGRESS).prop(Prop::VALUE, PValue::Float(x));
        page(vec![
            lead(
                "Value controls. The slider is a role=slider you can drag or drive with the keyboard, the stepper a spin button with hold-to-repeat, and progress a bar, ring or spinner.",
            ),
            h2("Playground"),
            column((
                progress(value).width(260.0),
                row((
                    view(Kind::CIRCULAR_PROGRESS).prop(Prop::VALUE, PValue::Float(v.get())),
                    spinner(),
                ))
                .spacing(16.0),
                row((
                    label("Progress value").color("Text/Secondary"),
                    slider(&v.binding(), 0.0, 1.0).width(200.0),
                    checkbox("indeterminate", &indeterminate.binding()),
                ))
                .spacing(12.0)
                .alignment(1),
            ))
            .spacing(14.0)
            .alignment(0),
            h2("Progress bar"),
            bars,
            h3("Indeterminate"),
            progress(None).width(260.0),
            h2("Circular"),
            row((ring(0.0), ring(0.25), ring(0.5), ring(0.75), ring(1.0))).spacing(16.0),
            h2("Spinner"),
            spinner(),
        ])
    }
}
