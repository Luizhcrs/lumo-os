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
        let mut s = Self { all_entries: entries.clone(), entries, ..Default::default() };
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
                SortBy::Name => {
                    a.file_name().unwrap_or_default().to_ascii_lowercase()
                        .cmp(&b.file_name().unwrap_or_default().to_ascii_lowercase())
                }
                SortBy::Size => {
                    let sa = a.metadata().map(|m| m.len()).unwrap_or(0);
                    let sb = b.metadata().map(|m| m.len()).unwrap_or(0);
                    sa.cmp(&sb)
                }
                SortBy::ModifiedDate => {
                    let ta = a.metadata().and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
                    let tb = b.metadata().and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
                    ta.cmp(&tb)
                }
                SortBy::Type => {
                    let ea = a.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
                    let eb = b.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
                    ea.cmp(&eb)
                }
            };
            if ascending { ord } else { ord.reverse() }
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
            self.entries = self.all_entries
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
        let (lo, hi) = if from <= idx { (from, idx) } else { (idx, from) };
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

    /// Retorna data modificada formatada (YYYY-MM-DD HH:MM).
    pub fn human_modified(path: &PathBuf) -> String {
        use std::time::{UNIX_EPOCH};
        let t = path.metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        // Simple YYYY-MM-DD HH:MM from unix timestamp
        let secs = t % 60;
        let _ = secs;
        let mins = (t / 60) % 60;
        let hours = (t / 3600) % 24;
        let days = t / 86400;
        // approx date from epoch
        let years = 1970 + days / 365;
        let day_of_year = days % 365;
        let month = day_of_year / 30 + 1;
        let day = day_of_year % 30 + 1;
        format!("{years:04}-{month:02}-{day:02} {hours:02}:{mins:02}")
    }
}
