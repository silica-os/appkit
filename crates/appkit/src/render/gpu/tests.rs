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

/// A backdrop inside a group: the left half is black, the right white, a red square is drawn
/// in the group before the backdrop, and a clip opens inside the group.
fn grouped_backdrop(p: &mut dyn Painter, opacity: f32, blur: bool) {
    p.fill_rect(Rect::new(0.0, 0.0, 32.0, 64.0), Color::rgb(0, 0, 0));
    p.fill_rect(Rect::new(32.0, 0.0, 32.0, 64.0), Color::rgb(255, 255, 255));
    p.save();
    if blur {
        p.push_blur(1.5);
    } else {
        p.push_opacity(opacity);
    }
    p.fill_rect(Rect::new(8.0, 8.0, 20.0, 20.0), Color::rgb(255, 0, 0));
    p.save();
    p.clip_rect(Rect::new(0.0, 0.0, 64.0, 48.0));
    let mut b = crate::render::path::PathBuilder::new();
    b.add_rect(Rect::new(14.0, 14.0, 36.0, 28.0));
    p.backdrop(
        &b.detach(),
        Backdrop {
            sigma: 3.0,
            saturation: 1.0,
            brightness: 1.0,
            glass: None,
            to_device: Transform::IDENTITY,
        },
    );
    p.restore();
    // Drawn after the backdrop, in the same group.
    p.fill_rect(Rect::new(40.0, 40.0, 16.0, 16.0), Color::rgb(0, 0, 255));
    p.restore();
}

#[test]
fn a_backdrop_inside_a_group_samples_the_group_and_agrees_with_the_cpu() {
    let Some(ctx) = context() else { return };
    for (opacity, blur) in [(0.5, false), (1.0, false), (1.0, true)] {
        let mut cpu = Canvas::new(64, 64);
        grouped_backdrop(&mut cpu, opacity, blur);
        let a = cpu.into_pixmap();
        let mut gpu = GpuCanvas::new(&ctx, 64, 64);
        gpu.begin(true);
        grouped_backdrop(&mut gpu, opacity, blur);
        let b = gpu.into_pixmap();
        let (mean, max) = diff(&a, &b);
        eprintln!("group opacity {opacity} blur {blur}: mean {mean:.3}, max {max}");
        assert!(mean < 2.0, "opacity {opacity} blur {blur}: mean {mean}");
    }
}

#[test]
fn the_gles_backend_agrees_with_the_cpu_too() {
    let Some(ctx) = GpuContext::with_backends(Some(wgpu::Backends::GL)) else {
        eprintln!("no GLES adapter: skipped");
        return;
    };
    let theme = Theme::light();
    let list = scene(&theme);
    let (cpu, gpu) = render_both(&ctx, 200, 120, &list);
    let (mean, max) = diff(&cpu, &gpu);
    eprintln!("{}: mean {mean:.3}, max {max}", ctx.adapter());
    assert!(mean < 2.5, "mean {mean}");
}

#[test]
fn text_from_the_atlas_agrees_with_the_cpu_and_is_cached() {
    use crate::text::layout::{TextLayout, TextStyle};
    let Some(ctx) = context() else { return };
    let style = TextStyle {
        color: Color::rgb(20, 20, 30),
        ..TextStyle::default()
    };
    let layout = TextLayout::plain(
        "Silica 1.0 — Amalfi: quick brown fox, 0123456789",
        &style,
        400.0,
    );
    for scale in [1.0f32, 1.25, 2.0] {
        let paint = |p: &mut dyn Painter| {
            p.fill_rect(Rect::new(0.0, 0.0, 420.0, 60.0), Color::rgb(245, 245, 240));
            p.save();
            p.scale((scale, scale));
            layout.paint(p, Point::new(6.3, 7.7));
            p.restore();
        };
        let (w, h) = ((420.0 * scale) as u32, (60.0 * scale) as u32);
        let mut cpu = Canvas::new(w, h);
        paint(&mut cpu);
        let a = cpu.into_pixmap();
        let mut gpu = GpuCanvas::new(&ctx, w, h);
        gpu.begin(true);
        paint(&mut gpu);
        let b = gpu.into_pixmap();
        let (mean, max) = diff(&a, &b);
        eprintln!("text at {scale}: mean {mean:.3}, max {max}");
        assert!(mean < 1.5, "scale {scale}: mean {mean}");
        // Painted twice, the second run adds nothing to the atlas.
        let cached = ctx.atlas_len();
        let mut again = GpuCanvas::new(&ctx, w, h);
        again.begin(true);
        paint(&mut again);
        again.submit();
        assert_eq!(ctx.atlas_len(), cached);
        assert!(cached > 10, "glyphs were cached: {cached}");
    }
}

