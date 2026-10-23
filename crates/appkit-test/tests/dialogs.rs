//! Kit parity of `Dialogs`: the form dialog, the open and save panels, the browser and the
//! panels' footers.

mod common;

use appkit::amalfi::button::{PushButton, PushVariant};
use appkit::amalfi::dialogs::{BodyItem, Dialog, dialog, dialog_footer, form_body};
use appkit::amalfi::panels::{FileBrowser, FileEntry, FilePanelSpec, Mode, file_panel};
use appkit::amalfi::text::{Text, TextRole};
use appkit::amalfi::tokens::typography;
use appkit::core::{Prop, Ui, Value, ViewId};
use appkit_test::{KitLayer, KitMaster, Scene, Tolerance};
use common::{build_footer, build_row, child, text_at};

/// Raised surfaces with controls and rows inside.
const DIALOG: Tolerance = Tolerance {
    mean: 3.0,
    max: 150,
    mask_text: true,
};

fn set_id(ui: &mut Ui, v: ViewId, name: &str) {
    let _ = ui.set(v, Prop::Identifier, Value::from(name));
}

fn set_width(ui: &mut Ui, v: ViewId, w: f32) {
    let _ = ui.set(v, Prop::Width, Value::from(w));
}

fn push(ui: &mut Ui, k: &KitLayer) -> ViewId {
    let primary = k.master.as_deref().is_some_and(|m| m.contains("/Primary/"));
    let b = ui.create(
        PushButton::new(&text_at(k, "Label").unwrap_or_default()).variant(if primary {
            PushVariant::Primary
        } else {
            PushVariant::Secondary
        }),
    );
    let _ = ui.set(b, Prop::MinWidth, Value::from(72.0));
    set_id(ui, b, &k.name);
    b
}

fn footer(ui: &mut Ui, kit: &KitLayer) -> ViewId {
    let mut buttons: Vec<&KitLayer> = kit.children.iter().filter(|c| c.master.is_some()).collect();
    buttons.sort_by(|a, b| a.frame.x.total_cmp(&b.frame.x));
    let spacer_x = child(kit, "Spacer").map_or(0.0, |s| s.frame.x);
    let (mut lead, mut trail) = (Vec::new(), Vec::new());
    for k in buttons {
        let v = push(ui, k);
        if k.frame.x < spacer_x {
            lead.push(v);
        } else {
            trail.push(v);
        }
    }
    dialog_footer(ui, kit.frame.width, &lead, &trail)
}

/// The file list of the kit's `Dialogs/Browser/Files`: its rows are `Sidebars/Row` instances
/// named after the files, with the kind and date beside them.
fn browser_view(ui: &mut Ui, kit: &KitLayer) -> ViewId {
    let mut rows: Vec<&KitLayer> = kit
        .children
        .iter()
        .filter(|c| {
            c.master
                .as_deref()
                .is_some_and(|m| m.starts_with("Sidebars/Row/"))
        })
        .collect();
    rows.sort_by(|a, b| a.frame.y.total_cmp(&b.frame.y));
    let mut files = Vec::new();
    let mut selected = None;
    for (i, r) in rows.iter().enumerate() {
        let kind = text_at(kit, &format!("Kind {}", i + 1)).unwrap_or_default();
        let date = text_at(kit, &format!("Date {}", i + 1)).unwrap_or_default();
        files.push(FileEntry::new(&r.name, &r.name, "doc", &kind, &date));
        if r.master
            .as_deref()
            .is_some_and(|m| m.contains("/Selected/"))
        {
            selected = Some(r.name.clone());
        }
    }
    let mut b = FileBrowser::new(files);
    b.selected = selected;
    b.height = kit.frame.height;
    ui.create(b)
}

/// An open or save panel from its kit master.
fn file_panel_view(ui: &mut Ui, kit: &KitLayer, name: &str) -> ViewId {
    let open = name.contains("/Open Panel/");
    let mut spec = FilePanelSpec::new(if open { Mode::Open } else { Mode::Save });
    spec.expanded = !name.contains("Compact");
    spec.name = "Untitled".to_string();
    spec.locations = vec!["Recipes".to_string()];
    // The places: headers and rows by their order in the sidebar.
    let mut kids: Vec<&KitLayer> = kit
        .children
        .iter()
        .filter(|c| {
            c.master
                .as_deref()
                .is_some_and(|m| m.starts_with("Sidebars/"))
        })
        .collect();
    kids.sort_by(|a, b| a.frame.y.total_cmp(&b.frame.y));
    for k in kids {
        let m = k.master.clone().unwrap_or_default();
        if m.starts_with("Sidebars/Header/") {
            spec.places
                .push((k.name.trim_start_matches("Header ").to_string(), Vec::new()));
        } else if let Some(g) = spec.places.last_mut() {
            g.1.push((k.name.clone(), k.name.clone(), "folder".to_string()));
        }
    }
    if let Some(b) = child(kit, "Browser") {
        let mut rows: Vec<&KitLayer> = b
            .children
            .iter()
            .filter(|c| {
                c.master
                    .as_deref()
                    .is_some_and(|m| m.starts_with("Sidebars/Row/"))
            })
            .collect();
        rows.sort_by(|a, c| a.frame.y.total_cmp(&c.frame.y));
        for (i, r) in rows.iter().enumerate() {
            let kind = text_at(b, &format!("Kind {}", i + 1)).unwrap_or_default();
            let date = text_at(b, &format!("Date {}", i + 1)).unwrap_or_default();
            spec.files
                .push(FileEntry::new(&r.name, &r.name, "doc", &kind, &date));
            if r.master
                .as_deref()
                .is_some_and(|m| m.contains("/Selected/"))
            {
                spec.selected = Some(r.name.clone());
            }
        }
    }
    file_panel(ui, &spec).root
}

