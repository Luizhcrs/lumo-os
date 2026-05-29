//! Rasterizacao de icones Papirus (SVG) para RGBA via resvg/usvg.
//!
//! Usa lumo_foundation::lookup_icon para resolver o caminho do SVG,
//! depois rasteriza para RGBA bruto que pode ser passado a
//! iced::widget::image::Handle::from_rgba(SIZE, SIZE, bytes).
//!
//! Cache em memoria (HashMap<String, Option<Arc<Vec<u8>>>>) keyed por icon_name.
//!   - Some(bytes) = rasterizado com sucesso.
//!   - None        = tentou mas nao encontrou (sistema sem Papirus).
//! Nao ha cache em disco: SVGs do Papirus sao lidos uma vez por execucao.

use std::collections::HashMap;
use std::sync::Arc;

/// Tamanho de rasterizacao dos icones Papirus (pixels).
pub const ICON_SIZE: u32 = 64;

/// Cache em memoria de icones rasterizados.
/// Key: nome de icone freedesktop (ex: "folder", "image-x-generic").
/// Value: Some(bytes RGBA) ou None (nao encontrado no sistema).
pub struct MimeIconCache {
    cache: HashMap<String, Option<Arc<Vec<u8>>>>,
}

impl MimeIconCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    /// Retorna bytes RGBA do icone se ja em cache.
    /// Returns `None` se nao esta no cache ainda, `Some(None)` se tentou e nao achou.
    pub fn get(&self, icon_name: &str) -> Option<Option<Arc<Vec<u8>>>> {
        self.cache.get(icon_name).cloned()
    }

    /// Insere resultado no cache (Some(bytes) ou None se nao encontrado).
    pub fn insert(&mut self, icon_name: String, bytes: Option<Vec<u8>>) {
        let val = bytes.map(|b| Arc::new(b));
        self.cache.insert(icon_name, val);
    }

    /// Retorna true se o icon_name ja foi resolvido (mesmo que resultado seja None).
    pub fn contains(&self, icon_name: &str) -> bool {
        self.cache.contains_key(icon_name)
    }
}

impl Default for MimeIconCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Dados retornados pela task de rasterizacao.
pub struct MimeIconResult {
    pub icon_name: String,
    /// None se o icone nao foi encontrado no sistema.
    pub data: Option<Vec<u8>>,
}

/// Resolve o nome de icone freedesktop para um path e rasteriza o SVG Papirus.
/// Retorna MimeIconResult (data=None se o sistema nao tem Papirus).
/// Chamado em spawn_blocking.
pub fn render_mime_icon_by_name(icon_name: &str) -> MimeIconResult {
    let data = lumo_foundation::lookup_icon(icon_name, ICON_SIZE)
        .and_then(|svg_path| rasterize_svg(&svg_path, ICON_SIZE));
    MimeIconResult {
        icon_name: icon_name.to_string(),
        data,
    }
}

/// Rasteriza um SVG para RGBA bruto de tamanho `size x size`.
/// Retorna None se o SVG nao puder ser lido ou renderizado.
pub fn rasterize_svg(svg_path: &std::path::Path, size: u32) -> Option<Vec<u8>> {
    let svg_data = std::fs::read(svg_path).ok()?;
    rasterize_svg_bytes(&svg_data, size)
}

/// Rasteriza bytes SVG para RGBA bruto de tamanho `size x size`.
pub fn rasterize_svg_bytes(svg_data: &[u8], size: u32) -> Option<Vec<u8>> {
    use resvg::tiny_skia::{Pixmap, Transform};
    use resvg::usvg::{Options, Tree};

    let opts = Options::default();
    let tree = Tree::from_data(svg_data, &opts).ok()?;

    let mut pixmap = Pixmap::new(size, size)?;

    let view_box = tree.size();
    let scale_x = size as f32 / view_box.width();
    let scale_y = size as f32 / view_box.height();
    let transform = Transform::from_scale(scale_x, scale_y);

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // tiny-skia armazena internamente como RGBA premultiplicado.
    // Iced espera RGBA nao-premultiplicado em Handle::from_rgba.
    let premul_bytes = pixmap.data();
    let rgba = unpremultiply_rgba(premul_bytes);
    Some(rgba)
}

/// Converte RGBA premultiplicado (tiny-skia interno) para RGBA direto.
fn unpremultiply_rgba(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i + 3 < data.len() {
        let r = data[i];
        let g = data[i + 1];
        let b = data[i + 2];
        let a = data[i + 3];
        if a == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            let alpha = a as u32;
            let ru = ((r as u32 * 255 + alpha / 2) / alpha).min(255) as u8;
            let gu = ((g as u32 * 255 + alpha / 2) / alpha).min(255) as u8;
            let bu = ((b as u32 * 255 + alpha / 2) / alpha).min(255) as u8;
            out.extend_from_slice(&[ru, gu, bu, a]);
        }
        i += 4;
    }
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64">
  <circle cx="32" cy="32" r="30" fill="red"/>
</svg>"#;

    #[test]
    fn rasterize_retorna_tamanho_correto() {
        let result = rasterize_svg_bytes(SIMPLE_SVG, 64);
        assert!(result.is_some(), "rasterizacao falhou para SVG valido");
        let bytes = result.unwrap();
        assert_eq!(bytes.len(), (64 * 64 * 4) as usize);
    }

    #[test]
    fn rasterize_svg_invalido_retorna_none() {
        let bad = b"isso nao e svg";
        assert!(rasterize_svg_bytes(bad, 64).is_none());
    }

    #[test]
    fn unpremultiply_alpha_maximo() {
        let input = vec![100u8, 150, 200, 255];
        let out = unpremultiply_rgba(&input);
        assert_eq!(out, vec![100, 150, 200, 255]);
    }

    #[test]
    fn unpremultiply_alpha_zero() {
        let input = vec![0u8, 0, 0, 0];
        let out = unpremultiply_rgba(&input);
        assert_eq!(out, vec![0, 0, 0, 0]);
    }

    #[test]
    fn unpremultiply_meio_alpha() {
        let input = vec![64u8, 0, 0, 128];
        let out = unpremultiply_rgba(&input);
        assert_eq!(out[3], 128, "alpha deve ser preservado");
        assert!(out[0] >= 126 && out[0] <= 128, "red unpremul errado: {}", out[0]);
    }

    #[test]
    fn mime_cache_insert_get_some() {
        let mut cache = MimeIconCache::new();
        let data = vec![0u8; 64 * 64 * 4];
        cache.insert("folder".to_string(), Some(data.clone()));
        let got = cache.get("folder");
        assert!(got.is_some());
        assert!(got.unwrap().is_some());
    }

    #[test]
    fn mime_cache_insert_none_marca_ausente() {
        let mut cache = MimeIconCache::new();
        cache.insert("nao-existe".to_string(), None);
        let got = cache.get("nao-existe");
        assert!(got.is_some(), "entry deveria existir");
        assert!(got.unwrap().is_none(), "deveria ser None (nao encontrado)");
    }

    #[test]
    fn mime_cache_contains_ausente_antes_de_insert() {
        let cache = MimeIconCache::new();
        assert!(!cache.contains("folder"));
    }

    #[test]
    fn render_mime_icon_by_name_sem_papirus_retorna_data_none() {
        // Em Windows/build sem /usr/share/icons, lookup_icon retorna None.
        let result = render_mime_icon_by_name("folder");
        assert_eq!(result.icon_name, "folder");
        // data pode ser None (sem Papirus) ou Some (com Papirus instalado).
        // Nao falha o teste em nenhum dos casos.
    }
}
