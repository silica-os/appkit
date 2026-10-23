//! WellsDoc: the colour well, the colour panel, swatches and the image well.

use appkit_ui::kit::Kind;
use appkit_ui::prelude::*;

use super::{code, field, h2, h3, lead, matrix, page, pick, playground};

/// The page: `only` picks the sections (1: colour wells, 2: image wells).
pub struct Page(pub &'static [usize]);

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let c = cx.state(|| rgba(0x0E77E6FF));
        let variant = cx.state(|| 1usize);
        let disabled = cx.state(|| false);
        let live = column((
            color_well(&c.binding())
                .style(variant.get() as i64)
                .disabled(disabled.get()),
            row((
                view(Kind::IMAGE_WELL)
                    .frame(64.0, 64.0)
                    .disabled(disabled.get()),
                view(Kind::IMAGE_WELL)
                    .frame(32.0, 32.0)
                    .disabled(disabled.get()),
            ))
            .spacing(12.0),
            label("no image: drop a file or click").color("Text/Secondary"),
        ))
        .spacing(12.0)
        .alignment(1);
        let controls = column((
            field(
                "Color well",
                segmented(&["Minimal", "Standard", "Expanded"], &variant.binding()),
            ),
            checkbox("disabled", &disabled.binding()),
        ))
        .spacing(10.0)
        .alignment(0);
        let wells = matrix(
            &["Rest", "Disabled"],
            ["Minimal", "Standard", "Expanded"]
                .iter()
                .enumerate()
                .map(|(i, n)| {
                    (
                        *n,
                        vec![
                            color_well(&c.binding()).style(i as i64),
                            color_well(&c.binding()).style(i as i64).disabled(true),
                        ],
                    )
                })
                .collect(),
        )
        .spacing(12.0);
        let image = matrix(
            &["Empty", "Disabled"],
            vec![
                (
                    "Regular",
                    vec![
                        view(Kind::IMAGE_WELL).frame(64.0, 64.0),
                        view(Kind::IMAGE_WELL).frame(64.0, 64.0).disabled(true),
                    ],
                ),
                (
                    "Small",
                    vec![
                        view(Kind::IMAGE_WELL).frame(32.0, 32.0),
                        view(Kind::IMAGE_WELL).frame(32.0, 32.0).disabled(true),
                    ],
                ),
            ],
        )
        .spacing(12.0);
        page(pick(
            self.0,
            vec![
                column((
                    lead("The colour well shows a colour and opens a colour panel (swatches, saturation/brightness pad, hue slider, hex field). The image well takes a dropped or chosen image and clears it. Swatch chips and rows are the pieces of the panel."),
                    h2("Playground"),
                    playground(live, controls),
                    code("color_well(&color).style(1)"),
                    h2("Color well"),
                    wells,
                    h2("Color panel"),
                    h3("Full panel"),
                    super::wrapped("Click a color well: its panel holds the swatches, the pad, the hue slider and the hex field.", "Callout", "Text/Secondary", 680.0),
                    h2("Swatch chip and row"),
                    super::wrapped("The swatch chips and rows are the panel's own pieces: open a color well to see them.", "Callout", "Text/Secondary", 680.0),
                ))
                .spacing(14.0)
                .alignment(0),
                column((h2("Image well"), h3("Empty"), image))
                    .spacing(14.0)
                    .alignment(0),
            ],
        ))
    }
}
