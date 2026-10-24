//! The rows of a plain list (kit `Lists/Row`, ui-kit `SList`): a glyph on a coloured tile,
//! a title, an optional subtitle and an optional chevron, 44 pt tall (60 with a subtitle);
//! and the list's section header (kit `Lists/Header`).

use crate::amalfi::foundation::Label;
use crate::amalfi::tokens::{TypeStyle, typography};
use crate::core::a11y::{A11y, Role};
use crate::core::layout::Proposal;
use crate::core::value::{Prop, Value};
use crate::core::widget::{LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Rect, Size};
use crate::shape::Shape;
use crate::style::Style;
use crate::text::Align;

/// Measures of `Lists/Row` and `Lists/Header`.
pub mod metrics {
    /// A row with a title.
    pub const SINGLE: f32 = 44.0;
    /// A row with a title and a subtitle.
    pub const TWO_LINE: f32 = 60.0;
    /// The tile's leading edge and size.
    pub const TILE_X: f32 = crate::amalfi::tokens::spacing::L;
    /// The tile's size.
    pub const TILE: f32 = 28.0;
    /// The tile's corner radius.
    pub const TILE_RADIUS: f32 = 7.0;
    /// The glyph on the tile.
    pub const GLYPH: f32 = 16.0;
    /// Where the text starts (and the separator under it).
    pub const TEXT_X: f32 = 56.0;
    /// The title's top in a single row and in a two-line row.
    pub const TITLE_Y: (f32, f32) = (13.0, 11.0);
    /// The subtitle's top in a two-line row.
    pub const SUBTITLE_Y: f32 = 31.0;
    /// The text's end, before the chevron.
    pub const TEXT_END: f32 = 40.0;
    /// The chevron's box, its end inset and its glyph size.
    pub const ACCESSORY: f32 = 14.0;
    /// The chevron's end inset.
    pub const ACCESSORY_END: f32 = 20.0;
    /// The chevron's glyph size.
    pub const ACCESSORY_GLYPH: f32 = 12.0;
    /// The header's height and the top of its title.
    pub const HEADER: f32 = crate::amalfi::tokens::control_heights::REGULAR;
    /// The header title's top.
    pub const HEADER_TITLE_Y: f32 = 7.0;
}

/// A row of a plain list.
#[derive(Debug, Default)]
pub struct ListRow {
    /// The glyph on the tile (none: no tile, the text starts at the same place).
    pub symbol: Option<String>,
    /// The title.
    pub title: String,
    /// The subtitle (a two-line row).
    pub subtitle: Option<String>,
    /// A chevron at the end (the row opens something).
    pub chevron: bool,
    title_text: Label,
    subtitle_text: Label,
}

impl ListRow {
    /// A row with `title`.
    pub fn new(title: &str) -> ListRow {
        ListRow {
            title: title.to_string(),
            ..ListRow::default()
        }
    }

    /// The glyph on the tile.
    pub fn symbol(mut self, symbol: &str) -> Self {
        self.symbol = Some(symbol.to_string());
        self
    }

    /// A subtitle under the title.
    pub fn subtitle(mut self, subtitle: &str) -> Self {
        self.subtitle = Some(subtitle.to_string());
        self
    }

    /// A chevron at the end.
    pub fn chevron(mut self, on: bool) -> Self {
        self.chevron = on;
        self
    }

    /// The row's height.
    pub fn height(&self) -> f32 {
        if self.subtitle.is_some() {
            metrics::TWO_LINE
        } else {
            metrics::SINGLE
        }
    }
}

/// Draws one line of text.
fn line(
    cx: &mut PaintCx<'_>,
    text: &mut Label,
    name: &str,
    s: &str,
    rect: Rect,
    token: &'static TypeStyle,
    ink: crate::color::Color,
) {
    let theme = *cx.theme();
    let style = theme.text(token, ink);
    let layout = text.layout(s, &style, Align::Start, rect.width, Some(1));
    cx.rec.text(name, rect, layout, Some(token.name), ink);
}

