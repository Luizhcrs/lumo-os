//! lumo-appctl - cliente CLI thin shim para lumo-appsd (W34.1 + W34.21).
//!
//! Uso:
//!   lumo-appctl about   # manda IPC pro daemon abrir About
//!   lumo-appctl calc    # idem Calc
//!
//! W34.21: auto-spawn daemon se nao roda + retry.

use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::time::Duration;

fn socket_path() -> PathBuf {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(runtime).join("lumo-appsd.sock")
}

fn resolve_appsd_bin() -> String {
    // W34.24: NAO spawn lumo-appsd --version pra probe (Iced runtime full spawn = hang).
    // Use which command OR PATH search direto.
    if let Ok(p) = std::env::var("PATH") {
        for dir in p.split(':') {
            let candidate = format!("{}/lumo-appsd", dir);
            if std::path::Path::new(&candidate).is_file() {
                return candidate;
            }
        }
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let candidate = format!("{}/Projects/lumo-shell/target/release/lumo-appsd", home);
    if std::path::Path::new(&candidate).exists() {
        return candidate;
    }
    "lumo-appsd".to_string()
}

fn try_connect(path: &PathBuf) -> Option<UnixStream> {
    UnixStream::connect(path).ok()
}

fn spawn_daemon() -> std::io::Result<()> {
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;
    let bin = resolve_appsd_bin();
    eprintln!("[appctl] spawning daemon: {}", bin);
    let mut cmd = Command::new(&bin);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    // W34.23: setsid pra detach do process group do appctl.
    // Sem setsid, signals propagam pro child quando appctl exit.
    unsafe {
        cmd.pre_exec(|| {
            // Cria new session + process group. Detacha completamente.
            libc::setsid();
            Ok(())
        });
    }
    let _child = cmd.spawn()?;
    Ok(())
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    let kind = match argv.get(1) {
        Some(k) => k.clone(),
        None => {
            eprintln!("uso: lumo-appctl <kind> [arg]");
            return ExitCode::from(2);
        }
    };
    let arg = match argv.get(2) {
        Some(p) => format!("{}:{}", kind, p),
        None => kind.clone(),
    };
    let path = socket_path();

    // W34.21: tenta connect; se falhar, spawn daemon + retry ate 10s.
    // W34.23: stale socket file pos appsd exit (W34.21). remove antes spawn.
    let mut stream = match try_connect(&path) {
        Some(s) => s,
        None => {
            // Remove stale socket file (peer fechou, file ainda existe).
            let _ = std::fs::remove_file(&path);
            if let Err(e) = spawn_daemon() {
                eprintln!("lumo-appctl: spawn daemon: {}", e);
                return ExitCode::from(1);
            }
            // Iced runtime cold start ~3-5s. Wait 10s deadline.
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            loop {
                std::thread::sleep(Duration::from_millis(100));
                if let Some(s) = try_connect(&path) {
                    break s;
                }
                if std::time::Instant::now() > deadline {
                    eprintln!("lumo-appctl: daemon nao subiu em 10s");
                    return ExitCode::from(1);
                }
            }
        }
    };
    if let Err(e) = writeln!(stream, "{}", arg) {
        eprintln!("lumo-appctl: write: {}", e);
        return ExitCode::from(1);
    }
    drop(stream);
    ExitCode::SUCCESS
}
