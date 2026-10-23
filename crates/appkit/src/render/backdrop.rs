//! Backdrop filters: the blur, saturation, brightness, refraction and lens of glass, applied
//! to what is already drawn under a coverage mask.
//!
//! The glass maths is Ossido's material pass (`crates/ossido-render/src/glass.rs` and
//! `shaders/material.frag` in silica-desktop), as Vetro's SkSL port had it:
//!
//! - the outline's signed distance is Ossido's `continuous_distance` (the corner a
//!   superellipse of exponent 2 + 2.55 × smoothing over r × (1 + smoothing));
//! - refraction: inside a band of `depth × min(w, h) / 2` from the edge, the backdrop is
//!   sampled further inside along the normal, `distortion × band × (1 − t)²` (t: 0 at the
//!   edge, 1 at the band's inner end), with a chromatic split of `chromatic × band` at the rim;
//! - a lens magnifies around the centre by `magnification`;
//! - saturation and brightness are a colour matrix on Rec. 709 luma weights.
//!
//! The blur is a Gaussian of σ approximated by three box passes, on a copy of the region
//! down-sampled by up to 8× for large σ (the result is sampled bilinearly, which hides the
//! reduction), so its cost does not grow with σ.

use crate::geom::{Point, Rect, Transform};
use crate::style::Glass;

/// What a backdrop does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Backdrop {
    /// Blur σ in device pixels.
    pub sigma: f32,
    /// Saturation.
    pub saturation: f32,
    /// Brightness.
    pub brightness: f32,
    /// Glass refraction: the layer's frame (in the coordinates `to_device` maps from), and
    /// the material.
    pub glass: Option<(Rect, Glass)>,
    /// Layer coordinates → device pixels.
    pub to_device: Transform,
}

/// The colour matrix of a backdrop's saturation and brightness (rows r, g, b of 3 weights).
pub fn saturation_brightness_matrix(saturation: f32, brightness: f32) -> [[f32; 3]; 3] {
    let s = saturation;
    let (lr, lg, lb) = (0.2126 * (1.0 - s), 0.7152 * (1.0 - s), 0.0722 * (1.0 - s));
    let k = brightness;
    [
        [(lr + s) * k, lg * k, lb * k],
        [lr * k, (lg + s) * k, lb * k],
        [lr * k, lg * k, (lb + s) * k],
    ]
}

/// Ossido's signed distance to a continuous rounded rect (negative inside).
pub fn continuous_distance(rect: Rect, radius: f32, smoothing: f32, p: Point) -> f32 {
    let c = rect.center();
    let lx = (p.x - c.x).abs();
    let ly = (p.y - c.y).abs();
    let a = (radius * (1.0 + smoothing)).min(rect.width.min(rect.height) * 0.5);
    let dx = lx - (rect.width * 0.5 - a);
    let dy = ly - (rect.height * 0.5 - a);
    if a > 0.0 && dx > 0.0 && dy > 0.0 {
        let n = 2.0 + 2.55 * smoothing.clamp(0.0, 1.0);
        let qx = dx / a;
        let qy = dy / a;
        let f = (qx.powf(n) + qy.powf(n)).powf(1.0 / n);
        let g = (qx.powf(2.0 * n - 2.0) + qy.powf(2.0 * n - 2.0)).sqrt() / f.powf(n - 1.0);
        return (f - 1.0) * a / g.max(0.001);
    }
    // `sqrt` is an instruction, `hypot` a libm call.
    let (ox, oy) = (dx.max(0.0), dy.max(0.0));
    dx.max(dy).min(0.0) + (ox * ox + oy * oy).sqrt() - a
}

/// A region of premultiplied RGBA as f32, for sampling.
struct Plane {
    w: usize,
    h: usize,
    px: Vec<[f32; 4]>,
}

impl Plane {
    fn get(&self, x: isize, y: isize) -> [f32; 4] {
        if self.w == 0 || self.h == 0 {
            return [0.0; 4];
        }
        let x = x.clamp(0, self.w as isize - 1) as usize;
        let y = y.clamp(0, self.h as isize - 1) as usize;
        self.px.get(y * self.w + x).copied().unwrap_or([0.0; 4])
    }

    /// Bilinear sample at continuous coordinates (pixel centres at +0.5).
    fn sample(&self, x: f32, y: f32) -> [f32; 4] {
        let fx = x - 0.5;
        let fy = y - 0.5;
        let (x0, y0) = (fx.floor_fast(), fy.floor_fast());
        let tx = fx - x0 as f32;
        let ty = fy - y0 as f32;
        let top = lerp(self.get(x0, y0), self.get(x0 + 1, y0), tx);
        let bot = lerp(self.get(x0, y0 + 1), self.get(x0 + 1, y0 + 1), tx);
        lerp(top, bot, ty)
    }
}

