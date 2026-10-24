//! Kit parity of `Menu Bar`: the titles (the Silica mark, the app's name, a menu, closed or
//! open) and the bar that lines them up.

mod common;

use appkit::amalfi::menubar::{MenuTitle, TitleKind, menu_bar};
use appkit::core::{Prop, Value};
use appkit_test::{KitLayer, KitMaster, Scene, Tolerance};
use common::text_at;

/// Thin labels on a transparent bar.
const BAR: Tolerance = Tolerance {
    mean: 2.5,
    max: 120,
    mask_text: true,
};

fn kind_of(master: &str) -> TitleKind {
    if master.contains("/Silica/") {
        TitleKind::Mark
    } else if master.contains("/Name/") {
        TitleKind::Name
    } else {
        TitleKind::Menu
    }
}

#[test]
fn menu_bar_masters_match_the_kit() {
    let names = KitMaster::names("Menu Bar");
    assert_eq!(names.len(), 12, "{} masters", names.len());
    let mut failures = Vec::new();
    for name in &names {
        // The open menu is a popup of its own, compared by the `Menus` masters.
        if name.contains("/Menu Open/") {
            continue;
        }
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let root = scene.master.root.clone();
        let size = scene.master.size();
        let is_bar = name.starts_with("Menu Bar/Bar/");
        scene.mount_sized(size, |ui| {
            if is_bar {
                let mut kids: Vec<&KitLayer> = root
                    .children
                    .iter()
                    .filter(|c| {
                        c.master
                            .as_deref()
                            .is_some_and(|m| m.starts_with("Menu Bar/Item/"))
                    })
                    .collect();
                kids.sort_by(|a, b| a.frame.x.total_cmp(&b.frame.x));
                // Instances carry their master's text: a menu's title is the instance's name.
                let titles: Vec<(TitleKind, String)> = kids
                    .iter()
                    .map(|k| {
                        let kind = kind_of(k.master.as_deref().unwrap_or_default());
                        let label = match kind {
                            TitleKind::Mark => String::new(),
                            TitleKind::Name => {
                                text_at(k, "Label").unwrap_or_else(|| "Recipes".to_string())
                            }
                            TitleKind::Menu => k.name.clone(),
                        };
                        (kind, label)
                    })
                    .collect();
                let (bar, views) = menu_bar(ui, &titles);
                for (v, k) in views.iter().zip(&kids) {
                    let _ = ui.set(*v, Prop::Identifier, Value::from(k.name.as_str()));
                }
                bar
            } else {
                let label = text_at(&root, "Label").unwrap_or_default();
                let mut title = MenuTitle::new(kind_of(name), &label);
                title.active = name.contains("/Active/");
                ui.create(title)
            }
        });
        scene.png_margin = true;
        scene.settle();
        // The bar's status cluster is the Status Bar's, on surfaces of its own; the labels of
        // the instances carry their master's text.
        let skip: &[&str] = if is_bar {
            scene.skip_trees = vec!["Status Bar".to_string()];
            &["/Label"]
        } else {
            &[]
        };
        let report = scene.auto_geometry_except(skip, &[]);
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            continue;
        }
        if let Err(e) = scene.pixels(BAR) {
            failures.push(format!("{name}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
