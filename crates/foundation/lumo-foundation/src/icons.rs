// ---------------------------------------------------------------------------
// icons -- freedesktop icon lookup (pure, no image deps)
// ---------------------------------------------------------------------------
//
// Responsabilidades:
//   - Resolver nome de icone a partir de um Path (dir ou arquivo).
//   - Procurar o SVG/PNG correspondente nas pastas de tema instaladas.
//
// Zero deps de imagem (resvg/usvg/tiny-skia). So std::path + std::fs.

use std::path::{Path, PathBuf};

use crate::{current_theme, LumoTheme};

// ---------------------------------------------------------------------------
// icon_theme_dirs
// ---------------------------------------------------------------------------

/// Retorna as pastas de tema na ordem de busca:
///   1. Papirus-Dark ou Papirus-Light conforme tema atual (via LUMO_THEME).
///   2. Papirus (fallback neutro).
///   3. hicolor (fallback universal freedesktop).
pub fn icon_theme_dirs() -> Vec<PathBuf> {
    let variant = match current_theme() {
        LumoTheme::Dark => "Papirus-Dark",
        LumoTheme::Light => "Papirus-Light",
    };

    let base = Path::new("/usr/share/icons");
    vec![
        base.join(variant),
        base.join("Papirus"),
        base.join("hicolor"),
    ]
}

// ---------------------------------------------------------------------------
// icon_name_for_path
// ---------------------------------------------------------------------------

/// Subdiretorios conhecidos de HOME e seus nomes de icone freedesktop.
static HOME_SUBDIRS: &[(&str, &str)] = &[
    ("Downloads", "folder-download"),
    ("Download", "folder-download"),
    ("Documents", "folder-documents"),
    ("Music", "folder-music"),
    ("Pictures", "folder-pictures"),
    ("Videos", "folder-videos"),
    ("Desktop", "user-desktop"),
];

/// Extensoes de imagem -> "image-x-generic".
static IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp"];

/// Extensoes de texto / codigo -> "text-x-generic".
static TEXT_EXTS: &[&str] = &["txt", "md", "rs", "toml", "json", "log", "yaml", "yml", "sh"];

/// Resolve o nome de icone freedesktop para um `Path`.
///
/// Regras (em ordem):
///   - Diretorio: "folder", salvo se o nome do componente final bater
///     com um subdiretorio conhecido de HOME -> nome especifico.
///   - Arquivo por extensao:
///     - png/jpg/jpeg/gif/webp/bmp -> "image-x-generic"
///     - txt/md/rs/toml/json/log   -> "text-x-generic"
///     - pdf                       -> "application-pdf"
///     - outros / sem extensao     -> "text-x-generic"
///
/// Retorna `&'static str` sempre (nome nunca e alocado).
pub fn icon_name_for_path(path: &Path) -> &'static str {
    if path.is_dir() {
        return icon_name_for_dir(path);
    }
    icon_name_for_file(path)
}

fn icon_name_for_dir(path: &Path) -> &'static str {
    let dir_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    for &(subdir, icon) in HOME_SUBDIRS {
        if dir_name.eq_ignore_ascii_case(subdir) {
            return icon;
        }
    }
    "folder"
}

fn icon_name_for_file(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    // Comparacao case-insensitive para extensoes.
    let ext_lower: &str = &ext.to_ascii_lowercase();

    if IMAGE_EXTS.contains(&ext_lower) {
        return "image-x-generic";
    }
    if TEXT_EXTS.contains(&ext_lower) {
        return "text-x-generic";
    }
    if ext_lower == "pdf" {
        return "application-pdf";
    }
    "text-x-generic"
}

// ---------------------------------------------------------------------------
// lookup_icon
// ---------------------------------------------------------------------------

/// Categorias de icone pesquisadas em ordem.
static ICON_CATEGORIES: &[&str] = &["places", "mimetypes", "apps", "devices", "categories"];

/// Tamanhos tentados em ordem de prioridade quando o tamanho exato nao existe.
/// O tamanho pedido e inserido no inicio dinamicamente.
static SIZE_FALLBACKS: &[u32] = &[64, 48, 128, 32, 24, 22, 16];

/// Procura `<dir>/<size>x<size>/<cat>/<name>.svg` (e .png como fallback)
/// em todas as pastas de tema retornadas por `icon_theme_dirs()`.
///
/// Ordem de busca:
///   - Para cada theme dir:
///     - Para cada size em [size, 64, 48, 128, 32, 24, 22, 16]:
///       - Para cada categoria em [places, mimetypes, apps, devices, categories]:
///         - Tenta `<size>x<size>/<cat>/<name>.svg`, depois `.png`.
///     - Tenta `scalable/<cat>/<name>.svg`.
///
/// Retorna o primeiro path existente.
pub fn lookup_icon(name: &str, size: u32) -> Option<PathBuf> {
    let dirs = icon_theme_dirs();

    // Monta lista de sizes sem duplicar o size pedido.
    let mut sizes: Vec<u32> = Vec::with_capacity(SIZE_FALLBACKS.len() + 1);
    sizes.push(size);
    for &s in SIZE_FALLBACKS {
        if s != size {
            sizes.push(s);
        }
    }

    for dir in &dirs {
        // Busca por tamanho.
        for &sz in &sizes {
            let size_dir = format!("{sz}x{sz}");
            for &cat in ICON_CATEGORIES {
                let base = dir.join(&size_dir).join(cat).join(name);

                let svg = base.with_extension("svg");
                if svg.exists() {
                    return Some(svg);
                }
                let png = base.with_extension("png");
                if png.exists() {
                    return Some(png);
                }
            }
        }

        // Busca scalable/.
        for &cat in ICON_CATEGORIES {
            let base = dir.join("scalable").join(cat).join(name);

            let svg = base.with_extension("svg");
            if svg.exists() {
                return Some(svg);
            }
        }
    }

    None
}