/// Floors without libm's `floorf` (a call per use on baseline x86-64).
trait FloorFast {
    fn floor_fast(self) -> isize;
}

impl FloorFast for f32 {
    fn floor_fast(self) -> isize {
        let t = self as isize;
        if (t as f32) > self { t - 1 } else { t }
    }
}

/// `a + (b - a) t`, per channel.
fn lerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let mut out = a;
    for (o, b) in out.iter_mut().zip(b) {
        *o += (b - *o) * t;
    }
    out
}

/// `acc += add - sub`, per channel.
fn slide(acc: &mut [f32; 4], add: [f32; 4], sub: [f32; 4]) {
    for ((a, p), m) in acc.iter_mut().zip(add).zip(sub) {
        *a += p - m;
    }
}

/// One box pass of radius `r` along rows (`horizontal`) or columns, clamped at the edges.
fn box_pass(p: &mut Plane, r: usize, horizontal: bool) {
    if r == 0 || p.w == 0 || p.h == 0 {
        return;
    }
    let (n, lines) = if horizontal { (p.w, p.h) } else { (p.h, p.w) };
    let w = p.w;
    let idx = |line: usize, i: usize| {
        if horizontal {
            line * w + i
        } else {
            i * w + line
        }
    };
    let norm = 1.0 / (2 * r + 1) as f32;
    let mut line_buf = vec![[0.0f32; 4]; n];
    for line in 0..lines {
        for (i, slot) in line_buf.iter_mut().enumerate() {
            *slot = p.px.get(idx(line, i)).copied().unwrap_or([0.0; 4]);
        }
        let at = |i: isize| {
            line_buf
                .get(i.clamp(0, n as isize - 1) as usize)
                .copied()
                .unwrap_or([0.0; 4])
        };
        let mut acc = [0.0f32; 4];
        for k in -(r as isize)..=(r as isize) {
            slide(&mut acc, at(k), [0.0; 4]);
        }
        for i in 0..n {
            if let Some(o) = p.px.get_mut(idx(line, i)) {
                *o = acc.map(|v| v * norm);
            }
            slide(
                &mut acc,
                at(i as isize + r as isize + 1),
                at(i as isize - r as isize),
            );
        }
    }
}

/// Box radii of three passes approximating a Gaussian of σ (W3C filter effects).
fn box_radii(sigma: f32) -> [usize; 3] {
    if sigma.is_nan() || sigma <= 0.0 {
        return [0; 3];
    }
    let d = (sigma * 3.0 * (2.0 * std::f32::consts::PI).sqrt() / 4.0 + 0.5).floor() as usize;
    // An even box of d is approximated by an odd one of d + 1.
    [d / 2; 3]
}

