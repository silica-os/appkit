//! Drawing for custom views (`ak_canvas_*`), valid inside a draw callback only.

use std::ffi::c_char;

use appkit_sys::*;

use super::{color_of, guard, opt_str, req_str, set_last};
use crate::geom::Rect;
use crate::render::canvas::{Brush, Canvas, StrokeStyle};
use crate::render::path::PathBuilder;
use crate::text::fonts::UI_FAMILY;
use crate::text::{TextLayout, TextStyle};

fn canvas<'a>(c: *mut AKCanvas) -> Option<&'a mut Canvas> {
    if c.is_null() {
        None
    } else {
        // SAFETY: the pointer is the `&mut Canvas` handed to the draw callback, valid and
        // not otherwise borrowed during it.
        Some(unsafe { &mut *(c as *mut Canvas) })
    }
}

fn rect(r: AKRect) -> Rect {
    Rect::new(r.x, r.y, r.width.max(0.0), r.height.max(0.0))
}

fn brush(color: AKColor) -> Brush {
    Brush::Solid(color_of(color))
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
            && c.save_count() > 0
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
pub extern "C" fn ak_canvas_clip_rect(c: *mut AKCanvas, r: AKRect) {
    guard((), || {
        if let Some(c) = canvas(c) {
            c.clip_rect(rect(r));
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_canvas_fill_rect(c: *mut AKCanvas, r: AKRect, color: AKColor) {
    guard((), || {
        if let Some(c) = canvas(c) {
            c.fill_rect(rect(r), color_of(color));
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_canvas_fill_rounded_rect(
    c: *mut AKCanvas,
    r: AKRect,
    radius: f32,
    color: AKColor,
) {
    guard((), || {
        if let Some(c) = canvas(c) {
            let path = crate::shape::Shape::rounded(radius).path(rect(r));
            c.fill_path(&path, &brush(color), 1.0);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_canvas_stroke_rect(c: *mut AKCanvas, r: AKRect, width: f32, color: AKColor) {
    guard((), || {
        if let Some(c) = canvas(c) {
            let mut b = PathBuilder::new();
            b.add_rect(rect(r));
            c.stroke_path(&b.detach(), &brush(color), 1.0, &StrokeStyle::new(width));
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_canvas_fill_ellipse(c: *mut AKCanvas, r: AKRect, color: AKColor) {
    guard((), || {
        if let Some(c) = canvas(c) {
            let mut b = PathBuilder::new();
            b.add_oval(rect(r));
            c.fill_path(&b.detach(), &brush(color), 1.0);
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
            let mut b = PathBuilder::new();
            b.move_to((x0, y0)).line_to((x1, y1));
            c.stroke_path(&b.detach(), &brush(color), 1.0, &StrokeStyle::new(width));
        }
    });
}

fn layout(text: &str, family: Option<&str>, size: f32, weight: u32, color: AKColor) -> TextLayout {
    let style = TextStyle {
        family: family.unwrap_or(UI_FAMILY).to_string(),
        size: size.max(0.0),
        weight: u16::try_from(weight).unwrap_or(400),
        line_height: None,
        color: color_of(color),
        ..TextStyle::default()
    };
    TextLayout::plain(text, &style, f32::INFINITY)
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
        // (x, y) is the top left of the text box.
        layout(text, family, size, weight, color).paint(c, crate::geom::Point::new(x, y));
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ak_canvas_glyph(
    c: *mut AKCanvas,
    r: AKRect,
    symbol: *const c_char,
    weight: u32,
    color: AKColor,
) {
    guard((), || {
        let (Some(c), Ok(name)) = (canvas(c), req_str(symbol)) else {
            return;
        };
        if let Some(ch) = crate::text::fonts::symbols::codepoint(name) {
            crate::core::display::paint_glyph(
                c,
                rect(r),
                ch,
                r.height,
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
        let l = layout(text, family, size, weight, AKColor::default());
        let s = l.size();
        if !out_width.is_null() {
            // SAFETY: the caller's out pointer, checked non-null.
            unsafe { *out_width = s.width };
        }
        if !out_height.is_null() {
            // SAFETY: as above.
            unsafe { *out_height = s.height };
        }
        set_last(AK_OK)
    })
}
