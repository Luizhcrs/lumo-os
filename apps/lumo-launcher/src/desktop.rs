//! desktop.rs - parse manual de arquivos .desktop XDG.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DesktopEntry {
    pub name: String,
    pub exec: String,
    pub comment: String,
    pub categories: String,
}

impl DesktopEntry {
    pub fn clean_exec(&self) -> String {
        self.exec
            .split_whitespace()
            .filter(|t| !t.starts_with('%'))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(exec: &str) -> DesktopEntry {
        DesktopEntry {
            name: "".into(),
            exec: exec.into(),
            comment: "".into(),
            categories: "".into(),
        }
    }

    #[test]
    fn clean_exec_removes_percent_tokens() {
        // %f, %F, %u, %U sao XDG field codes que nao queremos no spawn.
        let e = entry("firefox %u");
        assert_eq!(e.clean_exec(), "firefox");
    }

    #[test]
    fn clean_exec_preserves_args_without_percent() {
        let e = entry("mousepad /path/to/file.txt");
        assert_eq!(e.clean_exec(), "mousepad /path/to/file.txt");
    }

    #[test]
    fn clean_exec_removes_multiple_percent_tokens() {
        let e = entry("app %F %i %c %k arg1 arg2");
        assert_eq!(e.clean_exec(), "app arg1 arg2");
    }

    #[test]
    fn clean_exec_empty_returns_empty() {
        let e = entry("");
        assert_eq!(e.clean_exec(), "");
    }

    #[test]
    fn clean_exec_only_percent_tokens_returns_empty() {
        let e = entry("%F %U");
        assert_eq!(e.clean_exec(), "");
    }

    #[test]
    fn clean_exec_collapses_whitespace() {
        // split_whitespace collapsa runs de espaco. join com single space.
        let e = entry("firefox    %u   --new-window");
        assert_eq!(e.clean_exec(), "firefox --new-window");
    }
}

pub fn load_desktop_entries() -> Vec<DesktopEntry> {
    let mut dirs: Vec<PathBuf> = vec![PathBuf::from("/usr/share/applications")];
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(format!("{home}/.local/share/applications")));
    }
    let mut seen: HashSet<String> = HashSet::new();
    let mut entries = Vec::new();
    for dir in &dirs {
        let Ok(read) = fs::read_dir(dir) else {
            continue;
        };
        for entry in read.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let Ok(content) = fs::read_to_string(&path) else {
                continue;
            };
            if let Some(de) = parse_desktop(&content) {
                if seen.insert(de.name.clone()) {
                    entries.push(de);
                }
            }
        }
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

fn parse_desktop(content: &str) -> Option<DesktopEntry> {
    let mut in_desktop = false;
    let mut name = String::new();
    let mut exec = String::new();
    let mut comment = String::new();
    let mut categories = String::new();
    let mut no_display = false;
    let mut hidden = false;
    for line in content.lines() {
        let line = line.trim();
        if line == "[Desktop Entry]" {
            in_desktop = true;
            continue;
        }
        if line.starts_with('[') {
            in_desktop = false;
            continue;
        }
        if !in_desktop {
            continue;
        }
        if let Some(v) = strip_key(line, "Name") {
            if name.is_empty() {
                name = v.to_string();
            }
        } else if let Some(v) = strip_key(line, "Exec") {
            if exec.is_empty() {
                exec = v.to_string();
            }
        } else if let Some(v) = strip_key(line, "Comment") {
            if comment.is_empty() {
                comment = v.to_string();
            }
        } else if let Some(v) = strip_key(line, "Categories") {
            if categories.is_empty() {
                categories = v.to_string();
            }
        } else if let Some(v) = strip_key(line, "NoDisplay") {
            no_display = v.eq_ignore_ascii_case("true");
        } else if let Some(v) = strip_key(line, "Hidden") {
            hidden = v.eq_ignore_ascii_case("true");
        }
    }
    if no_display || hidden || name.is_empty() || exec.is_empty() {
        return None;
    }
    Some(DesktopEntry {
        name,
        exec,
        comment,
        categories,
    })
}

fn strip_key<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    line.strip_prefix(&format!("{key}="))
}
