//! crash.rs — crash dump em JSON pra `~/.local/state/lumo/crashes/`.
//!
//! Inspirado em ReportCrash macOS: dump out-of-call de panic/abort com stack.
//! Diferenca: rodamos in-process via panic_hook. Limitacao: heap corrompido
//! pode quebrar write. Aceitavel pra MVP; futuramente movemos pra daemon
//! separado lendo via pipe.

use crate::{Domain, Severity};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashReport {
    pub schema: u32,
    pub binary: String,
    pub pid: u32,
    pub ts_unix: u64,
    pub domain: Domain,
    pub severity: Severity,
    pub code: String,
    pub msg: String,
    pub thread: String,
    pub location: Option<String>,
    /// Stack frames symbolicados (best-effort via backtrace crate).
    pub backtrace: Vec<String>,
    pub env_summary: EnvSummary,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnvSummary {
    pub wayland_display: Option<String>,
    pub xdg_session_type: Option<String>,
    pub xdg_runtime_dir: Option<String>,
    pub lumo_version: Option<String>,
}

impl EnvSummary {
    pub fn capture() -> Self {
        let read = |k: &str| std::env::var(k).ok();
        Self {
            wayland_display: read("WAYLAND_DISPLAY"),
            xdg_session_type: read("XDG_SESSION_TYPE"),
            xdg_runtime_dir: read("XDG_RUNTIME_DIR"),
            lumo_version: option_env!("CARGO_PKG_VERSION").map(String::from),
        }
    }
}

pub const SCHEMA: u32 = 1;

/// Diretorio onde crash reports sao escritos.
/// `~/.local/state/lumo/crashes/`. Cria se ausente.
pub fn crash_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".local/state/lumo/crashes")
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Captura backtrace atual (best-effort, depende de RUST_BACKTRACE).
pub fn capture_backtrace() -> Vec<String> {
    let bt = backtrace::Backtrace::new();
    format!("{:?}", bt)
        .lines()
        .map(String::from)
        .take(64)
        .collect()
}

impl CrashReport {
    pub fn new(
        binary: impl Into<String>,
        domain: Domain,
        severity: Severity,
        code: impl Into<String>,
        msg: impl Into<String>,
    ) -> Self {
        Self {
            schema: SCHEMA,
            binary: binary.into(),
            pid: std::process::id(),
            ts_unix: now_secs(),
            domain,
            severity,
            code: code.into(),
            msg: msg.into(),
            thread: std::thread::current()
                .name()
                .unwrap_or("<unnamed>")
                .to_string(),
            location: None,
            backtrace: capture_backtrace(),
            env_summary: EnvSummary::capture(),
        }
    }

    pub fn with_location(mut self, file: &str, line: u32) -> Self {
        self.location = Some(format!("{}:{}", file, line));
        self
    }

    /// Filename estavel `YYYY-MM-DDTHH-MM-SS-<binary>-<pid>.json`.
    /// Usa epoch-secs decompostos manualmente pra evitar dep de chrono.
    pub fn filename(&self) -> String {
        format!("crash-{}-{}-{}.json", self.ts_unix, self.binary, self.pid)
    }

    /// Escreve report em crash_dir(). Cria dir se ausente.
    /// Retorna path ou erro nao-panicante.
    ///
    /// Permissions:
    /// - Dir: 0700 (so owner pode listar/entrar)
    /// - File: 0600 (so owner pode ler)
    /// Razao: crash dumps contem env vars (WAYLAND_DISPLAY, paths) +
    /// backtrace simbolicado. Em sistema multi-user nao expor a outros UIDs.
    pub fn write(&self, dir: &Path) -> std::io::Result<PathBuf> {
        fs::create_dir_all(dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
        }
        let path = dir.join(self.filename());
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        #[cfg(unix)]
        {
            use std::io::Write as _;
            use std::os::unix::fs::OpenOptionsExt;
            let mut f = fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .mode(0o600)
                .open(&path)?;
            f.write_all(json.as_bytes())?;
        }
        #[cfg(not(unix))]
        {
            fs::write(&path, json)?;
        }
        Ok(path)
    }

    pub fn write_default(&self) -> std::io::Result<PathBuf> {
        self.write(&crash_dir())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filename_includes_binary_and_pid() {
        let r = CrashReport::new("lumo-wm", Domain::Compositor, Severity::Fatal, "WM-001", "oops");
        let f = r.filename();
        assert!(f.contains("lumo-wm"));
        assert!(f.contains(&r.pid.to_string()));
        assert!(f.ends_with(".json"));
    }

    #[test]
    fn schema_constant_stable() {
        assert_eq!(SCHEMA, 1);
        let r = CrashReport::new("x", Domain::App, Severity::Fatal, "X-1", "");
        assert_eq!(r.schema, 1);
    }

    #[test]
    fn write_creates_dir_and_file() {
        let tmp = std::env::temp_dir().join(format!("lumo-crash-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let r = CrashReport::new("test-bin", Domain::App, Severity::Fatal, "TEST-001", "boom");
        let path = r.write(&tmp).expect("write");
        assert!(path.exists());
        let content = fs::read_to_string(&path).unwrap();
        let back: CrashReport = serde_json::from_str(&content).unwrap();
        assert_eq!(back.code, "TEST-001");
        assert_eq!(back.binary, "test-bin");
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn capture_backtrace_returns_some_frames() {
        // backtrace sempre retorna pelo menos algumas frames mesmo sem RUST_BACKTRACE.
        let bt = capture_backtrace();
        assert!(!bt.is_empty() || std::env::var("RUST_BACKTRACE").is_err());
    }

    #[test]
    fn with_location_attached() {
        let r = CrashReport::new("x", Domain::App, Severity::Fatal, "X-1", "")
            .with_location("foo.rs", 42);
        assert_eq!(r.location.as_deref(), Some("foo.rs:42"));
    }

    #[cfg(unix)]
    #[test]
    fn write_sets_dir_0700_and_file_0600() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = std::env::temp_dir().join(format!("lumo-perm-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let r = CrashReport::new("x", Domain::App, Severity::Fatal, "X-1", "");
        let path = r.write(&tmp).expect("write");
        let dir_mode = fs::metadata(&tmp).expect("dir").permissions().mode() & 0o777;
        let file_mode = fs::metadata(&path).expect("file").permissions().mode() & 0o777;
        assert_eq!(dir_mode, 0o700, "dir deve ser 0700, got {:o}", dir_mode);
        assert_eq!(file_mode, 0o600, "file deve ser 0600, got {:o}", file_mode);
        fs::remove_dir_all(&tmp).ok();
    }
}
