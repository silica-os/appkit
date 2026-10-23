//! Silica motion runtime: springs, cubic-bezier fades and animation tracks.
//!
//! No dependencies. The maths is the same as `src/motion.js` in this repo (the Animation Lab),
//! so what you tune in Storybook gives the same numbers here. Time is in milliseconds.
//!
//! ```ignore
//! let scale = animations::MY_ANIMATION.value(Prop::Scale, elapsed_ms);
//! ```

#[derive(Clone, Copy, Debug)]
pub struct Spring {
    pub response: f32,
    pub damping_ratio: f32,
}

impl Spring {
    pub const fn new(response: f32, damping_ratio: f32) -> Self {
        Spring { response, damping_ratio }
    }

    /// Mass 1: k = (2π / response)², c = 2 ζ √k.
    pub fn stiffness(&self) -> f32 {
        let w = core::f32::consts::TAU / self.response;
        w * w
    }

    pub fn damping(&self) -> f32 {
        2.0 * self.damping_ratio * self.stiffness().sqrt()
    }

    /// Progress 0 -> 1 after `ms`, released from rest.
    pub fn progress(&self, ms: f32) -> f32 {
        let t = ms / 1000.0;
        if t <= 0.0 {
            return 0.0;
        }
        let z = self.damping_ratio as f64;
        let w = (core::f64::consts::TAU) / self.response as f64;
        let t = t as f64;
        let p = if z < 1.0 {
            let wd = w * (1.0 - z * z).sqrt();
            1.0 - (-z * w * t).exp() * ((wd * t).cos() + (z * w / wd) * (wd * t).sin())
        } else if z == 1.0 {
            1.0 - (-w * t).exp() * (1.0 + w * t)
        } else {
            let s = w * (z * z - 1.0).sqrt();
            let (r1, r2) = (-z * w + s, -z * w - s);
            1.0 - (r2 * (r1 * t).exp() - r1 * (r2 * t).exp()) / (r2 - r1)
        };
        p as f32
    }

    /// Time after which the value stays within 0.5 % of the target.
    pub fn settle_ms(&self) -> f32 {
        let mut last = 0.0;
        let mut ms = 0.0;
        while ms <= 4000.0 {
            if (self.progress(ms) - 1.0).abs() > 0.005 {
                last = ms;
            }
            ms += 2.0;
        }
        last + 2.0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Fade {
    pub duration_ms: u32,
    pub curve: [f32; 4],
}

/// CSS `cubic-bezier(x1, y1, x2, y2)`: progress for a time fraction in 0..=1.
pub fn bezier_progress(x: f32, c: [f32; 4]) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let [x1, y1, x2, y2] = c;
    let (cx, cy) = (3.0 * x1, 3.0 * y1);
    let (bx, by) = (3.0 * (x2 - x1) - cx, 3.0 * (y2 - y1) - cy);
    let (ax, ay) = (1.0 - cx - bx, 1.0 - cy - by);
    let sx = |t: f32| ((ax * t + bx) * t + cx) * t;
    let sy = |t: f32| ((ay * t + by) * t + cy) * t;
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    let mut t = x;
    for _ in 0..40 {
        let e = sx(t);
        if (e - x).abs() < 1e-6 {
            break;
        }
        if x > e {
            lo = t;
        } else {
            hi = t;
        }
        t = (hi + lo) / 2.0;
    }
    sy(t)
}

impl Fade {
    pub fn progress(&self, ms: f32) -> f32 {
        bezier_progress(ms / self.duration_ms as f32, self.curve)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Driver {
    Spring(Spring),
    Fade(Fade),
    Bezier { duration_ms: u32, curve: [f32; 4] },
}

impl Driver {
    pub fn progress(&self, ms: f32) -> f32 {
        match *self {
            Driver::Spring(s) => s.progress(ms),
            Driver::Fade(f) => f.progress(ms),
            Driver::Bezier { duration_ms, curve } => bezier_progress(ms / duration_ms as f32, curve),
        }
    }

    /// When the animation can stop being ticked.
    pub fn end_ms(&self) -> f32 {
        match *self {
            Driver::Spring(s) => s.settle_ms(),
            Driver::Fade(f) => f.duration_ms as f32,
            Driver::Bezier { duration_ms, .. } => duration_ms as f32,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prop {
    TranslateX,
    TranslateY,
    Scale,
    Rotate,
    Opacity,
}

#[derive(Clone, Copy, Debug)]
pub struct Track {
    pub prop: Prop,
    pub from: f32,
    pub to: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Animation {
    pub name: &'static str,
    pub delay_ms: u32,
    pub driver: Driver,
    pub tracks: &'static [Track],
}

impl Animation {
    /// Value of `prop` at `elapsed_ms` since the animation was started (delay included).
    /// A property without a track returns `None`.
    pub fn value(&self, prop: Prop, elapsed_ms: f32) -> Option<f32> {
        let t = self.tracks.iter().find(|t| t.prop == prop)?;
        let p = self.driver.progress(elapsed_ms - self.delay_ms as f32);
        Some(t.from + (t.to - t.from) * p)
    }

    pub fn is_finished(&self, elapsed_ms: f32) -> bool {
        elapsed_ms >= self.delay_ms as f32 + self.driver.end_ms()
    }
}
