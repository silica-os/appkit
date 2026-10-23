//! The pages of the gallery, one module per family, in the order of the sidebar.

use appkit_ui::prelude::*;

mod alerts;
mod buttons;
mod callouts;
mod collections;
mod dialogs;
mod disclosure;
mod fields;
mod forms;
mod group_boxes;
mod materials;
mod menus;
mod motion;
mod panels;
mod popovers;
mod popup;
mod progress;
mod scrollbars;
mod segments;
mod settings;
mod sidebars;
mod sliders;
mod statusbar;
mod steppers;
mod titlebars;
mod toggles;
mod tooltips;
mod wells;
mod windows;

/// A page of the sidebar.
pub struct Entry {
    /// Its name.
    pub name: &'static str,
    /// Its symbol.
    pub symbol: &'static str,
    /// Builds its content.
    pub build: fn() -> Element,
}

macro_rules! page {
    ($name:expr, $symbol:expr, $m:ident) => {
        Entry {
            name: $name,
            symbol: $symbol,
            build: || component($m::Page),
        }
    };
}

/// Every page: the families of the kit, the motion page and the Settings example.
pub const PAGES: &[Entry] = &[
    page!("Alerts", "warning", alerts),
    page!("Buttons", "square", buttons),
    page!("Callouts", "message", callouts),
    page!("Collections", "list-bullet", collections),
    page!("Dialogs", "compose", dialogs),
    page!("Disclosure", "chevron-right", disclosure),
    page!("Fields", "text-size", fields),
    page!("Forms", "list-bullet", forms),
    page!("Group Boxes", "square", group_boxes),
    page!("Materials", "drop", materials),
    page!("Menus", "list-bullet", menus),
    page!("Pop-up and Pull-down", "chevron-down", popup),
    page!("Popovers", "message", popovers),
    page!("Panels", "palette", panels),
    page!("Progress", "gauge", progress),
    page!("Scrollbars", "chevron-down", scrollbars),
    page!("Segments", "window-split", segments),
    page!("Sidebars", "sidebar", sidebars),
    page!("Sliders", "sliders", sliders),
    page!("Status Bar", "minus", statusbar),
    page!("Steppers", "plus", steppers),
    page!("Titlebars", "window", titlebars),
    page!("Toggles", "square", toggles),
    page!("Tooltips", "info", tooltips),
    page!("Wells", "palette", wells),
    page!("Windows", "window", windows),
    page!("Motion", "waveform", motion),
    page!("Example: Settings", "gear", settings),
];

/// A titled block of a page.
pub fn section(title: &str, content: Element) -> Element {
    column((
        label(title).text_style("Headline").color("Text/Secondary"),
        content,
    ))
    .spacing(8.0)
    .alignment(0)
}

/// `content` with a caption under it (a variant, a size, a state).
pub fn captioned(caption: &str, content: Element) -> Element {
    column((
        content,
        label(caption).text_style("Caption").color("Text/Tertiary"),
    ))
    .spacing(4.0)
    .alignment(1)
}

/// A page: its sections, top to bottom.
pub fn page(sections: Vec<Element>) -> Element {
    column(sections)
        .spacing(20.0)
        .alignment(0)
        .padding(24.0)
        .fill_width()
        .align(0)
}