/// Applies `bd` to `target` (premultiplied RGBA8, `width` × `height`) where `mask` covers
/// (one coverage byte per pixel of `region`, row-major), at `opacity`.
pub fn apply(
    target: &mut [u8],
    width: usize,
    height: usize,
    region: (usize, usize, usize, usize),
    mask: &[u8],
    bd: &Backdrop,
    opacity: f32,
) {
    let (rx, ry, rw, rh) = region;
    if rw == 0 || rh == 0 || mask.len() < rw * rh || target.len() < width * height * 4 {
        return;
    }
    // Where samples can come from: the region grown by the blur's reach and the refraction's.
    let refract_reach = bd.glass.map_or(0.0, |(r, g)| {
        let band = g.depth.max(0.0) * r.width.min(r.height) / 2.0;
        let lens = if (g.magnification - 1.0).abs() > 1e-3 {
            r.width.max(r.height)
        } else {
            0.0
        };
        (band * (g.distortion.max(0.0) + g.chromatic_aberration.max(0.0)) + lens)
            * bd.to_device.m[0].abs().max(bd.to_device.m[3].abs())
    });
    let reach = (bd.sigma.max(0.0) * 3.0 + refract_reach).ceil() as usize + 2;
    let sx0 = rx.saturating_sub(reach);
    let sy0 = ry.saturating_sub(reach);
    let sx1 = (rx + rw + reach).min(width);
    let sy1 = (ry + rh + reach).min(height);
    let (sw, sh) = (sx1 - sx0, sy1 - sy0);
    if sw == 0 || sh == 0 {
        return;
    }
    // Down-sample for a large blur.
    let k = if bd.sigma >= 6.0 {
        ((bd.sigma / 3.0) as usize).clamp(1, 8)
    } else {
        1
    };
    let (pw, ph) = (sw.div_ceil(k), sh.div_ceil(k));
    let mut plane = Plane {
        w: pw,
        h: ph,
        px: vec![[0.0; 4]; pw * ph],
    };
    for py in 0..ph {
        for px in 0..pw {
            let mut acc = [0.0f32; 4];
            let mut n = 0.0;
            for yy in 0..k {
                let y = sy0 + py * k + yy;
                if y >= sy1 {
                    break;
                }
                for xx in 0..k {
                    let x = sx0 + px * k + xx;
                    if x >= sx1 {
                        break;
                    }
                    let i = (y * width + x) * 4;
                    if let Some(px) = target.get(i..i + 4) {
                        for (a, v) in acc.iter_mut().zip(px) {
                            *a += f32::from(*v);
                        }
                        n += 1.0;
                    }
                }
            }
            if n > 0.0
                && let Some(slot) = plane.px.get_mut(py * pw + px)
            {
                *slot = acc.map(|v| v / n);
            }
        }
    }
    let radii = box_radii(bd.sigma / k as f32);
    for r in radii {
        box_pass(&mut plane, r, true);
    }
    for r in radii {
        box_pass(&mut plane, r, false);
    }
    let m = saturation_brightness_matrix(bd.saturation, bd.brightness);
    let identity = (bd.saturation - 1.0).abs() < 1e-4 && (bd.brightness - 1.0).abs() < 1e-4;
    let inv = bd.to_device.invert().unwrap_or(Transform::IDENTITY);
    let glass = bd.glass.and_then(|(rect, g)| {
        let band = g.depth.max(0.0) * rect.width.min(rect.height) / 2.0;
        let magnify = g.magnification.is_finite() && (g.magnification - 1.0).abs() > 1e-3;
        ((band > 0.0 && g.distortion > 0.0) || magnify).then_some((rect, g, band))
    });
    let fetch = |dev: Point| -> [f32; 4] {
        plane.sample(
            (dev.x - sx0 as f32) / k as f32,
            (dev.y - sy0 as f32) / k as f32,
        )
    };
    let opacity = opacity.clamp(0.0, 1.0);
    let magnify = glass.is_some_and(|(_, g, _)| (g.magnification - 1.0).abs() > 1e-3);
    // Row `y` of the region into `out` (that target row).
    let paint_row = |y: usize, row: &mut Vec<[f32; 4]>, out: &mut [u8]| {
        let dy = ry + y;
        // The plane interpolated at this row's height once: a sample at a pixel centre is
        // then one lerp across the row.
        let fy = (dy as f32 + 0.5 - sy0 as f32) / k as f32 - 0.5;
        let y0 = fy.floor_fast();
        let ty = fy - y0 as f32;
        row.clear();
        row.extend(
            (0..plane.w)
                .map(|i| lerp(plane.get(i as isize, y0), plane.get(i as isize, y0 + 1), ty)),
        );
        let last = row.len() as isize - 1;
        let at = |dx: usize| -> [f32; 4] {
            let fx = (dx as f32 + 0.5 - sx0 as f32) / k as f32 - 0.5;
            let x0 = fx.floor_fast();
            let pick = |i: isize| {
                row.get(i.clamp(0, last.max(0)) as usize)
                    .copied()
                    .unwrap_or([0.0; 4])
            };
            lerp(pick(x0), pick(x0 + 1), fx - x0 as f32)
        };
        for x in 0..rw {
            let cov = mask.get(y * rw + x).copied().unwrap_or(0);
            if cov == 0 {
                continue;
            }
            let dx = rx + x;
            if dx >= width {
                continue;
            }
            let dev = Point::new(dx as f32 + 0.5, dy as f32 + 0.5);
            let mut s: [f32; 4] = match glass {
                None => at(dx),
                Some((rect, g, band)) => {
                    let p = inv.apply(dev);
                    let c = rect.center();
                    let mag = g.magnification.max(0.01);
                    let q = Point::new(c.x + (p.x - c.x) / mag, c.y + (p.y - c.y) / mag);
                    let radius = rect.width.min(rect.height) / 2.0;
                    let sm = crate::shape::AMALFI_SMOOTHING;
                    let sd = |p: Point| continuous_distance(rect, radius, sm, p);
                    let d = -sd(p);
                    if (band <= 0.0 || d >= band || d < 0.0) && !magnify {
                        at(dx)
                    } else if band <= 0.0 || d >= band || d < 0.0 {
                        fetch(bd.to_device.apply(q))
                    } else {
                        let e = 0.5;
                        let mut n = Point::new(
                            sd(Point::new(p.x + e, p.y)) - sd(Point::new(p.x - e, p.y)),
                            sd(Point::new(p.x, p.y + e)) - sd(Point::new(p.x, p.y - e)),
                        );
                        let ln = (n.x * n.x + n.y * n.y).sqrt();
                        n = if ln > 1e-4 {
                            Point::new(n.x / ln, n.y / ln)
                        } else {
                            Point::ZERO
                        };
                        let t = d / band;
                        let shift = g.distortion.max(0.0) * band * (1.0 - t) * (1.0 - t);
                        let sp = Point::new(q.x - n.x * shift, q.y - n.y * shift);
                        let split = g.chromatic_aberration.max(0.0) * band * (1.0 - t);
                        let mid = fetch(bd.to_device.apply(sp));
                        if split > 0.0 {
                            let r = fetch(
                                bd.to_device
                                    .apply(Point::new(sp.x + n.x * split, sp.y + n.y * split)),
                            )[0];
                            let b = fetch(
                                bd.to_device
                                    .apply(Point::new(sp.x - n.x * split, sp.y - n.y * split)),
                            )[2];
                            [r.min(mid[3]), mid[1], b.min(mid[3]), mid[3]]
                        } else {
                            mid
                        }
                    }
                }
            };
            if !identity {
                let [r, g, b, a] = s;
                let [m0, m1, m2] = m.map(|[x, y, z]| (x * r + y * g + z * b).min(a).max(0.0));
                s = [m0, m1, m2, a];
            }
            // Source-over of the filtered backdrop, weighted by coverage and opacity.
            let w = f32::from(cov) / 255.0 * opacity;
            let sa = s[3] * w / 255.0;
            if let Some(px) = out.get_mut(dx * 4..dx * 4 + 4) {
                for (d, v) in px.iter_mut().zip(s) {
                    // `as` saturates: rounded and clamped without libm's `roundf`.
                    *d = (v * w + f32::from(*d) * (1.0 - sa) + 0.5) as u8;
                }
            }
        }
    };
    let rh = rh.min(height.saturating_sub(ry));
    let Some(rows) = target.get_mut(ry * width * 4..(ry + rh) * width * 4) else {
        return;
    };
    // Big regions in bands on threads: the rows are independent.
    let bands = if rw * rh >= 1 << 15 {
        std::thread::available_parallelism().map_or(1, |n| n.get().min(8))
    } else {
        1
    };
    // Chunks of 4 rows dealt round-robin: the refracting rims sit at the top and bottom.
    let per = 4;
    let band = |first: usize, part: &mut [u8]| {
        let mut row = Vec::with_capacity(plane.w);
        for (i, out) in part.chunks_exact_mut(width * 4).enumerate() {
            paint_row(first + i, &mut row, out);
        }
    };
    if bands == 1 {
        band(0, rows);
        return;
    }
    let mut deals: Vec<Vec<(usize, &mut [u8])>> = (0..bands).map(|_| Vec::new()).collect();
    for (i, part) in rows.chunks_mut(per * width * 4).enumerate() {
        if let Some(d) = deals.get_mut(i % bands) {
            d.push((i * per, part));
        }
    }
    std::thread::scope(|scope| {
        for deal in deals {
            let band = &band;
            scope.spawn(move || {
                for (first, part) in deal {
                    band(first, part);
                }
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blur_softens_an_edge() {
        let (w, h) = (40usize, 10usize);
        let mut t = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 20..w {
                let i = (y * w + x) * 4;
                t[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
            }
        }
        let mask = vec![255u8; w * h];
        let bd = Backdrop {
            sigma: 3.0,
            saturation: 1.0,
            brightness: 1.0,
            glass: None,
            to_device: Transform::IDENTITY,
        };
        apply(&mut t, w, h, (0, 0, w, h), &mask, &bd, 1.0);
        let a = |x: usize| t[(5 * w + x) * 4 + 3];
        assert!(a(18) > 0 && a(18) < 255);
        assert!(a(2) < 5);
        assert!(a(38) > 250);
    }

    #[test]
    fn saturation_zero_is_grey() {
        let m = saturation_brightness_matrix(0.0, 1.0);
        let r = m[0][0] * 255.0;
        let g = m[1][0] * 255.0;
        assert!((r - g).abs() < 1e-3);
    }

    #[test]
    fn distance_sign() {
        let r = Rect::new(0.0, 0.0, 40.0, 20.0);
        assert!(continuous_distance(r, 10.0, 0.6, Point::new(20.0, 10.0)) < 0.0);
        assert!(continuous_distance(r, 10.0, 0.6, Point::new(-2.0, 10.0)) > 0.0);
    }
}
