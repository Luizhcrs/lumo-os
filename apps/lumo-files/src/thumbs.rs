//! Thumbnail cache para imagens.
//!
//! Gera thumbs 128x128 (Lanczos3) para image/jpeg, image/png, image/webp.
//! Cache em ~/.cache/lumo-files/thumbs/<hash>_<w>x<h>.raw (RGBA bruto).
//! LRU max 500 entries.
//!
//! FORMATO INTERNO: os bytes armazenados no cache sao RGBA bruto (4 bytes/pixel),
//! sem cabecalho. O tamanho em pixels e sempre THUMB_SIZE x THUMB_SIZE.
//! Use `Handle::from_rgba(THUMB_SIZE, THUMB_SIZE, bytes)` para criar o handle Iced.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Tamanho do thumbnail gerado (pixels).
pub const THUMB_SIZE: u32 = 128;

/// Cache de thumbnails em memoria.
/// Armazena bytes RGBA brutos (4 * THUMB_SIZE * THUMB_SIZE bytes por entry).
pub struct ThumbCache {
    /// Path hash -> bytes RGBA brutos (sem cabecalho).
    cache: HashMap<String, Vec<u8>>,
    /// Ordem de acesso (LRU simplificado: fila FIFO de chaves).
    order: std::collections::VecDeque<String>,
    max: usize,
}

impl ThumbCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            order: std::collections::VecDeque::new(),
            max: 500,
        }
    }

    /// Retorna bytes RGBA brutos se ja carregados em memoria.
    pub fn get(&self, key: &str) -> Option<&Vec<u8>> {
        self.cache.get(key)
    }

    /// Insere bytes RGBA brutos no cache LRU.
    pub fn insert(&mut self, key: String, data: Vec<u8>) {
        if self.cache.len() >= self.max {
            if let Some(oldest) = self.order.pop_front() {
                self.cache.remove(&oldest);
            }
        }
        self.cache.insert(key.clone(), data);
        self.order.push_back(key);
    }
}

impl Default for ThumbCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Retorna chave de cache para um path.
pub fn cache_key(path: &Path) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    path.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// Retorna path do thumb em disco (bytes RGBA brutos).
pub fn thumb_disk_path(key: &str) -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home)
        .join(".cache/lumo-files/thumbs")
        .join(format!("{}.rgba", key))
}

/// Retorna true se o arquivo e uma imagem suportada.
pub fn is_image(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase());
    matches!(
        ext.as_deref(),
        Some("jpg") | Some("jpeg") | Some("png") | Some("webp") | Some("gif") | Some("bmp")
    )
}

/// Gera thumbnail THUMB_SIZE x THUMB_SIZE e salva em disco como bytes RGBA brutos.
/// Retorna bytes RGBA (4 * THUMB_SIZE * THUMB_SIZE bytes).
/// Chamado em spawn_blocking.
pub fn generate_thumb(path: &Path, key: &str) -> Option<Vec<u8>> {
    let disk_path = thumb_disk_path(key);

    // Tenta carregar do disco primeiro: valida tamanho esperado.
    let expected_len = (THUMB_SIZE * THUMB_SIZE * 4) as usize;
    if let Ok(data) = std::fs::read(&disk_path) {
        if data.len() == expected_len {
            return Some(data);
        }
        // Arquivo corrompido ou formato antigo (PNG): apaga e regenera.
        let _ = std::fs::remove_file(&disk_path);
    }

    // Decodifica a imagem original.
    let img = image::open(path).ok()?;

    // Redimensiona para THUMB_SIZE x THUMB_SIZE.
    let thumb = img.resize_to_fill(THUMB_SIZE, THUMB_SIZE, image::imageops::FilterType::Lanczos3);

    // Converte para RGBA8 (4 bytes/pixel, alpha nao-premultiplicado).
    let rgba = thumb.into_rgba8();

    // Verifica dimensoes apos resize.
    let (w, h) = rgba.dimensions();
    if w == 0 || h == 0 {
        return None;
    }

    let raw_bytes: Vec<u8> = rgba.into_raw();

    // Salva bytes brutos no disco para cache persistente.
    if let Some(parent) = disk_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&disk_path, &raw_bytes);

    Some(raw_bytes)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn cache_insert_e_get_round_trip() {
        let mut c = ThumbCache::new();
        let key = "abc123".to_string();
        let data: Vec<u8> = vec![255u8; (THUMB_SIZE * THUMB_SIZE * 4) as usize];
        c.insert(key.clone(), data.clone());
        assert_eq!(c.get(&key), Some(&data));
    }

    #[test]
    fn cache_key_deterministico() {
        let p = PathBuf::from("/home/user/foto.png");
        assert_eq!(cache_key(&p), cache_key(&p));
    }

    #[test]
    fn cache_key_diferente_para_paths_diferentes() {
        let a = PathBuf::from("/a/b.png");
        let b = PathBuf::from("/a/c.png");
        assert_ne!(cache_key(&a), cache_key(&b));
    }

    #[test]
    fn is_image_reconhece_extensoes() {
        assert!(is_image(&PathBuf::from("foto.jpg")));
        assert!(is_image(&PathBuf::from("foto.jpeg")));
        assert!(is_image(&PathBuf::from("foto.PNG")));
        assert!(is_image(&PathBuf::from("anim.webp")));
        assert!(!is_image(&PathBuf::from("doc.pdf")));
        assert!(!is_image(&PathBuf::from("noext")));
    }

    #[test]
    fn generate_thumb_retorna_none_para_nao_imagem() {
        // Arquivo de texto nao e imagem valida.
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("test.txt");
        std::fs::write(&f, b"hello").unwrap();
        let key = cache_key(&f);
        assert!(generate_thumb(&f, &key).is_none());
    }

    #[test]
    fn generate_thumb_rgba_size_correto() {
        // Cria um PNG 10x10 e verifica que o thumb tem THUMB_SIZE*THUMB_SIZE*4 bytes.
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("test.png");

        // Cria PNG minimo 10x10 RGB.
        let img = image::RgbImage::from_fn(10, 10, |x, y| {
            image::Rgb([(x * 25) as u8, (y * 25) as u8, 128])
        });
        img.save(&f).unwrap();

        let key = cache_key(&f);
        let result = generate_thumb(&f, &key);
        assert!(result.is_some(), "esperava Some(bytes) para PNG valido");
        let bytes = result.unwrap();
        let expected = (THUMB_SIZE * THUMB_SIZE * 4) as usize;
        assert_eq!(
            bytes.len(),
            expected,
            "tamanho RGBA errado: {} vs {}",
            bytes.len(),
            expected
        );
    }
}
