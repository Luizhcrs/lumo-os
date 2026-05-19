//! Sidebar com atalhos de navegacao rapida.
//!
//! Lista Home/Documentos/Downloads/Imagens/Videos/Musicas/Desktop/Lixeira
//! e drives montados em /run/media/.

use std::path::PathBuf;

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

/// Retorna lista de itens da sidebar.
/// Detecta home via env HOME. Drives via /run/media/<user>/*.
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