#[test]
fn a_layer_is_rendered_once_and_composited_from_its_texture() {
    let Some(ctx) = context() else { return };
    let mut gpu = GpuCanvas::new(&ctx, 64, 64);
    let mut drawn = 0;
    let mut direct = GpuCanvas::new(&ctx, 64, 64);
    let scene = |p: &mut dyn Painter| {
        p.fill_rect(Rect::new(4.0, 4.0, 20.0, 12.0), Color::rgb(30, 120, 220));
        p.fill_rect(
            Rect::new(10.0, 10.0, 30.0, 20.0),
            Color::rgba(250, 60, 60, 160),
        );
    };
    direct.begin(true);
    direct.save();
    direct.translate((8.0, 6.0));
    scene(&mut direct);
    direct.restore();
    let want = direct.into_pixmap();
    for version in [1u64, 1, 1, 2] {
        gpu.begin(true);
        gpu.save();
        gpu.translate((8.0, 6.0));
        let ok = gpu.layer_texture(7, version, Rect::new(0.0, 0.0, 44.0, 34.0), 1.0, &mut |p| {
            drawn += 1;
            scene(p);
        });
        gpu.restore();
        assert!(ok);
        if version == 1 {
            let got = gpu.into_pixmap();
            let (mean, _) = diff(&want, &got);
            assert!(mean < 0.2, "mean {mean}");
            gpu = GpuCanvas::new(&ctx, 64, 64);
        }
    }
    // Rendered for version 1 (once: the second and third frames composite) and again for 2.
    assert_eq!(drawn, 2);
    assert_eq!(ctx.layers.borrow().len(), 1);
    // Not on whole pixels: not kept. Partly outside the frame: kept.
    gpu.begin(true);
    gpu.save();
    gpu.translate((0.5, 0.0));
    assert!(!gpu.layer_texture(8, 1, Rect::new(0.0, 0.0, 10.0, 10.0), 1.0, &mut |_| {}));
    gpu.restore();
    gpu.save();
    gpu.translate((60.0, 0.0));
    assert!(gpu.layer_texture(9, 1, Rect::new(0.0, 0.0, 10.0, 10.0), 1.0, &mut |_| {}));
    gpu.restore();
}

#[test]
fn a_static_subtree_of_a_window_is_kept_as_a_texture_with_the_same_pixels() {
    use crate::amalfi::button::{PushButton, PushVariant};
    use crate::core::layout::{Alignment, Grid, Stack};
    use crate::core::{Prop, Ui, Value, WindowKind};
    use crate::geom::Size;
    let Some(ctx) = context() else { return };
    let scene = |cache: bool| {
        let mut ui = Ui::new_headless();
        ui.set_raster_cache(cache);
        let w = ui.create_window(WindowKind::Normal, Size::new(300.0, 200.0));
        let col = ui.create(Stack::column(8.0));
        let grid = ui.create(Grid {
            columns: 4,
            column_spacing: 8.0,
            row_spacing: 8.0,
            alignment: Alignment::TOP_LEADING,
        });
        for i in 0..24 {
            let b = ui.create(PushButton::new(&format!("B{i}")).variant(PushVariant::Secondary));
            ui.add_child(grid, b);
        }
        ui.add_child(col, grid);
        let _ = ui.set(col, Prop::Padding, Value::from(12.0));
        ui.set_content(w, col);
        ui.show_window(w);
        (ui, w)
    };
    let render = |ui: &mut Ui, w| {
        let _ = ui.prepare_frame(w);
        let mut gpu = GpuCanvas::new(&ctx, 600, 400);
        gpu.begin(true);
        gpu.scale((2.0, 2.0));
        ui.paint_window(w, &mut gpu, None);
        gpu.into_pixmap()
    };
    let (mut on, w1) = scene(true);
    let (mut off, w2) = scene(false);
    let (mut a, mut b) = (None, None);
    for _ in 0..8 {
        on.advance(1.0 / 60.0);
        off.advance(1.0 / 60.0);
        a = Some(render(&mut on, w1));
        b = Some(render(&mut off, w2));
    }
    let (Some(a), Some(b)) = (a, b) else { return };
    // The grid became a texture, and no CPU image was made for it.
    assert!(ctx.layers.borrow().len() > 0, "a layer texture was kept");
    assert_eq!(on.raster_cache_bytes(), 0);
    let (mean, max) = diff(&a, &b);
    eprintln!("cached layers: mean {mean:.4}, max {max}");
    assert!(mean < 0.1, "mean {mean}");
}

#[test]
fn venus_is_told_from_the_other_adapters() {
    assert!(is_venus(
        "Virtio-GPU Venus (AMD Radeon RX 7800 XT)",
        "venus"
    ));
    assert!(is_venus("", "Venus"));
    assert!(!is_venus("virgl (AMD Radeon RX 7800 XT)", ""));
    assert!(!is_venus("AMD Radeon RX 7800 XT (RADV NAVI32)", "radv"));
}
