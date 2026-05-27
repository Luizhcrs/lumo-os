//! text_render.rs — render title text pra SSD titlebar.
//!
//! Pipeline:
//! 1. cosmic_text shape title string → glyphs + posicoes
//! 2. swash cache rasteriza glyph -> alpha bitmap
//! 3. tiny-skia compose em ARGB8888 premultiplied
//! 4. smithay MemoryRenderBuffer wrap pixels → render element
//!
//! Cache per-surface: HashMap<WlSurface, CachedTitle>. Invalida quando title
//! muda. Evita re-render todo frame (chamado so on title_changed).
//!
//! Thread-local FontSystem + SwashCache pq cosmic_text nao e Send.

use std::cell::RefCell;
use std::collections::HashMap;

use cosmic_text::{
    Attrs, Buffer, Color as CtColor, Family, FontSystem, Metrics, Shaping, SwashCache,
};
use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::memory::MemoryRenderBuffer;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::Transform;

thread_local! {
    static FONT_SYSTEM: RefCell<FontSystem> = RefCell::new(FontSystem::new());
    static SWASH_CACHE: RefCell<SwashCache> = RefCell::new(SwashCache::new());
}

#[derive(Clone)]
pub struct CachedTitle {
    pub title: String,
    pub buffer: MemoryRenderBuffer,
    pub width: u32,
    pub height: u32,
}

#[derive(Default)]
pub struct TitleTextCache {
    entries: HashMap<WlSurface, CachedTitle>,
}

impl TitleTextCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Retorna cached buffer pra surface. Re-renderiza se title mudou ou ausente.
    /// width = pixel width disponivel (titlebar - botoes - padding).
    pub fn get_or_render(
        &mut self,
        surface: &WlSurface,
        title: &str,
        width: u32,
        height: u32,
    ) -> Option<&CachedTitle> {
        if let Some(existing) = self.entries.get(surface) {
            if existing.title == title
                && existing.width == width
                && existing.height == height
            {
                return self.entries.get(surface);
            }
        }
        let pixels = render_title_to_argb(title, width, height)?;
        let buffer = MemoryRenderBuffer::from_slice(
            &pixels,
            Fourcc::Argb8888,
            (width as i32, height as i32),
            1,
            Transform::Normal,
            None,
        );
        self.entries.insert(
            surface.clone(),
            CachedTitle {
                title: title.to_string(),
                buffer,
                width,
                height,
            },
        );
        self.entries.get(surface)
    }

    pub fn remove(&mut self, surface: &WlSurface) {
        self.entries.remove(surface);
    }

    pub fn invalidate_all(&mut self) {
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

/// Renderiza title em ARGB8888 premultiplied buffer (Vec<u8>).
/// width/height em pixels. Retorna None se width=0 ou render falhou.
pub fn render_title_to_argb(title: &str, width: u32, height: u32) -> Option<Vec<u8>> {
    if width == 0 || height == 0 || title.is_empty() {
        return None;
    }
    let mut pixmap = tiny_skia::Pixmap::new(width, height)?;
    pixmap.fill(tiny_skia::Color::TRANSPARENT);

    FONT_SYSTEM.with(|fs| {
        let mut fs = fs.borrow_mut();
        let metrics = Metrics::new(13.0, 18.0);
        let mut buf = Buffer::new(&mut fs, metrics);
        buf.set_size(&mut fs, Some(width as f32), Some(height as f32));
        let attrs = Attrs::new().family(Family::SansSerif);
        buf.set_text(&mut fs, title, &attrs, Shaping::Advanced);
        // Centraliza vertical: line_y absoluto sobre height/2 do ascent.
        let line_y_offset = ((height as f32 - metrics.line_height) / 2.0).max(0.0);

        SWASH_CACHE.with(|sc| {
            let mut sc = sc.borrow_mut();
            for run in buf.layout_runs() {
                for glyph in run.glyphs.iter() {
                    let physical = glyph.physical((0., 0.), 1.0);
                    let color = CtColor::rgba(0xE0, 0xE0, 0xE0, 0xFF);
                    sc.with_pixels(&mut fs, physical.cache_key, color, |x, y, c| {
                        let px = physical.x + x;
                        let py =
                            (run.line_y + line_y_offset) as i32 + physical.y + y;
                        if px < 0
                            || py < 0
                            || (px as u32) >= width
                            || (py as u32) >= height
                        {
                            return;
                        }
                        let idx = ((py as u32) * width + (px as u32)) as usize * 4;
                        let pixels = pixmap.data_mut();
                        let a = c.a() as u32;
                        // tiny-skia usa RGBA premultiplied. Fourcc::Argb8888
                        // wayland = ARGB little-endian = B G R A em memoria.
                        pixels[idx] = (c.b() as u32 * a / 255) as u8;
                        pixels[idx + 1] = (c.g() as u32 * a / 255) as u8;
                        pixels[idx + 2] = (c.r() as u32 * a / 255) as u8;
                        pixels[idx + 3] = a as u8;
                    });
                }
            }
        });
    });

    Some(pixmap.data().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_empty_title_returns_none() {
        assert!(render_title_to_argb("", 200, 30).is_none());
    }

    #[test]
    fn render_zero_width_returns_none() {
        assert!(render_title_to_argb("test", 0, 30).is_none());
    }

    #[test]
    fn render_zero_height_returns_none() {
        assert!(render_title_to_argb("test", 200, 0).is_none());
    }

    #[test]
    fn render_returns_argb_buffer_with_expected_size() {
        let buf = render_title_to_argb("hello", 200, 30).expect("render ok");
        assert_eq!(buf.len(), 200 * 30 * 4, "ARGB8888 = 4 bytes/pixel");
    }

    #[test]
    fn render_writes_some_non_transparent_pixels() {
        // Glyphs preenchidos = pelo menos 1 pixel com alpha > 0.
        let buf = render_title_to_argb("Lumo", 200, 30).expect("render ok");
        let non_zero_count = buf
            .chunks_exact(4)
            .filter(|px| px[3] > 0)
            .count();
        assert!(non_zero_count > 0, "esperado >=1 pixel rendered, got 0");
    }

    #[test]
    fn cache_new_is_empty() {
        let c = TitleTextCache::new();
        assert_eq!(c.len(), 0);
    }
}
