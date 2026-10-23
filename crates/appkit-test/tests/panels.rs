//! Parity of the panels with ui-kit's `SColorPanel`, `SFileBrowser` and `SFilePanel`.
//!
//! The kit export in silica-design has no masters for these: its `Wells/Color Panel` swatch grid
//! is covered by `wells.rs` (the picker reuses the same rows), and its `Dialogs` masters draw the
//! older card layout, not the Finder layout of ui-kit (22 Oct). The geometry is therefore
//! checked against the measures in the .vue files.

#![allow(clippy::panic, clippy::indexing_slicing)]

use appkit::amalfi::panels::{
    FileEntry, FilePanelSpec, Mode, color_picker, file_browser, file_panel,
};
use appkit::color::Color;
use appkit::core::{Appearance, Proposal, Ui, ViewId};
use appkit::geom::{Rect, Size};

fn mount(ui: &mut Ui, v: ViewId, w: f32, h: f32) {
    let win = ui.create_window(appkit::core::window::WindowKind::Normal, Size::new(w, h));
    ui.set_appearance(Appearance::light());
    ui.set_content(win, v);
    ui.show_window(win);
    let _ = ui.prepare_frame(win);
}

fn layer(ui: &mut Ui, root: ViewId, name: &str) -> Rect {
    let o = ui.tree.window_rect(root).origin();
    let ls = ui.inspect_layers(root);
    let l = ls
        .iter()
        .find(|l| l.path == name || l.path.ends_with(&format!("/{name}")))
        .unwrap_or_else(|| {
            panic!(
                "no layer {name} in {:?}",
                ls.iter().map(|l| &l.path).collect::<Vec<_>>()
            )
        });
    let _ = o;
    l.frame
}

fn rows() -> Vec<Vec<(String, Color)>> {
    (0..4)
        .map(|r| {
            (0..6)
                .map(|c| {
                    (
                        format!("{r}{c}"),
                        Color::rgb((r * 40) as u8, (c * 40) as u8, 128),
                    )
                })
                .collect()
        })
        .collect()
}

#[test]
fn the_colour_panel_content_follows_the_vue_measures() {
    let mut ui = Ui::new_headless();
    let p = color_picker(&mut ui, &rows(), Color::rgb(14, 119, 230), false);
    let size = ui.measure(p, Proposal::UNSPECIFIED);
    // Title 2 + 18, four rows of 32, the rule (4 + 1 + 8), the pad 96, the hue slider (8 above)
    // and the hex row (8 above, 28), 4 below.
    assert_eq!(size.width, 192.0);
    mount(&mut ui, p, 300.0, 500.0);
    assert_eq!(layer(&mut ui, p, "Pad"), Rect::new(4.0, 161.0, 184.0, 96.0));
    assert_eq!(layer(&mut ui, p, "Chip"), Rect::new(4.0, 293.0, 28.0, 28.0));
    assert_eq!(layer(&mut ui, p, "Separator").y, 152.0);
    assert_eq!(size.height, 325.0);
    let only = color_picker(&mut ui, &rows(), Color::rgb(14, 119, 230), true);
    assert_eq!(
        ui.measure(only, Proposal::UNSPECIFIED).height,
        20.0 + 128.0 + 4.0
    );
}

fn files() -> Vec<FileEntry> {
    vec![
        FileEntry::new("a", "Lemon cake.recipe", "doc", "Recipe", "Today"),
        FileEntry::new("b", "Focaccia.recipe", "doc", "Recipe", "Today"),
    ]
}

#[test]
fn the_file_list_follows_the_vue_measures() {
    let mut ui = Ui::new_headless();
    let b = file_browser(&mut ui, files(), Some("b"));
    assert_eq!(
        ui.measure(b, Proposal::new(Some(560.0), None)),
        Size::new(560.0, 220.0)
    );
    mount(&mut ui, b, 560.0, 220.0);
    // The header: 28 high, its hairline at 27; the rows of 32 from 28; the selection 8 in.
    assert_eq!(
        layer(&mut ui, b, "Header rule"),
        Rect::new(0.0, 27.0, 560.0, 1.0)
    );
    assert_eq!(
        layer(&mut ui, b, "Stripe 2"),
        Rect::new(0.0, 60.0, 560.0, 32.0)
    );
    assert_eq!(
        layer(&mut ui, b, "Focaccia.recipe/Selection"),
        Rect::new(8.0, 60.0, 544.0, 32.0)
    );
    assert_eq!(layer(&mut ui, b, "Column Name").x, 44.0);
    assert_eq!(layer(&mut ui, b, "Column Kind").x, 300.0);
    assert_eq!(layer(&mut ui, b, "Column Date").x, 396.0);
    // The stripes carry on through the empty space.
    let n = ui
        .inspect_layers(b)
        .iter()
        .filter(|l| l.name.starts_with("Stripe"))
        .count();
    assert_eq!(n, 3);
}

#[test]
fn the_file_panel_follows_the_vue_measures() {
    let mut ui = Ui::new_headless();
    let mut spec = FilePanelSpec::new(Mode::Save);
    spec.name = "Untitled".into();
    spec.locations = vec!["Recipes".into()];
    spec.files = files();
    spec.places = vec![(
        "Favourites".into(),
        vec![("docs".into(), "Documents".into(), "folder".into())],
    )];
    let parts = file_panel(&mut ui, &spec);
    assert_eq!(ui.measure(parts.root, Proposal::UNSPECIFIED).width, 720.0);
    mount(&mut ui, parts.root, 720.0, 600.0);
    let root = ui.tree.window_rect(parts.root);
    let places = ui
        .tree
        .window_rect(ui.tree.children(ui.tree.children(parts.root)[0])[0]);
    assert_eq!(places.width, 176.0);
    // The buttons sit 16 from the right edge, Save last.
    let save = ui.tree.window_rect(parts.primary);
    assert_eq!(root.right() - save.right(), 16.0);
    assert!(save.x > ui.tree.window_rect(parts.secondary).right());
    // Compact: 440 wide, no places column.
    spec.expanded = false;
    let c = file_panel(&mut ui, &spec);
    assert_eq!(ui.measure(c.root, Proposal::UNSPECIFIED).width, 440.0);
    assert!(c.browser.is_none());
    // Open: no fields.
    let o = file_panel(&mut ui, &FilePanelSpec::new(Mode::Open));
    assert!(o.name.is_none() && o.browser.is_some());
}
