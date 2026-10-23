//! GPU against CPU: the same display list painted by both canvases must agree within the
//! antialiasing's tolerance. Skipped when the machine has no adapter.

use super::*;
use crate::amalfi::theme::Theme;
use crate::core::display::Recorder;
use crate::geom::Point;
use crate::render::canvas::Canvas;
use crate::shape::Shape;
use crate::style::PaintOptions;
use crate::style::{Border, Paint, Shadow, Style};

fn context() -> Option<Rc<GpuContext>> {
    let c = GpuContext::new();
    if c.is_none() {
        eprintln!("no GPU adapter: skipped");
    }
    c
}

/// Mean absolute difference per channel between two premultiplied RGBA8 pixmaps, and the
/// largest single difference.
fn diff(a: &Pixmap, b: &Pixmap) -> (f64, u8) {
    let (x, y) = (a.data_as_u8_slice(), b.data_as_u8_slice());
    let n = x.len().min(y.len()).max(1);
    let mut sum = 0u64;
    let mut max = 0u8;
    for (p, q) in x.iter().zip(y) {
        let d = p.abs_diff(*q);
        sum += u64::from(d);
        max = max.max(d);
    }
    (sum as f64 / n as f64, max)
}

fn render_both(
    ctx: &Rc<GpuContext>,
    w: u32,
    h: u32,
    list: &crate::core::display::DisplayList,
) -> (Pixmap, Pixmap) {
    let mut cpu = Canvas::new(w, h);
    list.paint(&mut cpu, PaintOptions::default());
    let a = cpu.into_pixmap();
    let mut gpu = GpuCanvas::new(ctx, w, h);
    gpu.begin(true);
    list.paint(&mut gpu, PaintOptions::default());
    let b = gpu.into_pixmap();
    (a, b)
}

fn scene(theme: &Theme) -> crate::core::display::DisplayList {
    let c = theme.colors;
    let mut r = Recorder::new();
    r.layer(
        "Background",
        Rect::new(0.0, 0.0, 200.0, 120.0),
        Shape::RECT,
        Style::default().with_fill(Paint::Linear {
            from: Point::new(0.0, 0.0),
            to: Point::new(1.0, 1.0),
            stops: vec![
                (0.0, Color::rgb(0xe6, 0x7e, 0x22)),
                (1.0, Color::rgb(0x2e, 0x86, 0xc1)),
            ],
        }),
    );
    r.layer(
        "Card",
        Rect::new(20.0, 20.0, 90.0, 60.0),
        Shape::rounded(14.0),
        Style::fill(c.surface_raised)
            .with_border(Border::inside(c.line_border, 1.0))
            .with_shadow(Shadow::new(Color::rgba(0, 0, 0, 90), 0.0, 6.0, 12.0)),
    );
    r.push_opacity(0.5);
    r.layer(
        "Dot",
        Rect::new(130.0, 30.0, 40.0, 40.0),
        Shape::Ellipse,
        Style::fill(Color::rgb(0xff, 0xff, 0xff)),
    );
    r.pop();
    r.layer(
        "Glass",
        Rect::new(100.0, 70.0, 80.0, 36.0),
        Shape::rounded(12.0),
        theme.material_style("Glass/Regular"),
    );
    r.glyph(
        "Icon",
        Rect::new(40.0, 36.0, 28.0, 28.0),
        "chevron-down",
        400,
        c.text_primary,
    );
    r.finish()
}

#[test]
fn shapes_gradients_shadows_and_glass_agree_with_the_cpu() {
    let Some(ctx) = context() else { return };
    let theme = Theme::light();
    let list = scene(&theme);
    let (cpu, gpu) = render_both(&ctx, 200, 120, &list);
    let (mean, max) = diff(&cpu, &gpu);
    eprintln!("{}: mean {mean:.3}, max {max}", ctx.adapter());
    assert!(mean < 2.5, "mean {mean}");
    assert!(max < 255);
}

#[test]
fn dark_glass_agrees_too() {
    let Some(ctx) = context() else { return };
    let theme = Theme::dark_theme();
    let list = scene(&theme);
    let (cpu, gpu) = render_both(&ctx, 200, 120, &list);
    let (mean, _) = diff(&cpu, &gpu);
    assert!(mean < 2.5, "mean {mean}");
}

