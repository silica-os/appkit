//! The pages of the gallery, one module per family, in the order of the sidebar.

use appkit_ui::kit;
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
    /// Builds its content.
    pub build: fn() -> Element,
}

macro_rules! page {
    ($name:expr, $m:ident) => {
        Entry {
            name: $name,
            build: || component($m::Page),
        }
    };
    ($name:expr, $m:ident, $only:expr) => {
        Entry {
            name: $name,
            build: || component($m::Page($only)),
        }
    };
}

/// Every page, in the order and under the names of the ui-kit Library (`pages.js`), without
/// the shell-only ones (Control Center, Menu Bar and Dock, Notifications). Collections, Panels
/// and the Status Bar have no Library page: they are in `examples/extras`.
pub const PAGES: &[Entry] = &[
    page!("Alerts", alerts),
    page!("Buttons", buttons),
    page!("Color Wells", wells, &[1]),
    page!("Combo Boxes", fields, &[5]),
    page!("Dialogs", dialogs),
    page!("Disclosure Controls", disclosure),
    page!("Forms", forms),
    page!("Group Boxes", group_boxes),
    page!("Image Wells", wells, &[2]),
    page!("Menus", menus),
    page!("Pop-up and Pull-down Buttons", popup),
    page!("Popovers", popovers),
    page!("Progress Indicators", progress),
    page!("Scrollbars", scrollbars),
    page!("Search Fields", fields, &[3]),
    page!("Segmented Controls", segments),
    page!("Sidebars", sidebars),
    page!("Sliders", sliders),
    page!("Steppers", steppers),
    page!("Text Fields", fields, &[1, 2]),
    page!("Titlebars and toolbars", titlebars),
    page!("Toggles - Checkboxes", toggles, &[1, 2]),
    page!("Toggles - Radio Buttons", toggles, &[3]),
    page!("Toggles - Switches", toggles, &[4]),
    page!("Tooltips", tooltips),
    page!("Windows", windows),
    page!("Example: Settings", settings),
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
        .max_width(width)
}

/// The lead of a Doc: one paragraph under the title (`.lead`, 620 wide).
pub fn lead(text: &str) -> Element {
    wrapped(text, "Body", "Text/Secondary", 620.0)
}

/// A paragraph of a Doc (`<p class="muted">`).
pub fn para(text: &str) -> Element {
    wrapped(text, "Body", "Text/Secondary", 760.0)
}

/// A section heading of a Doc (`<h2>`: 32 above, 12 under; the page's 12 plus 20).
pub fn h2(text: &str) -> Element {
    label(text)
        .text_style("Title 3")
        .padding_edges(20.0, 0.0, 0.0, 0.0)
}

/// A sub heading of a Doc (`<h3>`).
pub fn h3(text: &str) -> Element {
    label(text)
        .text_style("Headline")
        .padding_edges(8.0, 0.0, 0.0, 0.0)
}

/// `content` on `fill` inside a 1 pt `Line/Separator` rim, corners of `radius`: the
/// storybook's bordered surfaces, in the tokens of the current appearance.
pub fn boxed(content: Element, fill: &str, radius: f32) -> Element {
    // The fill is its own view, so a content of fixed size still fills the box.
    column((column((content,))
        .alignment(0)
        .fill_width()
        .background_role(fill)
        .corner_radius(radius - 1.0),))
    .alignment(0)
    .padding(1.0)
    .background_role("Line/Separator")
    .corner_radius(radius)
    .fill_width()
}

/// A code block of a Doc (`Code.vue`: 12 by 14 in, corners of 10, on the panel).
pub fn code(text: &str) -> Element {
    boxed(
        label(text)
            .text_style("Mono")
            .color("Text/Primary")
            .padding_xy(14.0, 12.0),
        "Surface/Sidebar",
        10.0,
    )
}

/// The Stage of a Doc (`Stage.vue`): live controls on `Surface/Window`, 24 in, corners of 14.
pub fn stage(content: Element) -> Element {
    boxed(content.padding(24.0), "Surface/Window", 14.0)
}

/// A card of controls beside a playground (`.card.ctrl`: 16 in, items 14 apart).
pub fn card(content: Element) -> Element {
    boxed(content.padding(16.0), "Surface/Sidebar", 14.0)
}

