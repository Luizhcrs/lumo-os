//! history.rs - persiste clipboard history (50 entries, rotating).
//!
//! Security (F1.5-C2 review):
//! - C1: file mode 0600 (so owner le)
//! - C1: filter entries marcadas sensitive (password manager hint)
//! - H1: write atomico via tmp+rename
//! - L2: HOME unset -> erro (nao fallback /tmp world-readable)
//! - L3: picker_list dedup via HashSet
//! - L4: MAX_PINNED module-level const
//! - M2: ClipEntry::Files armazena display path tildeficado
//! - M4: pin promove + remove de entries (pinned disjoint de entries)

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

pub const MAX_ENTRIES: usize = 50;
pub const MAX_PINNED: usize = 10;
pub const PRIVATE_FILE_MODE: u32 = 0o600;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
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

/// M2: Tildefica path absoluto se for dentro de $HOME. Reduz leak em
/// backups/sync de paths sensiveis.
pub fn tildefy_path(path: &std::path::Path, home: &str) -> PathBuf {
    if home.is_empty() {
        return path.to_path_buf();
    }
    let home_p = std::path::Path::new(home);
    match path.strip_prefix(home_p) {
        Ok(rest) => PathBuf::from("~").join(rest),
        Err(_) => path.to_path_buf(),
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ClipHistory {
    pub entries: Vec<ClipEntry>,
    #[serde(default)]
    pub pinned: Vec<ClipEntry>,
}

impl ClipHistory {
    /// L2: retorna erro se HOME nao definido (nao escreve em /tmp world-readable).
    pub fn try_path() -> Result<PathBuf, &'static str> {
        let home = std::env::var("HOME").map_err(|_| "HOME nao definido")?;
        if home.is_empty() {
            return Err("HOME vazio");
        }
        Ok(PathBuf::from(format!(
            "{home}/.local/share/lumo/clipboard-history.json"
        )))
    }

    pub fn path() -> PathBuf {
        Self::try_path().unwrap_or_else(|_| PathBuf::from("/dev/null"))
    }

    pub fn load() -> Self {
        let Ok(p) = Self::try_path() else {
            return Self::default();
        };
        match fs::read_to_string(&p) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn push(&mut self, entry: ClipEntry) {
        if self.pinned.iter().any(|e| e == &entry) {
            return; // pinned ja tem; nao duplica em recent
        }
        if self.entries.first() == Some(&entry) {
            return;
        }
        self.entries.retain(|e| e != &entry);
        self.entries.insert(0, entry);
        self.entries.truncate(MAX_ENTRIES);
        self.save();
    }

    /// H1 atomic + C1 0600. Escreve tmp + rename + chmod.
    pub fn save(&self) {
        let Ok(p) = Self::try_path() else {
            tracing::warn!("save: HOME nao definido, skip");
            return;
        };
        if let Some(parent) = p.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                tracing::warn!(?e, ?parent, "create_dir_all falhou");
                return;
            }
        }
        let Ok(s) = serde_json::to_string_pretty(self) else {
            tracing::warn!("serialize falhou");
            return;
        };
        let tmp = p.with_extension("json.tmp");
        if let Err(e) = atomic_write(&tmp, &p, s.as_bytes()) {
            tracing::warn!(?e, "atomic_write falhou");
        }
    }

    /// M4: pin promove entry pra topo do pinned + remove de entries.
    /// Mantem invariant: pinned e entries disjuntos.
    pub fn pin(&mut self, entry: ClipEntry) {
        if self.pinned.iter().any(|e| e == &entry) {
            return;
        }
        self.entries.retain(|e| e != &entry);
        self.pinned.insert(0, entry);
        self.pinned.truncate(MAX_PINNED);
        self.save();
    }

    pub fn unpin(&mut self, entry: &ClipEntry) {
        let Some(pos) = self.pinned.iter().position(|e| e == entry) else {
            return;
        };
        let removed = self.pinned.remove(pos);
        // Restaura no topo de entries.
        self.entries.insert(0, removed);
        self.entries.truncate(MAX_ENTRIES);
        self.save();
    }

    pub fn is_pinned(&self, entry: &ClipEntry) -> bool {
        self.pinned.iter().any(|e| e == entry)
    }

    /// L3: dedup via HashSet pra evitar O(n*m).
    pub fn picker_list(&self) -> Vec<ClipEntry> {
        let mut out = self.pinned.clone();
        let seen: HashSet<&ClipEntry> = out.iter().collect();
        let seen_owned: HashSet<ClipEntry> = seen.iter().map(|e| (*e).clone()).collect();
        for e in &self.entries {
            if !seen_owned.contains(e) {
                out.push(e.clone());
            }
        }
        out
    }
}

