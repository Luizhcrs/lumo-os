//! Operacoes de arquivo: rename, mkdir, trash, copy/move.
//!
//! Modulo auto-contido. Retorna Result<(), OpsError> em todas operacoes.
//! IO sincrono aqui — caller decide se roda em thread/spawn_blocking.

use std::path::{Path, PathBuf};
use std::{fmt, fs, io};

/// Erros de operacao de arquivo.
#[derive(Debug)]
pub enum OpsError {
    Io(io::Error),
    Trash(String),
    InvalidPath(String),
}

impl fmt::Display for OpsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpsError::Io(e) => write!(f, "IO: {e}"),
            OpsError::Trash(s) => write!(f, "Lixeira: {s}"),
            OpsError::InvalidPath(s) => write!(f, "Path invalido: {s}"),
        }
    }
}

impl From<io::Error> for OpsError {
    fn from(e: io::Error) -> Self {
        OpsError::Io(e)
    }
}

/// Renomeia um item (arquivo ou pasta). `new_name` e apenas o nome, sem path.
pub fn rename(path: &Path, new_name: &str) -> Result<PathBuf, OpsError> {
    if new_name.is_empty() || new_name.contains('/') || new_name.contains('\0') {
        return Err(OpsError::InvalidPath(format!(
            "nome invalido: {new_name:?}"
        )));
    }
    let parent = path
        .parent()
        .ok_or_else(|| OpsError::InvalidPath("sem parent".into()))?;
    let dest = parent.join(new_name);
    fs::rename(path, &dest)?;
    Ok(dest)
}

/// Cria diretorio. `name` e apenas o nome, sem path separador.
pub fn mkdir(parent: &Path, name: &str) -> Result<PathBuf, OpsError> {
    if name.is_empty() || name.contains('/') || name.contains('\0') {
        return Err(OpsError::InvalidPath(format!("nome invalido: {name:?}")));
    }
    let dest = parent.join(name);
    fs::create_dir(&dest)?;
    Ok(dest)
}

/// Move item para lixeira via trash-rs.
pub fn move_to_trash(path: &Path) -> Result<(), OpsError> {
    trash::delete(path).map_err(|e| OpsError::Trash(e.to_string()))
}

/// Copia arquivo ou diretorio recursivamente para `dest_dir`.
/// Retorna path do item copiado.
pub fn copy_to(src: &Path, dest_dir: &Path) -> Result<PathBuf, OpsError> {
    let name = src
        .file_name()
        .ok_or_else(|| OpsError::InvalidPath("sem file_name".into()))?;
    let dest = dest_dir.join(name);
    if src.is_dir() {
        copy_dir_recursive(src, &dest)?;
    } else {
        fs::copy(src, &dest)?;
    }
    Ok(dest)
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), OpsError> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dest_path)?;
        } else {
            fs::copy(&src_path, &dest_path)?;
        }
    }
    Ok(())
}

/// Move item para `dest_dir`. Tenta rename atomico; fallback copy+delete.
pub fn move_to(src: &Path, dest_dir: &Path) -> Result<PathBuf, OpsError> {
    let name = src
        .file_name()
        .ok_or_else(|| OpsError::InvalidPath("sem file_name".into()))?;
    let dest = dest_dir.join(name);
    if fs::rename(src, &dest).is_ok() {
        return Ok(dest);
    }
    // cross-device: copy + delete
    copy_to(src, dest_dir)?;
    if src.is_dir() {
        fs::remove_dir_all(src)?;
    } else {
        fs::remove_file(src)?;
    }
    Ok(dest)
}

/// Lista entradas de um diretorio ordenadas: pastas primeiro, depois arquivos.
/// Retorna Vec<PathBuf>.
pub fn list_dir(path: &Path) -> Result<Vec<PathBuf>, OpsError> {
    let mut entries: Vec<PathBuf> = fs::read_dir(path)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    entries.sort_by(|a, b| {
        let a_dir = a.is_dir();
        let b_dir = b.is_dir();
        b_dir.cmp(&a_dir).then_with(|| {
            a.file_name()
                .unwrap_or_default()
                .to_ascii_lowercase()
                .cmp(&b.file_name().unwrap_or_default().to_ascii_lowercase())
        })
    });
    Ok(entries)
}

// ---------------------------------------------------------------------------
// Testes
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn tmp() -> TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn mkdir_cria_diretorio() {
        let dir = tmp();
        let result = mkdir(dir.path(), "nova_pasta").unwrap();
        assert!(result.is_dir());
        assert_eq!(result.file_name().unwrap(), "nova_pasta");
    }

    #[test]
    fn mkdir_nome_vazio_falha() {
        let dir = tmp();
        assert!(matches!(
            mkdir(dir.path(), ""),
            Err(OpsError::InvalidPath(_))
        ));
    }

    #[test]
    fn mkdir_nome_com_barra_falha() {
        let dir = tmp();
        assert!(matches!(
            mkdir(dir.path(), "a/b"),
            Err(OpsError::InvalidPath(_))
        ));
    }

    #[test]
    fn rename_arquivo() {
        let dir = tmp();
        let orig = dir.path().join("orig.txt");
        fs::write(&orig, b"hi").unwrap();
        let dest = rename(&orig, "renomeado.txt").unwrap();
        assert!(dest.exists());
        assert!(!orig.exists());
    }

    #[test]
    fn rename_nome_vazio_falha() {
        let dir = tmp();
        let f = dir.path().join("f.txt");
        fs::write(&f, b"x").unwrap();
        assert!(matches!(rename(&f, ""), Err(OpsError::InvalidPath(_))));
    }

    #[test]
    fn rename_pasta() {
        let dir = tmp();
        let pasta = dir.path().join("pasta_antiga");
        fs::create_dir(&pasta).unwrap();
        let dest = rename(&pasta, "pasta_nova").unwrap();
        assert!(dest.is_dir());
    }

    #[test]
    fn copy_to_arquivo() {
        let src_dir = tmp();
        let dst_dir = tmp();
        let src = src_dir.path().join("origem.txt");
        fs::write(&src, b"conteudo").unwrap();
        let dest = copy_to(&src, dst_dir.path()).unwrap();
        assert!(dest.exists());
        assert_eq!(fs::read(&dest).unwrap(), b"conteudo");
        assert!(src.exists()); // original permanece
    }

    #[test]
    fn copy_to_diretorio_recursivo() {
        let src_dir = tmp();
        let dst_dir = tmp();
        let src = src_dir.path().join("pasta");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("arq.txt"), b"abc").unwrap();
        let dest = copy_to(&src, dst_dir.path()).unwrap();
        assert!(dest.is_dir());
        assert!(dest.join("arq.txt").exists());
    }

    #[test]
    fn move_to_arquivo() {
        let src_dir = tmp();
        let dst_dir = tmp();
        let src = src_dir.path().join("mover.txt");
        fs::write(&src, b"dados").unwrap();
        let dest = move_to(&src, dst_dir.path()).unwrap();
        assert!(dest.exists());
        // src pode nao existir apos move atomico
    }

    #[test]
    fn list_dir_pastas_primeiro() {
        let dir = tmp();
        fs::write(dir.path().join("arquivo.txt"), b"").unwrap();
        fs::create_dir(dir.path().join("zpasta")).unwrap();
        let entries = list_dir(dir.path()).unwrap();
        assert!(entries[0].is_dir(), "primeiro entry deve ser pasta");
    }

    #[test]
    fn list_dir_dir_invalido_falha() {
        let result = list_dir(Path::new("/nao/existe/nunca"));
        assert!(matches!(result, Err(OpsError::Io(_))));
    }
}
