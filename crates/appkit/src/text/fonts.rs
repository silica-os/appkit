//! Fonts: Inter, JetBrains Mono and Silica Symbols are bundled in the library (identical
//! rendering everywhere, no packaging dependency); fontconfig serves every other family and
//! the fallback (Noto Color Emoji, CJK).

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};

use parley::fontique::{Blob, Collection, CollectionOptions, SourceCache};
use parley::{FontContext, FontData, LayoutContext};

use crate::color::Color;

/// The bundled faces: (family, CSS weight, bytes).
static BUNDLED: &[(&str, u16, &[u8])] = &[
    (
        "Inter",
        400,
        include_bytes!("../../fonts/Inter-Regular.ttf"),
    ),
    ("Inter", 500, include_bytes!("../../fonts/Inter-Medium.ttf")),
    (
        "Inter",
        600,
        include_bytes!("../../fonts/Inter-SemiBold.ttf"),
    ),
    ("Inter", 700, include_bytes!("../../fonts/Inter-Bold.ttf")),
    (
        "JetBrains Mono",
        400,
        include_bytes!("../../fonts/JetBrainsMono-Regular.ttf"),
    ),
    (
        "Silica Symbols",
        300,
        include_bytes!("../../fonts/SilicaSymbols-Light.otf"),
    ),
    (
        "Silica Symbols",
        400,
        include_bytes!("../../fonts/SilicaSymbols-Regular.otf"),
    ),
    (
        "Silica Symbols",
        600,
        include_bytes!("../../fonts/SilicaSymbols-Semibold.otf"),
    ),
];

/// The UI font family.
pub const UI_FAMILY: &str = "Inter";
/// The symbol font family.
pub const SYMBOLS_FAMILY: &str = "Silica Symbols";
/// The monospaced family.
pub const MONO_FAMILY: &str = "JetBrains Mono";

/// The font system of the thread (all UI runs on the main thread).
pub struct Fonts {
    /// Families and faces (parley's font context).
    pub fcx: FontContext,
    /// Scratch space of the layout builder.
    pub lcx: LayoutContext<Color>,
    bundled: Vec<(String, u16, FontData)>,
    /// The system fonts (fontconfig): scanned on a thread of their own, since a cold cache
    /// can take a second and the first frame needs only the bundled faces.
    system: System,
    /// Symbol glyphs: (codepoint, weight) → (glyph id, advance per em).
    symbols: HashMap<(char, u16), Option<(u32, f32)>>,
}

impl std::fmt::Debug for Fonts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fonts")
            .field("bundled", &self.bundled.len())
            .finish()
    }
}

enum System {
    /// Not wanted (deterministic rendering).
    Off,
    /// Being made.
    Loading(Receiver<Collection>),
    /// Installed.
    Ready,
    /// The loader failed: no system fonts.
    Failed,
}

/// Bumped when the fonts change (the system fonts are loaded): text is laid out again.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// The fonts' generation (layouts made with an older one are stale).
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

thread_local! {
    static FONTS: RefCell<Option<Fonts>> = const { RefCell::new(None) };
}

fn register_bundled(collection: &mut Collection) -> Vec<(String, u16, FontData)> {
    let mut out = Vec::new();
    for (family, weight, bytes) in BUNDLED {
        let blob = Blob::new(std::sync::Arc::new(*bytes));
        let faces = collection.register_fonts(blob.clone(), None);
        if faces.is_empty() {
            tracing::error!("bundled font {family} {weight} failed to load");
            continue;
        }
        out.push(((*family).to_string(), *weight, FontData::new(blob, 0)));
    }
    out
}

impl Fonts {
    fn new(system_fallback: bool) -> Fonts {
        // The bundled faces load without scanning the system's fonts.
        let mut collection = Collection::new(CollectionOptions {
            shared: false,
            system_fonts: false,
        });
        let bundled = register_bundled(&mut collection);
        let system = if system_fallback {
            let (tx, rx) = std::sync::mpsc::channel();
            let spawned = std::thread::Builder::new()
                .name("appkit-fonts".into())
                .spawn(move || {
                    let mut c = Collection::new(CollectionOptions {
                        shared: false,
                        system_fonts: true,
                    });
                    // Touch the families so that the scan happens here, then lay out a line
                    // in each bundled family: the first lookups in a system collection are
                    // slow, and would land on the first frame that lays text out again.
                    let _ = c.family_names().count();
                    let _ = register_bundled(&mut c);
                    let mut fcx = FontContext {
                        collection: c,
                        source_cache: SourceCache::default(),
                    };
                    let mut lcx: LayoutContext<Color> = LayoutContext::new();
                    for (family, weight, _) in BUNDLED {
                        let mut b = lcx.ranged_builder(&mut fcx, "Ag Button", 1.0, false);
                        b.push_default(parley::StyleProperty::FontFamily(
                            parley::FontFamily::Source(
                                format!("\"{family}\", \"{UI_FAMILY}\"").into(),
                            ),
                        ));
                        b.push_default(parley::StyleProperty::FontWeight(parley::FontWeight::new(
                            f32::from(*weight),
                        )));
                        let mut l = b.build("Ag Button");
                        l.break_all_lines(None);
                    }
                    let _ = tx.send(fcx.collection);
                });
            match spawned {
                Ok(_) => System::Loading(rx),
                Err(_) => System::Failed,
            }
        } else {
            System::Off
        };
        let mut f = Fonts {
            fcx: FontContext {
                collection,
                source_cache: SourceCache::default(),
            },
            lcx: LayoutContext::new(),
            bundled,
            system,
            symbols: HashMap::new(),
        };
        f.install_system();
        f
    }

