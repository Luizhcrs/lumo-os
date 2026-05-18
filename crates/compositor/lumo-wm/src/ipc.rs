//! Servidor IPC do lumo-wm. Socket unix em
//! `$XDG_RUNTIME_DIR/lumo-wm.sock`, integrado no calloop (mesmo
//! event loop que ja roda Wayland + winit). Sem thread extra.
//!
//! Justificativa do design:
//! - **Listener**: source calloop dedicada (Generic). Aceita
//!   conexoes nao-bloqueantes.
//! - **Clients**: NAO sao sources independentes. O bin
//!   `lumo-wm.rs` ja roda um Timer de 4ms (dispatch Wayland
//!   periodico). Reusamos esse mesmo tick pra fazer drain
//!   round-robin de todos os clients IPC. Clients esperados < 5,
//!   payload < 100B/s -> sobra orcamento. Evita complexidade
//!   de re-registrar/desregistrar source por client + simplifica
//!   borrow rules vs. LumoState.
//! - **Sem async runtime**: I/O nao-bloqueante puro, calloop.

use std::collections::VecDeque;
use std::io::{ErrorKind, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::mpsc;

use anyhow::{anyhow, Result};
use lumo_ipc::{
    default_socket_path, encode_event, parse_command, LumoCommand, LumoEvent, MAX_WORKSPACES,
};
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{Interest, LoopHandle, Mode, PostAction};

use crate::state::LumoState;

/// Conexao IPC ativa.
pub struct IpcClient {
    stream: UnixStream,
    rx_buf: Vec<u8>,
    tx_queue: VecDeque<Vec<u8>>,
}

impl IpcClient {
    fn new(stream: UnixStream) -> std::io::Result<Self> {
        stream.set_nonblocking(true)?;
        Ok(Self {
            stream,
            rx_buf: Vec::with_capacity(256),
            tx_queue: VecDeque::new(),
        })
    }

    /// Le bytes disponiveis e devolve linhas completas. Retorna
    /// Err se peer fechou ou se buffer cresceu sem `\n` (proteção
    /// contra peer cuspindo lixo).
    fn read_lines(&mut self) -> std::io::Result<Vec<String>> {
        let mut tmp = [0u8; 512];
        loop {
            match self.stream.read(&mut tmp) {
                Ok(0) => {
                    return Err(std::io::Error::new(
                        ErrorKind::UnexpectedEof,
                        "peer fechou",
                    ));
                }
                Ok(n) => self.rx_buf.extend_from_slice(&tmp[..n]),
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(e) => return Err(e),
            }
        }
        let mut out = Vec::new();
        while let Some(nl) = self.rx_buf.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.rx_buf.drain(..=nl).collect();
            if let Ok(s) = std::str::from_utf8(&line[..line.len() - 1]) {
                out.push(s.to_string());
            }
        }
        if self.rx_buf.len() > 4096 {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                "rx buffer overflow",
            ));
        }
        Ok(out)
    }

    /// Drena tx_queue ate WouldBlock. Returns Ok(()) sempre que
    /// nao deu hard error; queue pode ter sobrado pra prox tick.
    fn drain_tx(&mut self) -> std::io::Result<()> {
        while let Some(front) = self.tx_queue.pop_front() {
            match self.stream.write(&front) {
                Ok(n) if n == front.len() => {}
                Ok(n) => {
                    // partial write -> re-enfileirar resto na frente
                    self.tx_queue.push_front(front[n..].to_vec());
                    return Ok(());
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                    self.tx_queue.push_front(front);
                    return Ok(());
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    fn enqueue(&mut self, payload: Vec<u8>) {
        self.tx_queue.push_back(payload);
    }
}

/// Estado do servidor IPC.
pub struct IpcServer {
    pub socket_path: Option<PathBuf>,
    pub clients: Vec<IpcClient>,
    /// L6: receiver de eventos de theme change do watcher thread.
    pub theme_rx: Option<mpsc::Receiver<lumo_ipc::ThemeMode>>,
}

impl Default for IpcServer {
    fn default() -> Self {
        Self {
            socket_path: None,
            clients: Vec::new(),
            theme_rx: None,
        }
    }
}

impl IpcServer {
    pub fn workspaces_event(active: u8, total: u8) -> LumoEvent {
        let total = total.max(1);
        LumoEvent::Workspaces {
            active: active.clamp(1, total),
            total,
        }
    }

    /// Broadcast: enfileira em todos + tenta drenar uma vez.
    /// Clients com erro sao removidos.
    pub fn broadcast(&mut self, ev: &LumoEvent) {
        let bytes = encode_event(ev).into_bytes();
        let mut dead: Vec<usize> = Vec::new();
        for (i, client) in self.clients.iter_mut().enumerate() {
            client.enqueue(bytes.clone());
            if let Err(err) = client.drain_tx() {
                tracing::debug!(?err, "IPC client write erro, dropando");
                dead.push(i);
            }
        }
        for i in dead.into_iter().rev() {
            self.clients.swap_remove(i);
        }
    }
}

/// L6: inicia thread watcher de ~/.config/lumo/theme.toml.
/// Retorna Receiver que tick() usa pra drenar notificacoes.
pub fn spawn_theme_watcher() -> Option<mpsc::Receiver<lumo_ipc::ThemeMode>> {
    let (tx, rx) = mpsc::channel::<lumo_ipc::ThemeMode>();
    lumo_foundation::watch_theme(move |tokens| {
        let mode = match tokens.mode {
            lumo_foundation::LumoTheme::Light => lumo_ipc::ThemeMode::Light,
            lumo_foundation::LumoTheme::Dark => lumo_ipc::ThemeMode::Dark,
        };
        let _ = tx.send(mode);
    });
    Some(rx)
}

/// Inicializa listener e registra source de accept no calloop.
/// IpcServer fica vivo em LumoState; o tick de drain de clients
/// roda pelo callback fornecido em `tick()`.
pub fn init(loop_handle: LoopHandle<'static, LumoState>) -> Result<IpcServer> {
    let path = default_socket_path()
        .ok_or_else(|| anyhow!("XDG_RUNTIME_DIR ausente; IPC desativado"))?;

    if path.exists() {
        if let Err(err) = std::fs::remove_file(&path) {
            tracing::warn!(?err, ?path, "remove socket antigo falhou");
        }
    }

    let listener = UnixListener::bind(&path)
        .map_err(|e| anyhow!("bind {}: {e}", path.display()))?;
    listener.set_nonblocking(true)?;

    tracing::info!(socket = %path.display(), "lumo-wm IPC listening");

    loop_handle
        .insert_source(
            Generic::new(listener, Interest::READ, Mode::Level),
            move |_, listener, state: &mut LumoState| {
                loop {
                    match listener.accept() {
                        Ok((stream, _)) => match IpcClient::new(stream) {
                            Ok(client) => {
                                state.ipc.clients.push(client);
                                // Snapshot inicial pro novo client.
                                let ev = IpcServer::workspaces_event(
                                    state.active_workspace,
                                    MAX_WORKSPACES,
                                );
                                let bytes = encode_event(&ev).into_bytes();
                                if let Some(last) = state.ipc.clients.last_mut() {
                                    last.enqueue(bytes);
                                    let _ = last.drain_tx();
                                }
                                tracing::info!(
                                    "IPC client conectado (total={})",
                                    state.ipc.clients.len()
                                );
                            }
                            Err(err) => {
                                tracing::warn!(?err, "IPC set_nonblocking falhou");
                            }
                        },
                        Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                        Err(err) => {
                            tracing::warn!(?err, "IPC accept erro");
                            break;
                        }
                    }
                }
                Ok(PostAction::Continue)
            },
        )
        .map_err(|e| anyhow!("calloop listener register: {e}"))?;

    Ok(IpcServer {
        socket_path: Some(path),
        clients: Vec::new(),
        theme_rx: spawn_theme_watcher(),
    })
}

/// Drena leitura+escrita de todos os clients. Chamado no mesmo
/// tick de 4ms do dispatch Wayland (custo proporcional a `clients.len()`,
/// esperado < 5).
pub fn tick(state: &mut LumoState) {
    let mut dead: Vec<usize> = Vec::new();
    let mut commands: Vec<LumoCommand> = Vec::new();
    for (i, client) in state.ipc.clients.iter_mut().enumerate() {
        match client.read_lines() {
            Ok(lines) => {
                for line in lines {
                    match parse_command(&line) {
                        Some(Ok(cmd)) => commands.push(cmd),
                        Some(Err(err)) => {
                            tracing::warn!(?err, line, "IPC parse erro");
                        }
                        None => {}
                    }
                }
            }
            Err(err) => {
                tracing::debug!(?err, "IPC client read erro");
                dead.push(i);
                continue;
            }
        }
        if let Err(err) = client.drain_tx() {
            tracing::debug!(?err, "IPC client write erro");
            dead.push(i);
        }
    }
    for i in dead.into_iter().rev() {
        state.ipc.clients.swap_remove(i);
    }
    for cmd in commands {
        state.handle_ipc_command(cmd);
    }
    // L6: drena notificacoes do theme watcher e broadcast ThemeReloaded.
    // Coleta modos primeiro (borrow imutavel de theme_rx), depois broadcast
    // (borrow mutavel de ipc.clients). Nao podem ser combinados no mesmo scope.
    let pending_modes: Vec<lumo_ipc::ThemeMode> = {
        if let Some(rx) = state.ipc.theme_rx.as_ref() {
            let mut v = Vec::new();
            while let Ok(mode) = rx.try_recv() {
                v.push(mode);
            }
            v
        } else {
            Vec::new()
        }
    };
    for mode in pending_modes {
        tracing::info!(?mode, "L6: theme change detectado via watcher, broadcast");
        state.ipc.broadcast(&LumoEvent::ThemeReloaded { mode });
    }
}
