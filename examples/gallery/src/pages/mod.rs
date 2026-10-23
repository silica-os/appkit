//! The pages of the gallery, one module per family, in the order of the sidebar.

use appkit_ui::prelude::*;

mod alerts;
mod buttons;
mod dialogs;
mod disclosure;
mod fields;
mod forms;
mod group_boxes;
mod menus;
mod popovers;
mod popup;
mod progress;
mod scrollbars;
mod segments;
mod settings;
mod sidebars;
mod sliders;
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
    ($name:expr, $symbol:expr, $m:ident, $only:expr) => {
        Entry {
            name: $name,
            symbol: $symbol,
            build: || component($m::Page($only)),
        }
    };
}

/// Every page, in the order and under the names of the ui-kit Library (`pages.js`), without
/// the shell-only ones (Control Center, Menu Bar and Dock, Notifications). Collections, Panels
/// and the Status Bar have no Library page: they are in `examples/extras`.
pub const PAGES: &[Entry] = &[
    page!("Alerts", "warning", alerts),
    page!("Buttons", "square", buttons),
    page!("Color Wells", "palette", wells, &[1]),
    page!("Combo Boxes", "chevron-down", fields, &[5]),
    page!("Dialogs", "compose", dialogs),
    page!("Disclosure Controls", "chevron-right", disclosure),
    page!("Forms", "list-bullet", forms),
    page!("Group Boxes", "square", group_boxes),
    page!("Image Wells", "photo", wells, &[2]),
    page!("Menus", "list-bullet", menus),
    page!("Pop-up and Pull-down Buttons", "chevron-down", popup),
    page!("Popovers", "message", popovers),
    page!("Progress Indicators", "gauge", progress),
    page!("Scrollbars", "chevron-down", scrollbars),
    page!("Search Fields", "search", fields, &[3]),
    page!("Segmented Controls", "window-split", segments),
    page!("Sidebars", "sidebar", sidebars),
    page!("Sliders", "sliders", sliders),
    page!("Steppers", "plus", steppers),
    page!("Text Fields", "text-size", fields, &[1, 2]),
    page!("Titlebars and toolbars", "window", titlebars),
    page!("Toggles - Checkboxes", "square", toggles, &[1, 2]),
    page!("Toggles - Radio Buttons", "square", toggles, &[3]),
    page!("Toggles - Switches", "square", toggles, &[4]),
    page!("Tooltips", "info", tooltips),
    page!("Windows", "window", windows),
    page!("Example: Settings", "gear", settings),
];

/// The sections of a page that a sidebar entry shows: `only` lists them (1-based; empty: all).
pub fn pick(only: &[usize], sections: Vec<Element>) -> Vec<Element> {
    if only.is_empty() {
        return sections;
    }
    sections
        .into_iter()
        .enumerate()
        .filter(|(i, _)| only.contains(&(i + 1)))
        .map(|(_, e)| e)
        .collect()
}

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

/// A wrapping paragraph of `width` points.
pub fn wrapped(text: &str, style: &str, role: &str, width: f32) -> Element {
    label(text)
        .text_style(style)
        .color(role)
        .prop(appkit_ui::kit::Prop::WRAPS, appkit_ui::PValue::Bool(true))
        .width(width)
}

/// The lead of a Doc: one paragraph under the title.
pub fn lead(text: &str) -> Element {
    wrapped(text, "Body", "Text/Secondary", 680.0)
}

/// A paragraph of a Doc (`<p class="muted">`).
pub fn para(text: &str) -> Element {
    wrapped(text, "Callout", "Text/Secondary", 680.0)
}

/// A section heading of a Doc (`<h2>`).
pub fn h2(text: &str) -> Element {
    label(text).text_style("Title 3")
}

/// A sub heading of a Doc (`<h3>`).
pub fn h3(text: &str) -> Element {
    label(text).text_style("Headline")
}

/// A code block of a Doc.
pub fn code(text: &str) -> Element {
    label(text)
        .text_style("Mono")
        .color("Text/Secondary")
        .padding(12.0)
        .background(rgba(0x80808014))
        .corner_radius(10.0)
}

/// The Stage of a Doc: controls on a tinted panel.
pub fn stage(content: Element) -> Element {
    content
        .padding(28.0)
        .background(rgba(0x80808014))
        .corner_radius(14.0)
}

/// A card of controls beside a playground.
pub fn card(content: Element) -> Element {
    content
        .padding(16.0)
        .background(rgba(0x80808010))
        .corner_radius(14.0)
}

/// A labelled field of a controls card.
pub fn field(name: &str, control: Element) -> Element {
    column((
        label(name).text_style("Caption").color("Text/Tertiary"),
        control,
    ))
    .spacing(4.0)
    .alignment(0)
}

/// A matrix of live controls: a caption per column, a name per row.
pub fn matrix(cols: &[&str], rows: Vec<(&str, Vec<Element>)>) -> Element {
    let mut cells = vec![label("")];
    for c in cols {
        cells.push(label(*c).text_style("Caption").color("Text/Tertiary"));
    }
    for (name, items) in rows {
        cells.push(label(name).text_style("Caption").color("Text/Secondary"));
        cells.extend(items);
    }
    grid(cols.len() as i64 + 1, cells)
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