    /// Installs the system fonts if they finished loading; returns whether they just did.
    pub fn install_system(&mut self) -> bool {
        let System::Loading(rx) = &self.system else {
            return false;
        };
        match rx.try_recv() {
            Ok(c) => {
                self.fcx.collection = c;
                self.system = System::Ready;
                GENERATION.fetch_add(1, Ordering::Relaxed);
                true
            }
            Err(TryRecvError::Empty) => false,
            Err(TryRecvError::Disconnected) => {
                self.system = System::Failed;
                false
            }
        }
    }

    /// Runs `f` with the thread's font system, creating it on first use.
    pub fn with<R>(f: impl FnOnce(&mut Fonts) -> R) -> R {
        FONTS.with(|cell| {
            let mut slot = cell.borrow_mut();
            let fonts = slot.get_or_insert_with(|| Fonts::new(!deterministic()));
            // The system fonts join the fallback once they are loaded.
            fonts.install_system();
            f(fonts)
        })
    }

    /// A bundled face of `family` closest to `weight`.
    pub fn bundled(&self, family: &str, weight: u16) -> Option<FontData> {
        self.bundled
            .iter()
            .filter(|(f, _, _)| f == family)
            .min_by_key(|(_, w, _)| (i32::from(*w) - i32::from(weight)).abs())
            .map(|(_, _, d)| d.clone())
    }

    /// The glyph of a symbol and its advance per em, from the bundled Silica Symbols face
    /// closest to `weight`.
    pub fn symbol_glyph(&mut self, ch: char, weight: u16) -> Option<(FontData, u32, f32)> {
        let font = self.bundled(SYMBOLS_FAMILY, weight)?;
        if let Some(hit) = self.symbols.get(&(ch, weight)) {
            return hit.map(|(id, adv)| (font, id, adv));
        }
        let mut buf = [0u8; 4];
        let s: &str = ch.encode_utf8(&mut buf);
        let mut b = self.lcx.ranged_builder(&mut self.fcx, s, 1.0, false);
        b.push_default(parley::StyleProperty::FontFamily(
            parley::FontFamily::named(SYMBOLS_FAMILY),
        ));
        b.push_default(parley::StyleProperty::FontSize(100.0));
        b.push_default(parley::StyleProperty::FontWeight(parley::FontWeight::new(
            f32::from(weight),
        )));
        let mut layout: parley::Layout<Color> = b.build(s);
        layout.break_all_lines(None);
        let found = layout.lines().next().and_then(|line| {
            line.items().find_map(|item| match item {
                parley::PositionedLayoutItem::GlyphRun(run) => {
                    run.glyphs().next().map(|g| (g.id, g.advance / 100.0))
                }
                parley::PositionedLayoutItem::InlineBox(_) => None,
            })
        });
        self.symbols.insert((ch, weight), found);
        found.map(|(id, adv)| (font, id, adv))
    }
}

/// Whether text must render identically on every machine (tests, `APPKIT_DETERMINISTIC`):
/// only the bundled fonts, no system fallback.
pub fn deterministic() -> bool {
    cfg!(test) || std::env::var_os("APPKIT_DETERMINISTIC").is_some()
}

/// Silica Symbols glyphs by name (silica-icons `fonts/codepoints.json`).
pub mod symbols {
    include!(concat!(env!("OUT_DIR"), "/silica_symbols.rs"));

    /// The codepoint of the symbol `name` (`chevron-down`, `trash` …).
    pub fn codepoint(name: &str) -> Option<char> {
        SYMBOLS
            .binary_search_by(|(k, _)| (*k).cmp(name))
            .ok()
            .and_then(|i| SYMBOLS.get(i))
            .and_then(|(_, cp)| char::from_u32(*cp))
    }

    /// Paints one symbol: font size `size`, centred horizontally in `rect`, its em box
    /// (ascender = em, descender 0) filling the box vertically.
    pub fn paint(
        canvas: &mut crate::render::canvas::Canvas,
        rect: crate::geom::Rect,
        ch: char,
        size: f32,
        weight: u16,
        color: crate::color::Color,
    ) {
        let Some((font, id, adv)) = super::Fonts::with(|f| f.symbol_glyph(ch, weight)) else {
            return;
        };
        let x = rect.x + (rect.width - adv * size) / 2.0;
        // Ascender = em: the baseline is the box's bottom when the box is the font size.
        let y = rect.y + (rect.height + size) / 2.0;
        let glyphs = std::iter::once(vello_cpu::Glyph { id, x, y });
        canvas.fill_glyphs(&font, size, &[], glyphs, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_faces_load() {
        Fonts::with(|f| {
            assert!(f.bundled("Inter", 400).is_some());
            assert!(f.bundled("Inter", 650).is_some());
            assert!(f.bundled("Silica Symbols", 400).is_some());
            let ch = symbols::codepoint("chevron-down").unwrap();
            assert!(
                f.symbol_glyph(ch, 400)
                    .is_some_and(|(_, id, adv)| id > 0 && adv > 0.0)
            );
        });
        assert!(symbols::codepoint("chevron-down").is_some());
        assert!(symbols::codepoint("no-such-symbol").is_none());
    }
}
