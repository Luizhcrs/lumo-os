//! Mapeamento mime-type -> icone por tipo de arquivo.
//!
//! Retorna um label de texto simples para ser exibido no grid
//! enquanto nao ha carregamento de icone de tema sistema.
//! Futuramente expandir para carregar de Adwaita/Papirus.

use std::path::Path;

/// Categoria de icone para um path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IconKind {
    Folder,
    Home,
    Trash,
    Image,
    Video,
    Audio,
    Document,
    Archive,
    Code,
    Executable,
    Generic,
}

/// Retorna o IconKind para um path dado.
pub fn icon_for_path(path: &Path) -> IconKind {
    if path.is_dir() {
        return IconKind::Folder;
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase());
    match ext.as_deref() {
        Some("png") | Some("jpg") | Some("jpeg") | Some("gif") | Some("webp") | Some("bmp")
        | Some("svg") | Some("ico") => IconKind::Image,
        Some("mp4") | Some("mkv") | Some("avi") | Some("mov") | Some("webm") | Some("flv") => {
            IconKind::Video
        }
        Some("mp3") | Some("flac") | Some("ogg") | Some("wav") | Some("opus") | Some("aac") => {
            IconKind::Audio
        }
        Some("pdf") | Some("doc") | Some("docx") | Some("odt") | Some("txt") | Some("md")
        | Some("rst") => IconKind::Document,
        Some("zip") | Some("tar") | Some("gz") | Some("bz2") | Some("xz") | Some("7z")
        | Some("rar") | Some("zst") => IconKind::Archive,
        Some("rs") | Some("py") | Some("js") | Some("ts") | Some("c") | Some("cpp")
        | Some("h") | Some("go") | Some("java") | Some("sh") | Some("toml") | Some("yaml")
        | Some("json") | Some("xml") | Some("html") | Some("css") => IconKind::Code,
        Some("exe") | Some("bin") | Some("appimage") | Some("deb") | Some("rpm") => {
            IconKind::Executable
        }
        _ => IconKind::Generic,
    }
}

/// Retorna label ASCII para exibicao no grid (fallback sem SVG carregado).
pub fn icon_label(kind: &IconKind) -> &'static str {
    match kind {
        IconKind::Folder => "[dir]",
        IconKind::Home => "[home]",
        IconKind::Trash => "[trash]",
        IconKind::Image => "[img]",
        IconKind::Video => "[vid]",
        IconKind::Audio => "[aud]",
        IconKind::Document => "[doc]",
        IconKind::Archive => "[zip]",
        IconKind::Code => "[src]",
        IconKind::Executable => "[bin]",
        IconKind::Generic => "[file]",
    }
}
