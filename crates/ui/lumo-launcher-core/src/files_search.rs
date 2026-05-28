//! files_search.rs — busca arquivos pelo home directory.
//!
//! Estrategia: walk recursive limitado a 3 niveis ate 500 hits. Filtra
//! por substring case-insensitive. Skipa .git, node_modules, target,
//! .cache, build dirs.
//!
//! Sem dep external. Para Spotlight nao bloquear UI, caller deve chamar
//! search em thread separada.

use std::path::{Path, PathBuf};

pub const MAX_RESULTS: usize = 50;
pub const MAX_DEPTH: usize = 3;

const SKIP_DIRS: &[&str] = &[
    ".git",
    ".cache",
    "node_modules",
    "target",
    "build",
    "dist",
    ".venv",
    "venv",
    "__pycache__",
    ".cargo",
    ".rustup",
    ".npm",
    ".local",
    "snap",
    ".steam",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMatch {
    pub path: PathBuf,
    pub name: String,
}

pub fn search(root: &Path, query: &str) -> Vec<FileMatch> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let mut results = Vec::new();
    walk(root, &q, 0, &mut results);
    results.truncate(MAX_RESULTS);
    results
}

fn walk(dir: &Path, query: &str, depth: usize, out: &mut Vec<FileMatch>) {
    if depth > MAX_DEPTH || out.len() >= MAX_RESULTS {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        if out.len() >= MAX_RESULTS {
            return;
        }
        let name = match entry.file_name().into_string() {
            Ok(n) => n,
            Err(_) => continue,
        };
        if name.starts_with('.') && depth == 0 {
            continue;
        }
        if SKIP_DIRS.contains(&name.as_str()) {
            continue;
        }
        let lc = name.to_lowercase();
        if lc.contains(query) {
            out.push(FileMatch {
                path: entry.path(),
                name: name.clone(),
            });
        }
        if let Ok(ft) = entry.file_type() {
            if ft.is_dir() {
                walk(&entry.path(), query, depth + 1, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp() -> PathBuf {
        std::env::temp_dir().join(format!(
            "lumo-launcher-fsearch-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    #[test]
    fn search_empty_query_returns_empty() {
        let t = tmp();
        fs::create_dir_all(&t).unwrap();
        fs::write(t.join("foo.txt"), "").unwrap();
        let r = search(&t, "");
        assert!(r.is_empty());
        fs::remove_dir_all(&t).ok();
    }

    #[test]
    fn search_substring_matches() {
        let t = tmp();
        fs::create_dir_all(&t).unwrap();
        fs::write(t.join("notes.md"), "").unwrap();
        fs::write(t.join("README.txt"), "").unwrap();
        let r = search(&t, "note");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].name, "notes.md");
        fs::remove_dir_all(&t).ok();
    }

    #[test]
    fn search_case_insensitive() {
        let t = tmp();
        fs::create_dir_all(&t).unwrap();
        fs::write(t.join("MyFile.RS"), "").unwrap();
        let r = search(&t, "myfile");
        assert_eq!(r.len(), 1);
        fs::remove_dir_all(&t).ok();
    }

    #[test]
    fn search_recursive_within_depth() {
        let t = tmp();
        let nested = t.join("a").join("b");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("deep.txt"), "").unwrap();
        let r = search(&t, "deep");
        assert_eq!(r.len(), 1);
        fs::remove_dir_all(&t).ok();
    }

    #[test]
    fn search_skips_git_dir() {
        let t = tmp();
        let git = t.join(".git");
        fs::create_dir_all(&git).unwrap();
        fs::write(git.join("HEAD"), "").unwrap();
        fs::write(t.join("regular.txt"), "regular").unwrap();
        let r = search(&t, "head");
        assert!(r.is_empty(), "results={:?}", r);
        fs::remove_dir_all(&t).ok();
    }

    #[test]
    fn search_skips_node_modules() {
        let t = tmp();
        let nm = t.join("node_modules");
        fs::create_dir_all(&nm).unwrap();
        fs::write(nm.join("package.json"), "").unwrap();
        let r = search(&t, "package");
        assert!(r.is_empty());
        fs::remove_dir_all(&t).ok();
    }

    #[test]
    fn search_skips_target() {
        let t = tmp();
        let tg = t.join("target");
        fs::create_dir_all(&tg).unwrap();
        fs::write(tg.join("debug.txt"), "").unwrap();
        let r = search(&t, "debug");
        assert!(r.is_empty());
        fs::remove_dir_all(&t).ok();
    }

    #[test]
    fn search_skips_hidden_at_root() {
        let t = tmp();
        fs::create_dir_all(&t).unwrap();
        fs::write(t.join(".hidden"), "").unwrap();
        let r = search(&t, "hidden");
        assert!(r.is_empty());
        fs::remove_dir_all(&t).ok();
    }

    #[test]
    fn search_caps_at_max_results() {
        let t = tmp();
        fs::create_dir_all(&t).unwrap();
        for i in 0..100 {
            fs::write(t.join(format!("file{}.txt", i)), "").unwrap();
        }
        let r = search(&t, "file");
        assert!(r.len() <= MAX_RESULTS);
        fs::remove_dir_all(&t).ok();
    }

    #[test]
    fn search_missing_root_empty() {
        let r = search(&PathBuf::from("/this/does/not/exist/xyz"), "any");
        assert!(r.is_empty());
    }

    #[test]
    fn search_depth_limit_respected() {
        let t = tmp();
        // 4 levels deep > MAX_DEPTH 3.
        let deep = t.join("l1").join("l2").join("l3").join("l4");
        fs::create_dir_all(&deep).unwrap();
        fs::write(deep.join("target.txt"), "").unwrap();
        let r = search(&t, "target");
        // Pode encontrar ou nao dependendo if depth counts from 0 or 1.
        // Atual implementacao depth=0 root, +1 per descent. l4 = depth 4.
        // MAX_DEPTH=3 = walks l1,l2,l3 mas nao desce em l4.
        // Logo target.txt em l4 NAO encontrado.
        assert!(r.is_empty(), "depth limit deveria bloquear l4: {:?}", r);
        fs::remove_dir_all(&t).ok();
    }

    #[test]
    fn search_file_match_struct_eq() {
        let a = FileMatch {
            path: PathBuf::from("/foo"),
            name: "foo".into(),
        };
        let b = FileMatch {
            path: PathBuf::from("/foo"),
            name: "foo".into(),
        };
        assert_eq!(a, b);
    }
}
