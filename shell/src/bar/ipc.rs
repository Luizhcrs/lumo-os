//! bar/ipc.rs - Cliente IPC pra compositor lumo-wm.
//!
//! Conexao UnixStream non-blocking. Mensagens line-delimited JSON.
//! Read side: drain_ipc le LumoEvent (Workspaces, CloseDropdowns, etc).
//! Write side: send_close_* despacha LumoCommand pro compositor broadcastar.

use std::io::{ErrorKind, Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc,
};

use lumo_ipc::{default_socket_path, LumoCommand, LumoEvent, MAX_WORKSPACES};

use crate::bar::state::LumoBar;

pub fn connect_ipc() -> Option<UnixStream> {
    let path = default_socket_path()?;
    match UnixStream::connect(&path) {
        Ok(s) => {
            s.set_nonblocking(true).ok()?;
            eprintln!("[lumo-bar] IPC conectado em {}", path.display());
            Some(s)
        }
        Err(e) => {
            eprintln!("[lumo-bar] IPC nao conectou ({}): standalone mode", e);
            None
        }
    }
}

/// A25: retorna (alive, close_dropdowns_requested). Caller usa flag pra
/// fechar dropdown ativo + redraw imediato (memory feedback_input_feedback_imediato).
pub fn drain_ipc(
    stream: &mut UnixStream,
    rx_buf: &mut Vec<u8>,
    active_ws: &Arc<AtomicU8>,
) -> (bool, bool) {
    let mut tmp = [0u8; 256];
    let mut alive = true;
    let mut close_dropdowns = false;
    loop {
        match stream.read(&mut tmp) {
            Ok(0) => {
                alive = false;
                break;
            }
            Ok(n) => rx_buf.extend_from_slice(&tmp[..n]),
            Err(e) if e.kind() == ErrorKind::WouldBlock => break,
            Err(_) => {
                alive = false;
                break;
            }
        }
    }
    while let Some(nl) = rx_buf.iter().position(|b| *b == b'\n') {
        let line: Vec<u8> = rx_buf.drain(..=nl).collect();
        if let Ok(s) = std::str::from_utf8(&line[..line.len() - 1]) {
            if let Ok(ev) = serde_json::from_str::<LumoEvent>(s.trim()) {
                match ev {
                    LumoEvent::Workspaces { active, .. } => {
                        active_ws.store(active.clamp(1, MAX_WORKSPACES), Ordering::Relaxed);
                    }
                    LumoEvent::CloseDropdowns => {
                        // A25: lumo-desktop pediu fechar dropdowns via IPC.
                        close_dropdowns = true;
                    }
                    LumoEvent::CloseDesktopMenu => {
                        // A26: evento destinado ao lumo-desktop, bar ignora.
                    }
                }
            }
        }
    }
    (alive, close_dropdowns)
}

impl LumoBar {
    /// A26: envia LumoCommand::CloseDesktopMenu pelo socket IPC.
    /// Usado quando bar abre dropdown -> mutex pede lumo-desktop fechar menu.
    pub fn send_ipc_close_desktop_menu(&mut self) {
        let Some(s) = self.ipc_stream.as_mut() else { return };
        let mut payload = match serde_json::to_string(&LumoCommand::CloseDesktopMenu) {
            Ok(p) => p,
            Err(_) => return,
        };
        payload.push('\n');
        if let Err(e) = s.write_all(payload.as_bytes()) {
            if e.kind() != ErrorKind::WouldBlock {
                eprintln!("[lumo-bar] IPC write CloseDesktopMenu erro: {}; dropando socket", e);
                self.ipc_stream = None;
            }
        }
    }

    /// A26: envia LumoCommand::CloseDropdowns (broadcast a todos os clients).
    /// Usado pelo right-click na bar.
    pub fn send_ipc_close_dropdowns(&mut self) {
        let Some(s) = self.ipc_stream.as_mut() else { return };
        let mut payload = match serde_json::to_string(&LumoCommand::CloseDropdowns) {
            Ok(p) => p,
            Err(_) => return,
        };
        payload.push('\n');
        if let Err(e) = s.write_all(payload.as_bytes()) {
            if e.kind() != ErrorKind::WouldBlock {
                eprintln!("[lumo-bar] IPC write CloseDropdowns erro: {}; dropando socket", e);
                self.ipc_stream = None;
            }
        }
    }
}
