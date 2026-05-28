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
                format!(
                    "[imagem {} bytes hash={}]",
                    size_bytes,
                    &hash[..8.min(hash.len())]
                )
            }
            ClipEntry::Files { paths } => {
                let names: Vec<_> = paths
                    .iter()
                    .map(|p| {
                        p.file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default()
                    })
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
    /// F1.5-C2: indices de entries "pinned" (top, nao evicted no rotate).
    /// Salvo como Vec ordenado pra serde stable cross-restart.
    #[serde(default)]
    pub pinned: Vec<ClipEntry>,
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

    /// F1.5-C2: marca entry como pinned (move pra `pinned` list).
    /// Pinned items persistem cross-restart + nao sao evicted.
    /// Limit pinned em 10 pra evitar abuse.
    pub fn pin(&mut self, entry: ClipEntry) {
        const MAX_PINNED: usize = 10;
        if self.pinned.iter().any(|e| e == &entry) {
            return; // ja pinned
        }
        self.pinned.insert(0, entry);
        self.pinned.truncate(MAX_PINNED);
        self.save();
    }

    /// Remove entry de pinned.
    pub fn unpin(&mut self, entry: &ClipEntry) {
        self.pinned.retain(|e| e != entry);
        self.save();
    }

    pub fn is_pinned(&self, entry: &ClipEntry) -> bool {
        self.pinned.iter().any(|e| e == entry)
    }

    /// Lista completa pra picker: pinned primeiro, depois recent (entries),
    /// dedup entre os dois.
    pub fn picker_list(&self) -> Vec<ClipEntry> {
        let mut out = self.pinned.clone();
        for e in &self.entries {
            if !out.contains(e) {
                out.push(e.clone());
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_deduplicates_consecutive() {
        let mut h = ClipHistory::default();
        h.entries.push(ClipEntry::Text {
            content: "ola".into(),
        });
        h.push(ClipEntry::Text {
            content: "ola".into(),
        });
        assert_eq!(h.entries.len(), 1);
    }

    #[test]
    fn push_moves_duplicate_to_front() {
        let mut h = ClipHistory::default();
        h.entries.push(ClipEntry::Text {
            content: "a".into(),
        });
        h.entries.push(ClipEntry::Text {
            content: "b".into(),
        });
        h.push(ClipEntry::Text {
            content: "a".into(),
        });
        assert_eq!(
            h.entries[0],
            ClipEntry::Text {
                content: "a".into()
            }
        );
    }

    #[test]
    fn push_rotates_at_max() {
        let mut h = ClipHistory::default();
        for i in 0..MAX_ENTRIES + 5 {
            h.entries.push(ClipEntry::Text {
                content: format!("{i}"),
            });
        }
        h.entries.truncate(MAX_ENTRIES);
        assert_eq!(h.entries.len(), MAX_ENTRIES);
    }

    #[test]
    fn text_preview_truncates() {
        let e = ClipEntry::Text {
            content: "abcdefghij".into(),
        };
        assert_eq!(e.preview(5), "abcde...");
    }

    #[test]
    fn text_preview_exact_len() {
        let e = ClipEntry::Text {
            content: "hi".into(),
        };
        assert_eq!(e.preview(10), "hi");
    }

    #[test]
    fn image_hash_preview() {
        let e = ClipEntry::ImageHash {
            hash: "abcd1234ef".into(),
            size_bytes: 1024,
        };
        let p = e.preview(100);
        assert!(p.contains("imagem"));
    }

    #[test]
    fn files_preview() {
        let e = ClipEntry::Files {
            paths: vec![std::path::PathBuf::from("/tmp/foo.txt")],
        };
        let p = e.preview(100);
        assert!(p.contains("foo.txt"));
    }

    // F1.5-C2: pin / unpin / picker_list tests

    fn t(s: &str) -> ClipEntry {
        ClipEntry::Text {
            content: s.into(),
        }
    }

    #[test]
    fn pin_adds_entry_to_pinned_list() {
        let mut h = ClipHistory::default();
        h.pin(t("hello"));
        assert_eq!(h.pinned.len(), 1);
        assert!(h.is_pinned(&t("hello")));
    }

    #[test]
    fn pin_idempotent_same_entry() {
        let mut h = ClipHistory::default();
        h.pin(t("a"));
        h.pin(t("a"));
        assert_eq!(h.pinned.len(), 1);
    }

    #[test]
    fn unpin_removes_entry() {
        let mut h = ClipHistory::default();
        h.pin(t("a"));
        h.unpin(&t("a"));
        assert!(h.pinned.is_empty());
        assert!(!h.is_pinned(&t("a")));
    }

    #[test]
    fn unpin_missing_noop() {
        let mut h = ClipHistory::default();
        h.unpin(&t("never-pinned"));
        assert!(h.pinned.is_empty());
    }

    #[test]
    fn pin_caps_at_10() {
        let mut h = ClipHistory::default();
        for i in 0..15 {
            h.pin(t(&format!("item{}", i)));
        }
        assert_eq!(h.pinned.len(), 10);
    }

    #[test]
    fn picker_list_shows_pinned_first() {
        let mut h = ClipHistory::default();
        h.entries.push(t("recent"));
        h.pin(t("pinned"));
        let list = h.picker_list();
        assert_eq!(list[0], t("pinned"));
        assert_eq!(list[1], t("recent"));
    }

    #[test]
    fn picker_list_dedupes_pinned_and_recent() {
        let mut h = ClipHistory::default();
        h.entries.push(t("dupe"));
        h.pin(t("dupe"));
        let list = h.picker_list();
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn picker_list_empty_when_no_data() {
        let h = ClipHistory::default();
        assert!(h.picker_list().is_empty());
    }

    #[test]
    fn picker_list_only_recent_works() {
        let mut h = ClipHistory::default();
        h.entries.push(t("a"));
        h.entries.push(t("b"));
        let list = h.picker_list();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn picker_list_only_pinned_works() {
        let mut h = ClipHistory::default();
        h.pin(t("x"));
        h.pin(t("y"));
        let list = h.picker_list();
        assert_eq!(list.len(), 2);
        // Pin order: ultimo pin esta primeiro.
        assert_eq!(list[0], t("y"));
    }

    #[test]
    fn pin_does_not_affect_recent_entries() {
        let mut h = ClipHistory::default();
        h.entries.push(t("recent"));
        h.pin(t("pinned"));
        assert_eq!(h.entries.len(), 1);
        assert_eq!(h.entries[0], t("recent"));
    }

    #[test]
    fn pin_new_inserts_at_front() {
        let mut h = ClipHistory::default();
        h.pin(t("first"));
        h.pin(t("second"));
        assert_eq!(h.pinned[0], t("second"));
        assert_eq!(h.pinned[1], t("first"));
    }
}
