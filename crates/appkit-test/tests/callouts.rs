//! Kit parity of the callouts: `Callout` takes the outline of the popover bubbles (the kit has
//! no masters of its own for `SCallout`; the popovers are its bubbles).

use appkit::amalfi::callouts::{Callout, CalloutShape};
use appkit::amalfi::popovers::{Align, Edge};
use appkit::core::{Empty, Prop, Value};
use appkit::geom::Rect;
use appkit_test::{KitMaster, Scene, Tolerance};

/// Glass, rounded corners and the arrow's antialiasing.
const BUBBLE: Tolerance = Tolerance {
    mean: 3.0,
    max: 140,
    mask_text: true,
};

fn shape_of(name: &str) -> CalloutShape {
    let place = name.split('/').nth(3).unwrap_or("");
    let mut words = place.split(' ');
    let edge = match words.next() {
        Some("Top") => Some(Edge::Top),
        Some("Bottom") => Some(Edge::Bottom),
        Some("Leading") => Some(Edge::Leading),
        Some("Trailing") => Some(Edge::Trailing),
        _ => None,
    };
    let align = match words.next() {
        Some("Start") => Align::Start,
        Some("End") => Align::End,
        _ => Align::Center,
    };
    CalloutShape::new(edge, align)
}

#[test]
fn callouts_take_the_outline_of_the_popover_bubbles() {
    let names: Vec<String> = KitMaster::names("Popovers/Popover/");
    assert_eq!(names.len(), 78, "{} masters", names.len());
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let size = scene.master.size();
        let content = scene.master.layer("Content").map(|c| c.frame);
        let pad = if name.contains("/List/") { 8.0 } else { 16.0 };
        scene.mount_sized(size, |ui| {
            let inner = ui.create(Empty);
            if let Some(c) = content {
                let _ = ui.set(inner, Prop::Width, Value::from(c.width));
                let _ = ui.set(inner, Prop::Height, Value::from(c.height));
            }
            let mut c = Callout::new(shape_of(name)).padded(pad);
            if c.shape.edge.is_none() {
                c.layer = "Body".to_string();
            }
            let call = ui.create(c);
            ui.add_child(call, inner);
            call
        });
        scene.png_margin = true;
        if let Some(c) = content {
            scene.extra_mask = vec![Rect::new(c.x, c.y, c.width, c.height)];
        }
        let layer = if shape_of(name).edge.is_none() {
            "Body"
        } else {
            "Surface"
        };
        let report = scene.geometry(&[(layer, layer)], &[]);
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            continue;
        }
        if let Err(e) = scene.pixels(BUBBLE) {
            failures.push(format!("{name}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
