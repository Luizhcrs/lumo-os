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
    if Command::new("lumo-appsd").arg("--version").output().is_ok() {
        return "lumo-appsd".to_string();
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
    use std::process::Stdio;
    let bin = resolve_appsd_bin();
    eprintln!("[appctl] spawning daemon: {}", bin);
    // Detach: setsid + nohup pattern via std::process::Command + setpgid.
    let mut cmd = Command::new(&bin);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    // Preserve env (XDG_RUNTIME_DIR, WAYLAND_DISPLAY, ICED_BACKEND).
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

    // W34.21: tenta connect; se falhar, spawn daemon + retry ate 3s.
    let mut stream = match try_connect(&path) {
        Some(s) => s,
        None => {
            if let Err(e) = spawn_daemon() {
                eprintln!("lumo-appctl: spawn daemon: {}", e);
                return ExitCode::from(1);
            }
            // Aguarda socket aparecer + bind.
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            loop {
                std::thread::sleep(Duration::from_millis(50));
                if let Some(s) = try_connect(&path) { break s; }
                if std::time::Instant::now() > deadline {
                    eprintln!("lumo-appctl: daemon nao subiu em 5s");
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
