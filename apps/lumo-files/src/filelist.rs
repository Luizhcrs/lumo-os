//! Grid/list view de arquivos/pastas.
//!
//! Exibe entradas do diretorio atual.
//! Selecao: single click, Ctrl+click multi-add, Shift+click range.
//! Suporta ordenacao por nome/tamanho/data/tipo e filtragem por substring.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::SystemTime;

/// Criterio de ordenacao.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortBy {
    #[default]
    Name,
    Size,
    ModifiedDate,
    Type,
}

/// Estado do grid de arquivos.
#[derive(Debug, Clone, Default)]
pub struct FileList {
    /// Entradas brutas do diretorio (antes de sort/filtro).
    pub all_entries: Vec<PathBuf>,
    /// Entradas apos sort e filtro.
    pub entries: Vec<PathBuf>,
    /// Indices selecionados (relativos a `entries`).
    pub selected: HashSet<usize>,
    /// Ultimo indice clicado (para range selection Shift+click).
    pub last_clicked: Option<usize>,
    /// Indice em renomeacao inline (None = nenhum).
    pub renaming: Option<usize>,
    /// Buffer de texto para renomeacao.
    pub rename_input: String,
}

impl FileList {
    pub fn new(entries: Vec<PathBuf>) -> Self {
        let mut s = Self {
            all_entries: entries.clone(),
            entries,
            ..Default::default()
        };
        s.sort(SortBy::Name, true);
        s
    }

    /// Substitui entradas e limpa selecao.
    pub fn set_entries(&mut self, entries: Vec<PathBuf>) {
        self.all_entries = entries.clone();
        self.entries = entries;
        self.selected.clear();
        self.last_clicked = None;
        self.renaming = None;
        self.rename_input.clear();
    }

    /// Ordena entries por criterio + ordem. Re-aplica filtro se existir.
    pub fn sort(&mut self, by: SortBy, ascending: bool) {
        self.entries.sort_by(|a, b| {
            let a_dir = a.is_dir();
            let b_dir = b.is_dir();
            // pastas sempre antes
            let dir_ord = b_dir.cmp(&a_dir);
            if dir_ord != std::cmp::Ordering::Equal {
                return dir_ord;
            }
            let ord = match by {
                SortBy::Name => a
                    .file_name()
                    .unwrap_or_default()
                    .to_ascii_lowercase()
                    .cmp(&b.file_name().unwrap_or_default().to_ascii_lowercase()),
                SortBy::Size => {
                    let sa = a.metadata().map(|m| m.len()).unwrap_or(0);
                    let sb = b.metadata().map(|m| m.len()).unwrap_or(0);
                    sa.cmp(&sb)
                }
                SortBy::ModifiedDate => {
                    let ta = a
                        .metadata()
                        .and_then(|m| m.modified())
                        .unwrap_or(SystemTime::UNIX_EPOCH);
                    let tb = b
                        .metadata()
                        .and_then(|m| m.modified())
                        .unwrap_or(SystemTime::UNIX_EPOCH);
                    ta.cmp(&tb)
                }
                SortBy::Type => {
                    let ea = a
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    let eb = b
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    ea.cmp(&eb)
                }
            };
            if ascending {
                ord
            } else {
                ord.reverse()
            }
        });
        self.selected.clear();
        self.last_clicked = None;
    }

    /// Aplica filtro substring case-insensitive sobre all_entries.
    pub fn apply_filter(&mut self, query: &str, by: SortBy, ascending: bool) {
        if query.is_empty() {
            self.entries = self.all_entries.clone();
        } else {
            let q = query.to_ascii_lowercase();
            self.entries = self
                .all_entries
                .iter()
                .filter(|p| {
                    p.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_ascii_lowercase()
                        .contains(&q)
                })
                .cloned()
                .collect();
        }
        self.sort(by, ascending);
    }

    /// Click simples: seleciona apenas idx.
    pub fn click(&mut self, idx: usize) {
        self.selected.clear();
        self.selected.insert(idx);
        self.last_clicked = Some(idx);
    }

    /// Ctrl+click: toggle idx na selecao.
    pub fn ctrl_click(&mut self, idx: usize) {
        if self.selected.contains(&idx) {
            self.selected.remove(&idx);
        } else {
            self.selected.insert(idx);
        }
        self.last_clicked = Some(idx);
    }

    /// Shift+click: seleciona range do last_clicked ate idx.
    pub fn shift_click(&mut self, idx: usize) {
        let from = self.last_clicked.unwrap_or(idx);
        let (lo, hi) = if from <= idx {
            (from, idx)
        } else {
            (idx, from)
        };
        for i in lo..=hi {
            self.selected.insert(i);
        }
        self.last_clicked = Some(idx);
    }

    /// Limpa selecao.
    pub fn clear_selection(&mut self) {
        self.selected.clear();
        self.last_clicked = None;
    }

    /// Retorna paths dos itens selecionados.
    pub fn selected_paths(&self) -> Vec<PathBuf> {
        let mut idxs: Vec<usize> = self.selected.iter().copied().collect();
        idxs.sort();
        idxs.iter()
            .filter_map(|&i| self.entries.get(i))
            .cloned()
            .collect()
    }

    /// Inicia renomeacao inline do item idx.
    pub fn start_rename(&mut self, idx: usize) {
        if let Some(path) = self.entries.get(idx) {
            self.rename_input = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            self.renaming = Some(idx);
        }
    }

    /// Cancela renomeacao.
    pub fn cancel_rename(&mut self) {
        self.renaming = None;
        self.rename_input.clear();
    }

