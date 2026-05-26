//! SI.1: cliente IPC para o compositor lumo-wm.
//!
//! Substitui ydotool/uinput pelas primitivas synthetic input expostas em
//! LumoCommand. Conecta ao socket unix `$XDG_RUNTIME_DIR/lumo-wm.sock`,
//! escreve uma linha JSON e fecha.
//!
//! Protocolo do compositor eh fire-and-forget para LumoCommand -- nao ha
//! ack por-comando. Sucesso = write_all sem erro. Falha = (conexao recusada
//! | broken pipe | serialize error).
//!
//! Fallback opt-in: se a env LUMO_BRIDGE_FALLBACK_YDOTOOL=1 estiver presente
//! e o IPC falhar, o caller pode tentar ydotool. Esse modulo apenas reporta
//! o erro -- a decisao fica nas routes.

use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

use lumo_ipc::LumoCommand;

/// Tipo de erro do client IPC. Convertido pra HTTP 503 nas routes.
#[derive(Debug)]
pub enum IpcError {
    NoRuntimeDir,
    Connect(std::io::Error, PathBuf),
    Serialize(serde_json::Error),
    Write(std::io::Error),
}

impl std::fmt::Display for IpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IpcError::NoRuntimeDir => write!(f, "XDG_RUNTIME_DIR ausente"),
            IpcError::Connect(e, p) => write!(f, "connect {}: {}", p.display(), e),
            IpcError::Serialize(e) => write!(f, "serialize: {}", e),
            IpcError::Write(e) => write!(f, "write: {}", e),
        }
    }
}

impl std::error::Error for IpcError {}

/// Resolve o socket path do compositor.
///
/// Prefere `lumo_ipc::default_socket_path()` (XDG_RUNTIME_DIR). Como o bridge
/// pode rodar como user service que herda env limpa, aceita LUMO_WM_SOCKET
/// como override explicito.
pub fn socket_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("LUMO_WM_SOCKET") {
        return Some(PathBuf::from(p));
    }
    lumo_ipc::default_socket_path()
}

/// Envia um LumoCommand pra um path explicito. Util pra testes.
pub fn send_command_to(path: &std::path::Path, cmd: &LumoCommand) -> Result<(), IpcError> {
    let mut stream =
        UnixStream::connect(path).map_err(|e| IpcError::Connect(e, path.to_path_buf()))?;
    // Timeouts curtos -- compositor processa em <1ms o tick de calloop.
    let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
    let mut payload = serde_json::to_string(cmd).map_err(IpcError::Serialize)?;
    payload.push('\n');
    stream
        .write_all(payload.as_bytes())
        .map_err(IpcError::Write)?;
    let _ = stream.flush();
    // Compositor calloop ticks a ~16ms; aguardar garante leitura antes do close.
    std::thread::sleep(std::time::Duration::from_millis(25));
    Ok(())
}

/// Envia um LumoCommand serializado como linha JSON usando o socket default.
///
/// Chamada bloqueante mas curtissima (write < 1KB, socket local).
/// Spawna em `tokio::task::spawn_blocking` nas routes pra nao bloquear runtime.
pub fn send_command(cmd: &LumoCommand) -> Result<(), IpcError> {
    let path = socket_path().ok_or(IpcError::NoRuntimeDir)?;
    send_command_to(&path, cmd)
}

/// Conveniencia pra routes assincronas: roda send_command em pool blocking.
pub async fn send_command_async(cmd: LumoCommand) -> Result<(), IpcError> {
    tokio::task::spawn_blocking(move || send_command(&cmd))
        .await
        .map_err(|e| IpcError::Write(std::io::Error::new(std::io::ErrorKind::Other, e)))?
}

/// SI.1: true se a env LUMO_BRIDGE_FALLBACK_YDOTOOL=1.
pub fn ydotool_fallback_enabled() -> bool {
    std::env::var("LUMO_BRIDGE_FALLBACK_YDOTOOL")
        .ok()
        .as_deref()
        == Some("1")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::os::unix::net::UnixListener;

    /// SI.1: send_command_to escreve LumoCommand serializado + '\n' no socket.
    /// Usa UnixListener mock; verifica roundtrip via parse_command no peer.
    #[test]
    fn send_command_to_writes_json_line() {
        let dir = tempdir_simple();
        let path = dir.join("lumo-wm.sock");

        let listener = UnixListener::bind(&path).expect("bind");
        let path_for_thread = path.clone();
        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).expect("read");
            (line, path_for_thread)
        });

        let cmd = LumoCommand::SyntheticPointerMove { x: 100.0, y: 200.0 };
        send_command_to(&path, &cmd).expect("send_command_to ok");
        let (line, _) = handle.join().expect("join");
        let parsed = lumo_ipc::parse_command(&line).expect("Some").expect("ok");
        assert_eq!(parsed, cmd);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    /// Sem socket no path = Connect error (NotFound).
    #[test]
    fn send_command_to_no_socket_errors() {
        let dir = tempdir_simple();
        let path = dir.join("does-not-exist.sock");
        let cmd = LumoCommand::CloseDropdowns;
        let err = send_command_to(&path, &cmd).expect_err("must fail");
        assert!(matches!(err, IpcError::Connect(_, _)), "got {err:?}");
        let _ = std::fs::remove_dir(&dir);
    }

    fn tempdir_simple() -> PathBuf {
        let base = std::env::temp_dir();
        let unique = format!(
            "lumo-bridge-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let p = base.join(unique);
        std::fs::create_dir_all(&p).expect("mkdir");
        p
    }
}
