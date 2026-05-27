//! lumo-error — taxonomia compartilhada de erros + crash dump.
//!
//! Todo erro user-facing ou logado deve ter `code` estavel (ex: `WM-RENDER-001`).
//! Mapa central em `docs/error-codes.md`. Adicionar codigo novo: append + doc + bump.
//!
//! Politica:
//! - `Severity::Fatal`: sessao precisa terminar / processo abortar.
//! - `Severity::Degraded`: continua mas feature off (color mgmt, vsync).
//! - `Severity::Recoverable`: retry possivel (sensor read, IPC reconnect).
//! - `Severity::UserError`: input invalido, file nao existe.
//!
//! Convencao de codigo: `<DOMAIN>-<SUBSYS>-<NNN>` em uppercase.

use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Fatal,
    Degraded,
    Recoverable,
    UserError,
    /// Config opt-out PERMANENTE por design (ADR). Nao e bug nem runtime
    /// drop. Exemplo: ADR-002 (color-mgmt OFF default) + ADR-003 (toplevel-icon
    /// OFF default). Bar NAO mostra pill amber pra ConfigInfo — visivel
    /// apenas via lumoctl diag.
    ConfigInfo,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Fatal => "fatal",
            Severity::Degraded => "degraded",
            Severity::Recoverable => "recoverable",
            Severity::UserError => "user_error",
            Severity::ConfigInfo => "config_info",
        }
    }

    /// True se Severity merece UI pill (degraded runtime).
    /// False = silencioso (config_info, recoverable, user_error).
    pub fn warrants_pill(self) -> bool {
        matches!(self, Severity::Degraded | Severity::Fatal)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Domain {
    Compositor,
    Render,
    Input,
    Ipc,
    Sensor,
    Shell,
    App,
    Bridge,
    Theme,
    Telemetry,
    Other,
}

impl Domain {
    pub fn as_str(self) -> &'static str {
        match self {
            Domain::Compositor => "wm",
            Domain::Render => "render",
            Domain::Input => "input",
            Domain::Ipc => "ipc",
            Domain::Sensor => "sensor",
            Domain::Shell => "shell",
            Domain::App => "app",
            Domain::Bridge => "bridge",
            Domain::Theme => "theme",
            Domain::Telemetry => "telemetry",
            Domain::Other => "other",
        }
    }
}

/// Hint estruturado pra UX/recovery, opcional.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "action")]
pub enum RecoveryHint {
    Retry { after_ms: u32 },
    Restart { component: String },
    DisableFeature { feature: String },
    UserAction { hint: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LumoError {
    pub domain: Domain,
    pub severity: Severity,
    /// Codigo estavel `WM-RENDER-001`. Documentar em docs/error-codes.md.
    pub code: std::borrow::Cow<'static, str>,
    pub msg: String,
    /// Causa em string (source().to_string()) — perde tipo mas serializavel.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery: Option<RecoveryHint>,
}

impl LumoError {
    pub fn new(
        domain: Domain,
        severity: Severity,
        code: &'static str,
        msg: impl Into<String>,
    ) -> Self {
        Self {
            domain,
            severity,
            code: std::borrow::Cow::Borrowed(code),
            msg: msg.into(),
            cause: None,
            recovery: None,
        }
    }

    pub fn with_cause<E: Error>(mut self, cause: &E) -> Self {
        self.cause = Some(cause.to_string());
        self
    }

    pub fn with_recovery(mut self, hint: RecoveryHint) -> Self {
        self.recovery = Some(hint);
        self
    }

    pub fn is_fatal(&self) -> bool {
        matches!(self.severity, Severity::Fatal)
    }

    /// Tupla (code, severity_str) pra passar a sistemas de telemetria.
    /// Evita dep circular lumo-error <-> lumo-telemetry; caller faz:
    ///   let (c, s) = err.telemetry_key();
    ///   lumo_telemetry::record_error(c, s);
    pub fn telemetry_key(&self) -> (&str, &'static str) {
        (self.code.as_ref(), self.severity.as_str())
    }
}

impl fmt::Display for LumoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}/{}] {}: {}",
            self.code,
            self.severity.as_str(),
            self.domain.as_str(),
            self.msg
        )?;
        if let Some(c) = &self.cause {
            write!(f, " (caused by: {})", c)?;
        }
        Ok(())
    }
}

impl Error for LumoError {}

#[macro_export]
macro_rules! lumo_err {
    ($domain:expr, $sev:expr, $code:literal, $($arg:tt)*) => {
        $crate::LumoError::new($domain, $sev, $code, format!($($arg)*))
    };
}

