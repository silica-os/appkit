//! Drawing for custom views (`ak_canvas_*`), valid inside a draw callback only.

use std::ffi::c_char;

use skia_safe::{Canvas, ClipOp, Font, Paint, PaintStyle};
use appkit_sys::*;

use super::{color_of, guard, opt_str, req_str, set_last};
use crate::text::fonts::{Fonts, UI_FAMILY};

fn canvas<'a>(c: *mut AKCanvas) -> Option<&'a Canvas> {
    if c.is_null() {
        None
    } else {
        // SAFETY: the pointer is the `&Canvas` handed to the draw callback, valid during it.
        Some(unsafe { &*(c as *const Canvas) })
    }
}

fn paint(color: AKColor, stroke: Option<f32>) -> Paint {
    let mut p = Paint::default();
    p.set_anti_alias(true);
    p.set_color(color_of(color).to_sk());
    if let Some(w) = stroke {
        p.set_style(PaintStyle::Stroke);
        p.set_stroke_width(w);
    }
    p
}

fn sk(r: AKRect) -> skia_safe::Rect {
    skia_safe::Rect::from_xywh(r.x, r.y, r.width.max(0.0), r.height.max(0.0))
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_canvas_save(c: *mut AKCanvas) {
    guard((), || {
        if let Some(c) = canvas(c) {
            c.save();
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_canvas_restore(c: *mut AKCanvas) {
    guard((), || {
        if let Some(c) = canvas(c)
            && c.save_count() > 1
        {
            c.restore();
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_canvas_translate(c: *mut AKCanvas, x: f32, y: f32) {
    guard((), || {
        if let Some(c) = canvas(c) {
            c.translate((x, y));
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_canvas_clip_rect(c: *mut AKCanvas, rect: AKRect) {
    guard((), || {
        if let Some(c) = canvas(c) {
            c.clip_rect(sk(rect), ClipOp::Intersect, true);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_canvas_fill_rect(c: *mut AKCanvas, rect: AKRect, color: AKColor) {
    guard((), || {
        if let Some(c) = canvas(c) {
            c.draw_rect(sk(rect), &paint(color, None));
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_canvas_fill_rounded_rect(
    c: *mut AKCanvas,
    rect: AKRect,
    radius: f32,
    color: AKColor,
) {
    guard((), || {
        if let Some(c) = canvas(c) {
            let r = crate::geom::Rect::new(rect.x, rect.y, rect.width, rect.height);
            c.draw_path(
                &crate::shape::Shape::rounded(radius).path(r),
                &paint(color, None),
            );
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_canvas_stroke_rect(
    c: *mut AKCanvas,
    rect: AKRect,
    width: f32,
    color: AKColor,
) {
    guard((), || {
        if let Some(c) = canvas(c) {
            c.draw_rect(sk(rect), &paint(color, Some(width)));
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_canvas_fill_ellipse(
    c: *mut AKCanvas,
    rect: AKRect,
    color: AKColor,
) {
    guard((), || {
        if let Some(c) = canvas(c) {
            c.draw_oval(sk(rect), &paint(color, None));
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_canvas_line(
    c: *mut AKCanvas,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    width: f32,
    color: AKColor,
) {
    guard((), || {
        if let Some(c) = canvas(c) {
            c.draw_line((x0, y0), (x1, y1), &paint(color, Some(width)));
        }
    });
}

fn font(family: Option<&str>, size: f32, weight: u32) -> Option<Font> {
    let tf = Fonts::with(|f| {
        f.typeface(
            family.unwrap_or(UI_FAMILY),
            u16::try_from(weight).unwrap_or(400),
            false,
        )
    })?;
    let mut font = Font::from_typeface(tf, size.max(0.0));
    font.set_subpixel(true);
    Some(font)
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_canvas_text(
    c: *mut AKCanvas,
    x: f32,
    y: f32,
    text: *const c_char,
    family: *const c_char,
    size: f32,
    weight: u32,
    color: AKColor,
) {
    guard((), || {
        let (Some(c), Ok(text), Ok(family)) = (canvas(c), req_str(text), opt_str(family)) else {
            return;
        };
        if let Some(f) = font(family, size, weight) {
            // (x, y) is the top left of the text box; the baseline is the ascent below it.
            let (_, m) = f.metrics();
            c.draw_str(text, (x, y - m.ascent), &f, &paint(color, None));
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_canvas_glyph(
    c: *mut AKCanvas,
    rect: AKRect,
    symbol: *const c_char,
    weight: u32,
    color: AKColor,
) {
    guard((), || {
        let (Some(c), Ok(name)) = (canvas(c), req_str(symbol)) else {
            return;
        };
        if let Some(ch) = crate::text::fonts::symbols::codepoint(name) {
            let r = crate::geom::Rect::new(rect.x, rect.y, rect.width, rect.height);
            crate::core::display::paint_glyph(
                c,
                r,
                ch,
                rect.height,
                u16::try_from(weight).unwrap_or(400),
                color_of(color),
            );
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_measure_text(
    text: *const c_char,
    family: *const c_char,
    size: f32,
    weight: u32,
    out_width: *mut f32,
    out_height: *mut f32,
) -> AKStatus {
    guard(AK_ERROR_PLATFORM, || {
        let (Ok(text), Ok(family)) = (req_str(text), opt_str(family)) else {
            return set_last(AK_ERROR_INVALID_ARGUMENT);
        };
        let Some(f) = font(family, size, weight) else {
            return set_last(AK_ERROR_PLATFORM);
        };
        let (w, _) = f.measure_str(text, None);
        let (_, m) = f.metrics();
        if !out_width.is_null() {
            // SAFETY: the caller's out pointer, checked non-null.
            unsafe { *out_width = w };
        }
        if !out_height.is_null() {
            // SAFETY: as above.
            unsafe { *out_height = m.descent - m.ascent };
        }
        set_last(AK_OK)
    })
}