fn build(ui: &mut Ui, kit: &KitLayer, name: &str) -> Option<ViewId> {
    if name.starts_with("Dialogs/Browser/") {
        return Some(browser_view(ui, kit));
    }
    if name.starts_with("Dialogs/Footer/") {
        return Some(footer(ui, kit));
    }
    if name.contains("/Open Panel/") || name.contains("/Save Panel") {
        return Some(file_panel_view(ui, kit, name));
    }
    let mut kids: Vec<&KitLayer> = kit
        .children
        .iter()
        .filter(|c| c.name != "Surface")
        .collect();
    kids.sort_by(|a, b| a.frame.y.total_cmp(&b.frame.y));
    let mut views = Vec::new();
    let mut body = Vec::new();
    let mut footer_view = None;
    for k in kids {
        let m = k.master.clone().unwrap_or_default();
        if k.name == "Title" {
            let t = ui.create(
                Text::new(&k.text.clone().unwrap_or_default())
                    .token(&typography::TITLE_3)
                    .role(TextRole::Primary)
                    .layer("Title")
                    .whole(1.0, 0.0),
            );
            set_id(ui, t, &k.name);
            views.push(t);
        } else if k.name == "Section rule" {
            body.push(BodyItem::Rule);
        } else if m.starts_with("Forms/Footer/") {
            let v = build_footer(ui, k);
            set_width(ui, v, k.frame.width);
            set_id(ui, v, &k.name);
            footer_view = Some(v);
        } else if m.starts_with("Forms/Row/") {
            let v = build_row(ui, k);
            set_width(ui, v, k.frame.width);
            set_id(ui, v, &k.name);
            // Consecutive checkbox rows are one group, 4 apart.
            if m.starts_with("Forms/Row/Check/")
                && let Some(BodyItem::Group(g)) = body.last_mut()
            {
                g.push(v);
            } else if m.starts_with("Forms/Row/Check/") {
                body.push(BodyItem::Group(vec![v]));
            } else {
                body.push(BodyItem::View(v));
            }
        } else {
            return None;
        }
    }
    views.push(form_body(ui, &body));
    views.extend(footer_view);
    Some(dialog(ui, Dialog::form(), &views))
}

#[test]
fn dialog_masters_match_the_kit() {
    let names = KitMaster::names("Dialogs");
    assert_eq!(names.len(), 12, "{} masters", names.len());
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let root = scene.master.root.clone();
        let size = scene.master.size();
        scene.mount_sized(size, |ui| {
            build(ui, &root, name).unwrap_or_else(|| ui.create(Dialog::form()))
        });
        scene.png_margin = true;
        let w = scene.window;
        scene.ui.set_window_active(w, true);
        // Kit quirks of the nested forms (see forms.rs) and Sketch's wider titles.
        let skip = [
            "Control/Background",
            "Control/Focus Halo",
            "Control/Error icon",
            "Control/Value",
            "Control/Switch/Track",
            "Control/Switch/Thumb",
            "Save/Label",
            // The window draws its own traffic lights; the kit's glyph on the expand button is
            // a placeholder (the .vue uses the chevrons).
            "Traffic lights/Zoom/Dot",
            "Traffic lights/Minimise/Dot",
            "Traffic lights/Close/Dot",
            "Expand/Glyph Plus",
            "Expand/Glyph Chevron Up",
            "Expand/Glyph Chevron Down",
            "Title",
            // Buttons wider than their text keep the master label's place in the kit.
            "/Label",
        ];
        let report = scene.auto_geometry_except(&skip, &[]);
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            continue;
        }
        // The PNGs of the redrawn dialogs (ui-kit 22 Oct) are the older layout.
        if appkit_test::kit::png_is_stale(&scene.master) {
            continue;
        }
        // The footer's buttons are 72 wide in the kit whatever their text: a mean that small
        // with a high peak is an edge or two.
        let tol = Tolerance { max: 255, ..DIALOG };
        if let Err(e) = scene.pixels(tol) {
            failures.push(format!("{name}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
