//! Glyph outlines, cached: a glyph is read from its font once (in em units, y down) and then
//! filled as a path. vello_cpu reads the font again for every run, which costs more than
//! filling a small glyph.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use parley::FontData;
use skrifa::instance::{LocationRef, NormalizedCoord, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::raw::TableProvider;
use skrifa::{FontRef, GlyphId, MetadataProvider, Tag};
use vello_cpu::kurbo::BezPath;

/// Entries kept before the cache starts over.
const MAX_GLYPHS: usize = 8192;

#[derive(Hash, PartialEq, Eq)]
struct Key {
    blob: u64,
    index: u32,
    glyph: u32,
    coords: Vec<i16>,
}

#[derive(Default)]
struct Cache {
    glyphs: HashMap<Key, Option<Rc<BezPath>>>,
    /// Whether a face (blob, index) has only plain outlines (no colour or bitmap glyphs).
    plain: HashMap<(u64, u32), bool>,
}

thread_local! {
    static CACHE: RefCell<Cache> = RefCell::new(Cache::default());
}

/// Whether the glyphs of `font` can be drawn from their outlines (no COLR, CBDT or sbix).
pub fn plain(font: &FontData) -> bool {
    let key = (font.data.id(), font.index);
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        *c.plain.entry(key).or_insert_with(|| {
            FontRef::from_index(font.data.data(), font.index).is_ok_and(|f| {
                [b"COLR", b"CBDT", b"sbix", b"SVG "]
                    .iter()
                    .all(|t| f.table_data(Tag::new(t)).is_none())
            })
        })
    })
}

/// The outline of `glyph` in em units (y down, origin on the baseline), or `None` when the
/// font has none for it.
pub fn outline(font: &FontData, coords: &[i16], glyph: u32) -> Option<Rc<BezPath>> {
    let key = Key {
        blob: font.data.id(),
        index: font.index,
        glyph,
        coords: coords.to_vec(),
    };
    CACHE.with(|c| {
        if let Some(hit) = c.borrow().glyphs.get(&key) {
            return hit.clone();
        }
        let path = read(font, coords, glyph).map(Rc::new);
        let mut c = c.borrow_mut();
        if c.glyphs.len() >= MAX_GLYPHS {
            c.glyphs.clear();
        }
        c.glyphs.insert(key, path.clone());
        path
    })
}

fn read(font: &FontData, coords: &[i16], glyph: u32) -> Option<BezPath> {
    let f = FontRef::from_index(font.data.data(), font.index).ok()?;
    let upem = f.head().ok()?.units_per_em();
    if upem == 0 {
        return None;
    }
    let outline = f.outline_glyphs().get(GlyphId::new(glyph))?;
    let coords: Vec<NormalizedCoord> = coords
        .iter()
        .map(|c| NormalizedCoord::from_bits(*c))
        .collect();
    let mut pen = Pen {
        path: BezPath::new(),
        k: 1.0 / f64::from(upem),
    };
    outline
        .draw(
            DrawSettings::unhinted(Size::unscaled(), LocationRef::new(&coords)),
            &mut pen,
        )
        .ok()?;
    Some(pen.path)
}

/// Collects an outline, scaled to em units and flipped to y down.
struct Pen {
    path: BezPath,
    k: f64,
}

impl Pen {
    fn p(&self, x: f32, y: f32) -> (f64, f64) {
        (f64::from(x) * self.k, -f64::from(y) * self.k)
    }
}

impl OutlinePen for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        let p = self.p(x, y);
        self.path.move_to(p);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.p(x, y);
        self.path.line_to(p);
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        let (c, p) = (self.p(cx0, cy0), self.p(x, y));
        self.path.quad_to(c, p);
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let (a, b, p) = (self.p(cx0, cy0), self.p(cx1, cy1), self.p(x, y));
        self.path.curve_to(a, b, p);
    }

    fn close(&mut self) {
        self.path.close_path();
    }
}
