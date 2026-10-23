//! Paths: outlines of shapes, arrows, rings and glyph-free drawings, on kurbo's `BezPath`.
//!
//! `PathBuilder` keeps the small builder API the toolkit's shapes are written against
//! (move, line, cubic, arcs, rectangles, ovals); a `Path` is the finished outline, filled
//! with the non-zero rule unless it says even-odd.

use vello_cpu::kurbo::{self, Arc, BezPath, PathEl, Shape as _, SvgArc, Vec2};

use crate::geom::{Point, Rect, Transform};

/// A finished outline.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Path {
    /// The outline.
    pub bez: BezPath,
    /// Even-odd fill (rings, holes); non-zero otherwise.
    pub even_odd: bool,
}

impl Path {
    /// Whether the path has no segments.
    pub fn is_empty(&self) -> bool {
        self.bez.elements().is_empty()
    }

    /// The bounds of the outline (control points included for curves: never smaller).
    pub fn bounds(&self) -> Rect {
        if self.is_empty() {
            return Rect::ZERO;
        }
        let b = self.bez.bounding_box();
        Rect::new(
            b.x0 as f32,
            b.y0 as f32,
            (b.x1 - b.x0) as f32,
            (b.y1 - b.y0) as f32,
        )
    }

    /// The path moved by (dx, dy).
    pub fn make_offset(&self, (dx, dy): (f32, f32)) -> Path {
        let mut bez = self.bez.clone();
        bez.apply_affine(kurbo::Affine::translate((f64::from(dx), f64::from(dy))));
        Path {
            bez,
            even_odd: self.even_odd,
        }
    }

    /// The path through `t`.
    pub fn transformed(&self, t: Transform) -> Path {
        let mut bez = self.bez.clone();
        bez.apply_affine(t.to_affine());
        Path {
            bez,
            even_odd: self.even_odd,
        }
    }

    /// Whether `p` is inside the filled outline.
    pub fn contains(&self, p: Point) -> bool {
        let w = self
            .bez
            .winding(kurbo::Point::new(f64::from(p.x), f64::from(p.y)));
        if self.even_odd { w % 2 != 0 } else { w != 0 }
    }

    /// This outline and `other` as one path (non-zero: their union when both wind the same
    /// way; even-odd: `other` cut out of this one when it lies inside).
    pub fn with(&self, other: &Path, even_odd: bool) -> Path {
        let mut bez = self.bez.clone();
        for el in other.bez.elements() {
            bez.push(*el);
        }
        Path { bez, even_odd }
    }
}

impl Path {
    /// The union of this outline and `other` under the non-zero rule: `other` is reversed
    /// when it winds the other way, so the overlap stays filled.
    pub fn union(&self, other: &Path) -> Path {
        let a = self.bez.area();
        let b = other.bez.area();
        let mut bez = self.bez.clone();
        let o = if a * b < 0.0 {
            other.bez.reverse_subpaths()
        } else {
            other.bez.clone()
        };
        for el in o.elements() {
            bez.push(*el);
        }
        Path {
            bez,
            even_odd: false,
        }
    }
}

/// Builds a [`Path`].
#[derive(Debug, Clone, Default)]
pub struct PathBuilder {
    bez: BezPath,
    current: Option<kurbo::Point>,
    start: Option<kurbo::Point>,
}

fn kp(x: f32, y: f32) -> kurbo::Point {
    kurbo::Point::new(f64::from(x), f64::from(y))
}

impl PathBuilder {
    /// An empty builder.
    pub fn new() -> Self {
        PathBuilder::default()
    }

    /// Starts a contour at (x, y).
    pub fn move_to(&mut self, (x, y): (f32, f32)) -> &mut Self {
        let p = kp(x, y);
        self.bez.move_to(p);
        self.current = Some(p);
        self.start = Some(p);
        self
    }

    fn ensure_started(&mut self, p: kurbo::Point) {
        if self.current.is_none() {
            self.bez.move_to(p);
            self.current = Some(p);
            self.start = Some(p);
        }
    }

