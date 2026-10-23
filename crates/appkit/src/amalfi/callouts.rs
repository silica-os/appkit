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
use crate::shape::{AMALFI_SMOOTHING, Shape};

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
            radius: crate::amalfi::tokens::radii::POPOVER,
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

/// A closed polygon with a radius at each vertex and the kit's smooth corners (ui-kit
/// `smoothPath.js`), `shift` added to every point.
///
/// Each rounded vertex is an arc that turns φ(1 − s) (φ: how much the outline turns there),
/// joined to both sides by a cubic Bézier, and takes L = (1 + s)·r·tan(φ/2) of each side, at
/// any angle (the arrow's base and tip too). Two vertices that need more than the side
/// between them both scale down by side / (Lᵢ + Lⱼ); a vertex then gives up smoothing first
/// and radius last.
pub(crate) fn rounded_polygon(v: &[(f32, f32, f32)], shift: (f32, f32)) -> Path {
    let mut b = PathBuilder::new();
    let n = v.len();
    if n < 3 {
        return b.detach();
    }
    let sh = |(x, y): (f32, f32)| (x + shift.0, y + shift.1);
    for (i, c) in corners(v, AMALFI_SMOOTHING).iter().enumerate() {
        let Some(c) = c else {
            let (x, y, _) = v.get(i).copied().unwrap_or_default();
            if i == 0 {
                b.move_to(sh((x, y)));
            } else {
                b.line_to(sh((x, y)));
            }
            continue;
        };
        if i == 0 {
            b.move_to(sh(c.start));
        } else {
            b.line_to(sh(c.start));
        }
        match c.curves {
            Some([p1, p2, p3, q0, q1, q2]) => {
                b.cubic_to(sh(p1), sh(p2), sh(p3));
                b.arc_to_radius((c.radius, c.radius), c.clockwise, sh(q0));
                b.cubic_to(sh(q1), sh(q2), sh(c.end));
            }
            None => {
                b.arc_to_radius((c.radius, c.radius), c.clockwise, sh(c.end));
            }
        }
    }
    b.close();
    b.detach()
}

/// One rounded vertex of [`rounded_polygon`].
#[derive(Debug, Clone, Copy)]
struct Corner {
    /// Where the corner leaves the incoming side, and where it joins the outgoing one.
    start: (f32, f32),
    end: (f32, f32),
    /// The radius and smoothing left after fitting.
    radius: f32,
    smoothing: f32,
    clockwise: bool,
    /// The Béziers' points: the first's two controls and end, then the second's start
    /// (the arc's end) and two controls; `None` for a plain arc.
    curves: Option<[(f32, f32); 6]>,
}

