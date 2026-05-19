//! history.rs - persiste clipboard history (50 entries, rotating).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const MAX_ENTRIES: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum ClipEntry {
    Text { content: String },
    ImageHash { hash: String, size_bytes: usize },
    Files { paths: Vec<PathBuf> },
}

impl ClipEntry {
    pub fn preview(&self, max_len: usize) -> String {
        match self {
            ClipEntry::Text { content } => {
                let s: String = content.chars().take(max_len).collect();
                if content.chars().count() > max_len {
                    format!("{s}...")
                } else {
                    s
                }
            }
            ClipEntry::ImageHash { hash, size_bytes } => {
                format!("[imagem {} bytes hash={}]", size_bytes, &hash[..8.min(hash.len())])
            }
            ClipEntry::Files { paths } => {
                let names: Vec<_> = paths
                    .iter()
                    .map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
                    .take(3)
                    .collect();
                format!("[arquivos: {}]", names.join(", "))
            }
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ClipHistory {
    pub entries: Vec<ClipEntry>,
}

impl ClipHistory {
    pub fn path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        PathBuf::from(format!("{home}/.local/share/lumo/clipboard-history.json"))
    }

    pub fn load() -> Self {
        let p = Self::path();
        match fs::read_to_string(&p) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn push(&mut self, entry: ClipEntry) {
        if self.entries.first() == Some(&entry) {
            return;
        }
        self.entries.retain(|e| e != &entry);
        self.entries.insert(0, entry);
        self.entries.truncate(MAX_ENTRIES);
        self.save();
    }

    pub fn save(&self) {
        let p = Self::path();
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).ok();
        }
        if let Ok(s) = serde_json::to_string_pretty(self) {
            fs::write(&p, s).ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_deduplicates_consecutive() {
        let mut h = ClipHistory::default();
        h.entries.push(ClipEntry::Text { content: "ola".into() });
        h.push(ClipEntry::Text { content: "ola".into() });
        assert_eq!(h.entries.len(), 1);
    }

    #[test]
    fn push_moves_duplicate_to_front() {
        let mut h = ClipHistory::default();
        h.entries.push(ClipEntry::Text { content: "a".into() });
        h.entries.push(ClipEntry::Text { content: "b".into() });
        h.push(ClipEntry::Text { content: "a".into() });
        assert_eq!(h.entries[0], ClipEntry::Text { content: "a".into() });
    }

    #[test]
    fn push_rotates_at_max() {
        let mut h = ClipHistory::default();
        for i in 0..MAX_ENTRIES + 5 {
            h.entries.push(ClipEntry::Text { content: format!("{i}") });
        }
        h.entries.truncate(MAX_ENTRIES);
        assert_eq!(h.entries.len(), MAX_ENTRIES);
    }

    #[test]
    fn text_preview_truncates() {
        let e = ClipEntry::Text { content: "abcdefghij".into() };
        assert_eq!(e.preview(5), "abcde...");
    }

    #[test]
    fn text_preview_exact_len() {
        let e = ClipEntry::Text { content: "hi".into() };
        assert_eq!(e.preview(10), "hi");
    }

    #[test]
    fn image_hash_preview() {
        let e = ClipEntry::ImageHash { hash: "abcd1234ef".into(), size_bytes: 1024 };
        let p = e.preview(100);
        assert!(p.contains("imagem"));
    }

    #[test]
    fn files_preview() {
        let e = ClipEntry::Files { paths: vec![std::path::PathBuf::from("/tmp/foo.txt")] };
        let p = e.preview(100);
        assert!(p.contains("foo.txt"));
    }
}