/// A Doc's playground (`.play`): the stage takes the width (48 in, at least 200 tall, the
/// control centred), the card of controls 300 beside it.
pub fn playground(live: Element, controls: Element) -> Element {
    row((
        overlay((
            {
                // Read here: during paint the UI is borrowed and `is_dark` answers false.
                let dark = kit::is_dark();
                custom(move || GlassBackdrop { dark }).key(i64::from(dark))
            },
            column((spacer(), live, spacer()))
                .alignment(1)
                .padding(48.0)
                .min_height(200.0)
                .fill_width(),
        ))
        .corner_radius(14.0)
        .clipped()
        .fill_width(),
        column((card(controls.spacing(14.0)),))
            .alignment(0)
            .width(300.0),
    ))
    .spacing(16.0)
    .alignment(0)
    .fill_width()
}

/// A labelled field of a controls card (`.field > label`: 11, semibold, upper case).
pub fn field(name: &str, control: Element) -> Element {
    column((
        label(name.to_uppercase())
            .text_style("Label")
            .color("Text/Secondary"),
        control,
    ))
    .spacing(6.0)
    .alignment(0)
}

/// A matrix of live controls: a caption per column, a name per row.
pub fn matrix(cols: &[&str], rows: Vec<(&str, Vec<Element>)>) -> Element {
    let mut cells = vec![label("")];
    for c in cols {
        cells.push(label(*c).text_style("Caption").color("Text/Tertiary"));
    }
    for (name, items) in rows {
        // Centred on its row, which is as tall as its tallest control.
        cells.push(
            column((
                spacer(),
                label(name).text_style("Caption").color("Text/Secondary"),
                spacer(),
            ))
            .alignment(0),
        );
        cells.extend(items);
    }
    // Columns of at most 200 pt, as the storybook's tables size to their content.
    let n = cols.len() + 1;
    grid(n as i64, cells).max_width(200.0 * n as f32)
}

/// A page: its sections, top to bottom (the window adds the title and the 36 pt margins).
pub fn page(sections: Vec<Element>) -> Element {
    column(sections).spacing(12.0).alignment(0).fill_width()
}

/// Stage.vue's glass backdrop: a 135° wash of four colours under 45° stripes, so the glass of
/// the controls has something to refract and tint.
struct GlassBackdrop {
    dark: bool,
}

impl kit::Custom for GlassBackdrop {
    fn draw(&mut self, canvas: &kit::Canvas, width: f32, height: f32) {
        let (wash, stripes): (&[(f32, u32)], (u32, u32)) = if self.dark {
            (
                &[(0.0, 0x3A1D6EFF), (0.5, 0x0B2A55FF), (1.0, 0x064B4AFF)],
                (0x00000044, 0xFFFFFF11),
            )
        } else {
            (
                &[
                    (0.0, 0xFFD29CFF),
                    (0.4, 0xF7A6CBFF),
                    (0.7, 0x8EC2F5FF),
                    (1.0, 0x9FE0B5FF),
                ],
                (0x0000000D, 0xFFFFFF40),
            )
        };
        canvas.save();
        canvas.clip(kit::Rect {
            x: 0.0,
            y: 0.0,
            width,
            height,
        });
        // At 135° a point's place on the gradient is (x + y) / (w + h): bands along x + y = c.
        let span = (width + height).max(1.0);
        let mut c = 0.0;
        while c <= span {
            let color = sample(wash, c / span);
            canvas.line(c + 2.0, -2.0, -2.0, c + 2.0, 2.5, color);
            c += 1.5;
        }
        // 45° stripes 12 wide every 24, along x − y = c (a step of 24·√2 in c).
        let step = 24.0 * std::f32::consts::SQRT_2;
        let mut c = -height - step;
        let mut i = 0usize;
        while c <= width + step {
            let color = rgba(if i.is_multiple_of(2) {
                stripes.0
            } else {
                stripes.1
            });
            canvas.line(
                c - 24.0,
                -24.0,
                c + height + 24.0,
                height + 24.0,
                12.0,
                color,
            );
            c += step / 2.0;
            i += 1;
        }
        canvas.restore();
    }
}

/// The colour at `t` of a gradient with stops `(offset, 0xRRGGBBAA)`.
fn sample(stops: &[(f32, u32)], t: f32) -> Color {
    let mut prev = stops.first().copied().unwrap_or((0.0, 0));
    for &(o, c) in stops {
        if t <= o {
            let k = if o > prev.0 {
                (t - prev.0) / (o - prev.0)
            } else {
                1.0
            };
            return mix(rgba(prev.1), rgba(c), k.clamp(0.0, 1.0));
        }
        prev = (o, c);
    }
    rgba(prev.1)
}

fn mix(a: Color, b: Color, k: f32) -> Color {
    let l = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * k).round() as u8;
    Color {
        r: l(a.r, b.r),
        g: l(a.g, b.g),
        b: l(a.b, b.b),
        a: l(a.a, b.a),
    }
}
