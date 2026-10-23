//! Text styles, attributed strings and laid-out paragraphs (parley: HarfRust shaping, ICU4X
//! bidi and line breaking; fontique families and fallback).
//!
//! Every offset in this API is a UTF-8 byte offset into the paragraph's text, as parley's.

use std::ops::Range;

use parley::{
    Affinity, Alignment, AlignmentOptions, Cursor, FontFamily, FontFeatures, FontStyle, FontWeight,
    InlineBox, InlineBoxKind, Layout, LineHeight, PositionedLayoutItem, Selection, StyleProperty,
};

use crate::amalfi::tokens::TypeStyle;
use crate::color::Color;
use crate::geom::{Point, Rect, Size};
use crate::render::canvas::Canvas;
use crate::text::fonts::{Fonts, UI_FAMILY};

/// Horizontal alignment of lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    /// Leading edge (left in left-to-right text).
    #[default]
    Start,
    /// Centred.
    Center,
    /// Trailing edge.
    End,
    /// Justified.
    Justify,
}

/// The style of a run of text.
#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    /// Font family.
    pub family: String,
    /// Size in points.
    pub size: f32,
    /// CSS weight.
    pub weight: u16,
    /// Italic.
    pub italic: bool,
    /// Line height in points (`None`: natural); the extra space is split evenly above and
    /// below the glyphs, as in the kit.
    pub line_height: Option<f32>,
    /// Tracking in points.
    pub tracking: f32,
    /// Colour.
    pub color: Color,
    /// OpenType features (`tnum` …).
    pub features: Vec<String>,
    /// Underline.
    pub underline: bool,
    /// Strikethrough.
    pub strikethrough: bool,
    /// Background highlight.
    pub background: Option<Color>,
}

impl Default for TextStyle {
    fn default() -> Self {
        TextStyle {
            family: UI_FAMILY.to_string(),
            size: 13.0,
            weight: 400,
            italic: false,
            line_height: Some(18.0),
            tracking: 0.0,
            color: Color::BLACK,
            features: Vec::new(),
            underline: false,
            strikethrough: false,
            background: None,
        }
    }
}

impl TextStyle {
    /// The style of a type token in `color`.
    pub fn from_token(t: &TypeStyle, color: Color) -> Self {
        TextStyle {
            family: t.family.to_string(),
            size: t.size,
            weight: t.weight,
            italic: false,
            line_height: t.line_height,
            tracking: t.tracking,
            color,
            features: t.features.iter().map(|s| (*s).to_string()).collect(),
            underline: false,
            strikethrough: false,
            background: None,
        }
    }

    /// This style with another colour.
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// This style with another weight.
    pub fn with_weight(mut self, weight: u16) -> Self {
        self.weight = weight;
        self
    }

    /// The parley properties of this style.
    fn properties(&self) -> Vec<StyleProperty<'static, Color>> {
        let mut out = Vec::with_capacity(10);
        let families = format!("\"{}\", \"{}\"", self.family, UI_FAMILY);
        out.push(StyleProperty::FontFamily(FontFamily::Source(
            families.into(),
        )));
        out.push(StyleProperty::FontSize(self.size.max(0.0)));
        out.push(StyleProperty::FontWeight(FontWeight::new(f32::from(
            self.weight,
        ))));
        if self.italic {
            out.push(StyleProperty::FontStyle(FontStyle::Italic));
        }
        out.push(StyleProperty::Brush(self.color));
        if let Some(lh) = self.line_height
            && self.size > 0.0
            && lh > 0.0
        {
            out.push(StyleProperty::LineHeight(LineHeight::Absolute(lh)));
        } else {
            out.push(StyleProperty::LineHeight(LineHeight::MetricsRelative(1.0)));
        }
        if self.tracking != 0.0 {
            out.push(StyleProperty::LetterSpacing(self.tracking));
        }
        if !self.features.is_empty() {
            let css = self
                .features
                .iter()
                .map(|f| format!("\"{f}\" 1"))
                .collect::<Vec<_>>()
                .join(", ");
            out.push(StyleProperty::FontFeatures(FontFeatures::Source(
                css.into(),
            )));
        }
        if self.underline {
            out.push(StyleProperty::Underline(true));
        }
        if self.strikethrough {
            out.push(StyleProperty::Strikethrough(true));
        }
        out
    }

    /// Sets this style over `range`.
    fn push(&self, b: &mut parley::RangedBuilder<'_, Color>, range: Range<usize>) {
        for p in self.properties() {
            b.push(p, range.clone());
        }
    }

    /// Makes this style the default of the whole layout (the line strut included).
    fn push_default(&self, b: &mut parley::RangedBuilder<'_, Color>) {
        for p in self.properties() {
            b.push_default(p);
        }
    }
}

