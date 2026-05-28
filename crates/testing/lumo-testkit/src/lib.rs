//! lumo-testkit — fixtures e helpers compartilhados pra tests do workspace.
//!
//! Resolve issues identificados em review estrutura de testes:
//! - Race env vars (HOME unset/set entre tests parallelos) — `EnvGuard` serializa
//! - Tmpdir boilerplate repetido em N arquivos — `tempdir()` re-export de `tempfile`
//! - Factories de struct duplicadas — providers genericos
//!
//! Usar como `[dev-dependencies] lumo-testkit = { path = "..." }`.

use std::sync::{Mutex, MutexGuard, OnceLock};

pub use tempfile::TempDir;

/// Cria tmpdir com cleanup auto. Wrapper de tempfile::TempDir
/// padronizando uso em todo workspace.
pub fn tempdir() -> TempDir {
    tempfile::TempDir::new().expect("tempfile new")
}

/// Mutex global pra serializar tests que mexem env vars.
/// Cargo roda tests em parallel; sem guard `set_var("HOME", x)` em test A
/// vaza pra test B. Pegar guard antes de mexer:
///
/// ```ignore
/// let _g = lumo_testkit::env_guard();
/// std::env::set_var("HOME", "/x");
/// ```
fn env_mutex() -> &'static Mutex<()> {
    static M: OnceLock<Mutex<()>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(()))
}

/// Guard exclusivo pra mexer em env vars sem race. Drop libera.
pub fn env_guard() -> MutexGuard<'static, ()> {
    // Sobrevive poison (se outro test panicou com guard segurado, devolve inner).
    env_mutex().lock().unwrap_or_else(|e| e.into_inner())
}

/// Scoped env var setter. Restaura valor antigo no drop.
pub struct EnvVarGuard {
    key: String,
    old: Option<String>,
    _lock: MutexGuard<'static, ()>,
}

impl EnvVarGuard {
    pub fn set(key: &str, value: &str) -> Self {
        let lock = env_guard();
        let old = std::env::var(key).ok();
        std::env::set_var(key, value);
        Self {
            key: key.to_string(),
            old,
            _lock: lock,
        }
    }

    pub fn unset(key: &str) -> Self {
        let lock = env_guard();
        let old = std::env::var(key).ok();
        std::env::remove_var(key);
        Self {
            key: key.to_string(),
            old,
            _lock: lock,
        }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match &self.old {
            Some(v) => std::env::set_var(&self.key, v),
            None => std::env::remove_var(&self.key),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tempdir_exists_and_cleans_up() {
        let path;
        {
            let d = tempdir();
            path = d.path().to_path_buf();
            assert!(path.exists());
        }
        // Drop -> cleanup.
        assert!(!path.exists());
    }

    #[test]
    fn env_var_guard_sets_and_restores() {
        // Garante que o original (se houver) e restaurado.
        let _g_original = EnvVarGuard::set("LUMO_TEST_VAR_X", "original");
        {
            let _g = EnvVarGuard::set("LUMO_TEST_VAR_X", "scoped");
            assert_eq!(std::env::var("LUMO_TEST_VAR_X").unwrap(), "scoped");
        }
        // Restored
        assert_eq!(std::env::var("LUMO_TEST_VAR_X").unwrap(), "original");
    }

    #[test]
    fn env_var_guard_unset_restores() {
        let _g = EnvVarGuard::set("LUMO_TEST_VAR_Y", "value");
        {
            let _u = EnvVarGuard::unset("LUMO_TEST_VAR_Y");
            assert!(std::env::var("LUMO_TEST_VAR_Y").is_err());
        }
        assert_eq!(std::env::var("LUMO_TEST_VAR_Y").unwrap(), "value");
    }

    #[test]
    fn env_var_guard_set_when_originally_unset() {
        std::env::remove_var("LUMO_TEST_VAR_Z");
        {
            let _g = EnvVarGuard::set("LUMO_TEST_VAR_Z", "x");
            assert_eq!(std::env::var("LUMO_TEST_VAR_Z").unwrap(), "x");
        }
        assert!(std::env::var("LUMO_TEST_VAR_Z").is_err());
    }
}
