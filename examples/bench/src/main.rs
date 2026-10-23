//! Headless frame-time budgets: a window of 500 controls; the first frame (layout and full
//! paint), a hover change (one control repaints), a 10 MB document typing, a Dock of 20
//! magnifying tiles on glass, and the symbols grid. Prints the times and exits non-zero when
//! a budget is exceeded (release builds only are meaningful). `--snapshot DIR` also writes
//! the scenes as PNGs.

use std::time::{Duration, Instant};

use appkit::amalfi::button::{PushButton, PushVariant};
use appkit::amalfi::toggles::Toggle;
use appkit::color::Color;
use appkit::core::layout::Alignment;
use appkit::core::{Event, Grid, Mods, Prop, Ui, Value, WindowKind};
use appkit::geom::{Point, Rect, Size};
use appkit::motion::{Animated, SpringToken, Timing};
use appkit::render::backdrop::Backdrop;
use appkit::render::canvas::Canvas;
use appkit::render::image::Image;
use appkit::shape::Shape;
use appkit::style::Glass;
use appkit::text::TextStyle;
use appkit::text::fonts::symbols;
use appkit::text::view::TextView;

/// The 120 Hz frame.
const FRAME: Duration = Duration::from_micros(8333);

fn main() {
    let snapshot = std::env::args()
        .skip_while(|a| a != "--snapshot")
        .nth(1)
        .map(std::path::PathBuf::from);
    let mut failures = Vec::new();
    let mut ui = Ui::new_headless();
    let win = ui.create_window(WindowKind::Normal, Size::new(1600.0, 1000.0));
    let grid = ui.create(Grid {
        columns: 20,
        column_spacing: 8.0,
        row_spacing: 8.0,
        alignment: Alignment::TOP_LEADING,
    });
    let mut first = None;
    for i in 0..500 {
        let v = if i % 2 == 0 {
            ui.create(PushButton::new("Button").variant(PushVariant::Secondary))
        } else {
            ui.create(Toggle::checkbox().label("Check"))
        };
        first.get_or_insert(v);
        ui.add_child(grid, v);
    }
    let _ = ui.set(grid, Prop::Padding, Value::from(16.0));
    ui.set_content(win, grid);
    ui.show_window(win);
    let t = Instant::now();
    let img = ui.snapshot(win, 1.0);
    let first_frame = t.elapsed();
    println!("first frame (500 controls, layout + full raster paint): {first_frame:?}");
    if img.is_none() || first_frame > Duration::from_millis(400) {
        failures.push("first frame over 400 ms");
    }
    // Hover one control: only it is recorded again; the damage is its bounds.
    let Some(b) = first else { return };
    let center = ui.tree.window_rect(b).center();
    let t = Instant::now();
    ui.pointer_move(win, center, Mods::empty());
    let damage = ui.prepare_frame(win);
    let hover = t.elapsed();
    println!(
        "hover change: {hover:?}, damage {damage:?}, control {:?}",
        ui.tree.window_rect(b)
    );
    let t = Instant::now();
    for _ in 0..30 {
        ui.advance(1.0 / 60.0);
        let _ = ui.prepare_frame(win);
    }
    println!("hover fade, per frame: {:?}", t.elapsed() / 30);
    if hover > Duration::from_millis(8) {
        failures.push("hover over 8 ms");
    }
    // An animation beside 500 static controls (a spinner over a busy window), drawn at 2×:
    // the frame (layout, recording, raster painting) must fit 120 Hz, 8.3 ms. The raster
    // cache draws the static controls from an image.
    for cache in [false, true] {
        let mut ui = Ui::new_headless();
        ui.set_raster_cache(cache);
        let win = ui.create_window(WindowKind::Normal, Size::new(1600.0, 1000.0));
        let col = ui.create(appkit::core::Stack::column(8.0));
        let grid = ui.create(Grid {
            columns: 20,
            column_spacing: 8.0,
            row_spacing: 8.0,
            alignment: Alignment::TOP_LEADING,
        });
        for i in 0..500 {
            let v = if i % 2 == 0 {
                ui.create(PushButton::new("Button").variant(PushVariant::Secondary))
            } else {
                ui.create(Toggle::checkbox().label("Check"))
            };
            ui.add_child(grid, v);
        }
        let spin = ui.create(appkit::amalfi::progress::Spinner::new());
        ui.add_child(col, spin);
        ui.add_child(col, grid);
        ui.set_content(win, col);
        ui.show_window(win);
        let mut canvas = appkit::render::canvas::Canvas::new(3200, 2000);
        let mut worst = Duration::ZERO;
        let mut total = Duration::ZERO;
        let frames = 60;
        for f in 0..frames + 5 {
            let t = Instant::now();
            ui.advance(1.0 / 120.0);
            let _ = ui.prepare_frame(win);
            canvas.save();
            canvas.scale((2.0, 2.0));
            ui.paint_window(win, &mut canvas, None);
            canvas.restore();
            canvas.flush();
            let dt = t.elapsed();
            // The first frames record everything and make the images.
            if f >= 5 {
                worst = worst.max(dt);
                total += dt;
            }
        }
        if cache {
            // What the platform does: only the damage is painted, in a region of its own.
            let mut damaged = Duration::ZERO;
            for _ in 0..frames {
                let t = Instant::now();
                ui.advance(1.0 / 120.0);
                let d = ui.prepare_frame(win);
                let device =
                    Rect::new(d.x * 2.0, d.y * 2.0, d.width * 2.0, d.height * 2.0).round_out(1.0);
                canvas.begin_region(device);
                canvas.save();
                canvas.clear_rect(device);
                canvas.clip_rect(device);
                canvas.scale((2.0, 2.0));
                ui.paint_window(win, &mut canvas, Some(d));
                canvas.restore();
                canvas.end_region();
                damaged += t.elapsed();
            }
            println!(
                "animation beside 500 controls at 2×, damage only: {:?} per frame",
                damaged / frames
            );
        }
        if cache {
            save(snapshot.as_deref(), "controls.png", &mut canvas);
        }
        let label = if cache { "on" } else { "off" };
        println!(
            "animation beside 500 controls at 2×, raster cache {label}: {:?} per frame, worst {worst:?}, cache {} KB",
            total / frames,
            ui.raster_cache_bytes() / 1024
        );
        if cache && worst > Duration::from_micros(8333) {
            failures.push("animated frame over 8.3 ms (120 Hz)");
        }
    }
    let _ = Event::Activate;
    let _ = Point::ZERO;
    // Typing in a 10 MB document.
    let mut ui = Ui::new_headless();
    let w = ui.create_window(WindowKind::Normal, Size::new(800.0, 600.0));
    let text = "The quick brown fox jumps over the lazy dog, again and again, every day! ok\n"
        .repeat(130_000);
    let t = Instant::now();
    let v = ui.create(TextView::new(&text, TextStyle::default()));
    let _ = ui.set(v, Prop::MaxWidth, Value::from(f32::INFINITY));
    let _ = ui.set(v, Prop::MaxHeight, Value::from(f32::INFINITY));
    ui.set_content(w, v);
    ui.show_window(w);
    ui.prepare_frame(w);
    let open = t.elapsed();
    ui.set_focus(v, false);
    let t = Instant::now();
    for _ in 0..50 {
        ui.text_input(w, "a");
        ui.prepare_frame(w);
    }
    let per_key = t.elapsed() / 50;
    println!("10 MB document: open {open:?}, typing {per_key:?} per key");
    if per_key > Duration::from_millis(8) {
        failures.push("typing over 8 ms");
    }
    dock(&mut failures, snapshot.as_deref());
    symbols_grid(&mut failures, snapshot.as_deref());
    if failures.is_empty() {
        println!("budgets met");
    } else {
        eprintln!("budgets exceeded: {}", failures.join(", "));
        std::process::exit(1);
    }
}

