//! Thumbnail cache para imagens.
//!
//! Gera thumbs 128x128 (Lanczos3) para image/jpeg, image/png, image/webp.
//! Cache em ~/.cache/lumo-files/thumbs/<sha256_path>.png.
//! LRU max 500 entries.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Cache de thumbnails em memoria.
pub struct ThumbCache {
    /// Path hash -> bytes PNG do thumb.
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

    /// Retorna thumb se ja carregado em memoria.
    pub fn get(&self, key: &str) -> Option<&Vec<u8>> {
        self.cache.get(key)
    }

    /// Insere thumb no cache LRU.
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

/// Retorna path do thumb em disco.
pub fn thumb_disk_path(key: &str) -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home)
        .join(".cache/lumo-files/thumbs")
        .join(format!("{}.png", key))
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

/// Gera thumbnail 128x128 e salva em disco.
/// Retorna bytes PNG do thumb.
/// Chamado em spawn_blocking.
pub fn generate_thumb(path: &Path, key: &str) -> Option<Vec<u8>> {
    let disk_path = thumb_disk_path(key);

    // Tenta carregar do disco primeiro
    if let Ok(data) = std::fs::read(&disk_path) {
        return Some(data);
    }

    // Gera novo thumb usando image crate
    let img = image::open(path).ok()?;
    let thumb = img.resize(128, 128, image::imageops::FilterType::Lanczos3);

    // Salva no disco
    if let Some(parent) = disk_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let mut buf = Vec::new();
    thumb
        .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .ok()?;
    let _ = std::fs::write(&disk_path, &buf);
    Some(buf)
}