/// Helper pra init-time fatal. Log codigo + msg, depois panic.
/// Crash dump escrito por panic_hook. Use no lugar de `.expect("...")`
/// quando init falha = processo nao pode continuar.
///
/// ```ignore
/// let conn = Connection::connect_to_env()
///     .unwrap_or_else(|e| lumo_error::fatal_init("SHELL-INIT-001", "Wayland", &e));
/// ```
pub fn fatal_init<E: std::fmt::Display>(code: &'static str, what: &str, err: &E) -> ! {
    eprintln!("[{}] init fatal: {}: {}", code, what, err);
    // Log via tracing tambem (caso subscriber esteja instalado).
    // Nao podemos importar tracing aqui (lumo-error nao depende).
    // Caller deve ter feito tracing::error antes se quiser estruturado.
    panic!("[{code}] {what}: {err}");
}

pub mod crash;
pub mod hook;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_includes_code_and_severity() {
        let e = LumoError::new(Domain::Render, Severity::Fatal, "WM-RENDER-001", "GPU dead");
        let s = e.to_string();
        assert!(s.contains("WM-RENDER-001"));
        assert!(s.contains("fatal"));
        assert!(s.contains("GPU dead"));
    }

    #[test]
    fn is_fatal_only_for_fatal() {
        let f = LumoError::new(Domain::Compositor, Severity::Fatal, "X-1", "");
        let d = LumoError::new(Domain::Compositor, Severity::Degraded, "X-2", "");
        let r = LumoError::new(Domain::Compositor, Severity::Recoverable, "X-3", "");
        assert!(f.is_fatal());
        assert!(!d.is_fatal());
        assert!(!r.is_fatal());
    }

    #[test]
    fn with_cause_attaches_source_string() {
        let inner = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
        let e = LumoError::new(Domain::Theme, Severity::Recoverable, "THEME-LOAD-001", "load failed")
            .with_cause(&inner);
        assert_eq!(e.cause.as_deref(), Some("no such file"));
        assert!(e.to_string().contains("no such file"));
    }

    #[test]
    fn macro_formats_message() {
        let e = lumo_err!(Domain::Ipc, Severity::Recoverable, "IPC-CONN-002", "peer {} dropped", 42);
        assert_eq!(e.code, "IPC-CONN-002");
        assert!(e.msg.contains("42"));
    }

    #[test]
    fn serializes_to_json_roundtrip() {
        let e = LumoError::new(Domain::Bridge, Severity::UserError, "BRIDGE-AUTH-001", "missing token")
            .with_recovery(RecoveryHint::UserAction { hint: "set token".into() });
        let json = serde_json::to_string(&e).unwrap();
        let back: LumoError = serde_json::from_str(&json).unwrap();
        assert_eq!(back.code, "BRIDGE-AUTH-001");
        assert!(matches!(back.recovery, Some(RecoveryHint::UserAction { .. })));
    }

    #[test]
    fn domain_as_str_stable() {
        assert_eq!(Domain::Compositor.as_str(), "wm");
        assert_eq!(Domain::Render.as_str(), "render");
        assert_eq!(Domain::Ipc.as_str(), "ipc");
    }

    #[test]
    fn severity_as_str_stable() {
        assert_eq!(Severity::Fatal.as_str(), "fatal");
        assert_eq!(Severity::Degraded.as_str(), "degraded");
        assert_eq!(Severity::Recoverable.as_str(), "recoverable");
        assert_eq!(Severity::UserError.as_str(), "user_error");
        assert_eq!(Severity::ConfigInfo.as_str(), "config_info");
    }

    #[test]
    fn warrants_pill_only_for_fatal_and_degraded() {
        assert!(Severity::Fatal.warrants_pill());
        assert!(Severity::Degraded.warrants_pill());
        assert!(!Severity::Recoverable.warrants_pill());
        assert!(!Severity::UserError.warrants_pill());
        assert!(!Severity::ConfigInfo.warrants_pill(), "config_info silencioso");
    }

    #[test]
    fn config_info_serializes_roundtrip() {
        let e = LumoError::new(Domain::Compositor, Severity::ConfigInfo, "WM-COLOR-OFF", "off by design");
        let json = serde_json::to_string(&e).unwrap();
        let back: LumoError = serde_json::from_str(&json).unwrap();
        assert!(matches!(back.severity, Severity::ConfigInfo));
    }
}
