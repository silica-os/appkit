//! Images: premultiplied RGBA8 pixels, shared and immutable once made.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use vello_cpu::Pixmap;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// A raster image (premultiplied RGBA8, row-major, no padding).
#[derive(Clone)]
pub struct Image {
    pixmap: Arc<Pixmap>,
    id: u64,
}

impl std::fmt::Debug for Image {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Image({}×{} #{})", self.width(), self.height(), self.id)
    }
}

impl PartialEq for Image {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Image {
    /// Wraps a pixmap.
    pub fn from_pixmap(pixmap: Pixmap) -> Image {
        Image {
            pixmap: Arc::new(pixmap),
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
        }
    }

    /// An image from premultiplied RGBA8 bytes; `None` when the sizes disagree or exceed
    /// 65535.
    pub fn from_premul_rgba(width: u32, height: u32, bytes: &[u8]) -> Option<Image> {
        let (w, h) = (u16::try_from(width).ok()?, u16::try_from(height).ok()?);
        if bytes.len() != width as usize * height as usize * 4 {
            return None;
        }
        let mut p = Pixmap::new(w, h);
        p.data_as_u8_slice_mut().copy_from_slice(bytes);
        p.recompute_may_have_transparency();
        Some(Image::from_pixmap(p))
    }

    /// An image from straight (not premultiplied) RGBA8 bytes.
    pub fn from_rgba(width: u32, height: u32, bytes: &[u8]) -> Option<Image> {
        let mut v = bytes.to_vec();
        for [r, g, b, a] in v.as_chunks_mut::<4>().0 {
            let alpha = u16::from(*a);
            for c in [r, g, b] {
                *c = ((u16::from(*c) * alpha + 127) / 255) as u8;
            }
        }
        Image::from_premul_rgba(width, height, &v)
    }

    /// Decodes a PNG.
    pub fn from_png(data: &[u8]) -> Option<Image> {
        Pixmap::from_png(std::io::Cursor::new(data))
            .ok()
            .map(Image::from_pixmap)
    }

    /// Renders an SVG fitted in `size` × `size` pixels, its aspect kept (app and file icons
    /// often ship as SVG). No text: icons draw none.
    pub fn from_svg(data: &[u8], size: u32) -> Option<Image> {
        use resvg::{tiny_skia, usvg};
        let tree = usvg::Tree::from_data(data, &usvg::Options::default()).ok()?;
        let s = tree.size();
        let k = size.min(4096) as f32 / s.width().max(s.height());
        if !k.is_finite() || k <= 0.0 {
            return None;
        }
        let w = (s.width() * k).round().max(1.0) as u32;
        let h = (s.height() * k).round().max(1.0) as u32;
        let mut pixmap = tiny_skia::Pixmap::new(w, h)?;
        resvg::render(
            &tree,
            tiny_skia::Transform::from_scale(k, k),
            &mut pixmap.as_mut(),
        );
        Image::from_premul_rgba(w, h, pixmap.data())
    }

    /// Encodes as PNG (straight alpha).
    pub fn encode_png(&self) -> Option<Vec<u8>> {
        Pixmap::clone(&self.pixmap).into_png().ok()
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        u32::from(self.pixmap.width())
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        u32::from(self.pixmap.height())
    }

    /// The premultiplied RGBA8 bytes.
    pub fn bytes(&self) -> &[u8] {
        self.pixmap.data_as_u8_slice()
    }

    /// The pixel at (x, y), premultiplied RGBA (transparent outside).
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        if x >= self.width() || y >= self.height() {
            return [0; 4];
        }
        let i = (y as usize * self.width() as usize + x as usize) * 4;
        match self.bytes().get(i..i + 4) {
            Some([r, g, b, a]) => [*r, *g, *b, *a],
            _ => [0; 4],
        }
    }

    /// The shared pixmap (for the rasterizer).
    pub fn pixmap(&self) -> &Arc<Pixmap> {
        &self.pixmap
    }

    /// A stable identity (GPU texture caches key on it).
    pub fn id(&self) -> u64 {
        self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_round_trip() {
        let img = Image::from_rgba(2, 1, &[255, 0, 0, 255, 0, 0, 255, 128]).unwrap();
        let png = img.encode_png().unwrap();
        let back = Image::from_png(&png).unwrap();
        assert_eq!((back.width(), back.height()), (2, 1));
        assert_eq!(back.pixel(0, 0), [255, 0, 0, 255]);
        assert_eq!(back.pixel(1, 0)[3], 128);
        assert!(Image::from_premul_rgba(3, 3, &[0; 4]).is_none());
    }

    #[test]
    fn svg_renders_fitted_in_its_size() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="8">
            <rect width="8" height="8" fill="red"/></svg>"#;
        let img = Image::from_svg(svg, 64).unwrap();
        assert_eq!((img.width(), img.height()), (64, 32));
        assert_eq!(img.pixel(10, 10), [255, 0, 0, 255]);
        assert_eq!(img.pixel(50, 10), [0; 4]);
        assert!(Image::from_svg(b"not svg", 64).is_none());
    }
}