    /// A line to (x, y).
    pub fn line_to(&mut self, (x, y): (f32, f32)) -> &mut Self {
        let p = kp(x, y);
        if self.current.is_none() {
            return self.move_to((x, y));
        }
        self.bez.line_to(p);
        self.current = Some(p);
        self
    }

    /// A quadratic Bézier.
    pub fn quad_to(&mut self, c: (f32, f32), e: (f32, f32)) -> &mut Self {
        self.ensure_started(kp(c.0, c.1));
        let end = kp(e.0, e.1);
        self.bez.quad_to(kp(c.0, c.1), end);
        self.current = Some(end);
        self
    }

    /// A cubic Bézier.
    pub fn cubic_to(&mut self, c1: (f32, f32), c2: (f32, f32), e: (f32, f32)) -> &mut Self {
        self.ensure_started(kp(c1.0, c1.1));
        let end = kp(e.0, e.1);
        self.bez.curve_to(kp(c1.0, c1.1), kp(c2.0, c2.1), end);
        self.current = Some(end);
        self
    }

    /// An elliptical arc of radii `r` to `e` (SVG semantics: the small arc, clockwise in
    /// y-down coordinates when `clockwise`).
    pub fn arc_to_radius(&mut self, r: (f32, f32), clockwise: bool, e: (f32, f32)) -> &mut Self {
        let Some(from) = self.current else {
            return self.move_to(e);
        };
        let to = kp(e.0, e.1);
        let arc = SvgArc {
            from,
            to,
            radii: Vec2::new(f64::from(r.0), f64::from(r.1)),
            x_rotation: 0.0,
            large_arc: false,
            sweep: clockwise,
        };
        match Arc::from_svg_arc(&arc) {
            Some(a) => a.to_cubic_beziers(0.05, |c1, c2, p| self.bez.curve_to(c1, c2, p)),
            None => self.bez.line_to(to),
        }
        self.current = Some(to);
        self
    }

    /// The arc of the circle tangent to the lines (current → p1) and (p1 → p2), of radius
    /// `r` (Skia's `arcTo(x1, y1, x2, y2, r)`): a line to the first tangent point, then the
    /// arc to the second.
    pub fn arc_to_tangent(&mut self, p1: (f32, f32), p2: (f32, f32), r: f32) -> &mut Self {
        let Some(p0) = self.current else {
            return self.move_to(p1);
        };
        let p1k = kp(p1.0, p1.1);
        let p2k = kp(p2.0, p2.1);
        let d0 = (p0 - p1k).normalize();
        let d2 = (p2k - p1k).normalize();
        let cos = d0.dot(d2).clamp(-1.0, 1.0);
        let angle = cos.acos();
        if r <= 0.0 || !angle.is_finite() || angle < 1e-6 || (std::f64::consts::PI - angle) < 1e-6 {
            return self.line_to(p1);
        }
        let dist = f64::from(r) / (angle / 2.0).tan();
        let t0 = p1k + d0 * dist;
        let t2 = p1k + d2 * dist;
        self.bez.line_to(t0);
        self.current = Some(t0);
        let cross = d0.x * d2.y - d0.y * d2.x;
        // Turning right (y down) when the cross product is negative: a clockwise arc.
        self.arc_to_radius((r, r), cross < 0.0, (t2.x as f32, t2.y as f32))
    }

    /// A circular arc of the oval in `rect` from `start` degrees sweeping `sweep` degrees
    /// (0 at 3 o'clock, positive clockwise in y-down coordinates), as a new contour unless it
    /// continues the current one.
    pub fn add_arc(&mut self, rect: Rect, start: f32, sweep: f32) -> &mut Self {
        let c = rect.center();
        let arc = Arc::new(
            kp(c.x, c.y),
            Vec2::new(f64::from(rect.width / 2.0), f64::from(rect.height / 2.0)),
            f64::from(start).to_radians(),
            f64::from(sweep).to_radians(),
            0.0,
        );
        let mut first = true;
        for el in arc.path_elements(0.05) {
            match el {
                PathEl::MoveTo(p) => {
                    if first {
                        self.bez.move_to(p);
                        self.start = Some(p);
                    }
                    self.current = Some(p);
                }
                PathEl::CurveTo(a, b, p) => {
                    self.bez.curve_to(a, b, p);
                    self.current = Some(p);
                }
                PathEl::LineTo(p) => {
                    self.bez.line_to(p);
                    self.current = Some(p);
                }
                PathEl::QuadTo(a, p) => {
                    self.bez.quad_to(a, p);
                    self.current = Some(p);
                }
                PathEl::ClosePath => {}
            }
            first = false;
        }
        self
    }