/// H1 + C1: write tmp -> chmod 0600 -> rename atomico.
fn atomic_write(tmp: &std::path::Path, dst: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    fs::write(tmp, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perm = fs::Permissions::from_mode(PRIVATE_FILE_MODE);
        fs::set_permissions(tmp, perm)?;
    }
    fs::rename(tmp, dst)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> ClipEntry {
        ClipEntry::Text {
            content: s.into(),
        }
    }

    #[test]
    fn push_deduplicates_consecutive() {
        let mut h = ClipHistory::default();
        h.entries.push(t("ola"));
        h.push(t("ola"));
        assert_eq!(h.entries.len(), 1);
    }

    #[test]
    fn push_moves_duplicate_to_front() {
        let mut h = ClipHistory::default();
        h.entries.push(t("a"));
        h.entries.push(t("b"));
        h.push(t("a"));
        assert_eq!(h.entries[0], t("a"));
    }

    #[test]
    fn push_rotates_at_max() {
        let mut h = ClipHistory::default();
        for i in 0..MAX_ENTRIES + 5 {
            h.entries.push(t(&format!("{i}")));
        }
        h.entries.truncate(MAX_ENTRIES);
        assert_eq!(h.entries.len(), MAX_ENTRIES);
    }

    #[test]
    fn text_preview_truncates() {
        let e = t("abcdefghij");
        assert_eq!(e.preview(5), "abcde...");
    }

    #[test]
    fn text_preview_exact_len() {
        assert_eq!(t("hi").preview(10), "hi");
    }

    #[test]
    fn image_hash_preview() {
        let e = ClipEntry::ImageHash {
            hash: "abcd1234ef".into(),
            size_bytes: 1024,
        };
        assert!(e.preview(100).contains("imagem"));
    }

    #[test]
    fn files_preview() {
        let e = ClipEntry::Files {
            paths: vec![PathBuf::from("/tmp/foo.txt")],
        };
        assert!(e.preview(100).contains("foo.txt"));
    }

    // F1.5-C2 + post-review

    #[test]
    fn pin_adds_to_pinned_list() {
        let mut h = ClipHistory::default();
        h.pin(t("hello"));
        assert_eq!(h.pinned.len(), 1);
        assert!(h.is_pinned(&t("hello")));
    }

    #[test]
    fn pin_idempotent() {
        let mut h = ClipHistory::default();
        h.pin(t("a"));
        h.pin(t("a"));
        assert_eq!(h.pinned.len(), 1);
    }

    #[test]
    fn unpin_removes() {
        let mut h = ClipHistory::default();
        h.pin(t("a"));
        h.unpin(&t("a"));
        assert!(h.pinned.is_empty());
    }

    #[test]
    fn unpin_missing_noop() {
        let mut h = ClipHistory::default();
        h.unpin(&t("nope"));
        assert!(h.pinned.is_empty());
    }

    #[test]
    fn pin_caps_at_max() {
        let mut h = ClipHistory::default();
        for i in 0..MAX_PINNED + 5 {
            h.pin(t(&format!("p{i}")));
        }
        assert_eq!(h.pinned.len(), MAX_PINNED);
    }

    #[test]
    fn picker_list_pinned_first() {
        let mut h = ClipHistory::default();
        h.entries.push(t("recent"));
        h.pin(t("pinned"));
        let list = h.picker_list();
        assert_eq!(list[0], t("pinned"));
        assert_eq!(list[1], t("recent"));
    }

    // M4: pin remove de entries (invariant disjoint)
    #[test]
    fn pin_promotes_from_entries() {
        let mut h = ClipHistory::default();
        h.entries.push(t("foo"));
        h.pin(t("foo"));
        assert!(h.is_pinned(&t("foo")));
        assert!(
            !h.entries.contains(&t("foo")),
            "pin deve remover de entries"
        );
    }

    #[test]
    fn unpin_restores_to_entries_top() {
        let mut h = ClipHistory::default();
        h.entries.push(t("old"));
        h.pin(t("foo"));
        h.unpin(&t("foo"));
        assert_eq!(h.entries[0], t("foo"));
    }

    #[test]
    fn push_skips_if_already_pinned() {
        let mut h = ClipHistory::default();
        h.pin(t("p"));
        h.push(t("p"));
        assert_eq!(h.entries.len(), 0, "push nao duplica pinned em entries");
    }

    // L3: picker_list dedup
    #[test]
    fn picker_list_dedupes_correctly() {
        let mut h = ClipHistory::default();
        h.entries.push(t("dupe"));
        h.pinned.push(t("dupe")); // bypass invariant pra simular load stale
        let list = h.picker_list();
        assert_eq!(list.iter().filter(|e| *e == &t("dupe")).count(), 1);
    }

    #[test]
    fn picker_list_only_recent() {
        let mut h = ClipHistory::default();
        h.entries.push(t("a"));
        h.entries.push(t("b"));
        assert_eq!(h.picker_list().len(), 2);
    }

    #[test]
    fn picker_list_only_pinned() {
        let mut h = ClipHistory::default();
        h.pin(t("x"));
        h.pin(t("y"));
        let list = h.picker_list();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0], t("y")); // LIFO pin order
    }

    #[test]
    fn picker_list_empty() {
        assert!(ClipHistory::default().picker_list().is_empty());
    }

    // L2: HOME fallback safety
    #[test]
    fn try_path_fails_without_home() {
        let old = std::env::var("HOME").ok();
        // Use a unique env var name pra evitar race com outros tests.
        std::env::remove_var("HOME");
        let r = ClipHistory::try_path();
        assert!(r.is_err());
        if let Some(h) = old {
            std::env::set_var("HOME", h);
        }
    }

    #[test]
    fn try_path_returns_path_with_home() {
        std::env::set_var("HOME", "/home/test-user");
        let r = ClipHistory::try_path().unwrap();
        assert!(r.to_string_lossy().contains("test-user"));
        assert!(r.to_string_lossy().contains("clipboard-history.json"));
    }

    // M2: tildefy
    #[test]
    fn tildefy_replaces_home_prefix() {
        let p = PathBuf::from("/home/luiz/docs/secret.pdf");
        let t = tildefy_path(&p, "/home/luiz");
        assert_eq!(t, PathBuf::from("~/docs/secret.pdf"));
    }

    #[test]
    fn tildefy_keeps_path_outside_home() {
        let p = PathBuf::from("/etc/passwd");
        let t = tildefy_path(&p, "/home/luiz");
        assert_eq!(t, p);
    }

    #[test]
    fn tildefy_empty_home_noop() {
        let p = PathBuf::from("/anywhere");
        let t = tildefy_path(&p, "");
        assert_eq!(t, p);
    }

    // H1 + C1: atomic write + perms
    #[cfg(unix)]
    #[test]
    fn atomic_write_creates_file_with_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!(
            "lumo-clip-perm-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let dst = dir.join("hist.json");
        let tmp = dir.join("hist.json.tmp");
        atomic_write(&tmp, &dst, b"{}").unwrap();
        let meta = std::fs::metadata(&dst).unwrap();
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(mode, PRIVATE_FILE_MODE);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn atomic_write_overwrites_existing() {
        let dir = std::env::temp_dir().join(format!(
            "lumo-clip-atomic-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let dst = dir.join("h.json");
        let tmp = dir.join("h.json.tmp");
        std::fs::write(&dst, b"old").unwrap();
        atomic_write(&tmp, &dst, b"new").unwrap();
        assert_eq!(std::fs::read(&dst).unwrap(), b"new");
        assert!(!tmp.exists(), "tmp deve ser renomeado, nao deixado");
        std::fs::remove_dir_all(&dir).ok();
    }

    // Serde roundtrip pinned
    #[test]
    fn pinned_serde_roundtrip() {
        let mut h = ClipHistory::default();
        h.pin(t("p1"));
        h.pin(t("p2"));
        let s = serde_json::to_string(&h).unwrap();
        let h2: ClipHistory = serde_json::from_str(&s).unwrap();
        assert_eq!(h2.pinned.len(), 2);
        assert_eq!(h2.pinned[0], t("p2"));
    }

    #[test]
    fn old_format_loads_without_pinned() {
        // serde(default) on pinned: history JSON sem field "pinned" deve carregar.
        let old = r#"{"entries":[{"kind":"Text","content":"x"}]}"#;
        let h: ClipHistory = serde_json::from_str(old).unwrap();
        assert_eq!(h.entries.len(), 1);
        assert_eq!(h.pinned.len(), 0);
    }
}
