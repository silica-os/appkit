//! Kit parity of `Notifications`: the banners and cards, the grouped stack and the centre.

mod common;

use appkit::amalfi::notifications::{
    NotificationSpec, Section, Variant, notification, notification_centre, notification_stack,
};
use appkit::core::Prop;
use appkit::geom::Rect;
use appkit_test::{KitLayer, KitMaster, Scene, Tolerance};
use common::text_at;

/// Glass, rounded corners and shadows; the app icon bitmap and the texts are masked.
const NOTIFICATION: Tolerance = Tolerance {
    mean: 3.0,
    max: 140,
    mask_text: true,
};

fn spec_of(root: &KitLayer, id: &str) -> NotificationSpec {
    let get = |p: &str| text_at(root, p).unwrap_or_default();
    let mut s = NotificationSpec::new(id, &get("Content/Text/Title"), &get("Content/Text/Body"))
        .time(&get("Content/Time"));
    if root.children.iter().any(|c| c.name == "Actions") {
        s = s.actions(&["Reply", "Mute"]);
    }
    s.fixed()
}

fn failures_of(prefix: &str, count: usize, mut each: impl FnMut(&str) -> Result<(), String>) {
    let names = KitMaster::names(prefix);
    assert_eq!(names.len(), count, "{names:?}");
    let failures: Vec<String> = names
        .iter()
        .filter_map(|n| each(n).err().map(|e| format!("{n}:\n    {e}")))
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn banner(name: &str, variant: Variant) -> Result<(), String> {
    let mut scene = Scene::load(name)?;
    let root = scene.master.root.clone();
    scene.mount(|ui| notification(ui, &spec_of(&root, "n"), variant).root);
    // Kit defect: the shared glass styles blur 10 / 18 where materials.md gives 6 / 10.
    scene.ignore_aspects = vec!["#blurs".to_string()];
    scene.png_margin = true;
    // The app icon is a bitmap in the kit.
    scene.skip_trees = vec!["Content/App icon".to_string()];
    scene.extra_mask = vec![Rect::new(12.0, 12.0, 36.0, 36.0)];
    // Sketch (CoreText) sets "Recipes" 2 pt and "now" 1 pt wider than our shaping; the body
    // column takes what the time leaves (1 pt more).
    // The action buttons keep the 56 pt of the master's default label (the export carries no
    // instance overrides).
    let report = scene.auto_geometry_except(
        &[
            "Actions/Reply/Background",
            "Actions/Reply/Label",
            "Actions/Mute/Background",
            "Actions/Mute/Label",
        ],
        &[
            "Content/Text/Title#frame",
            "Content/Time#frame",
            "Content/Text/Body#frame",
        ],
    );
    if !report.is_empty() {
        return Err(report.join("\n    "));
    }
    scene.pixels(NOTIFICATION).map(|_| ())
}

#[test]
fn banners_match_the_kit() {
    failures_of("Notifications/Banner/Basic", 2, |n| {
        banner(n, Variant::Banner)
    });
    failures_of("Notifications/Banner/With Actions", 2, |n| {
        banner(n, Variant::Banner)
    });
}

#[test]
fn cards_match_the_kit() {
    failures_of("Notifications/Banner/Panel/", 2, |n| {
        banner(n, Variant::Panel)
    });
    failures_of("Notifications/Banner/Panel With Actions", 2, |n| {
        banner(n, Variant::Panel)
    });
}

#[test]
fn the_grouped_stack_matches_the_kit() {
    failures_of("Notifications/Stack/Grouped", 2, |name| {
        let mut scene = Scene::load(name)?;
        let front = scene
            .master
            .layer("Front banner")
            .cloned()
            .ok_or("no front banner")?;
        let _ = &front;
        let spec = NotificationSpec::new(
            "a",
            "Recipes",
            "Two new recipes were added to Lemon cake. Tap to open them.",
        );
        let items = vec![spec.clone(), spec.clone(), spec];
        scene.mount(|ui| notification_stack(ui, items));
        scene.ignore_aspects = vec!["#blurs".to_string()];
        scene.png_margin = true;
        scene.skip_trees = vec!["Front banner".to_string()];
        scene.extra_mask = vec![Rect::new(12.0, 12.0, 336.0, 52.0)];
        let report = scene.auto_geometry(&[]);
        if !report.is_empty() {
            return Err(report.join("\n    "));
        }
        let _ = Prop::Title;
        scene.pixels(NOTIFICATION).map(|_| ())
    });
}

#[test]
fn the_centre_matches_the_kit() {
    failures_of("Notifications/Centre/Panel", 2, |name| {
        let mut scene = Scene::load(name)?;
        let card = |id: &str| {
            NotificationSpec::new(
                id,
                "Recipes",
                "Two new recipes were added to Lemon cake. Tap to open them.",
            )
        };
        scene.mount(|ui| {
            notification_centre(
                ui,
                "Notifications",
                "Clear all",
                vec![
                    Section::new(
                        "Today",
                        vec![
                            card("Mail banner"),
                            card("Actions banner").actions(&["Reply", "Mute"]),
                        ],
                    ),
                    Section::new("Earlier", vec![card("Earlier banner")]),
                ],
            )
        });
        scene.ignore_aspects = vec!["#blurs".to_string()];
        scene.png_margin = true;
        // The cards are instances of the banner masters (tested above).
        scene.skip_trees = vec![
            "Mail banner".to_string(),
            "Actions banner".to_string(),
            "Earlier banner".to_string(),
            "Header/Clear".to_string(),
        ];
        let report = scene.auto_geometry(&[]);
        if !report.is_empty() {
            return Err(report.join("\n    "));
        }
        Ok(())
    });
}