#[test]
fn a_clip_path_and_a_clip_rect_confine_the_drawing() {
    let Some(ctx) = context() else { return };
    let mut gpu = GpuCanvas::new(&ctx, 64, 64);
    gpu.begin(true);
    gpu.save();
    gpu.clip_rect(Rect::new(8.0, 8.0, 32.0, 32.0));
    let mut b = crate::render::path::PathBuilder::new();
    b.add_oval(Rect::new(0.0, 0.0, 64.0, 64.0));
    gpu.clip_path(&b.detach());
    gpu.fill_rect(Rect::new(0.0, 0.0, 64.0, 64.0), Color::rgb(255, 0, 0));
    gpu.restore();
    let px = gpu.into_pixmap();
    let at = |x: usize, y: usize| px.data_as_u8_slice()[(y * 64 + x) * 4 + 3];
    assert_eq!(at(24, 24), 255, "inside both clips");
    assert_eq!(at(4, 4), 0, "outside the rect clip");
    assert_eq!(at(50, 50), 0, "outside the rect clip");
}

#[test]
fn a_backdrop_blurs_without_smearing_outside_its_outline() {
    let Some(ctx) = context() else { return };
    let mut gpu = GpuCanvas::new(&ctx, 64, 32);
    gpu.begin(true);
    // Black on the left half, white on the right: a hard edge.
    gpu.fill_rect(Rect::new(0.0, 0.0, 32.0, 32.0), Color::rgb(0, 0, 0));
    gpu.fill_rect(Rect::new(32.0, 0.0, 32.0, 32.0), Color::rgb(255, 255, 255));
    let mut b = crate::render::path::PathBuilder::new();
    b.add_rect(Rect::new(16.0, 8.0, 32.0, 16.0));
    let outline = b.detach();
    gpu.backdrop(
        &outline,
        Backdrop {
            sigma: 3.0,
            saturation: 1.0,
            brightness: 1.0,
            glass: None,
            to_device: Transform::IDENTITY,
        },
    );
    let px = gpu.into_pixmap();
    let r = |x: usize, y: usize| px.data_as_u8_slice()[(y * 64 + x) * 4];
    assert!(
        r(31, 16) > 20 && r(31, 16) < 235,
        "the edge is blurred inside: {}",
        r(31, 16)
    );
    assert_eq!(r(31, 2), 0, "and untouched outside the outline");
    assert_eq!(r(0, 16), 0);
}

#[test]
fn the_frame_is_retained_and_damage_is_reported() {
    let Some(ctx) = context() else { return };
    let mut gpu = GpuCanvas::new(&ctx, 32, 32);
    gpu.begin(true);
    gpu.fill_rect(Rect::new(0.0, 0.0, 32.0, 32.0), Color::rgb(0, 0, 255));
    gpu.submit();
    assert!(gpu.take_damage().is_some());
    // A second frame that paints a small region leaves the rest as it was.
    gpu.begin(false);
    gpu.save();
    gpu.clip_rect(Rect::new(0.0, 0.0, 8.0, 8.0));
    gpu.fill_rect(Rect::new(0.0, 0.0, 32.0, 32.0), Color::rgb(255, 0, 0));
    gpu.restore();
    let damage = gpu.take_damage();
    let px = gpu.into_pixmap();
    let d = px.data_as_u8_slice();
    assert_eq!(&d[0..4], &[255, 0, 0, 255]);
    assert_eq!(
        &d[(20 * 32 + 20) * 4..(20 * 32 + 20) * 4 + 4],
        &[0, 0, 255, 255]
    );
    assert_eq!(damage, Some(Rect::new(0.0, 0.0, 8.0, 8.0)));
}

#[test]
fn custom_drawing_is_rasterized_and_agrees_with_the_cpu() {
    let Some(ctx) = context() else { return };
    let mut r = Recorder::new();
    r.push_opacity(0.8);
    r.custom(
        "Custom",
        Rect::new(10.0, 12.0, 60.0, 40.0),
        Rc::new(|c, rect| {
            c.translate((rect.x, rect.y));
            c.fill_rect(Rect::new(0.0, 0.0, 30.0, 40.0), Color::rgb(220, 40, 40));
            c.fill_rect(Rect::new(30.0, 10.0, 30.0, 20.0), Color::rgb(40, 40, 220));
        }),
    );
    r.pop();
    let list = r.finish();
    let (cpu, gpu) = render_both(&ctx, 100, 80, &list);
    let (mean, _) = diff(&cpu, &gpu);
    assert!(mean < 1.5, "mean {mean}");
    let px = gpu.data_as_u8_slice();
    let at = |x: usize, y: usize| &px[(y * 100 + x) * 4..(y * 100 + x) * 4 + 4];
    assert!(at(20, 30)[0] > 100 && at(20, 30)[3] > 150, "red part drawn");
    assert!(at(60, 30)[2] > 100, "blue part drawn");
    assert_eq!(at(90, 70)[3], 0, "nothing outside");
}
