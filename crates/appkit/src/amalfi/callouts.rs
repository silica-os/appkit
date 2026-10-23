//! Callouts (ui-kit `SCallout` and `calloutPath.js`): a glass bubble whose edge
//! flows into an arrow with rounded transitions, drawn as ONE outline. The outline is a polygon
//! (the body's four corners and the arrow's base, tip and base) with a radius at every vertex:
//! 16 at the corners, 4 at the arrow's base and 3 at its tip; every measure can be changed.

use crate::amalfi::materials::{Tier, glass_style};
use crate::amalfi::popovers::{Align, Edge};
use crate::core::a11y::{A11y, Role};
use crate::core::layout::Proposal;
use crate::core::tree::ViewId;
use crate::core::ui::Ui;
use crate::core::value::{Prop, Value};
use crate::core::widget::{LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Point, Rect, Size};
use crate::render::path::{Path, PathBuilder};
use crate::shape::Shape;

/// The outline of a callout: a rounded rectangle and, unless `edge` is `None`, an arrow.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalloutShape {
    /// The edge that carries the arrow (it adds `arrow_height` on that side).
    pub edge: Option<Edge>,
    /// Where the arrow sits along its edge.
    pub align: Align,
    /// The tip's distance from the start of the edge (overrides `align`).
    pub offset: Option<f32>,
    /// The corner radius.
    pub radius: f32,
    /// The arrow's width at its base.
    pub arrow_width: f32,
    /// The arrow's height.
    pub arrow_height: f32,
    /// The radius at the arrow's base.
    pub base_radius: f32,
    /// The radius at the arrow's tip.
    pub tip_radius: f32,
}

impl CalloutShape {
    /// The popover-like callout: radius 16, a 20 x 8 arrow with radii 4 and 3.
    pub fn new(edge: Option<Edge>, align: Align) -> CalloutShape {
        CalloutShape {
            edge,
            align,
            offset: None,
            radius: 16.0,
            arrow_width: 20.0,
            arrow_height: 8.0,
            base_radius: 4.0,
            tip_radius: 3.0,
        }
    }

    /// With another corner radius.
    pub fn radius(mut self, radius: f32) -> CalloutShape {
        self.radius = radius;
        self
    }

    /// The tip at `offset` from the start of its edge.
    pub fn at(mut self, offset: f32) -> CalloutShape {
        self.offset = Some(offset);
        self
    }

    fn depth(&self) -> f32 {
        if self.edge.is_some() {
            self.arrow_height.max(0.0)
        } else {
            0.0
        }
    }

    /// The space the arrow adds on each side: left, top, right, bottom.
    pub fn insets(&self) -> [f32; 4] {
        let d = self.depth();
        match self.edge {
            None => [0.0; 4],
            Some(Edge::Top) => [0.0, d, 0.0, 0.0],
            Some(Edge::Bottom) => [0.0, 0.0, 0.0, d],
            Some(Edge::Leading) => [d, 0.0, 0.0, 0.0],
            Some(Edge::Trailing) => [0.0, 0.0, d, 0.0],
        }
    }

    /// The body's rectangle in `rect`.
    pub fn body(&self, rect: Rect) -> Rect {
        let [l, t, r, b] = self.insets();
        Rect::new(
            rect.x + l,
            rect.y + t,
            (rect.width - l - r).max(0.0),
            (rect.height - t - b).max(0.0),
        )
    }

    /// The tip's position along its edge, kept clear of the corners (`calloutPath.js`: the
    /// corner radius plus half the base, or the middle when the edge is too short).
    fn along(&self, len: f32) -> f32 {
        let from_start = self.offset.unwrap_or(match self.align {
            Align::Start => 30.0,
            Align::Center => len / 2.0,
            Align::End => len - 30.0,
        });
        let lo = self.radius + self.arrow_width / 2.0;
        if len < 2.0 * lo {
            len / 2.0
        } else {
            from_start.clamp(lo, len - lo)
        }
    }

    /// The arrow's tip in `rect` (`None` without an arrow).
    pub fn tip(&self, rect: Rect) -> Option<Point> {
        let edge = self.edge?;
        let b = self.body(rect);
        let horizontal = matches!(edge, Edge::Top | Edge::Bottom);
        let c = self.along(if horizontal { b.width } else { b.height });
        Some(match edge {
            Edge::Top => Point::new(b.x + c, rect.y),
            Edge::Bottom => Point::new(b.x + c, rect.bottom()),
            Edge::Leading => Point::new(rect.x, b.y + c),
            Edge::Trailing => Point::new(rect.right(), b.y + c),
        })
    }

