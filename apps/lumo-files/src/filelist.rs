//! Grid view de arquivos/pastas.
//!
//! Exibe entradas do diretorio atual como grid de celulas 64x64 + label.
//! Selecao: single click, Ctrl+click multi-add, Shift+click range.

use std::collections::HashSet;
use std::path::PathBuf;

/// Estado do grid de arquivos.
#[derive(Debug, Clone, Default)]
pub struct FileList {
    /// Entradas do diretorio atual (pastas primeiro, depois arquivos).
    pub entries: Vec<PathBuf>,
    /// Indices selecionados.
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
        Self {
            entries,
            ..Default::default()
        }
    }

    /// Substitui entradas e limpa selecao.
    pub fn set_entries(&mut self, entries: Vec<PathBuf>) {
        self.entries = entries;
        self.selected.clear();
        self.last_clicked = None;
        self.renaming = None;
        self.rename_input.clear();
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

    /// Trunca nome do arquivo para exibicao no grid (max 14 chars).
    pub fn display_name(path: &PathBuf) -> String {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if name.chars().count() <= 14 {
            name.to_string()
        } else {
            let t: String = name.chars().take(12).collect();
            format!("{t}..")
        }
    }
}
