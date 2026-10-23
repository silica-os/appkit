//! The touchpad probe: a panel that shows the touchpad events it receives live (the scroll
//! phase and deltas, the velocity of the last 100 ms, pinch, swipe, hold and navigation
//! swipes). It exists for the gallery and for people trying a touchpad.

use crate::amalfi::foundation::Label;
use crate::amalfi::tokens::typography;
use crate::core::a11y::{A11y, Role};
use crate::core::event::{Event, Handled, Phase};
use crate::core::layout::Proposal;
use crate::core::widget::{EventCx, LayoutCx, PaintCx, Widget};
use crate::geom::{Point, Rect, Size};
use crate::shape::Shape;
use crate::style::Style;
use crate::text::Align;

/// The probe's size.
pub const SIZE: Size = Size {
    width: 420.0,
    height: 168.0,
};

/// Shows touchpad events as they arrive.
#[derive(Debug, Default)]
pub struct TouchProbe {
    lines: [String; 6],
    samples: Vec<(f64, Point)>,
    pinch: f32,
    navigations: u32,
    caches: [Label; 6],
}

impl TouchProbe {
    /// A probe that has seen nothing.
    pub fn new() -> TouchProbe {
        let mut p = TouchProbe::default();
        p.reset();
        p
    }

    fn reset(&mut self) {
        self.lines = [
            "Scroll  none".to_string(),
            "Velocity  0, 0 pt/s".to_string(),
            "Pinch  1.00".to_string(),
            "Swipe  none".to_string(),
            "Hold  none".to_string(),
            "Navigate  none".to_string(),
        ];
        self.pinch = 1.0;
    }

    /// The text of the line `i` (for tests and assistive technologies).
    pub fn line(&self, i: usize) -> &str {
        self.lines.get(i).map_or("", String::as_str)
    }

    fn set(&mut self, i: usize, text: String) {
        if let Some(l) = self.lines.get_mut(i) {
            *l = text;
        }
    }
}

fn phase_name(p: Phase) -> &'static str {
    match p {
        Phase::None => "wheel",
        Phase::Began => "began",
        Phase::Changed => "changed",
        Phase::Ended => "ended",
    }
}

impl Widget for TouchProbe {
    fn kind(&self) -> &'static str {
        "TouchProbe"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        SIZE
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let r = cx.bounds();
        cx.rec.layer(
            "Surface",
            r,
            Shape::rounded(12.0),
            Style::fill(c.surface_raised)
                .with_border(crate::style::Border::inside(c.line_border, 1.0)),
        );
        for (i, (line, cache)) in self.lines.iter().zip(self.caches.iter_mut()).enumerate() {
            let style = theme.text(&typography::BODY, c.text_primary);
            let layout = cache.layout(line, &style, Align::Start, r.width - 32.0, Some(1));
            cx.rec.text(
                &format!("Row {}", i + 1),
                Rect::new(
                    r.x + 16.0,
                    r.y + 12.0 + i as f32 * 24.0,
                    r.width - 32.0,
                    18.0,
                ),
                layout,
                Some(typography::BODY.name),
                style.color,
            );
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        match event {
            Event::Scroll {
                dx,
                dy,
                precise,
                phase,
                ..
            } => {
                let now = cx.now();
                if *phase == Phase::Began || !*precise {
                    self.samples.clear();
                }
                self.samples.push((now, Point::new(*dx, *dy)));
                self.samples.retain(|(t, _)| *t >= now - 0.1);
                let sum = self
                    .samples
                    .iter()
                    .fold(Point::ZERO, |a, (_, d)| Point::new(a.x + d.x, a.y + d.y));
                let kind = if *precise { "touchpad" } else { "wheel" };
                self.set(
                    0,
                    format!(
                        "Scroll  {kind} {}  dx {dx:.1}  dy {dy:.1}",
                        phase_name(*phase)
                    ),
                );
                self.set(
                    1,
                    format!("Velocity  {:.0}, {:.0} pt/s", sum.x * 10.0, sum.y * 10.0),
                );
            }
            Event::Magnify { scale, phase } => {
                if *phase == Phase::Began {
                    self.pinch = 1.0;
                }
                self.pinch *= scale;
                self.set(
                    2,
                    format!("Pinch  {:.2}  {}", self.pinch, phase_name(*phase)),
                );
            }
            Event::Swipe {
                fingers,
                dx,
                dy,
                phase,
                cancelled,
            } => {
                let end = if *cancelled {
                    "cancelled"
                } else {
                    phase_name(*phase)
                };
                self.set(
                    3,
                    format!("Swipe  {fingers} fingers  dx {dx:.1}  dy {dy:.1}  {end}"),
                );
            }
            Event::Hold { fingers, phase } => {
                self.set(
                    4,
                    format!("Hold  {fingers} fingers  {}", phase_name(*phase)),
                );
            }
            Event::NavigateSwipe { forward } => {
                self.navigations += 1;
                self.set(
                    5,
                    format!(
                        "Navigate  {} ({})",
                        if *forward { "forward" } else { "back" },
                        self.navigations
                    ),
                );
            }
            _ => return Handled::No,
        }
        cx.request_paint();
        Handled::Yes
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Label;
        a.label = Some("Touchpad events".to_string());
        a.value = Some(self.lines.join(", "));
        a.live = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ui::Ui;
    use crate::core::window::WindowKind;

    #[test]
    fn it_shows_every_kind_of_touchpad_event() {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(500.0, 300.0));
        let p = ui.create(TouchProbe::new());
        ui.set_content(win, p);
        ui.show_window(win);
        ui.prepare_frame(win);
        let at = ui.tree.window_rect(p).center();
        for _ in 0..5 {
            ui.advance(0.016);
            ui.scroll(win, at, 0.0, 10.0, true, Phase::Changed);
        }
        ui.magnify(win, at, 1.1, Phase::Changed);
        ui.swipe(win, at, 3, 4.0, 0.0, Phase::Changed, false);
        ui.hold(win, at, 2, Phase::Began);
        let probe = ui.widget::<TouchProbe>(p).expect("probe");
        assert!(probe.line(0).contains("touchpad"), "{}", probe.line(0));
        assert!(probe.line(1).contains("500"), "{}", probe.line(1));
        assert!(probe.line(2).contains("1.10"));
        assert!(probe.line(3).contains("3 fingers"));
        assert!(probe.line(4).contains("2 fingers"));
    }
}
