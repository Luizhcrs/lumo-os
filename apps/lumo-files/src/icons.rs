//! Catalogo de icones SVG embutidos + classificacao mime-type -> IconKind.
//!
//! Todos os SVGs sao inlinados via `include_bytes!`. Nenhum lookup
//! de filesystem em runtime. Cada `handle_*` retorna um `Svg::Handle`
//! pronto pra ser passado pra `Svg::new()`. Cores aplicadas via
//! `Svg::style` + `currentColor` no SVG.

use iced::widget::svg::Handle;
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
    Pdf,
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
        Some("pdf") => IconKind::Pdf,
        Some("doc") | Some("docx") | Some("odt") | Some("txt") | Some("md") | Some("rst") => {
            IconKind::Document
        }
        Some("zip") | Some("tar") | Some("gz") | Some("bz2") | Some("xz") | Some("7z")
        | Some("rar") | Some("zst") => IconKind::Archive,
        Some("rs") | Some("py") | Some("js") | Some("ts") | Some("c") | Some("cpp") | Some("h")
        | Some("go") | Some("java") | Some("sh") | Some("toml") | Some("yaml") | Some("json")
        | Some("xml") | Some("html") | Some("css") => IconKind::Code,
        Some("exe") | Some("bin") | Some("appimage") | Some("deb") | Some("rpm") => {
            IconKind::Executable
        }
        _ => IconKind::Generic,
    }
}

/// Retorna label ASCII para exibicao fallback (usado em logs / accessibility).
pub fn icon_label(kind: &IconKind) -> &'static str {
    match kind {
        IconKind::Folder => "Pasta",
        IconKind::Home => "Inicio",
        IconKind::Trash => "Lixeira",
        IconKind::Image => "Imagem",
        IconKind::Video => "Video",
        IconKind::Audio => "Audio",
        IconKind::Document => "Documento",
        IconKind::Archive => "Arquivo compactado",
        IconKind::Code => "Codigo",
        IconKind::Executable => "Executavel",
        IconKind::Pdf => "PDF",
        IconKind::Generic => "Arquivo",
    }
}

// ---------------------------------------------------------------------------
// SVG bytes (compiled in)
// ---------------------------------------------------------------------------

pub const FOLDER: &[u8] = include_bytes!("../icons/folder.svg");
pub const FOLDER_OPEN: &[u8] = include_bytes!("../icons/folder_open.svg");
pub const HOME: &[u8] = include_bytes!("../icons/home.svg");
pub const TRASH: &[u8] = include_bytes!("../icons/trash.svg");
pub const DOCS: &[u8] = include_bytes!("../icons/docs.svg");
pub const DOWNLOADS: &[u8] = include_bytes!("../icons/downloads.svg");
pub const PICS: &[u8] = include_bytes!("../icons/pics.svg");
pub const VIDEOS: &[u8] = include_bytes!("../icons/videos.svg");
pub const MUSIC: &[u8] = include_bytes!("../icons/music.svg");
pub const DESKTOP: &[u8] = include_bytes!("../icons/desktop.svg");
pub const FILE_GENERIC: &[u8] = include_bytes!("../icons/file_generic.svg");
pub const FILE_TEXT: &[u8] = include_bytes!("../icons/file_text.svg");
pub const FILE_IMAGE: &[u8] = include_bytes!("../icons/file_image.svg");
pub const FILE_VIDEO: &[u8] = include_bytes!("../icons/file_video.svg");
pub const FILE_AUDIO: &[u8] = include_bytes!("../icons/file_audio.svg");
pub const FILE_ARCHIVE: &[u8] = include_bytes!("../icons/file_archive.svg");
pub const FILE_CODE: &[u8] = include_bytes!("../icons/file_code.svg");
pub const FILE_PDF: &[u8] = include_bytes!("../icons/file_pdf.svg");
pub const CHEVRON_LEFT: &[u8] = include_bytes!("../icons/chevron_left.svg");
pub const CHEVRON_RIGHT: &[u8] = include_bytes!("../icons/chevron_right.svg");
pub const ARROW_UP: &[u8] = include_bytes!("../icons/arrow_up.svg");
pub const SEARCH: &[u8] = include_bytes!("../icons/search.svg");
pub const PLUS: &[u8] = include_bytes!("../icons/plus.svg");
pub const GRID: &[u8] = include_bytes!("../icons/grid.svg");
pub const LIST: &[u8] = include_bytes!("../icons/list.svg");
pub const COLUMNS: &[u8] = include_bytes!("../icons/columns.svg");

/// Retorna bytes SVG apropriados para um IconKind (file content icons).
pub fn svg_bytes_for_kind(kind: &IconKind) -> &'static [u8] {
    match kind {
        IconKind::Folder => FOLDER,
        IconKind::Home => HOME,
        IconKind::Trash => TRASH,
        IconKind::Image => FILE_IMAGE,
        IconKind::Video => FILE_VIDEO,
        IconKind::Audio => FILE_AUDIO,
        IconKind::Document => FILE_TEXT,
        IconKind::Archive => FILE_ARCHIVE,
        IconKind::Code => FILE_CODE,
        IconKind::Executable => FILE_GENERIC,
        IconKind::Pdf => FILE_PDF,
        IconKind::Generic => FILE_GENERIC,
    }
}

/// Constroi um `svg::Handle` a partir de bytes estaticos.
pub fn handle(bytes: &'static [u8]) -> Handle {
    Handle::from_memory(bytes)
}

/// Handle para um IconKind.
pub fn handle_for_kind(kind: &IconKind) -> Handle {
    Handle::from_memory(svg_bytes_for_kind(kind))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn pdf_dedicado_separado_de_document() {
        let kind = icon_for_path(&PathBuf::from("relatorio.pdf"));
        assert_eq!(kind, IconKind::Pdf);
    }

    #[test]
    fn txt_classificado_como_document() {
        let kind = icon_for_path(&PathBuf::from("notas.txt"));
        assert_eq!(kind, IconKind::Document);
    }

    #[test]
    fn svg_bytes_existem_para_todos_kinds() {
        for kind in [
            IconKind::Folder,
            IconKind::Home,
            IconKind::Trash,
            IconKind::Image,
            IconKind::Video,
            IconKind::Audio,
            IconKind::Document,
            IconKind::Archive,
            IconKind::Code,
            IconKind::Executable,
            IconKind::Pdf,
            IconKind::Generic,
        ] {
            let bytes = svg_bytes_for_kind(&kind);
            assert!(bytes.len() > 16, "SVG vazio para {:?}", kind);
            assert!(
                bytes.starts_with(b"<svg") || bytes.starts_with(b"<?xml"),
                "bytes nao parecem SVG para {:?}",
                kind
            );
        }
    }

    #[test]
    fn icon_label_pdf_nao_e_emoji() {
        let label = icon_label(&IconKind::Pdf);
        assert!(!label.is_empty());
        assert!(label.chars().all(|c| c.is_ascii()));
    }
}