/// An inline attachment: a box of `size` in the line (an image, a token, a symbol view).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Attachment {
    /// Size of the box.
    pub size: Size,
    /// Distance from the box's top to the baseline.
    pub baseline: f32,
}

/// A run of attributes over a byte range.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    /// Byte range in the text.
    pub range: Range<usize>,
    /// The style.
    pub style: TextStyle,
    /// A link target.
    pub link: Option<String>,
}

/// Text with per-range styles, links and attachments.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AttributedString {
    /// The text.
    pub text: String,
    /// Runs over the text; gaps use the base style, later runs win on overlap.
    pub runs: Vec<Run>,
    /// Attachments at byte offsets (each replaces the object replacement character U+FFFC
    /// at that offset).
    pub attachments: Vec<(usize, Attachment)>,
}

impl AttributedString {
    /// Plain text.
    pub fn plain(text: impl Into<String>) -> Self {
        AttributedString {
            text: text.into(),
            runs: Vec::new(),
            attachments: Vec::new(),
        }
    }

    /// Sets `style` (and `link`) over `range`.
    pub fn set(&mut self, range: Range<usize>, style: TextStyle, link: Option<String>) {
        let end = range.end.min(self.text.len());
        let start = range.start.min(end);
        self.runs.push(Run {
            range: start..end,
            style,
            link,
        });
    }

    /// The link at byte offset `at`.
    pub fn link_at(&self, at: usize) -> Option<&str> {
        self.runs
            .iter()
            .rev()
            .find(|r| r.range.contains(&at))
            .and_then(|r| r.link.as_deref())
    }
}

/// How a paragraph is laid out.
#[derive(Debug, Clone, PartialEq)]
pub struct ParagraphOptions {
    /// Alignment.
    pub align: Align,
    /// Right-to-left base direction.
    pub rtl: bool,
    /// Maximum lines (`None`: unlimited).
    pub max_lines: Option<usize>,
    /// Ellipsis when the text is cut.
    pub ellipsis: bool,
}

impl Default for ParagraphOptions {
    fn default() -> Self {
        ParagraphOptions {
            align: Align::Start,
            rtl: false,
            max_lines: None,
            ellipsis: true,
        }
    }
}

/// One line of a laid-out paragraph.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineInfo {
    /// Byte range of the line, without its trailing newline.
    pub start: usize,
    /// End byte (exclusive).
    pub end: usize,
    /// Top of the line box.
    pub top: f32,
    /// Height of the line box.
    pub height: f32,
    /// Baseline, from the paragraph's top.
    pub baseline: f32,
    /// Left of the first glyph.
    pub left: f32,
    /// Width of the glyphs.
    pub width: f32,
}

/// A laid-out paragraph.
pub struct TextLayout {
    /// The layout of what is shown: the text, or its cut with an ellipsis.
    layout: Layout<Color>,
    text: String,
    source: AttributedString,
    base: TextStyle,
    options: ParagraphOptions,
    /// Byte ranges with a background colour.
    backgrounds: Vec<(Range<usize>, Color)>,
    /// Whether the shown text was cut by `max_lines`.
    cut: bool,
    width: f32,
}

impl std::fmt::Debug for TextLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextLayout")
            .field("text", &self.text)
            .field("width", &self.width)
            .finish()
    }
}

/// The zero-width space that stands in for U+FFFC (both three bytes in UTF-8, so offsets do
/// not move) where an attachment's inline box sits.
const ATTACHMENT_STAND_IN: char = '\u{200B}';