    /// A rectangle as a closed contour (clockwise).
    pub fn add_rect(&mut self, r: Rect) -> &mut Self {
        self.move_to((r.x, r.y));
        self.line_to((r.right(), r.y));
        self.line_to((r.right(), r.bottom()));
        self.line_to((r.x, r.bottom()));
        self.close()
    }

    /// The oval inscribed in `r` as a closed contour (clockwise).
    pub fn add_oval(&mut self, r: Rect) -> &mut Self {
        if r.is_empty() {
            return self;
        }
        let e = kurbo::Ellipse::from_rect(kurbo::Rect::new(
            f64::from(r.x),
            f64::from(r.y),
            f64::from(r.right()),
            f64::from(r.bottom()),
        ));
        for el in e.path_elements(0.05) {
            self.bez.push(el);
        }
        self.current = None;
        self.start = None;
        self
    }

    /// Closes the current contour.
    pub fn close(&mut self) -> &mut Self {
        if self.current.is_some() {
            self.bez.close_path();
        }
        self.current = None;
        self.start = None;
        self
    }

    /// The path built so far; the builder is left empty.
    pub fn detach(&mut self) -> Path {
        self.current = None;
        self.start = None;
        Path {
            bez: std::mem::take(&mut self.bez),
            even_odd: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_and_contains() {
        let mut b = PathBuilder::new();
        b.add_rect(Rect::new(1.0, 2.0, 10.0, 5.0));
        let p = b.detach();
        assert_eq!(p.bounds(), Rect::new(1.0, 2.0, 10.0, 5.0));
        assert!(p.contains(Point::new(5.0, 4.0)));
        assert!(!p.contains(Point::new(0.0, 0.0)));
    }

    #[test]
    fn even_odd_ring() {
        let mut b = PathBuilder::new();
        b.add_rect(Rect::new(0.0, 0.0, 10.0, 10.0));
        let outer = b.detach();
        b.add_rect(Rect::new(3.0, 3.0, 4.0, 4.0));
        let ring = outer.with(&b.detach(), true);
        assert!(ring.contains(Point::new(1.0, 1.0)));
        assert!(!ring.contains(Point::new(5.0, 5.0)));
    }

    #[test]
    fn tangent_arc_rounds_a_corner() {
        let mut b = PathBuilder::new();
        b.move_to((0.0, 0.0));
        b.arc_to_tangent((10.0, 0.0), (10.0, 10.0), 4.0);
        b.line_to((10.0, 10.0));
        let p = b.detach();
        let r = p.bounds();
        assert!((r.right() - 10.0).abs() < 0.01 && (r.bottom() - 10.0).abs() < 0.01);
    }

    #[test]
    fn union_keeps_the_overlap() {
        let mut b = PathBuilder::new();
        b.add_rect(Rect::new(0.0, 0.0, 10.0, 10.0));
        let a = b.detach();
        b.move_to((5.0, 5.0));
        b.line_to((5.0, 15.0));
        b.line_to((15.0, 15.0));
        b.line_to((15.0, 5.0));
        b.close();
        let u = a.union(&b.detach());
        assert!(u.contains(Point::new(7.0, 7.0)));
        assert!(u.contains(Point::new(12.0, 12.0)));
    }

    #[test]
    fn oval_bounds() {
        let mut b = PathBuilder::new();
        b.add_oval(Rect::new(0.0, 0.0, 8.0, 4.0));
        let r = b.detach().bounds();
        assert!((r.width - 8.0).abs() < 0.01 && (r.height - 4.0).abs() < 0.01);
    }
}
