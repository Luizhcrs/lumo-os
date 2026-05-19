//! Breadcrumb de navegacao de path.
//!
//! Exibe segmentos clicaveis do path atual.
//! Cada segmento e um botao que emite Message::Navigate(path).

use std::path::{Path, PathBuf};

/// Retorna vetor de (label, path) representando os segmentos do path.
/// Sempre inclui "/" (root) como primeiro elemento.
pub fn segments(path: &Path) -> Vec<(String, PathBuf)> {
    let mut result = Vec::new();
    let mut accumulated = PathBuf::from("/");
    result.push(("/".to_string(), accumulated.clone()));

    for component in path.components() {
        use std::path::Component;
        match component {
            Component::Normal(name) => {
                accumulated.push(name);
                let label = name.to_string_lossy().to_string();
                result.push((label, accumulated.clone()));
            }
            Component::RootDir => {}
            _ => {}
        }
    }
    result
}

/// Trunca label para exibicao no breadcrumb (max 20 chars).
pub fn truncate_label(label: &str, max: usize) -> String {
    if label.chars().count() <= max {
        label.to_string()
    } else {
        let t: String = label.chars().take(max - 2).collect();
        format!("{t}..")
    }
}
