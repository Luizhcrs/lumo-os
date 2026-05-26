//! install.rs -- instalacao/remocao de pacotes via pkexec pacman.

use std::process::Command;

/// Instala pacote via `pkexec pacman -S --noconfirm <pkg>`.
/// Retorna Ok(()) ou Err com stderr do processo.
pub fn install_pkg(pkg: &str) -> Result<(), String> {
    let out = Command::new("pkexec")
        .args(["pacman", "-S", "--noconfirm", pkg])
        .output()
        .map_err(|e| format!("pkexec: {e}"))?;

    if out.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        Err(stderr)
    }
}

/// Remove pacote via `pkexec pacman -R --noconfirm <pkg>`.
pub fn remove_pkg(pkg: &str) -> Result<(), String> {
    let out = Command::new("pkexec")
        .args(["pacman", "-R", "--noconfirm", pkg])
        .output()
        .map_err(|e| format!("pkexec: {e}"))?;

    if out.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        Err(stderr)
    }
}

/// Lista pacotes instalados via `pacman -Q`.
/// Retorna vetor de nomes de pacotes.
pub fn list_installed() -> Vec<String> {
    let out = Command::new("pacman").args(["-Q", "--noconfirm"]).output();

    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter_map(|line| {
                let pkg = line.split_whitespace().next()?;
                Some(pkg.to_string())
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Verifica se pacote esta instalado (busca exata).
pub fn is_installed(pkg: &str) -> bool {
    let out = Command::new("pacman").args(["-Q", pkg]).output();

    matches!(out, Ok(o) if o.status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_installed_returns_vec() {
        // pacman pode nao estar disponivel em CI; verificar que nao panicar
        let pkgs = list_installed();
        let _: Vec<String> = pkgs;
    }

    #[test]
    fn is_installed_nonexistent_returns_false() {
        // Pacote ficticio nunca esta instalado
        assert!(!is_installed("lumo-nonexistent-xyz-test-package"));
    }
}
