//! lumo-appctl - cliente CLI thin shim para lumo-appsd (W34.1).
//!
//! Uso:
//!   lumo-appctl about   # manda IPC pro daemon abrir About
//!   lumo-appctl calc    # idem Calc
//!
//! Se daemon nao roda, retorna exit code 1.

use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::ExitCode;

fn socket_path() -> PathBuf {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(runtime).join("lumo-appsd.sock")
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
    // Payload: kind ou kind:arg (path pra files/editor)
    let arg = match argv.get(2) {
        Some(p) => format!("{}:{}", kind, p),
        None => kind,
    };
    let path = socket_path();
    let mut stream = match UnixStream::connect(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("lumo-appctl: conectar {}: {}", path.display(), e);
            eprintln!("Daemon nao roda? Tenta: nohup lumo-appsd > /tmp/appsd.log 2>&1 &");
            return ExitCode::from(1);
        }
    };
    if let Err(e) = writeln!(stream, "{}", arg) {
        eprintln!("lumo-appctl: write: {}", e);
        return ExitCode::from(1);
    }
    drop(stream);
    ExitCode::SUCCESS
}