fn build(
    text: &AttributedString,
    shown: &str,
    base: &TextStyle,
    options: &ParagraphOptions,
) -> Layout<Color> {
    let s: String = shown
        .chars()
        .map(|c| {
            if c == '\u{FFFC}' {
                ATTACHMENT_STAND_IN
            } else {
                c
            }
        })
        .collect();
    Fonts::with(|fonts| {
        let mut b = fonts.lcx.ranged_builder(&mut fonts.fcx, &s, 1.0, false);
        base.push_default(&mut b);
        for r in &text.runs {
            let end = r.range.end.min(s.len());
            let start = r.range.start.min(end);
            if start < end {
                r.style.push(&mut b, start..end);
            }
        }
        for (i, (at, att)) in text.attachments.iter().enumerate() {
            if *at > s.len() {
                continue;
            }
            b.push_inline_box(InlineBox {
                id: i as u64,
                kind: InlineBoxKind::InFlow,
                index: *at,
                width: att.size.width.max(0.0),
                height: att.size.height.max(0.0),
                baseline: Some(att.baseline),
                vertical_align: parley::VerticalAlign::BASELINE,
            });
        }
        if options.rtl {
            b.set_base_direction(parley::BaseDirection::Rtl);
        }
        b.build(&s)
    })
}

fn break_and_align(layout: &mut Layout<Color>, width: f32, align: Align) {
    let max = (width.is_finite() && width >= 0.0).then_some(width);
    layout.break_all_lines(max);
    let alignment = match align {
        Align::Start => Alignment::Start,
        Align::Center => Alignment::Center,
        Align::End => Alignment::End,
        Align::Justify => Alignment::Justify,
    };
    layout.align(alignment, AlignmentOptions::default());
}

impl TextLayout {
    /// Lays out `text` in `base` style (and its runs) for `width` (infinite: one line per
    /// paragraph).
    pub fn new(
        text: &AttributedString,
        base: &TextStyle,
        options: &ParagraphOptions,
        width: f32,
    ) -> TextLayout {
        let mut backgrounds = Vec::new();
        if let Some(bg) = base.background {
            backgrounds.push((0..text.text.len(), bg));
        }
        for r in &text.runs {
            if let Some(bg) = r.style.background {
                backgrounds.push((r.range.clone(), bg));
            }
        }
        let mut layout = TextLayout {
            layout: build(text, &text.text, base, options),
            text: text.text.clone(),
            source: text.clone(),
            base: base.clone(),
            options: options.clone(),
            backgrounds,
            cut: false,
            width: f32::INFINITY,
        };
        layout.relayout(width);
        layout
    }

    /// Lays out plain `text`.
    pub fn plain(text: &str, style: &TextStyle, width: f32) -> TextLayout {
        TextLayout::new(
            &AttributedString::plain(text),
            style,
            &ParagraphOptions::default(),
            width,
        )
    }

    /// Lays out again for another width.
    pub fn relayout(&mut self, width: f32) {
        self.width = width;
        if self.cut {
            self.layout = build(&self.source, &self.text, &self.base, &self.options);
            self.cut = false;
        }
        break_and_align(&mut self.layout, width, self.options.align);
        let Some(max) = self.options.max_lines.map(|n| n.max(1)) else {
            return;
        };
        if self.layout.len() <= max {
            return;
        }
        // Cut after the last line that fits; with an ellipsis, shorten that line until the
        // ellipsis fits too.
        let Some(last) = self.layout.get(max - 1) else {
            return;
        };
        let mut end = last.text_range().end.min(self.text.len());
        while end > 0 && self.text[..end].ends_with(['\n', '\r', ' ']) {
            end = prev_char(&self.text, end);
        }
        self.cut = true;
        if !self.options.ellipsis {
            let shown = self.text[..end].to_string();
            self.layout = build(&self.source, &shown, &self.base, &self.options);
            break_and_align(&mut self.layout, width, self.options.align);
            return;
        }
        loop {
            let mut shown = self.text[..end].trim_end().to_string();
            shown.push('…');
            self.layout = build(&self.source, &shown, &self.base, &self.options);
            break_and_align(&mut self.layout, width, self.options.align);
            if self.layout.len() <= max || end == 0 {
                return;
            }
            end = prev_char(&self.text, end);
        }
    }

    /// The text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The width laid out for.
    pub fn layout_width(&self) -> f32 {
        self.width
    }

    /// The size of the text: the longest line's width and the total height.
    pub fn size(&self) -> Size {
        Size::new(self.layout.width(), self.layout.height())
    }

    /// The width with no wrapping.
    pub fn max_intrinsic_width(&self) -> f32 {
        self.layout.calculate_content_widths().max
    }

