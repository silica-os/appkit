//! Progress indicators: bar, indeterminate bar, circular, spinner.

use appkit_ui::PValue;
use appkit_ui::kit::{Kind, Prop};
use appkit_ui::prelude::*;

use super::{captioned, page, section};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let v = cx.state(|| 0.4f64);
        let bars = column((
            captioned("0 %", progress(Some(0.0)).width(240.0)),
            captioned("40 %", progress(Some(0.4)).width(240.0)),
            captioned("100 %", progress(Some(1.0)).width(240.0)),
            captioned("Indeterminate", progress(None).width(240.0)),
            captioned("Live", progress(Some(v.get())).width(240.0)),
            slider(&v.binding(), 0.0, 1.0).width(240.0),
        ))
        .spacing(12.0)
        .alignment(0);
        let circ = row((
            captioned(
                "Circular",
                view(Kind::CIRCULAR_PROGRESS).prop(Prop::VALUE, PValue::Float(v.get())),
            ),
            captioned("Spinner", spinner()),
        ))
        .spacing(24.0)
        .alignment(1);
        page(vec![
            section("Bars", bars),
            section("Circular and spinner", circ),
        ])
    }
}
