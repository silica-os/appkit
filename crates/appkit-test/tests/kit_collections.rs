//! Kit parity of the collections against ui-kit's `Tables`, `Outlines`, `Lists` and `Grids`
//! masters (`STable`, `SOutline`, `SList`, `SGrid`). The cells are the app's content; what is
//! compared is the collection's part (header, column origins, stripes, selections,
//! disclosure chevrons, separators, plates, pills, rings and badges) and where the content
//! lands.

use appkit::amalfi::collections::{
    GridLook, GridView, ListHeader, ListLook, ListRow, ListView, TableColumn, TableHeader,
    TableView, TreeSource,
};
use appkit::amalfi::label::{ColorRole, Label, SymbolImage};
use appkit::amalfi::tokens::typography;
use appkit::core::layout::{Alignment, Overlay, Stack};
use appkit::core::{Prop, Ui, Value, ViewId};
use appkit::geom::{Insets, Point};
use appkit_test::{KitMaster, Scene};
use common::text_at;

mod common;

fn text(
    ui: &mut Ui,
    id: &str,
    s: &str,
    style: &'static appkit::amalfi::tokens::TypeStyle,
    color: &str,
    width: f32,
    align: i64,
) -> ViewId {
    let v = ui.create(Label::new(s).style(style).color(color));
    let _ = ui.set(v, Prop::Identifier, Value::from(id));
    let _ = ui.set(v, Prop::Width, Value::from(width));
    if align != 0 {
        let _ = ui.set(v, Prop::TextAlignment, Value::Int(align));
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

/// An icon and a name side by side, the name ending at `end` from the column's edge at `x`.
fn named(ui: &mut Ui, symbol: &str, name: &str, width: f32) -> ViewId {
    let row = ui.create(Stack::row(8.0));
    let i = icon(ui, symbol, 16.0);
    let n = text(
        ui,
        "Name",
        name,
        &typography::BODY,
        "Text/Primary",
        width,
        0,
    );
    for v in [i, n] {
        ui.add_child(row, v);
        ui.release(v);
    }
    row
}

/// Gives the live rows of `collection` their index as identifier, so the cells' paths are
/// `<row>/<cell>`.
fn name_rows(scene: &mut Scene, collection: ViewId) {
    let Some(body) = scene.ui.tree.children(collection).first().copied() else {
        return;
    };
    let rows: Vec<ViewId> = scene.ui.tree.children(body).to_vec();
    for (i, r) in rows.iter().enumerate() {
        let _ = scene
            .ui
            .set(*r, Prop::Identifier, Value::from(i.to_string().as_str()));
    }
    scene.refresh();
}

/// Maps kit layers to ours: `(kit, our path prefix)`, our path being the first that starts
/// with the prefix.
fn resolve(scene: &mut Scene, map: &[(&str, String)], deviations: &[&str]) -> Vec<String> {
    let ours = scene.layers();
    let resolved: Vec<(String, String)> = map
        .iter()
        .map(|(k, o)| {
            let p = ours
                .iter()
                .find(|l| l.path == *o)
                .or_else(|| ours.iter().find(|l| l.path.starts_with(&format!("{o}/"))))
                .map_or_else(|| o.clone(), |l| l.path.clone());
            ((*k).to_string(), p)
        })
        .collect();
    let pairs: Vec<(&str, &str)> = resolved
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    scene.geometry(&pairs, deviations)
}

/// Selects `rows` and gives the collection the focus in an active window (`Selected`), or
/// leaves the window inactive (`Selected Inactive`).
fn select(scene: &mut Scene, view: ViewId, rows: Vec<i64>, active: bool) {
    let w = scene.window;
    scene.ui.set_window_active(w, active);
    scene.ui.set_focus(view, false);
    let _ = scene.ui.set(view, Prop::SelectedRows, Value::IntList(rows));
    scene.settle();
    scene.refresh();
}

fn check(failures: &mut Vec<String>, name: &str, report: Vec<String>) {
    if !report.is_empty() {
        failures.push(format!("{name}:\n    {}", report.join("\n    ")));
    }
}

// Tables (`STable`): Name after a 16 pt icon at 20, Kind at 300, Date at 396.

fn table_columns() -> Vec<TableColumn> {
    vec![
        TableColumn::new("Name", 272.0).title_inset(24.0),
        TableColumn::new("Kind", 88.0),
        TableColumn::new("Date Modified", 132.0),
    ]
}

fn table_cell(ui: &mut Ui, column: usize) -> Option<ViewId> {
    Some(match column {
        0 => named(ui, "doc-text", "Lemon cake.recipe", 236.0),
        1 => text(
            ui,
            "Kind",
            "Recipe",
            &typography::BODY,
            "Text/Secondary",
            88.0,
            0,
        ),
        _ => text(
            ui,
            "Date",
            "Today, 09:41",
            &typography::BODY,
            "Text/Secondary",
            132.0,
            0,
        ),
    })
}

#[test]
fn table_headers_match_the_kit() {
    let names = KitMaster::names("Tables/Header");
    assert_eq!(names.len(), 6, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let size = scene.master.size();
        let header = scene.mount_sized(size, |ui| {
            let h = ui.create(TableHeader::new(table_columns()));
            fill(ui, h);
            h
        });
        let sort = if name.contains("/Ascending/") {
            Value::Int(0)
        } else if name.contains("/Descending/") {
            Value::Int(-1)
        } else {
            Value::None
        };
        let _ = scene.ui.set(header, Prop::SortDescriptor, sort);
        scene.refresh();
        check(&mut failures, name, scene.auto_geometry_except(&[], &[]));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn table_rows_match_the_kit() {
    let names = KitMaster::names("Tables/Row");
    assert_eq!(names.len(), 8, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let size = scene.master.size();
        let odd = name.contains("/Odd/");
        let selected = name.contains("/Selected");
        let table = scene.mount_sized(size, |ui| {
            let mut t = TableView::new(table_columns())
                .header_visible(false)
                .alternating(true);
            t.set_cells(2, 28.0, Box::new(|ui, _, c| table_cell(ui, c)));
            let v = ui.create(t);
            fill(ui, v);
            v
        });
        // The odd row scrolled to the top.
        if odd {
            let _ = scene.ui.set(
                table,
                Prop::ScrollOffset,
                Value::Point(Point::new(0.0, 28.0)),
            );
        }
        if selected {
            select(&mut scene, table, vec![0], !name.contains("Inactive"));
        }
        scene.refresh();
        name_rows(&mut scene, table);
        let row = usize::from(odd);
        let mut map = vec![
            ("Icon", format!("{row}/Icon")),
            ("Name", format!("{row}/Name")),
            ("Kind", format!("{row}/Kind")),
            ("Date", format!("{row}/Date")),
        ];
        if odd {
            map.push(("Stripe", "Stripe".to_string()));
        }
        if selected {
            map.push(("Selection", "Selection".to_string()));
        }
        let map: Vec<(&str, String)> = map;
        check(&mut failures, name, resolve(&mut scene, &map, &[]));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// Outlines (`SOutline`): a disclosure at 12 and the icon at 32, 20 pt deeper per level; Size
// right-aligned at 424.

fn outline_columns() -> Vec<TableColumn> {
    vec![
        TableColumn::new("Name", 396.0).title_inset(12.0),
        TableColumn::new("Size", 96.0).align(appkit::text::Align::End),
    ]
}

/// root → 1 → 2 → 3, each with one child but the last.
struct Chain {
    depth: u64,
}

impl TreeSource for Chain {
    fn child_count(&mut self, _ui: &mut Ui, parent: Option<u64>) -> usize {
        usize::from(parent.unwrap_or(0) < self.depth)
    }

    fn child(&mut self, _ui: &mut Ui, parent: Option<u64>, _index: usize) -> u64 {
        parent.unwrap_or(0) + 1
    }

    fn is_expandable(&mut self, _ui: &mut Ui, node: u64) -> bool {
        node < self.depth
    }
}

#[test]
fn outline_headers_match_the_kit() {
    let names = KitMaster::names("Outlines/Header");
    assert_eq!(names.len(), 2, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let size = scene.master.size();
        scene.mount_sized(size, |ui| {
            let h = ui.create(TableHeader::new(outline_columns()));
            fill(ui, h);
            h
        });
        // Kit quirk: the divider is 12 before Size where the table's are 8 before their
        // column (`Tables/Header`).
        check(
            &mut failures,
            name,
            scene.auto_geometry_except(&[], &["Divider#frame"]),
        );
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn outline_rows_match_the_kit() {
    let names = KitMaster::names("Outlines/Row");
    assert_eq!(names.len(), 24, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let size = scene.master.size();
        let root = scene.master.root.clone();
        let label = text_at(&root, "Name").unwrap_or_default();
        let detail = text_at(&root, "Size").unwrap_or_default();
        // The row shown and its level: Open and Closed at the top, Leaf one level down,
        // Leaf Deep two.
        let (level, depth, open) = if name.contains("/Leaf Deep/") {
            (2usize, 2u64, vec![1i64, 2])
        } else if name.contains("/Leaf/") {
            (1, 1, vec![1])
        } else if name.contains("/Open/") {
            (0, 1, vec![1])
        } else {
            (0, 1, vec![])
        };
        let table = scene.mount_sized(size, |ui| {
            let mut t = TableView::new(outline_columns()).header_visible(false);
            t.set_tree(
                Box::new(Chain { depth: depth + 1 }),
                28.0,
                Box::new(move |ui, node, c| {
                    let level = (node - 1) as f32;
                    let leaf = node > depth;
                    Some(if c == 0 {
                        let symbol = if leaf { "doc-text" } else { "folder" };
                        named(ui, symbol, &label, 300.0 - 20.0 * level)
                    } else {
                        text(
                            ui,
                            "Size",
                            &detail,
                            &typography::BODY,
                            "Text/Secondary",
                            96.0,
                            2,
                        )
                    })
                }),
            );
            let v = ui.create(t);
            fill(ui, v);
            v
        });
        let _ = scene
            .ui
            .set(table, Prop::ExpandedItems, Value::IntList(open));
        scene.refresh();
        let _ = scene.ui.set(
            table,
            Prop::ScrollOffset,
            Value::Point(Point::new(0.0, 28.0 * level as f32)),
        );
        if name.contains("/Selected") {
            select(
                &mut scene,
                table,
                vec![level as i64 + 1],
                !name.contains("Inactive"),
            );
        }
        scene.refresh();
        name_rows(&mut scene, table);
        let mut map = vec![
            ("Icon", format!("{level}/Icon")),
            ("Name", format!("{level}/Name")),
            ("Size", format!("{level}/Size")),
        ];
        if !name.contains("/Leaf") {
            map.push(("Disclosure", "Disclosure".to_string()));
        }
        if name.contains("/Selected") {
            map.push(("Selection", "Selection".to_string()));
        }
        check(&mut failures, name, resolve(&mut scene, &map, &[]));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// Grids (`SGrid`).

fn grid_item(ui: &mut Ui, look: GridLook) -> ViewId {
    let o = ui.create(Overlay {
        alignment: Alignment::TOP,
    });
    if look == GridLook::Icons {
        let l = text(
            ui,
            "Label",
            "Lemon cake",
            &typography::CAPTION,
            "Text/Primary",
            80.0,
            1,
        );
        let _ = ui.set(
            l,
            Prop::Padding,
            Value::Insets(Insets::new(93.0, 0.0, 0.0, 0.0)),
        );
        ui.add_child(o, l);
        ui.release(l);
    } else {
        let _ = ui.set(o, Prop::Background, Value::from("Accent/Amalfi"));
        let _ = ui.set(o, Prop::CornerRadius, Value::from(10.0f32));
    }
    o
}

#[test]
fn grid_items_match_the_kit() {
    let names = KitMaster::names("Grids/Item");
    assert_eq!(names.len(), 12, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let size = scene.master.size();
        let look = if name.contains("/Icon/") {
            GridLook::Icons
        } else {
            GridLook::Thumbnails
        };
        let grid = scene.mount_sized(size, |ui| {
            let g = GridView::new(1, Box::new(move |ui, _| Some(grid_item(ui, look))))
                .look(look)
                .spacing(0.0, 0.0);
            let v = ui.create(g);
            fill(ui, v);
            v
        });
        let selected = name.contains("/Selected");
        if selected {
            select(&mut scene, grid, vec![0], !name.contains("Inactive"));
        }
        let mut map: Vec<(&str, String)> = Vec::new();
        if look == GridLook::Icons {
            map.push(("Label", "Label/Label".to_string()));
            if selected {
                map.push(("Plate", "Plate".to_string()));
                map.push(("Label Pill", "Label Pill".to_string()));
            }
        } else if selected {
            for l in ["Selection Ring", "Check Badge", "Check"] {
                map.push((l, l.to_string()));
            }
        }
        // The plate's light fill is 8 % ink where `Line/Separator` is 10 % (ui-kit draws it
        // with `Line/Separator`).
        let devs: &[&str] = if selected && look == GridLook::Icons && name.ends_with("/Light") {
            &["Plate#fills"]
        } else {
            &[]
        };
        check(&mut failures, name, resolve(&mut scene, &map, devs));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// Lists (`SList`): a glyph on a tile, a title (and a subtitle), a chevron; separators under
// the text.

#[test]
fn list_headers_match_the_kit() {
    let names = KitMaster::names("Lists/Header");
    assert_eq!(names.len(), 2, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let size = scene.master.size();
        let title = text_at(&scene.master.root, "Title").unwrap_or_default();
        scene.mount_sized(size, |ui| {
            let h = ui.create(ListHeader::new(&title));
            fill(ui, h);
            h
        });
        check(&mut failures, name, scene.auto_geometry_except(&[], &[]));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn list_rows_match_the_kit() {
    let names = KitMaster::names("Lists/Row");
    assert_eq!(names.len(), 16, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let size = scene.master.size();
        let root = scene.master.root.clone();
        let title = text_at(&root, "Title").unwrap_or_default();
        let subtitle = text_at(&root, "Subtitle");
        let disabled = name.contains("/Disabled/");
        let height = size.height;
        let list = scene.mount_sized(size, |ui| {
            let v = ui.create(
                ListView::new(
                    2,
                    height,
                    Box::new(move |ui, _| {
                        let mut row = ListRow::new(&title).symbol("doc-text").chevron(true);
                        if let Some(s) = &subtitle {
                            row = row.subtitle(s);
                        }
                        let v = ui.create(row);
                        if disabled {
                            let _ = ui.set(v, Prop::Disabled, Value::Bool(true));
                        }
                        Some(v)
                    }),
                )
                .look(ListLook::Separated),
            );
            fill(ui, v);
            v
        });
        let selected = name.contains("/Selected");
        if selected {
            select(&mut scene, list, vec![0], !name.contains("Inactive"));
        }
        scene.refresh();
        name_rows(&mut scene, list);
        let mut map: Vec<(&str, String)> = ["Tile", "Glyph", "Title", "Accessory"]
            .iter()
            .map(|l| (*l, format!("0/{l}")))
            .collect();
        if name.contains("/Two Line/") {
            map.push(("Subtitle", "0/Subtitle".to_string()));
        }
        if selected {
            map.push(("Selection", "Selection".to_string()));
        } else {
            map.push(("Separator", "Separator".to_string()));
        }
        check(&mut failures, name, resolve(&mut scene, &map, &[]));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