// ---------------------------------------------------------------------------
// icon_for_path
// ---------------------------------------------------------------------------

/// Combina `icon_name_for_path` + `lookup_icon`.
///
/// Retorna `None` se nenhum arquivo de icone for encontrado no sistema.
pub fn icon_for_path(path: &Path, size: u32) -> Option<PathBuf> {
    let name = icon_name_for_path(path);
    lookup_icon(name, size)
}

// ---------------------------------------------------------------------------
// Tests (logica pura de naming -- sem FS)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // Helpers: usamos Path::new direto; is_dir() retorna false pra paths
    // inexistentes -- workaround: testamos icon_name_for_file/dir diretamente.

    #[test]
    fn dir_path_is_folder() {
        // Nao toca FS: chama a funcao interna.
        assert_eq!(icon_name_for_dir(Path::new("/home/user/Projects")), "folder");
    }

    #[test]
    fn dir_downloads_maps_to_folder_download() {
        assert_eq!(
            icon_name_for_dir(Path::new("/home/user/Downloads")),
            "folder-download"
        );
    }

    #[test]
    fn dir_documents_maps_to_folder_documents() {
        assert_eq!(
            icon_name_for_dir(Path::new("/home/user/Documents")),
            "folder-documents"
        );
    }

    #[test]
    fn dir_music_maps_to_folder_music() {
        assert_eq!(
            icon_name_for_dir(Path::new("/home/user/Music")),
            "folder-music"
        );
    }

    #[test]
    fn dir_pictures_maps_to_folder_pictures() {
        assert_eq!(
            icon_name_for_dir(Path::new("/home/user/Pictures")),
            "folder-pictures"
        );
    }

    #[test]
    fn dir_videos_maps_to_folder_videos() {
        assert_eq!(
            icon_name_for_dir(Path::new("/home/user/Videos")),
            "folder-videos"
        );
    }

    #[test]
    fn dir_desktop_maps_to_user_desktop() {
        assert_eq!(
            icon_name_for_dir(Path::new("/home/user/Desktop")),
            "user-desktop"
        );
    }

    #[test]
    fn dir_unknown_subdir_is_folder() {
        assert_eq!(
            icon_name_for_dir(Path::new("/home/user/RandomDir")),
            "folder"
        );
    }

    #[test]
    fn file_png_is_image_x_generic() {
        assert_eq!(icon_name_for_file(Path::new("photo.png")), "image-x-generic");
    }

    #[test]
    fn file_jpg_is_image_x_generic() {
        assert_eq!(icon_name_for_file(Path::new("photo.jpg")), "image-x-generic");
    }

    #[test]
    fn file_jpeg_is_image_x_generic() {
        assert_eq!(
            icon_name_for_file(Path::new("photo.jpeg")),
            "image-x-generic"
        );
    }

    #[test]
    fn file_webp_is_image_x_generic() {
        assert_eq!(
            icon_name_for_file(Path::new("anim.webp")),
            "image-x-generic"
        );
    }

    #[test]
    fn file_gif_is_image_x_generic() {
        assert_eq!(icon_name_for_file(Path::new("anim.gif")), "image-x-generic");
    }

    #[test]
    fn file_rs_is_text_x_generic() {
        assert_eq!(icon_name_for_file(Path::new("main.rs")), "text-x-generic");
    }

    #[test]
    fn file_txt_is_text_x_generic() {
        assert_eq!(icon_name_for_file(Path::new("notes.txt")), "text-x-generic");
    }

    #[test]
    fn file_md_is_text_x_generic() {
        assert_eq!(
            icon_name_for_file(Path::new("README.md")),
            "text-x-generic"
        );
    }

    #[test]
    fn file_toml_is_text_x_generic() {
        assert_eq!(
            icon_name_for_file(Path::new("Cargo.toml")),
            "text-x-generic"
        );
    }

    #[test]
    fn file_json_is_text_x_generic() {
        assert_eq!(
            icon_name_for_file(Path::new("package.json")),
            "text-x-generic"
        );
    }

    #[test]
    fn file_pdf_is_application_pdf() {
        assert_eq!(
            icon_name_for_file(Path::new("doc.pdf")),
            "application-pdf"
        );
    }

    #[test]
    fn file_no_extension_is_text_x_generic() {
        assert_eq!(icon_name_for_file(Path::new("Makefile")), "text-x-generic");
    }

    #[test]
    fn file_unknown_extension_is_text_x_generic() {
        assert_eq!(
            icon_name_for_file(Path::new("archive.xz")),
            "text-x-generic"
        );
    }

    #[test]
    fn file_extension_case_insensitive_png() {
        assert_eq!(icon_name_for_file(Path::new("img.PNG")), "image-x-generic");
    }

    #[test]
    fn file_extension_case_insensitive_pdf() {
        assert_eq!(icon_name_for_file(Path::new("doc.PDF")), "application-pdf");
    }

    #[test]
    fn icon_theme_dirs_returns_three() {
        let dirs = icon_theme_dirs();
        assert_eq!(dirs.len(), 3);
        // Todos devem comecar com /usr/share/icons.
        for d in &dirs {
            assert!(d.starts_with("/usr/share/icons"), "unexpected dir: {d:?}");
        }
        // O ultimo deve ser hicolor.
        assert_eq!(dirs[2].file_name().unwrap(), "hicolor");
    }

    #[test]
    fn lookup_icon_returns_none_for_fake_name() {
        // Em ambiente de build (nao tem /usr/share/icons), deve retornar None.
        let result = lookup_icon("__nonexistent_icon_xyz__", 48);
        assert!(result.is_none());
    }
}
