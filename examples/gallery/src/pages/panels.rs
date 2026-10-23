//! Panels: the colour panel and the open and save panels.

use appkit_ui::prelude::*;

use super::{page, section};

/// The page.
pub struct Page;

const SWATCHES: [u32; 18] = [
    0xE5484D, 0xF76B15, 0xFFB224, 0x30A46C, 0x0E77E6, 0x8E4EC6, 0x9B1C1F, 0xA04A0B, 0xAB6400,
    0x1B6B49, 0x084D99, 0x5B2D8F, 0xFFFFFF, 0xD0D0D0, 0x909090, 0x606060, 0x303030, 0x000000,
];

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let colour = cx.state(|| rgba(0x0E77E6FF));
        let expanded = cx.state(|| true);
        let said = cx.state(String::new);
        let (s1, s2) = (said.clone(), said.clone());
        let picked = colour.get();
        page(vec![
            section(
                "Colour panel",
                color_panel(&SWATCHES, 6, false, &colour.binding()),
            ),
            label(format!("#{:02X}{:02X}{:02X}", picked.r, picked.g, picked.b))
                .color("Text/Secondary"),
            section(
                "Swatches only",
                color_panel(&SWATCHES, 6, true, &colour.binding()),
            ),
            section(
                "Save panel",
                file_panel(
                    false,
                    &expanded.binding(),
                    "Untitled",
                    &["Lemon cake.recipe", "Focaccia.recipe"],
                    move |a| s1.set(format!("Save panel: {a}")),
                ),
            ),
            section(
                "Open panel",
                file_panel(
                    true,
                    &expanded.binding(),
                    "",
                    &["Lemon cake.recipe", "Focaccia.recipe"],
                    move |a| s2.set(format!("Open panel: {a}")),
                ),
            ),
            label(said.get()).color("Text/Secondary"),
        ])
    }
}
