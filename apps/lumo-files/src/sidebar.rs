//! Sidebar com atalhos de navegacao rapida.
//!
//! Lista Home / Documentos / Downloads / Imagens / Videos / Musicas /
//! Desktop / Lixeira + drives montados em /run/media/<user>/.

use std::path::PathBuf;

use crate::icons;

/// Item da sidebar.
#[derive(Debug, Clone)]
pub struct SidebarItem {
    pub label: String,
    pub path: PathBuf,
    pub kind: SidebarKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidebarKind {
    Home,
    Documents,
    Downloads,
    Pictures,
    Videos,
    Music,
    Desktop,
    Trash,
    Drive,
}

impl SidebarKind {
    /// Retorna bytes SVG do icone associado.
    pub fn svg_bytes(&self) -> &'static [u8] {
        match self {
            SidebarKind::Home => icons::HOME,
            SidebarKind::Documents => icons::DOCS,
            SidebarKind::Downloads => icons::DOWNLOADS,
            SidebarKind::Pictures => icons::PICS,
            SidebarKind::Videos => icons::VIDEOS,
            SidebarKind::Music => icons::MUSIC,
            SidebarKind::Desktop => icons::DESKTOP,
            SidebarKind::Trash => icons::TRASH,
            SidebarKind::Drive => icons::FOLDER,
        }
    }
}

/// Retorna lista de itens da sidebar.
pub fn build_sidebar(username: &str) -> Vec<SidebarItem> {
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| format!("/home/{username}")));

    let mut items = vec![
        SidebarItem {
            label: "Inicio".to_string(),
            path: home.clone(),
            kind: SidebarKind::Home,
        },
        SidebarItem {
            label: "Documentos".to_string(),
            path: home.join("Documents"),
            kind: SidebarKind::Documents,
        },
        SidebarItem {
            label: "Downloads".to_string(),
            path: home.join("Downloads"),
            kind: SidebarKind::Downloads,
        },
        SidebarItem {
            label: "Imagens".to_string(),
            path: home.join("Pictures"),
            kind: SidebarKind::Pictures,
        },
        SidebarItem {
            label: "Videos".to_string(),
            path: home.join("Videos"),
            kind: SidebarKind::Videos,
        },
        SidebarItem {
            label: "Musicas".to_string(),
            path: home.join("Music"),
            kind: SidebarKind::Music,
        },
        SidebarItem {
            label: "Desktop".to_string(),
            path: home.join("Desktop"),
            kind: SidebarKind::Desktop,
        },
        SidebarItem {
            label: "Lixeira".to_string(),
            path: home.join(".local/share/Trash"),
            kind: SidebarKind::Trash,
        },
    ];

    // drives montados em /run/media/<user>/
    let media_path = PathBuf::from(format!("/run/media/{username}"));
    if let Ok(entries) = std::fs::read_dir(&media_path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let label = p.file_name().unwrap_or_default().to_string_lossy().to_string();
                items.push(SidebarItem {
                    label,
                    path: p,
                    kind: SidebarKind::Drive,
                });
            }
        }
    }

    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidebar_kind_svg_bytes_nao_vazios() {
        for kind in [
            SidebarKind::Home, SidebarKind::Documents, SidebarKind::Downloads,
            SidebarKind::Pictures, SidebarKind::Videos, SidebarKind::Music,
            SidebarKind::Desktop, SidebarKind::Trash, SidebarKind::Drive,
        ] {
            let bytes = kind.svg_bytes();
            assert!(bytes.len() > 16, "SVG vazio para {:?}", kind);
        }
    }
}
