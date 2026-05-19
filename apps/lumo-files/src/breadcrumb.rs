//! Breadcrumb de navegacao de path.
//!
//! Exibe segmentos clicaveis do path atual.
//! Cada segmento e um botao que emite `Message::Navigate(path)`.

use std::path::{Path, PathBuf};

/// Retorna vetor de `(label, path)` representando os segmentos do path.
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

/// Trunca label para exibicao no breadcrumb (max N chars).
pub fn truncate_label(label: &str, max: usize) -> String {
    if label.chars().count() <= max {
        label.to_string()
    } else {
        let t: String = label.chars().take(max - 2).collect();
        format!("{t}..")
    }
}

/// Trunca breadcrumb inteiro quando passa do limite. Se total caracteres
/// (sum of labels + separadores) > `max_total`, retorna apenas root + ".."
/// + ultimos 2 segmentos.
///
/// `segs` deve incluir root como primeiro elemento (vide `segments`).
pub fn smart_truncate(segs: &[(String, PathBuf)], max_total: usize) -> Vec<BreadcrumbEntry> {
    let total: usize = segs.iter().map(|(l, _)| l.chars().count() + 3).sum();
    if total <= max_total || segs.len() <= 4 {
        return segs.iter().map(|(l, p)| BreadcrumbEntry::Segment(l.clone(), p.clone())).collect();
    }
    let mut out: Vec<BreadcrumbEntry> = Vec::new();
    out.push(BreadcrumbEntry::Segment(segs[0].0.clone(), segs[0].1.clone()));
    out.push(BreadcrumbEntry::Ellipsis);
    let tail_start = segs.len().saturating_sub(2);
    for (l, p) in &segs[tail_start..] {
        out.push(BreadcrumbEntry::Segment(l.clone(), p.clone()));
    }
    out
}

/// Entrada do breadcrumb apos truncamento.
#[derive(Debug, Clone)]
pub enum BreadcrumbEntry {
    Segment(String, PathBuf),
    Ellipsis,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smart_truncate_curto_passa_intacto() {
        let segs = segments(Path::new("/home/luiz"));
        let out = smart_truncate(&segs, 60);
        assert_eq!(out.len(), segs.len());
        assert!(matches!(out[0], BreadcrumbEntry::Segment(_, _)));
    }

    #[test]
    fn smart_truncate_longo_inclui_ellipsis() {
        let path = Path::new("/home/luiz/Projetos/lumo-shell/apps/lumo-files/src/longo/path/aqui");
        let segs = segments(path);
        let out = smart_truncate(&segs, 30);
        assert!(out.iter().any(|e| matches!(e, BreadcrumbEntry::Ellipsis)));
        // Primeiro sempre eh root.
        assert!(matches!(&out[0], BreadcrumbEntry::Segment(l, _) if l == "/"));
        // Ultimo eh o segmento final do path.
        let last = out.last().unwrap();
        if let BreadcrumbEntry::Segment(l, _) = last {
            assert_eq!(l, "aqui");
        } else {
            panic!("ultimo entry deveria ser segmento");
        }
    }

    #[test]
    fn truncate_label_keeps_short() {
        assert_eq!(truncate_label("abc", 10), "abc");
        let out = truncate_label("abcdefghij", 5);
        assert!(out.ends_with(".."));
        assert_eq!(out.chars().count(), 5);
    }
}
