//! ButtonsMoreDoc (Disclosure button) and ExtraControlsDoc (Playground, Disclosure group,
//! Triangle).

use appkit_ui::kit::Kind;
use appkit_ui::prelude::*;

use super::{code, h2, lead, matrix, page, playground};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let (open, open2) = (cx.state(|| false), cx.state(|| true));
        let disabled = cx.state(|| false);
        let (e1, e2) = (cx.state(|| false), cx.state(|| true));
        let option = cx.state(|| true);
        let live = column((
            disclosure_group(
                "Advanced options",
                &open.binding(),
                label("Hidden until the group is opened."),
            )
            .disabled(disabled.get()),
            disclosure_group(
                "More",
                &open2.binding(),
                column((label("Another section."), label("With two paragraphs.")))
                    .spacing(4.0)
                    .alignment(0),
            ),
        ))
        .spacing(10.0)
        .alignment(0)
        .width(320.0);
        let controls = column((
            checkbox("disabled", &disabled.binding()),
            label("Focus a group and press Space to fold it.")
                .text_style("Caption")
                .color("Text/Tertiary"),
        ))
        .spacing(10.0)
        .alignment(0);
        let button_states = matrix(
            &["Rest", "Disabled"],
            vec![
                (
                    "Collapsed",
                    vec![
                        view(Kind::DISCLOSURE),
                        view(Kind::DISCLOSURE).disabled(true),
                    ],
                ),
                (
                    "Expanded",
                    vec![
                        view(Kind::DISCLOSURE).prop(
                            appkit_ui::kit::Prop::EXPANDED,
                            appkit_ui::PValue::Bool(true),
                        ),
                        view(Kind::DISCLOSURE)
                            .prop(
                                appkit_ui::kit::Prop::EXPANDED,
                                appkit_ui::PValue::Bool(true),
                            )
                            .disabled(true),
                    ],
                ),
            ],
        )
        .spacing(12.0);
        let groups = matrix(
            &["Rest", "Disabled"],
            vec![
                (
                    "Collapsed",
                    vec![
                        disclosure_group("Details", &e1.binding(), label("Content")).width(240.0),
                        disclosure_group("Details", &e1.binding(), label("Content"))
                            .width(240.0)
                            .disabled(true),
                    ],
                ),
                (
                    "Expanded",
                    vec![
                        disclosure_group(
                            "Details",
                            &e2.binding(),
                            checkbox("An option", &option.binding()),
                        )
                        .width(240.0),
                        disclosure_group("Details", &e2.binding(), label("Content"))
                            .width(240.0)
                            .disabled(true),
                    ],
                ),
            ],
        )
        .spacing(12.0);
        page(vec![
            lead(
                "A collapsible section with an animated height, and a circular dial you turn by dragging around the ring.",
            ),
            h2("Disclosure button"),
            button_states,
            h2("Playground"),
            playground(live, controls),
            code("disclosure_group(\"Advanced options\", &open, content)"),
            h2("Disclosure group"),
            groups,
            h2("Triangle"),
            super::wrapped(
                "The triangle is the chevron of the group headers above: it turns as the group opens.",
                "Callout",
                "Text/Secondary",
                680.0,
            ),
        ])
    }
}
