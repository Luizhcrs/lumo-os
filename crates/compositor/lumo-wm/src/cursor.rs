//! Cursor xcursor loader - carrega tema do sistema e prepara pixels
//! em formato pre-multiplicado pronto pra MemoryRenderBuffer
//! Fourcc::Abgr8888.
//!
//! Fase 5.4 A7: A6.5 colocou xcursor real mas saiu azul translucido.
//! Causa: xcursor::Image.pixels_argb e ordem de bytes A,R,G,B em
//! memoria; MemoryRenderBuffer com Fourcc::Argb8888 LE espera B,G,R,A.
//! Resultado: A->B, B->A => azul + alpha quebrado.
//!
//! Estrategia A7:
//!   1. Usar `pixels_rgba` (bytes [R,G,B,A]) - vem direto do file format.
//!   2. Pre-multiplicar RGB pelo alpha/255 (xcursor e straight alpha;
//!      Smithay GlesRenderer assume premultiplied).
//!   3. No state.rs montar MemoryRenderBuffer com Fourcc::Abgr8888 que
//!      em LE espera bytes [R,G,B,A] - bate com `pixels_rgba`.
//!
//! Fallback: se theme nao achado ou parse falhar, retorna None.

use std::path::PathBuf;

/// Resultado bem-sucedido do carregamento do cursor.
pub struct LoadedCursor {
    /// Pixels em formato RGBA8888 pre-multiplicado (4 bytes/pixel,
    /// ordem em memoria [R, G, B, A]). Pronto pra Fourcc::Abgr8888.
    pub pixels: Vec<u8>,
    /// Largura real da imagem em pixels.
    pub width: u32,
    /// Altura real da imagem em pixels.
    pub height: u32,
    /// Hotspot x (pixel onde "esta" a ponta da seta).
    pub hotspot_x: i32,
    /// Hotspot y.
    pub hotspot_y: i32,
    /// Nome do tema que efetivamente entregou o icone (debug).
    pub theme_name: String,
}

/// Converte buffer RGBA straight alpha em RGBA premultiplied.
/// Mantem ordem de bytes [R, G, B, A].
fn premultiply_rgba_inplace(buf: &mut [u8]) {
    for px in buf.chunks_exact_mut(4) {
        let a = px[3] as u32;
        // Premul: c' = c * a / 255 (round nearest).
        px[0] = ((px[0] as u32 * a + 127) / 255) as u8;
        px[1] = ((px[1] as u32 * a + 127) / 255) as u8;
        px[2] = ((px[2] as u32 * a + 127) / 255) as u8;
    }
}

/// Tenta carregar um cursor padrao do tema dado. Retorna None se nao
/// achar - chamador deve cair pra stub.
pub fn try_load(theme_name: &str, preferred_size: u32) -> Option<LoadedCursor> {
    let theme = xcursor::CursorTheme::load(theme_name);

    // Nomes comuns pro ponteiro padrao - tentar nessa ordem.
    let candidates = ["default", "left_ptr", "arrow", "top_left_arrow"];

    let path: PathBuf = candidates.iter().find_map(|n| theme.load_icon(n))?;

    let bytes = std::fs::read(&path).ok()?;
    let images = xcursor::parser::parse_xcursor(&bytes)?;

    if images.is_empty() {
        return None;
    }

    // Escolhe imagem com nominal size mais proximo do desejado.
    let img = images
        .iter()
        .min_by_key(|i| (i.size as i32 - preferred_size as i32).abs())
        .unwrap_or(&images[0]);

    // Clona pixels_rgba (ordem [R,G,B,A] em bytes) e pre-multiplica.
    let mut pixels = img.pixels_rgba.clone();
    premultiply_rgba_inplace(&mut pixels);

    Some(LoadedCursor {
        pixels,
        width: img.width,
        height: img.height,
        hotspot_x: img.xhot as i32,
        hotspot_y: img.yhot as i32,
        theme_name: theme_name.to_string(),
    })
}

/// Tenta uma sequencia de temas comuns. Primeiro que carregar vence.
pub fn try_load_first_available(preferred_size: u32) -> Option<LoadedCursor> {
    let themes = ["default", "Adwaita", "Bibata-Modern-Classic", "Qogir"];
    for t in themes.iter() {
        if let Some(c) = try_load(t, preferred_size) {
            tracing::info!(
                theme = c.theme_name,
                w = c.width,
                h = c.height,
                hx = c.hotspot_x,
                hy = c.hotspot_y,
                "xcursor carregado (RGBA premul, Fourcc::Abgr8888)"
            );
            return Some(c);
        }
    }
    tracing::warn!("nenhum tema xcursor encontrado - fallback SolidColor stub");
    None
}

/// Loads a cursor by a specific xcursor name (not the default arrow).
/// Falls back to first available theme if specific name not in theme.
pub fn try_load_named(name: &str, preferred_size: u32) -> Option<LoadedCursor> {
    let themes = ["default", "Adwaita", "Bibata-Modern-Classic", "Qogir"];
    for t in themes.iter() {
        let theme = xcursor::CursorTheme::load(t);
        if let Some(path) = theme.load_icon(name) {
            let bytes = std::fs::read(&path).ok()?;
            let images = xcursor::parser::parse_xcursor(&bytes)?;
            if images.is_empty() {
                continue;
            }
            let img = images
                .iter()
                .min_by_key(|i| (i.size as i32 - preferred_size as i32).abs())
                .unwrap_or(&images[0]);
            let mut pixels = img.pixels_rgba.clone();
            premultiply_rgba_inplace(&mut pixels);
            return Some(LoadedCursor {
                pixels,
                width: img.width,
                height: img.height,
                hotspot_x: img.xhot as i32,
                hotspot_y: img.yhot as i32,
                theme_name: t.to_string(),
            });
        }
    }
    None
}