    /// The closed outline in `rect` (empty for an empty rect).
    pub fn path(&self, rect: Rect) -> Path {
        if rect.width <= 0.0 || rect.height <= 0.0 {
            return Path::default();
        }
        let b = self.body(rect);
        let r = self.radius.max(0.0).min(b.width / 2.0).min(b.height / 2.0);
        let (x0, y0, x1, y1) = (b.x, b.y, b.right(), b.bottom());
        let (hw, ah) = (self.arrow_width / 2.0, self.depth());
        let horizontal = matches!(self.edge, Some(Edge::Top | Edge::Bottom));
        let c = self.along(if horizontal { b.width } else { b.height });
        let (ar, tr) = (self.base_radius, self.tip_radius);
        // Clockwise from the top left corner: (x, y, radius).
        let mut v: Vec<(f32, f32, f32)> = vec![(x0, y0, r)];
        if self.edge == Some(Edge::Top) {
            v.extend([
                (x0 + c - hw, y0, ar),
                (x0 + c, y0 - ah, tr),
                (x0 + c + hw, y0, ar),
            ]);
        }
        v.push((x1, y0, r));
        if self.edge == Some(Edge::Trailing) {
            v.extend([
                (x1, y0 + c - hw, ar),
                (x1 + ah, y0 + c, tr),
                (x1, y0 + c + hw, ar),
            ]);
        }
        v.push((x1, y1, r));
        if self.edge == Some(Edge::Bottom) {
            v.extend([
                (x0 + c + hw, y1, ar),
                (x0 + c, y1 + ah, tr),
                (x0 + c - hw, y1, ar),
            ]);
        }
        v.push((x0, y1, r));
        if self.edge == Some(Edge::Leading) {
            v.extend([
                (x0, y0 + c + hw, ar),
                (x0 - ah, y0 + c, tr),
                (x0, y0 + c - hw, ar),
            ]);
        }
        rounded_polygon(&v, (-rect.x, -rect.y))
    }
}

/// A closed polygon with a radius at each vertex (clamped to half the shorter neighbouring
/// side), `shift` added to every point.
fn rounded_polygon(v: &[(f32, f32, f32)], shift: (f32, f32)) -> Path {
    let n = v.len();
    let mut b = PathBuilder::new();
    if n < 3 {
        return b.detach();
    }
    let at = |i: usize| v.get(i % n).copied().unwrap_or((0.0, 0.0, 0.0));
    // Each vertex: the point where its arc starts, and the radius that fits.
    let mut starts = Vec::with_capacity(n);
    let mut radii = Vec::with_capacity(n);
    for i in 0..n {
        let (x, y, r) = at(i);
        let (px, py, _) = at(i + n - 1);
        let (nx, ny, _) = at(i + 1);
        let (ax, ay) = (px - x, py - y);
        let (bx, by) = (nx - x, ny - y);
        let (la, lb) = (ax.hypot(ay), bx.hypot(by));
        if la <= f32::EPSILON || lb <= f32::EPSILON {
            starts.push((x, y));
            radii.push(0.0);
            continue;
        }
        let cos = ((ax * bx + ay * by) / (la * lb)).clamp(-1.0, 1.0);
        let tn = (cos.acos() / 2.0).tan();
        let mut d = if tn.abs() > 1e-6 { r / tn } else { 0.0 };
        let mut rr = r;
        let dmax = la.min(lb) / 2.0;
        if d > dmax {
            d = dmax;
            rr = d * tn;
        }
        starts.push((x + ax / la * d, y + ay / la * d));
        radii.push(if r > 0.0 { rr } else { 0.0 });
    }
    let sh = |p: (f32, f32)| (p.0 + shift.0, p.1 + shift.1);
    for i in 0..n {
        let (x, y, _) = at(i);
        let rr = radii.get(i).copied().unwrap_or(0.0);
        let s = starts.get(i).copied().unwrap_or((x, y));
        if i == 0 {
            b.move_to(sh(s));
        } else {
            b.line_to(sh(s));
        }
        let (nx, ny, _) = at(i + 1);
        if rr > 0.01 {
            b.arc_to_tangent(sh((x, y)), sh((nx, ny)), rr);
        } else {
            b.line_to(sh((x, y)));
        }
    }
    b.close();
    b.detach()
}

/// A callout: glass in the outline of its [`CalloutShape`], its children inside `padding`
/// (and the arrow's side).
#[derive(Debug, Clone)]
pub struct Callout {
    /// The outline.
    pub shape: CalloutShape,
    /// The glass: Thick for menus and labels, Regular for the thinner control glass.
    pub tier: Tier,
    /// The space between the body's edge and the content.
    pub padding: f32,
    /// The surface layer's name.
    pub layer: String,
    /// Whether the callout takes the pointer (a menu does, a label does not).
    pub interactive: bool,
}

impl Callout {
    /// A thick-glass callout with `shape` and no padding.
    pub fn new(shape: CalloutShape) -> Callout {
        Callout {
            shape,
            tier: Tier::Thick,
            padding: 0.0,
            layer: "Surface".to_string(),
            interactive: true,
        }
    }

    /// A callout the pointer goes through (a label).
    pub fn passive(mut self) -> Callout {
        self.interactive = false;
        self
    }

    /// With a padding.
    pub fn padded(mut self, padding: f32) -> Callout {
        self.padding = padding;
        self
    }

    /// With the thinner control glass.
    pub fn thin(mut self) -> Callout {
        self.tier = Tier::Regular;
        self
    }
}

