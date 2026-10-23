//! Materials (kit `Materials`, ui-kit `SGlassPanel` and `SThinGlass`): the glass surfaces of the
//! shell. Thin is the shelf-like glass (Dock, Control Centre modules), Regular the control
//! glass (banners, menu bar) and Thick the menu, popover and panel glass.
//!
//! Thin is the Regular material with the `Glass/Tint Thin` tint and a lighter, more saturated
//! blur (the kit: blur 4 / saturation 1.5 against 10 / 1.6 for Regular).

use crate::amalfi::theme::Theme;
use crate::core::a11y::{A11y, Role};
use crate::core::layout::Proposal;
use crate::core::tree::ViewId;
use crate::core::ui::Ui;
use crate::core::value::{Prop, Value};
use crate::core::widget::{LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Rect, Size};
use crate::shape::Shape;
use crate::style::Style;

/// How thick a glass is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tier {
    /// The shelf glass (`Glass/Tint Thin`).
    Thin,
    /// The control glass (`Glass/Regular`).
    #[default]
    Regular,
    /// The menu and panel glass (`Glass/Thick`).
    Thick,
}

impl Tier {
    /// The tier of a `Prop::Style` integer (0 thin, 1 regular, 2 thick).
    pub fn from_int(n: i64) -> Option<Tier> {
        match n {
            0 => Some(Tier::Thin),
            1 => Some(Tier::Regular),
            2 => Some(Tier::Thick),
            _ => None,
        }
    }

    /// The tier as a `Prop::Style` integer.
    pub fn to_int(self) -> i64 {
        match self {
            Tier::Thin => 0,
            Tier::Regular => 1,
            Tier::Thick => 2,
        }
    }
}

/// The layer style of a glass of `tier`: blur and refraction, tint, rims and drop shadow. With
/// Reduce Transparency the blur is an opaque fill (the tint over `Surface/Window`).
pub fn glass_style(theme: &Theme, tier: Tier) -> Style {
    match tier {
        Tier::Thick => theme.material_style("Glass/Thick"),
        Tier::Regular => theme.material_style("Glass/Regular"),
        Tier::Thin => theme.material_style("Glass/Thin"),
    }
}

/// A glass panel: a [`glass_style`] surface in a smooth rounded rectangle, its children
/// (usually one) inside `padding`.
#[derive(Debug, Clone)]
pub struct GlassPanel {
    /// The glass.
    pub tier: Tier,
    /// The corner radius.
    pub radius: f32,
    /// The space between the edge and the content.
    pub padding: f32,
    /// The name of the surface layer (`Surface` in the kit; `Glass` for Control Centre).
    pub layer: String,
    /// The accessibility label (the panel is then a region).
    pub label: Option<String>,
}

impl GlassPanel {
    /// A panel of `tier` with corners of `radius`.
    pub fn new(tier: Tier, radius: f32) -> GlassPanel {
        GlassPanel {
            tier,
            radius,
            padding: 0.0,
            layer: "Surface".to_string(),
            label: None,
        }
    }

    /// With a padding.
    pub fn padded(mut self, padding: f32) -> GlassPanel {
        self.padding = padding;
        self
    }

    /// With another surface layer name.
    pub fn layer(mut self, name: &str) -> GlassPanel {
        self.layer = name.to_string();
        self
    }

    /// With an accessibility label.
    pub fn labelled(mut self, label: &str) -> GlassPanel {
        self.label = Some(label.to_string());
        self
    }
}

impl Widget for GlassPanel {
    fn kind(&self) -> &'static str {
        "GlassPanel"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let pad = self.padding.max(0.0);
        let inner = Proposal::new(
            p.width
                .filter(|w| w.is_finite())
                .map(|w| (w - 2.0 * pad).max(0.0)),
            p.height
                .filter(|h| h.is_finite())
                .map(|h| (h - 2.0 * pad).max(0.0)),
        );
        let mut size = Size::ZERO;
        for c in cx.children() {
            let s = cx.measure(c, inner);
            size.width = size.width.max(s.width);
            size.height = size.height.max(s.height);
        }
        Size::new(size.width + 2.0 * pad, size.height + 2.0 * pad)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        let pad = self.padding.max(0.0);
        let w = (size.width - 2.0 * pad).max(0.0);
        let h = (size.height - 2.0 * pad).max(0.0);
        for c in cx.children() {
            let s = cx.measure(c, Proposal::new(Some(w), Some(h)));
            cx.place(c, Rect::new(pad, pad, s.width.min(w), s.height.min(h)));
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let r = cx.bounds();
        let style = glass_style(&theme, self.tier);
        cx.rec
            .layer(&self.layer, r, Shape::rounded(self.radius), style);
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::Style => match value.as_int().and_then(Tier::from_int) {
                Some(t) => {
                    self.tier = t;
                    PropChange::Paint
                }
                None => PropChange::Unknown,
            },
            Prop::Value => match value.as_f32() {
                Some(r) if r >= 0.0 => {
                    self.radius = r;
                    PropChange::Paint
                }
                _ => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Style => Some(Value::Int(self.tier.to_int())),
            Prop::Value => Some(Value::from(self.radius)),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Group;
        a.label.clone_from(&self.label);
    }
}

/// Makes a glass panel around `children`.
pub fn glass_panel(
    ui: &mut Ui,
    tier: Tier,
    radius: f32,
    padding: f32,
    children: &[ViewId],
) -> ViewId {
    let p = ui.create(GlassPanel::new(tier, radius).padded(padding));
    for c in children {
        ui.add_child(p, *c);
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::amalfi::theme::Theme;
    use crate::style::Blur;

    #[test]
    fn thin_glass_is_lighter_than_regular() {
        let t = Theme::light();
        let thin = glass_style(&t, Tier::Thin);
        let regular = glass_style(&t, Tier::Regular);
        assert_eq!(thin.fills.len(), 1);
        let blur = |s: &Style| match s.blurs.first() {
            Some(Blur::Glass(g)) => g.radius,
            _ => 0.0,
        };
        assert!(blur(&thin) < blur(&regular));
        assert_ne!(thin.fills, regular.fills);
        assert_eq!(thin.shadows, regular.shadows);
        // SThinGlass: the `Glass/Tint Thin` tint over a drop shadow of elevation 2.
        assert_eq!(thin.fills.first(), Some(&t.colors.glass_tint_thin.into()));
    }

    #[test]
    fn reduce_transparency_makes_every_tier_opaque() {
        let mut t = Theme::light();
        t.appearance.reduce_transparency = true;
        for tier in [Tier::Thin, Tier::Regular, Tier::Thick] {
            assert!(glass_style(&t, tier).blurs.is_empty(), "{tier:?}");
        }
    }

    #[test]
    fn a_panel_survives_odd_constraints() {
        let mut ui = Ui::new_headless();
        let inner = ui.create(crate::amalfi::text::Text::new("Hello"));
        let p = glass_panel(&mut ui, Tier::Thick, 24.0, 16.0, &[inner]);
        for (w, h) in [(0.0, 0.0), (1e6, 1e6), (4.0, 4.0)] {
            let s = ui.measure(p, Proposal::new(Some(w), Some(h)));
            assert!(s.width.is_finite() && s.height.is_finite());
        }
    }
}