    /// Trunca nome do arquivo para exibicao (max N chars).
    pub fn display_name_max(path: &PathBuf, max: usize) -> String {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if name.chars().count() <= max {
            name.to_string()
        } else {
            let t: String = name.chars().take(max - 2).collect();
            format!("{t}..")
        }
    }

    /// Trunca nome para exibicao no grid (max 14 chars).
    pub fn display_name(path: &PathBuf) -> String {
        Self::display_name_max(path, 14)
    }

    /// Retorna tamanho humano legivel.
    pub fn human_size(path: &PathBuf) -> String {
        let size = path.metadata().map(|m| m.len()).unwrap_or(0);
        if size < 1024 {
            format!("{} B", size)
        } else if size < 1024 * 1024 {
            format!("{:.1} KB", size as f64 / 1024.0)
        } else if size < 1024 * 1024 * 1024 {
            format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.1} GB", size as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    }

    /// Retorna data modificada formatada (YYYY-MM-DD HH:MM) usando chrono.
    pub fn human_modified(path: &PathBuf) -> String {
        use std::time::UNIX_EPOCH;
        let unix_secs = path
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let dt = chrono::DateTime::from_timestamp(unix_secs as i64, 0)
            .unwrap_or_else(|| chrono::DateTime::UNIX_EPOCH);
        dt.format("%Y-%m-%d %H:%M").to_string()
    }

    /// Retorna data modificada em formato relativo curto se < 7 dias.
    /// Caso contrario delega para `human_modified` (formato absoluto).
    pub fn human_modified_relative(path: &PathBuf) -> String {
        use std::time::{SystemTime, UNIX_EPOCH};

        let modified = match path.metadata().and_then(|m| m.modified()) {
            Ok(t) => t,
            Err(_) => return "--".to_string(),
        };
        let now = SystemTime::now();
        let delta = now
            .duration_since(modified)
            .unwrap_or_else(|_| std::time::Duration::from_secs(0));
        let secs = delta.as_secs();
        if secs < 60 {
            return "agora".to_string();
        }
        if secs < 3600 {
            let m = secs / 60;
            return format!("{m} min atras");
        }
        if secs < 86_400 {
            let h = secs / 3600;
            return if h == 1 {
                "1 hora atras".into()
            } else {
                format!("{h} horas atras")
            };
        }
        if secs < 86_400 * 7 {
            let d = secs / 86_400;
            return if d == 1 {
                "ontem".into()
            } else {
                format!("{d} dias atras")
            };
        }
        let unix_secs = modified
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let dt = chrono::DateTime::from_timestamp(unix_secs as i64, 0)
            .unwrap_or_else(|| chrono::DateTime::UNIX_EPOCH);
        dt.format("%Y-%m-%d").to_string()
    }

    /// Retorna tipo "Pasta" para diretorios ou extensao uppercase, ou "Arquivo".
    pub fn human_type(path: &PathBuf) -> String {
        if path.is_dir() {
            return "Pasta".to_string();
        }
        match path.extension().and_then(|e| e.to_str()) {
            Some(ext) if !ext.is_empty() => ext.to_ascii_uppercase(),
            _ => "Arquivo".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_list(n: usize) -> FileList {
        let entries: Vec<PathBuf> = (0..n)
            .map(|i| PathBuf::from(format!("/tmp/file{}.txt", i)))
            .collect();
        let mut fl = FileList::default();
        fl.entries = entries.clone();
        fl.all_entries = entries;
        fl
    }

    #[test]
    fn test_ctrl_click_add() {
        let mut fl = make_list(5);
        fl.click(0);
        fl.ctrl_click(2);
        assert!(fl.selected.contains(&0));
        assert!(fl.selected.contains(&2));
        assert_eq!(fl.selected.len(), 2);
    }

    #[test]
    fn test_ctrl_click_remove() {
        let mut fl = make_list(5);
        fl.click(1);
        fl.ctrl_click(1);
        assert!(!fl.selected.contains(&1));
        assert!(fl.selected.is_empty());
    }

    #[test]
    fn test_shift_click_range() {
        let mut fl = make_list(10);
        fl.click(2);
        fl.shift_click(6);
        for i in 2..=6 {
            assert!(fl.selected.contains(&i), "idx {} missing", i);
        }
        assert_eq!(fl.selected.len(), 5);
    }

    #[test]
    fn test_shift_click_reverse_range() {
        let mut fl = make_list(10);
        fl.click(7);
        fl.shift_click(3);
        for i in 3..=7 {
            assert!(fl.selected.contains(&i), "idx {} missing", i);
        }
    }

    #[test]
    fn human_type_pasta_para_diretorio() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(FileList::human_type(&dir.path().to_path_buf()), "Pasta");
    }

    #[test]
    fn human_type_extensao_uppercase() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("relatorio.pdf");
        std::fs::write(&f, b"x").unwrap();
        assert_eq!(FileList::human_type(&f), "PDF");
    }

    #[test]
    fn human_type_arquivo_sem_extensao() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("README");
        std::fs::write(&f, b"x").unwrap();
        assert_eq!(FileList::human_type(&f), "Arquivo");
    }

    #[test]
    fn human_modified_relative_arquivo_inexistente_retorna_traco() {
        let p = PathBuf::from("/nao/existe/qualquer/lugar/12345");
        assert_eq!(FileList::human_modified_relative(&p), "--");
    }

    #[test]
    fn human_modified_relative_recente_e_agora_ou_min() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("agora.txt");
        std::fs::write(&f, b"x").unwrap();
        let s = FileList::human_modified_relative(&f);
        assert!(
            s == "agora" || s.contains("min") || s.contains("hora"),
            "esperava agora/min/hora, veio {s:?}"
        );
    }
}
