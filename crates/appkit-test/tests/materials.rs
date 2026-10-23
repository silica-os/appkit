//! Kit parity of the glass materials. The `Materials` page of the kit has no masters of its own,
//! so the surfaces are measured in the masters that use them. The kit export in silica-design
//! (19 Oct) draws the Dock shelf in Regular glass and the Control Centre in Thick glass, where
//! ui-kit's `SThinGlass` (newer) uses the Thin glass: the parity runs pin the tier the export
//! used, and the Thin glass is checked against the .vue measures in `materials.rs`.

use appkit::amalfi::materials::{GlassPanel, Tier};
use appkit::geom::{Rect, Size};
use appkit_test::{KitMaster, Scene, Tolerance};

/// Glass, rounded corners and rims.
const GLASS: Tolerance = Tolerance {
    mean: 3.0,
    max: 140,
    mask_text: true,
};

fn check(name: &str, tier: Tier, radius: f32, mask: &[Rect], pixels: bool) -> Result<(), String> {
    let mut scene = Scene::load(name)?;
    let size: Size = scene.master.size();
    scene.mount_sized(size, |ui| ui.create(GlassPanel::new(tier, radius)));
    // Kit defect: the shared glass styles blur 4 / 10 / 18 where materials.md gives other values.
    scene.ignore_aspects = vec!["#blurs".to_string()];
    scene.png_margin = true;
    scene.extra_mask = mask.to_vec();
    scene.skip_trees = Vec::new();
    let report = scene.geometry(&[("Surface", "Surface")], &[]);
    if !report.is_empty() {
        return Err(report.join("\n    "));
    }
    if pixels {
        scene.pixels(GLASS).map(|_| ())?;
    }
    Ok(())
}

fn run(prefix: &str, tier: Tier, radius: f32, layer_mask: &[Rect], pixels: bool, count: usize) {
    let names = KitMaster::names(prefix);
    assert_eq!(names.len(), count, "{names:?}");
    let failures: Vec<String> = names
        .iter()
        .filter_map(|n| {
            check(n, tier, radius, layer_mask, pixels)
                .err()
                .map(|e| format!("{n}: {e}"))
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn thick_glass_matches_the_edit_button_pixels() {
    let names = KitMaster::names("Control Center/Module/Edit Button");
    assert_eq!(names.len(), 2, "{names:?}");
    for n in &names {
        let mut scene = Scene::load(n).expect("master");
        let size = scene.master.size();
        scene.mount_sized(size, |ui| {
            ui.create(GlassPanel::new(Tier::Thick, 12.0).layer("Glass"))
        });
        scene.ignore_aspects = vec!["#blurs".to_string()];
        scene.png_margin = true;
        let report = scene.geometry(&[("Glass", "Glass")], &[]);
        assert!(report.is_empty(), "{n}: {}", report.join("\n"));
        if let Err(e) = scene.pixels(GLASS) {
            panic!("{n}: {e}");
        }
    }
}

#[test]
fn regular_glass_matches_the_shelf() {
    run("Dock/Shelf/Default", Tier::Regular, 24.0, &[], false, 2);
}

#[test]
fn regular_glass_matches_the_banner() {
    // The banner's content (a text block and the app icon) is left out of the pixels.
    let names = KitMaster::names("Notifications/Banner/Basic");
    assert_eq!(names.len(), 2, "{names:?}");
    let mask = [Rect::new(12.0, 12.0, 336.0, 52.0)];
    let failures: Vec<String> = names
        .iter()
        .filter_map(|n| {
            check(n, Tier::Regular, 18.0, &mask, true)
                .err()
                .map(|e| format!("{n}: {e}"))
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn thick_glass_matches_the_notification_centre_and_the_detail_panes() {
    run(
        "Notifications/Centre/Panel",
        Tier::Thick,
        24.0,
        &[],
        false,
        2,
    );
    let names = KitMaster::names("Control Center/Detail/Sound");
    assert_eq!(names.len(), 2, "{names:?}");
    for n in &names {
        let mut scene = Scene::load(n).expect("master");
        let size = scene.master.size();
        scene.mount_sized(size, |ui| {
            ui.create(GlassPanel::new(Tier::Thick, 16.0).layer("Glass"))
        });
        scene.ignore_aspects = vec!["#blurs".to_string()];
        let report = scene.geometry(&[("Glass", "Glass")], &[]);
        assert!(report.is_empty(), "{n}: {}", report.join("\n"));
    }
}