fn save(dir: Option<&std::path::Path>, name: &str, canvas: &mut Canvas) {
    let Some(dir) = dir else { return };
    let img = Image::from_pixmap(canvas.pixmap().clone());
    let path = dir.join(name);
    match img.encode_png().map(|png| std::fs::write(&path, png)) {
        Some(Ok(())) => println!("wrote {}", path.display()),
        _ => eprintln!("could not write {}", path.display()),
    }
}

/// The wallpaper at `w` × `h` (Ravello when the checkout is beside this one, else a
/// gradient-like stand-in), drawn once.
fn wallpaper(w: u32, h: u32) -> Image {
    let mut c = Canvas::new(w, h);
    let file = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../silica-wallpapers/amalfi/ravello/light.png"
    );
    match std::fs::read(file).ok().and_then(|b| Image::from_png(&b)) {
        Some(img) => {
            // Cover: the square image scaled to the longer side, centred.
            let side = w.max(h) as f32;
            let dst = Rect::new((w as f32 - side) / 2.0, (h as f32 - side) / 2.0, side, side);
            c.draw_image(&img, dst, 1.0);
        }
        None => {
            for i in 0..h / 8 {
                let t = i as f32 / (h / 8) as f32;
                let col = Color::rgba((60.0 + 150.0 * t) as u8, (120.0 + 60.0 * t) as u8, 200, 255);
                c.fill_rect(Rect::new(0.0, i as f32 * 8.0, w as f32, 8.0), col);
            }
        }
    }
    c.finish()
}

