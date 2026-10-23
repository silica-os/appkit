//! The pages of the extras app: families the ui-kit has no storybook page for.

use appkit_ui::prelude::*;

mod collections;
mod panels;
mod statusbar;

/// A page of the sidebar.
pub struct Entry {
    /// Its name.
    pub name: &'static str,
    /// Its symbol.
    pub symbol: &'static str,
    /// Builds its content.
    pub build: fn() -> Element,
}

/// Every page.
pub const PAGES: &[Entry] = &[
    Entry {
        name: "Collections",
        symbol: "list-bullet",
        build: || component(collections::Page),
    },
    Entry {
        name: "Panels",
        symbol: "palette",
        build: || component(panels::Page),
    },
    Entry {
        name: "Status Bar",
        symbol: "minus",
        build: || component(statusbar::Page),
    },
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

/// A page: its sections, top to bottom.
pub fn page(sections: Vec<Element>) -> Element {
    column(sections)
        .spacing(20.0)
        .alignment(0)
        .padding(24.0)
        .fill_width()
        .align(0)
}