    /// The narrowest width without breaking words.
    pub fn min_intrinsic_width(&self) -> f32 {
        self.layout.calculate_content_widths().min
    }

    /// The first line's baseline from the top.
    pub fn first_baseline(&self) -> f32 {
        self.layout.get(0).map_or(0.0, |l| l.metrics().baseline)
    }

    /// Whether lines were cut by `max_lines`.
    pub fn did_exceed_max_lines(&self) -> bool {
        self.cut
    }

    /// Paints the text with its top-left at `origin`.
    pub fn paint(&self, canvas: &mut Canvas, origin: Point) {
        for (range, color) in &self.backgrounds {
            for r in self.selection_rects(range.clone()) {
                canvas.fill_rect(r.translate(origin), *color);
            }
        }
        for line in self.layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };
                let run = glyph_run.run();
                let style = glyph_run.style();
                let color = style.brush;
                let glyphs = glyph_run.positioned_glyphs().map(|g| vello_cpu::Glyph {
                    id: g.id,
                    x: origin.x + g.x,
                    y: origin.y + g.y,
                });
                let coords: Vec<i16> = run
                    .normalized_coords()
                    .iter()
                    .map(|c| c.to_bits())
                    .collect();
                canvas.fill_glyphs(run.font(), run.font_size(), &coords, glyphs, color);
                let metrics = run.font_metrics();
                let x = origin.x + glyph_run.offset();
                let w = glyph_run.advance();
                let base = origin.y + glyph_run.baseline();
                if let Some(d) = &style.underline {
                    let size = d.size.unwrap_or(metrics.underline_size).max(0.5);
                    let off = d.offset.unwrap_or(metrics.underline_offset);
                    let r = Rect::new(x, base - off, w, size);
                    canvas.fill_rect(r, d.brush);
                }
                if let Some(d) = &style.strikethrough {
                    let size = d.size.unwrap_or(metrics.strikethrough_size).max(0.5);
                    let off = d.offset.unwrap_or(metrics.strikethrough_offset);
                    let r = Rect::new(x, base - off, w, size);
                    canvas.fill_rect(r, d.brush);
                }
            }
        }
    }

    fn shown_len(&self) -> usize {
        self.layout.text_len()
    }

    /// The byte offset nearest to `p` (paragraph coordinates), and whether the caret sits
    /// at the end of the previous line (upstream affinity).
    pub fn offset_at(&self, p: Point) -> (usize, bool) {
        let c = Cursor::from_point(&self.layout, p.x, p.y);
        (
            c.index().min(self.text.len()),
            c.affinity() == Affinity::Upstream,
        )
    }

    /// The rectangles covering the byte range (selection highlight), tight to the line boxes.
    pub fn selection_rects(&self, range: Range<usize>) -> Vec<Rect> {
        let end = range.end.min(self.shown_len());
        if range.start >= end {
            return Vec::new();
        }
        let sel = Selection::new(
            Cursor::from_byte_index(&self.layout, range.start, Affinity::Downstream),
            Cursor::from_byte_index(&self.layout, end, Affinity::Upstream),
        );
        sel.geometry(&self.layout)
            .into_iter()
            .map(|(b, _)| {
                Rect::new(
                    b.x0 as f32,
                    b.y0 as f32,
                    (b.x1 - b.x0) as f32,
                    (b.y1 - b.y0) as f32,
                )
            })
            .collect()
    }

    /// The caret rectangle (1 pt wide) at byte offset `at`; `upstream` puts a caret at a
    /// line break at the end of the first line.
    pub fn caret_rect(&self, at: usize, upstream: bool) -> Rect {
        let at = at.min(self.shown_len());
        let affinity = if upstream {
            Affinity::Upstream
        } else {
            Affinity::Downstream
        };
        let b = Cursor::from_byte_index(&self.layout, at, affinity).geometry(&self.layout, 1.0);
        let r = Rect::new(
            b.x0 as f32,
            b.y0 as f32,
            (b.x1 - b.x0) as f32,
            (b.y1 - b.y0) as f32,
        );
        if r.height <= 0.0 {
            let h = self.layout.height().max(1.0);
            return Rect::new(r.x, 0.0, 1.0, h);
        }
        r
    }

    /// The index of the line containing byte `at`.
    pub fn line_index(&self, at: usize, upstream: bool) -> usize {
        let lines = self.lines();
        for (i, l) in lines.iter().enumerate() {
            if at < l.end || (at == l.end && (upstream || i + 1 == lines.len())) {
                return i;
            }
            if let Some(next) = lines.get(i + 1)
                && at < next.start
            {
                return i;
            }
        }
        lines.len().saturating_sub(1)
    }

    /// The lines.
    pub fn lines(&self) -> Vec<LineInfo> {
        let shown = self.layout.text_len();
        self.layout
            .lines()
            .map(|l| {
                let m = l.metrics();
                let r = l.text_range();
                let start = r.start.min(shown);
                let mut end = r.end.min(shown).max(start);
                // Without the trailing newline.
                if let Some(t) = self.text.get(start..end) {
                    if t.ends_with("\r\n") {
                        end -= 2;
                    } else if t.ends_with(['\n', '\r', '\u{2028}', '\u{2029}']) {
                        end = prev_char(&self.text, end);
                    }
                }
                LineInfo {
                    start,
                    end,
                    top: m.block_min_coord,
                    height: m.block_max_coord - m.block_min_coord,
                    baseline: m.baseline,
                    left: m.offset,
                    width: m.advance - m.hanging_advance,
                }
            })
            .collect()
    }

    /// The byte range of the word at `at` (double-click selection).
    pub fn word_at(&self, at: usize) -> Range<usize> {
        let at = at.min(self.shown_len());
        let next = next_char(&self.text, at);
        let Some(r) = self.selection_rects(at..next).into_iter().next() else {
            return at..at;
        };
        let c = r.center();
        Selection::word_from_point(&self.layout, c.x, c.y).text_range()
    }
}