impl Widget for ListRow {
    fn kind(&self) -> &'static str {
        "ListRow"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        Size::new(p.width.unwrap_or(320.0), self.height())
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let r = cx.bounds();
        let disabled = cx.is_disabled();
        let two = self.subtitle.is_some();
        if let Some(symbol) = self.symbol.clone() {
            // On the accent selection the tile is a white wash; disabled, it is grey.
            let tile = if theme.emphasized {
                crate::color::Color::WHITE.with_alpha(0x40)
            } else if disabled {
                c.text_tertiary
            } else {
                c.accent_amalfi
            };
            let at = Rect::new(
                r.x + metrics::TILE_X,
                r.y + ((r.height - metrics::TILE) / 2.0).round(),
                metrics::TILE,
                metrics::TILE,
            );
            cx.rec.layer(
                "Tile",
                at,
                Shape::rounded(metrics::TILE_RADIUS),
                Style::fill(tile),
            );
            let g = metrics::GLYPH;
            cx.rec.glyph_sized(
                "Glyph",
                Rect::new(
                    at.x + (metrics::TILE - g) / 2.0,
                    at.y + (metrics::TILE - g) / 2.0,
                    g,
                    g,
                ),
                g,
                &symbol,
                500,
                crate::color::Color::WHITE,
            );
        }
        let w = (r.width - metrics::TEXT_X - metrics::TEXT_END).max(0.0);
        let x = r.x + metrics::TEXT_X;
        let title_y = if two {
            metrics::TITLE_Y.1
        } else {
            metrics::TITLE_Y.0
        };
        let ink = if disabled {
            c.text_tertiary
        } else {
            c.text_primary
        };
        let title = self.title.clone();
        line(
            cx,
            &mut self.title_text,
            "Title",
            &title,
            Rect::new(x, r.y + title_y, w, 18.0),
            &typography::BODY,
            ink,
        );
        if let Some(sub) = self.subtitle.clone() {
            let ink = if disabled {
                c.text_tertiary
            } else {
                c.text_secondary
            };
            line(
                cx,
                &mut self.subtitle_text,
                "Subtitle",
                &sub,
                Rect::new(x, r.y + metrics::SUBTITLE_Y, w, 16.0),
                &typography::CALLOUT,
                ink,
            );
        }
        if self.chevron {
            let a = metrics::ACCESSORY;
            cx.rec.glyph_sized(
                "Accessory",
                Rect::new(
                    r.right() - metrics::ACCESSORY_END - a,
                    r.y + ((r.height - a) / 2.0).round(),
                    a,
                    a,
                ),
                metrics::ACCESSORY_GLYPH,
                "chevron-right",
                500,
                c.text_tertiary,
            );
        }
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::Title => value.as_str().map_or(PropChange::Unknown, |s| {
                self.title = s.to_string();
                PropChange::Paint
            }),
            Prop::Subtitle => match value {
                Value::None => {
                    self.subtitle = None;
                    PropChange::Layout
                }
                v => v.as_str().map_or(PropChange::Unknown, |s| {
                    self.subtitle = Some(s.to_string());
                    PropChange::Layout
                }),
            },
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Title => Some(Value::Str(self.title.clone())),
            Prop::Subtitle => self.subtitle.clone().map(Value::Str),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::ListItem;
        a.label = Some(self.title.clone());
        a.description = self.subtitle.clone();
    }
}

/// A plain list's section header (kit `Lists/Header`): its title in `Body Emphasis` on
/// `Text/Secondary`, 16 from the start.
#[derive(Debug, Default)]
pub struct ListHeader {
    /// The title.
    pub title: String,
    text: Label,
}

impl ListHeader {
    /// A header titled `title`.
    pub fn new(title: &str) -> ListHeader {
        ListHeader {
            title: title.to_string(),
            text: Label::default(),
        }
    }
}

impl Widget for ListHeader {
    fn kind(&self) -> &'static str {
        "ListHeader"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        Size::new(p.width.unwrap_or(320.0), metrics::HEADER)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let r = cx.bounds();
        let token = &typography::BODY_EMPHASIS;
        let ink = theme.colors.text_secondary;
        let style = theme.text(token, ink);
        let title = self.title.clone();
        let room = (r.width - 2.0 * metrics::TILE_X).max(1.0);
        let w = self.text.width(&title, &style).ceil().min(room);
        let layout = self.text.layout(&title, &style, Align::Start, w, Some(1));
        cx.rec.text(
            "Title",
            Rect::new(
                r.x + metrics::TILE_X,
                r.y + metrics::HEADER_TITLE_Y,
                w,
                18.0,
            ),
            layout,
            Some(token.name),
            ink,
        );
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::Title => value.as_str().map_or(PropChange::Unknown, |s| {
                self.title = s.to_string();
                PropChange::Paint
            }),
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Title => Some(Value::Str(self.title.clone())),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Heading;
        a.label = Some(self.title.clone());
    }
}