/// A Dock of 20 tiles at 1920×1200 on a glass slab: the pointer sweeps along it and each
/// tile's magnification follows a spring. Only the Dock's band is repainted, as the
/// platform does with its damage.
fn dock(failures: &mut Vec<&'static str>, snapshot: Option<&std::path::Path>) {
    const W: f32 = 1920.0;
    const H: f32 = 1200.0;
    const TILE: f32 = 56.0;
    const GAP: f32 = 8.0;
    const N: usize = 20;
    let wall = wallpaper(W as u32, H as u32);
    let mut canvas = Canvas::new(W as u32, H as u32);
    canvas.draw_image(&wall, Rect::new(0.0, 0.0, W, H), 1.0);
    let spring = Timing::Spring(SpringToken {
        response: 0.3,
        damping_ratio: 0.85,
    });
    let mut mag: Vec<Animated<f32>> = (0..N).map(|_| Animated::new(1.0)).collect();
    let slots = [
        "folder",
        "terminal",
        "gear",
        "calendar",
        "photo",
        "music-note",
        "envelope",
        "globe",
    ];
    let glyphs: Vec<char> = (0..N)
        .filter_map(|i| {
            slots
                .get(i % slots.len())
                .and_then(|n| symbols::codepoint(n))
        })
        .collect();
    let band = Rect::new(0.0, H - 200.0, W, 200.0);
    let rest = N as f32 * TILE + (N - 1) as f32 * GAP;
    let left = (W - rest) / 2.0;
    let frames = 120u32;
    let mut total = Duration::ZERO;
    let mut worst = Duration::ZERO;
    for f in 0..frames + 5 {
        let t = Instant::now();
        // The pointer crosses the Dock once over the frames.
        let px = left + rest * (f as f32 / frames as f32);
        for (i, m) in mag.iter_mut().enumerate() {
            let cx = left + i as f32 * (TILE + GAP) + TILE / 2.0;
            let d = ((px - cx) / (TILE * 2.5)).abs();
            let target = 1.0 + 0.8 * (1.0 - d).max(0.0).powi(2);
            if (m.target() - target).abs() > 1e-3 {
                m.animate_to(target, spring);
            }
            m.tick(1.0 / 120.0);
        }
        let sizes: Vec<f32> = mag.iter().map(|m| TILE * m.get()).collect();
        let width: f32 = sizes.iter().sum::<f32>() + (N - 1) as f32 * GAP;
        let slab = Rect::new(
            (W - width) / 2.0 - 12.0,
            H - 16.0 - TILE - 24.0,
            width + 24.0,
            TILE + 24.0,
        );
        canvas.begin_region(band);
        canvas.save();
        canvas.clip_rect(band);
        canvas.draw_image(&wall, Rect::new(0.0, 0.0, W, H), 1.0);
        let outline = Shape::rounded(22.0).path(slab);
        canvas.backdrop(
            &outline,
            Backdrop {
                sigma: 20.0,
                saturation: 1.6,
                brightness: 1.05,
                glass: Some((
                    slab,
                    Glass {
                        radius: 20.0,
                        saturation: 1.6,
                        brightness: 1.05,
                        distortion: 0.4,
                        depth: 0.3,
                        specular: true,
                        ..Glass::default()
                    },
                )),
                to_device: canvas.transform(),
            },
        );
        canvas.fill_path(
            &outline,
            &appkit::render::canvas::Brush::Solid(Color::rgba(255, 255, 255, 60)),
            1.0,
        );
        let mut x = (W - width) / 2.0;
        for (i, size) in sizes.iter().enumerate() {
            let tile = Rect::new(x, slab.bottom() - 12.0 - size, *size, *size);
            let hue = i as f32 / N as f32;
            let col = Color::rgba(
                (80.0 + 160.0 * hue) as u8,
                (200.0 - 120.0 * hue) as u8,
                230,
                255,
            );
            canvas.fill_path(
                &Shape::rounded(size * 0.225).path(tile),
                &appkit::render::canvas::Brush::Solid(col),
                1.0,
            );
            if let Some(ch) = glyphs.get(i) {
                symbols::paint(
                    &mut canvas,
                    tile.inset(appkit::geom::Insets::all(size * 0.2)),
                    *ch,
                    size * 0.6,
                    500,
                    Color::rgba(255, 255, 255, 255),
                );
            }
            x += size + GAP;
        }
        canvas.restore();
        canvas.end_region();
        let dt = t.elapsed();
        if f >= 5 {
            worst = worst.max(dt);
            total += dt;
        }
        if f == frames / 2 {
            save(snapshot, "dock.png", &mut canvas);
        }
    }
    println!(
        "Dock, 20 tiles magnifying on glass at 1920×1200: {:?} per frame, worst {worst:?}",
        total / frames
    );
    if worst > FRAME {
        failures.push("Dock frame over 8.3 ms (120 Hz)");
    }
}