/// The rounded vertices of a polygon (`None` keeps a sharp point).
fn corners(v: &[(f32, f32, f32)], smoothing: f32) -> Vec<Option<Corner>> {
    let n = v.len();
    let s = smoothing.clamp(0.0, 1.0);
    let at = |i: usize| v.get(i % n).copied().unwrap_or_default();
    // Per vertex: unit incoming direction, turn angle φ, turn sign, tan(φ/2), the outgoing
    // side's length, and the tangent length it asks for.
    struct G {
        dir: (f32, f32),
        phi: f32,
        cross: f32,
        t: f32,
        out: f32,
        want: f32,
    }
    let g: Vec<G> = (0..n)
        .map(|i| {
            let (x, y, r) = at(i);
            let (px, py, _) = at(i + n - 1);
            let (nx, ny, _) = at(i + 1);
            let (ix, iy, ox, oy) = (x - px, y - py, nx - x, ny - y);
            let (li, lo) = (ix.hypot(iy), ox.hypot(oy));
            if li <= f32::EPSILON || lo <= f32::EPSILON {
                return G {
                    dir: (0.0, 0.0),
                    phi: 0.0,
                    cross: 0.0,
                    t: 0.0,
                    out: lo,
                    want: 0.0,
                };
            }
            let (ix, iy, ox, oy) = (ix / li, iy / li, ox / lo, oy / lo);
            let phi = (ix * ox + iy * oy).clamp(-1.0, 1.0).acos();
            let cross = ix * oy - iy * ox;
            let t = (phi / 2.0).tan();
            let ok = r > 0.0 && phi > 1e-4 && phi < std::f32::consts::PI - 1e-3 && cross != 0.0;
            let want = if ok { (1.0 + s) * r * t } else { 0.0 };
            G {
                dir: (ix, iy),
                phi,
                cross,
                t,
                out: lo,
                want,
            }
        })
        .collect();
    let mut fit = vec![1.0_f32; n];
    for i in 0..n {
        let j = (i + 1) % n;
        let (Some(a), Some(b)) = (g.get(i), g.get(j)) else {
            continue;
        };
        let need = a.want + b.want;
        if need > a.out && need > 0.0 {
            let k = a.out / need;
            for m in [i, j] {
                if let Some(f) = fit.get_mut(m) {
                    *f = f.min(k);
                }
            }
        }
    }
    g.iter()
        .enumerate()
        .map(|(i, p)| {
            if p.want <= 0.0 {
                return None;
            }
            let (x, y, r0) = at(i);
            let budget = p.want * fit.get(i).copied().unwrap_or(1.0);
            let (mut xi, mut r) = (s, r0);
            if (1.0 + xi) * r * p.t > budget {
                xi = (budget / (r * p.t) - 1.0).max(0.0);
                if xi == 0.0 && r * p.t > budget {
                    r = budget / p.t;
                }
            }
            let l = (1.0 + xi) * r * p.t;
            // Local frame: origin at the corner's start, u along the incoming side, v towards
            // the side the outline turns to.
            let sg = p.cross.signum();
            let (ex, ey) = p.dir;
            let (nx, ny) = (-ey * sg, ex * sg);
            let (sx, sy) = (x - ex * l, y - ey * l);
            let w = |(u, v): (f32, f32)| (sx + u * ex + v * nx, sy + u * ey + v * ny);
            let end = w((l + l * p.phi.cos(), l * p.phi.sin()));
            let mut c = Corner {
                start: (sx, sy),
                end,
                radius: r,
                smoothing: xi,
                clockwise: p.cross > 0.0,
                curves: None,
            };
            if xi < 1e-3 {
                return Some(c);
            }
            let beta = p.phi * xi / 2.0;
            let k = r * (p.phi * (1.0 - xi) / 2.0).tan();
            let h = r * (beta / 2.0).tan();
            let (cc, dd) = (h * beta.cos(), h * beta.sin());
            let (sh, ch) = ((p.phi / 2.0).sin(), (p.phi / 2.0).cos());
            let m = (dd + k * beta.sin()) / ch;
            let x1 = l - m * sh - k * beta.cos();
            let b = (x1 - cc) / 3.0;
            if b.is_nan() || b < 0.0 {
                c.smoothing = 0.0;
                return Some(c);
            }
            let a = 2.0 * b;
            // The second Bézier is the first mirrored across the bisector through (l, 0).
            let refl = |(u, v): (f32, f32)| {
                let du = u - l;
                let dot = du * -sh + v * ch;
                (l + 2.0 * dot * -sh - du, 2.0 * dot * ch - v)
            };
            let (p1, p2, p3) = ((a, 0.0), (a + b, 0.0), (x1, dd));
            c.curves = Some([w(p1), w(p2), w(p3), w(refl(p3)), w(refl(p2)), w(refl(p1))]);
            Some(c)
        })
        .collect()
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
            (Prop::Style, Some(n)) => match Tier::from_int(n) {
                Some(t) => {
                    self.tier = t;
                    PropChange::Paint
                }
                None => PropChange::Unknown,
            },
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
    fn corners_give_up_smoothing_before_radius() {
        // A rectangle's 90 degree corner takes (1 + 0.6) x 16 of each side.
        let rect = [
            (0.0, 0.0, 16.0),
            (200.0, 0.0, 16.0),
            (200.0, 100.0, 16.0),
            (0.0, 100.0, 16.0),
        ];
        let c = corners(&rect, AMALFI_SMOOTHING);
        let first = c
            .first()
            .copied()
            .flatten()
            .map(|c| (c.start, c.radius, c.smoothing));
        assert_eq!(first, Some(((0.0, 25.6), 16.0, 0.6)));
        // ui-kit's case: a 16 corner 20 from the arrow's base (r 4) keeps its radius and
        // lowers its smoothing to about 0.12.
        let v = [
            (0.0, 0.0, 16.0),
            (100.0, 0.0, 16.0),
            (100.0, 60.0, 16.0),
            (0.0, 60.0, 16.0),
            (0.0, 40.0, 4.0),
            (-8.0, 30.0, 3.0),
            (0.0, 20.0, 4.0),
        ];
        let c = corners(&v, AMALFI_SMOOTHING);
        let corner = c
            .first()
            .copied()
            .flatten()
            .map(|c| (c.radius, c.smoothing));
        let Some((r, s)) = corner else {
            panic!("a corner");
        };
        assert!((r - 16.0).abs() < 1e-3, "radius {r}");
        assert!(s > 0.05 && s < 0.2, "smoothing {s}");
        let far = c.get(1).copied().flatten().map(|c| c.smoothing);
        assert_eq!(far, Some(0.6));
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
