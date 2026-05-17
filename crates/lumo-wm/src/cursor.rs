//! Cursor xcursor loader - carrega tema do sistema e parseia o arquivo
//! Xcursor binario em pixels RGBA prontos pro MemoryRenderBuffer.
//!
//! Fase 5.4 fix 5: substitui o stub SolidColorRenderElement 10x14 por
//! um cursor real (seta Adwaita/default ou Bibata se instalado).
//!
//! Estrategia:
//!   1. xcursor::CursorTheme::load("default") -> resolve search paths
//!      ($XCURSOR_PATH, ~/.icons, /usr/share/icons).
//!   2. load_icon("default") com fallback "left_ptr" -> PathBuf .xcursor.
//!   3. xcursor::parser::parse_xcursor -> Vec<Image>.
//!   4. Escolhe imagem nominal size 24 (ou primeira disponivel).
//!   5. Converte ARGB (xcursor) -> RGBA (Smithay nao mexe em swizzle,
//!      mas Fourcc::Argb8888 espera A,R,G,B little-endian que em buffer
//!      bytes lido como B,G,R,A em LE - na pratica usamos pixels_argb
//!      direto com Fourcc::Argb8888 e Smithay GlesRenderer trata).
//!
//! Fallback: se theme nao achado ou parse falhar, retorna None e o
//! compositor mantem o stub SolidColor original.

use std::path::PathBuf;

/// Resultado bem-sucedido do carregamento do cursor.
pub struct LoadedCursor {
    /// Pixels em formato ARGB8888 (4 bytes/pixel, AABBGGRR no buffer LE
    /// que e o que xcursor entrega em `pixels_argb`).
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

/// Tenta carregar um cursor padrao do tema dado. Retorna None se nao
/// achar - chamador deve cair pra stub.
pub fn try_load(theme_name: &str, preferred_size: u32) -> Option<LoadedCursor> {
    let theme = xcursor::CursorTheme::load(theme_name);

    // Nomes comuns pro ponteiro padrao - tentar nessa ordem.
    let candidates = ["default", "left_ptr", "arrow", "top_left_arrow"];

    let path: PathBuf = candidates
        .iter()
        .find_map(|n| theme.load_icon(n))?;

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

    Some(LoadedCursor {
        pixels: img.pixels_argb.clone(),
        width: img.width,
        height: img.height,
        hotspot_x: img.xhot as i32,
        hotspot_y: img.yhot as i32,
        theme_name: theme_name.to_string(),
    })
}

/// Tenta uma sequencia de temas comuns. Primeiro que carregar vence.
pub fn try_load_first_available(preferred_size: u32) -> Option<LoadedCursor> {
    let themes = [
        "default",
        "Adwaita",
        "Bibata-Modern-Classic",
        "Qogir",
    ];
    for t in themes.iter() {
        if let Some(c) = try_load(t, preferred_size) {
            tracing::info!(
                theme = c.theme_name,
                w = c.width,
                h = c.height,
                hx = c.hotspot_x,
                hy = c.hotspot_y,
                "xcursor carregado"
            );
            return Some(c);
        }
    }
    tracing::warn!("nenhum tema xcursor encontrado - fallback SolidColor stub");
    None
}
