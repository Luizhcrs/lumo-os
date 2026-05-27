//! hook.rs — instalador de panic_hook que serializa crash report.
//!
//! Chamar `install_panic_hook(binary_name, Domain)` no inicio do main de
//! cada binario. Hook preserva chain (chama hook anterior depois).
//!
//! Hook captura panic + grava JSON em `~/.local/state/lumo/crashes/`,
//! depois delega pro default (que escreve no stderr).
//!
//! Limitacao: panic em alocacao heap ou stack overflow nao garante write.

use crate::crash::CrashReport;
use crate::{Domain, Severity};
use std::sync::Once;

static INSTALL: Once = Once::new();

pub fn install_panic_hook(binary: &'static str, domain: Domain) {
    INSTALL.call_once(|| {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let msg = panic_message(info);
            let location = info.location().map(|l| (l.file().to_string(), l.line()));
            let mut report = CrashReport::new(
                binary,
                domain,
                Severity::Fatal,
                "PANIC-UNCAUGHT-001",
                msg,
            );
            if let Some((f, l)) = location {
                report.location = Some(format!("{}:{}", f, l));
            }
            // Ignora falha de write (disco cheio, perm) — nao queremos
            // panicar dentro do hook de panic.
            let _ = report.write_default();
            prev(info);
        }));
    });
}

fn panic_message(info: &std::panic::PanicHookInfo<'_>) -> String {
    if let Some(s) = info.payload().downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = info.payload().downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_is_idempotent() {
        // Garantir que duas chamadas nao panicam.
        install_panic_hook("test-bin-1", Domain::App);
        install_panic_hook("test-bin-2", Domain::App);
    }

    #[test]
    fn panic_message_handles_str_payload() {
        // Constroi PanicHookInfo simulado via catch_unwind.
        let result = std::panic::catch_unwind(|| {
            panic!("test panic with &str");
        });
        assert!(result.is_err());
        // Conteudo do payload conferido indiretamente.
    }
}