/// The byte offset after the character at `at`.
pub fn next_char(s: &str, at: usize) -> usize {
    if at >= s.len() {
        return s.len();
    }
    let mut i = at + 1;
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

/// The byte offset of the character before `at`.
pub fn prev_char(s: &str, at: usize) -> usize {
    if at == 0 {
        return 0;
    }
    let mut i = at.min(s.len()) - 1;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body() -> TextStyle {
        TextStyle::default()
    }

    #[test]
    fn measures_with_line_height() {
        let l = TextLayout::plain("Add to favourites", &body(), f32::INFINITY);
        let s = l.size();
        assert_eq!(s.height, 18.0);
        // Inter 13 pt: about 100 pt wide.
        assert!(s.width > 90.0 && s.width < 115.0, "{}", s.width);
        // Even leading: the baseline sits a bit below the middle of the 18 pt line.
        let b = l.first_baseline();
        assert!(b > 12.0 && b < 15.0, "{b}");
    }

    #[test]
    fn wraps_and_hit_tests() {
        let l = TextLayout::plain("one two three four five six", &body(), 60.0);
        let lines = l.lines();
        assert!(lines.len() >= 3);
        assert_eq!(l.size().height, 18.0 * lines.len() as f32);
        let (off, _) = l.offset_at(Point::new(0.0, 1.0));
        assert_eq!(off, 0);
        let (off, _) = l.offset_at(Point::new(500.0, 1.0));
        assert!(off > 0 && off <= lines[0].end + 1);
    }

    #[test]
    fn utf8_offsets_round_trip() {
        let text = "a€😀b";
        let l = TextLayout::plain(text, &body(), f32::INFINITY);
        let r = l.selection_rects(1..4);
        assert!(!r.is_empty());
        let caret_end = l.caret_rect(text.len(), false);
        let caret_mid = l.caret_rect(4, false);
        assert!(caret_end.x > caret_mid.x);
        assert_eq!(l.word_at(0), 0..1);
    }

    #[test]
    fn attributed_runs_and_attachments() {
        let mut a = AttributedString::plain("Hello \u{FFFC} world");
        a.set(0..5, body().with_weight(700), Some("https://silica".into()));
        a.attachments.push((
            6,
            Attachment {
                size: Size::new(20.0, 10.0),
                baseline: 10.0,
            },
        ));
        let l = TextLayout::new(&a, &body(), &ParagraphOptions::default(), f32::INFINITY);
        assert!(l.size().width > 60.0);
        assert_eq!(a.link_at(2), Some("https://silica"));
        assert_eq!(a.link_at(7), None);
    }
}