impl Widget for Callout {
    fn kind(&self) -> &'static str {
        "Callout"
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        self.interactive
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let [l, t, r, b] = self.shape.insets();
        let pad = self.padding.max(0.0);
        let (ew, eh) = (l + r + 2.0 * pad, t + b + 2.0 * pad);
        let inner = Proposal::new(
            p.width.filter(|w| w.is_finite()).map(|w| (w - ew).max(0.0)),
            p.height
                .filter(|h| h.is_finite())
                .map(|h| (h - eh).max(0.0)),
        );
        let mut size = Size::ZERO;
        for c in cx.children() {
            let s = cx.measure(c, inner);
            size.width = size.width.max(s.width);
            size.height = size.height.max(s.height);
        }
        Size::new(size.width + ew, size.height + eh)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        let body = self.shape.body(Rect::from_size(size));
        let pad = self.padding.max(0.0);
        let w = (body.width - 2.0 * pad).max(0.0);
        let h = (body.height - 2.0 * pad).max(0.0);
        for c in cx.children() {
            let s = cx.measure(c, Proposal::new(Some(w), Some(h)));
            cx.place(
                c,
                Rect::new(body.x + pad, body.y + pad, s.width.min(w), s.height.min(h)),
            );
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let r = cx.bounds();
        let style = glass_style(&theme, self.tier);
        let path = self.shape.path(Rect::from_size(r.size()));
        let shape = if self.shape.edge.is_some() {
            Shape::Path(path)
        } else {
            Shape::rounded(self.shape.radius)
        };
        cx.rec.layer(&self.layer, r, shape, style);
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match (prop, value.as_int()) {
            (Prop::Edge, Some(n)) => {
                self.shape.edge = match n {
                    0 => Some(Edge::Top),
                    1 => Some(Edge::Bottom),
                    2 => Some(Edge::Leading),
                    3 => Some(Edge::Trailing),
                    _ => None,
                };
                PropChange::Layout
            }
            _ => PropChange::Unknown,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Group;
        a.hidden = !self.interactive;
    }
}

/// Makes a callout around `content`.
pub fn callout(ui: &mut Ui, callout: Callout, content: ViewId) -> ViewId {
    let c = ui.create(callout);
    ui.add_child(c, content);
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arrow_adds_its_height_on_its_side() {
        let s = CalloutShape::new(Some(Edge::Top), Align::Start);
        let rect = Rect::new(0.0, 0.0, 260.0, 128.0);
        assert_eq!(s.body(rect), Rect::new(0.0, 8.0, 260.0, 120.0));
        assert_eq!(s.tip(rect), Some(Point::new(30.0, 0.0)));
        let mut d = CalloutShape::new(Some(Edge::Bottom), Align::Center).radius(8.0);
        d.arrow_width = 12.0;
        d.arrow_height = 6.0;
        d.base_radius = 3.0;
        d.tip_radius = 2.0;
        let label = Rect::new(0.0, 0.0, 64.0, 34.0);
        assert_eq!(d.body(label), Rect::new(0.0, 0.0, 64.0, 28.0));
        assert_eq!(d.tip(label), Some(Point::new(32.0, 34.0)));
    }

    #[test]
    fn the_tip_stays_clear_of_the_corners() {
        let s = CalloutShape::new(Some(Edge::Bottom), Align::Center).at(2.0);
        let rect = Rect::new(0.0, 0.0, 260.0, 128.0);
        assert_eq!(s.tip(rect), Some(Point::new(26.0, 128.0)));
    }

    #[test]
    fn the_outline_reaches_the_tip_and_stays_in_the_frame() {
        let rect = Rect::new(0.0, 0.0, 64.0, 34.0);
        let mut s = CalloutShape::new(Some(Edge::Bottom), Align::Center).radius(8.0);
        s.arrow_width = 12.0;
        s.arrow_height = 6.0;
        s.base_radius = 3.0;
        s.tip_radius = 2.0;
        let b = s.path(rect).bounds();

        assert!(b.x >= -0.01 && b.right() <= 64.01 && b.y >= -0.01, "{b:?}");
        // The tip is rounded (radius 2): the outline stops short of the sharp point.
        assert!(b.bottom() > 33.0 && b.bottom() <= 34.0 + 0.01);
    }

    #[test]
    fn odd_sizes_give_a_path_without_panicking() {
        for (w, h) in [(0.0, 0.0), (5.0, 5.0), (1e5, 3.0), (30.0, 1e5)] {
            for e in [
                None,
                Some(Edge::Top),
                Some(Edge::Leading),
                Some(Edge::Bottom),
            ] {
                let p = CalloutShape::new(e, Align::Center).path(Rect::new(0.0, 0.0, w, h));
                let b = p.bounds();
                assert!(b.width.is_finite() && b.height.is_finite());
            }
        }
    }

    #[test]
    fn a_label_does_not_take_the_pointer() {
        let shape = CalloutShape::new(None, Align::Center);
        assert!(
            !Callout::new(shape)
                .passive()
                .hit(Point::new(1.0, 1.0), Size::new(64.0, 34.0))
        );
        assert!(Callout::new(shape).hit(Point::new(1.0, 1.0), Size::new(64.0, 34.0)));
    }
}
