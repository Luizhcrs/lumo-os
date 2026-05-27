//! lumo-clip - daemon clipboard history + SUPER+V picker overlay (W11.B).
//!
//! Arquitetura:
//!   - daemon: monitora selecao Wayland via wl_data_device, persiste em JSON
//!   - picker: overlay layer-shell invocado via IPC quando SUPER+V e detectado
//!
//! Hotkey SUPER+V: lumo-wm envia sinal SIGUSR1 ao pid do daemon.
//! Daemon recebe sinal, abre picker, cola entrada selecionada.

mod history;
mod picker;

use history::{ClipEntry, ClipHistory};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    lumo_error::hook::install_panic_hook("lumo-clip", lumo_error::Domain::App);
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("lumo_clip=info".parse().unwrap()),
        )
        .init();

    let show_picker = Arc::new(AtomicBool::new(false));
    let _show_picker_clone = show_picker.clone();

    // SIGUSR1 -> abrir picker
    unsafe {
        libc::signal(libc::SIGUSR1, sigusr1_handler as libc::sighandler_t);
    }

    tracing::info!("lumo-clip daemon iniciado, pid={}", std::process::id());

    // Daemon principal: poll clipboard via wl_data_device
    // Simplificado: monitora arquivo de clipboard temporario do compositor.
    // Producao: usar wl_data_device_manager offer events.
    let mut history = ClipHistory::load();
    let mut poll_interval = tokio::time::interval(tokio::time::Duration::from_millis(500));

    loop {
        poll_interval.tick().await;

        // Verificar sinal SIGUSR1 (picker request)
        if OPEN_PICKER.load(Ordering::Relaxed) {
            OPEN_PICKER.store(false, Ordering::Relaxed);
            let entries = history.entries.clone();
            tracing::info!("abrindo picker com {} entradas", entries.len());
            if let Some(selected) = picker::run_picker(entries) {
                tracing::info!("entrada selecionada: {}", selected.preview(60));
                // paste via xdotool/wtype -- integracao futura
            }
        }

        // Poll clipboard simples via primary selection file
        if let Some(text) = read_clipboard_text() {
            history.push(ClipEntry::Text { content: text });
        }
    }
}

static OPEN_PICKER: AtomicBool = AtomicBool::new(false);

extern "C" fn sigusr1_handler(_: libc::c_int) {
    OPEN_PICKER.store(true, Ordering::Relaxed);
}

fn read_clipboard_text() -> Option<String> {
    // Leitura via /tmp/lumo-clipboard (escrito pelo compositor ao mudar selecao)
    let p = "/tmp/lumo-clipboard";
    let s = std::fs::read_to_string(p).ok()?;
    let s = s.trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}
