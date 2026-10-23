//! WellsDoc: the colour well, the colour panel, swatches and the image well.

use appkit_ui::kit::Kind;
use appkit_ui::prelude::*;

use super::{card, code, field, h2, h3, lead, matrix, page, pick, stage};

/// The page: `only` picks the sections (1: colour wells, 2: image wells).
pub struct Page(pub &'static [usize]);

/// The colours of the sample panel (0xRRGGBB).
const PANEL: [u32; 24] = [
    0x0E77E6, 0xFFCF3D, 0xD9694A, 0xC2457C, 0x6E8F5A, 0x2F8FA3, 0x8A6FD1, 0x1F5FB8, 0x1D1E21,
    0x5F636B, 0x9A9EA6, 0xE6E8EC, 0xF4F5F7, 0xFFFFFF, 0xF98A17, 0xA67600, 0x3D95FF, 0x2F7FE6,
    0xD6D9DE, 0xB4B8BF, 0x7D818A, 0x41444A, 0x2A2B30, 0x000000,
];

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let c = cx.state(|| rgba(0x0E77E6FF));
        let variant = cx.state(|| 1usize);
        let disabled = cx.state(|| false);
        let chip = cx.state(|| Some(0usize));
        let samples: Vec<Color> = [
            0x0E77E6FF_u32,
            0xFFCF3DFF,
            0xD9694AFF,
            0xC2457CFF,
            0x6E8F5AFF,
            0x2F8FA3FF,
        ]
        .iter()
        .map(|c| rgba(*c))
        .collect();
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
                    row((stage(live), card(controls))).spacing(16.0).alignment(0),
                    code("color_well(&color).style(1)"),
                    h2("Color well"),
                    wells,
                    h2("Color panel"),
                    h3("Full panel"),
                    color_panel(&PANEL, 6, false, &c.binding()),
                    h2("Swatch chip and row"),
                    swatch_row(&samples, &chip.binding()),
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
