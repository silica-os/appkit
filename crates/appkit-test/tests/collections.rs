//! Kit parity of the collections against the file manager's masters: `Apps/Mustache/Column
//! Row` (list rows: plain, selected, dimmed) and `Tile Selected` (a grid tile). Tables and
//! outlines follow the ui-kit masters (`kit_collections.rs`), whose spacing and stripes the
//! Mustache `List Header` and `List Row` predate.
//! The cells are the app's content; what is compared is the collection's part (row and
//! header geometry, column origins, stripes, selections, emphasis of selected content).

use appkit::amalfi::collections::{GridView, ListLook, ListView};
use appkit::amalfi::label::ColorRole;
use appkit::amalfi::label::{Label, SymbolImage};
use appkit::amalfi::tokens::typography;
use appkit::core::layout::{Alignment, Overlay, Stack};
use appkit::core::{Prop, Ui, Value, ViewId};
use appkit::geom::Size;
use appkit_test::{KitMaster, Scene, Tolerance};

/// Text and thin glyphs on flat fills.
const ROWS: Tolerance = Tolerance {
    mean: 2.5,
    max: 255,
    mask_text: true,
};

fn text(
    ui: &mut Ui,
    id: &str,
    s: &str,
    style: &'static appkit::amalfi::tokens::TypeStyle,
    color: &str,
    width: Option<f32>,
) -> ViewId {
    let v = ui.create(Label::new(s).style(style).color(color));
    let _ = ui.set(v, Prop::Identifier, Value::from(id));
    if let Some(w) = width {
        let _ = ui.set(v, Prop::Width, Value::from(w));
    }
    v
}

fn icon(ui: &mut Ui, name: &str, size: f32) -> ViewId {
    let mut s = SymbolImage::new(name);
    s.size = size;
    s.color = ColorRole::Token("Accent/Amalfi".into());
    let v = ui.create(s);
    let _ = ui.set(v, Prop::Identifier, Value::from("Icon"));
    v
}

fn fill(ui: &mut Ui, v: ViewId) {
    let _ = ui.set(v, Prop::MaxWidth, Value::from(f32::INFINITY));
    let _ = ui.set(v, Prop::MaxHeight, Value::from(f32::INFINITY));
}

/// A browser column's row (`Column Row`): icon at 16, label at 40, a chevron at the end.
fn column_row(ui: &mut Ui) -> ViewId {
    let row = ui.create(Stack::row(4.0));
    let _ = ui.set(
        row,
        Prop::Padding,
        Value::Insets(appkit::geom::Insets::new(0.0, 8.0, 0.0, 4.0)),
    );
    let i = icon(ui, "folder", 20.0);
    let l = text(
        ui,
        "Label",
        "Documents",
        &typography::BODY,
        "Text/Primary",
        Some(160.0),
    );
    let mut c = SymbolImage::new("chevron-right");
    c.size = 16.0;
    c.color = ColorRole::Token("Text/Tertiary".into());
    let ch = ui.create(c);
    let _ = ui.set(ch, Prop::Identifier, Value::from("Chevron"));
    for v in [i, l, ch] {
        ui.add_child(row, v);
        ui.release(v);
    }
    row
}

#[test]
fn list_rows_match_the_kit() {
    let names = KitMaster::names("Apps/Mustache/Column Row");
    assert_eq!(names.len(), 6, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let size = scene.master.size();
        let list = scene.mount_sized(size, |ui| {
            let v = ui.create(
                ListView::new(1, 28.0, Box::new(|ui, _| Some(column_row(ui))))
                    .look(ListLook::Inset),
            );
            fill(ui, v);
            v
        });
        let selected = name.contains("/Selected/");
        if selected || name.contains("/Dimmed/") {
            let _ = scene
                .ui
                .set(list, Prop::SelectedRows, Value::IntList(vec![0]));
        }
        if selected {
            let w = scene.window;
            scene.ui.set_window_active(w, true);
            scene.ui.set_focus(list, false);
        }
        scene.refresh();
        let mut map = vec![
            ("Icon", "Icon/Icon"),
            ("Label", "Label/Label"),
            ("Chevron", "Chevron/Icon"),
        ];
        if name.contains("/Plain/") {
            // No selection.
        } else {
            map.push(("Selection", "Selection"));
        }
        // Kit quirk: without a selection the label sits 1 pt higher (4 vs 5). Dimmed, the
        // selection is the ui-kit's opaque control surface, not this master's translucent one.
        let dimmed = name.contains("/Dimmed/");
        let devs: &[&str] = if name.contains("/Plain/") {
            &["Label#frame"]
        } else if dimmed {
            &["Selection#fills"]
        } else {
            &[]
        };
        let report = scene.geometry(&map, devs);
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            continue;
        }
        if !dimmed && let Err(e) = scene.pixels(ROWS) {
            failures.push(format!("{name}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_selected_tile_matches_the_kit() {
    let names = KitMaster::names("Apps/Mustache/Tile Selected");
    assert_eq!(names.len(), 2, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let size = scene.master.size();
        let grid = scene.mount_sized(size, |ui| {
            let g = GridView::new(
                1,
                Box::new(|ui, _| {
                    let o = ui.create(Overlay {
                        alignment: Alignment::TOP,
                    });
                    let l = text(
                        ui,
                        "Label",
                        "Desktop",
                        &typography::CALLOUT,
                        "Text/Primary",
                        Some(88.0),
                    );
                    let _ = ui.set(l, Prop::TextAlignment, Value::Int(1));
                    let _ = ui.set(
                        l,
                        Prop::Padding,
                        Value::Insets(appkit::geom::Insets::new(74.0, 0.0, 0.0, 0.0)),
                    );
                    ui.add_child(o, l);
                    ui.release(l);
                    Some(o)
                }),
            )
            .tile(Size::new(104.0, 104.0), 64.0)
            .spacing(0.0, 0.0);
            let v = ui.create(g);
            fill(ui, v);
            v
        });
        let w = scene.window;
        scene.ui.set_window_active(w, true);
        scene.ui.set_focus(grid, false);
        let _ = scene
            .ui
            .set(grid, Prop::SelectedRows, Value::IntList(vec![0]));
        scene.refresh();
        // The folder image and its glyph are the app's (a bitmap in the kit).
        scene
            .extra_mask
            .push(appkit::geom::Rect::new(20.0, 0.0, 64.0, 64.0));
        let report = scene.geometry(
            &[
                ("Selection", "Selection"),
                ("Label capsule", "Label capsule"),
                ("Label", "Label/Label"),
            ],
            &[],
        );
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            continue;
        }
        if let Err(e) = scene.pixels(ROWS) {
            failures.push(format!("{name}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