/// Every symbol in three weights at 2×: the first frame (glyph outlines resolved) and a
/// repaint.
fn symbols_grid(failures: &mut Vec<&'static str>, snapshot: Option<&std::path::Path>) {
    const CELL: f32 = 28.0;
    const COLS: usize = 25;
    let all = symbols::SYMBOLS;
    let rows = all.len().div_ceil(COLS);
    let (w, h) = (COLS as f32 * CELL * 3.0 + 32.0, rows as f32 * CELL + 16.0);
    let mut canvas = Canvas::new((w * 2.0) as u32, (h * 2.0) as u32);
    let paint = |canvas: &mut Canvas| {
        canvas.save();
        canvas.scale((2.0, 2.0));
        canvas.fill_rect(Rect::new(0.0, 0.0, w, h), Color::rgba(250, 250, 250, 255));
        for (k, weight) in [300u16, 500, 700].into_iter().enumerate() {
            for (i, (_, cp)) in all.iter().enumerate() {
                let Some(ch) = char::from_u32(*cp) else {
                    continue;
                };
                let x = 8.0 + k as f32 * (COLS as f32 * CELL + 8.0) + (i % COLS) as f32 * CELL;
                let y = 8.0 + (i / COLS) as f32 * CELL;
                symbols::paint(
                    canvas,
                    Rect::new(x + 4.0, y + 4.0, 20.0, 20.0),
                    ch,
                    20.0,
                    weight,
                    Color::rgba(30, 30, 34, 255),
                );
            }
        }
        canvas.restore();
        canvas.flush();
    };
    let t = Instant::now();
    paint(&mut canvas);
    let first = t.elapsed();
    let t = Instant::now();
    paint(&mut canvas);
    let again = t.elapsed();
    println!(
        "symbols grid, {} glyphs × 3 weights at 2×: first {first:?}, repaint {again:?}",
        all.len()
    );
    save(snapshot, "symbols.png", &mut canvas);
    // A static page: its full repaint fits a 60 Hz frame (what changes is repainted alone).
    if again > FRAME * 2 {
        failures.push("symbols grid repaint over 16.7 ms");
    }
}
