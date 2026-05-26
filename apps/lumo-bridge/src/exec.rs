//! Wrapper Command com timeout, env Wayland injetado, e log estruturado.

use anyhow::{anyhow, Result};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// Env padrao pra comandos que precisam falar com Wayland/ydotoold.
pub fn wayland_env() -> Vec<(String, String)> {
    let xdg = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/run/user/1000".into());
    let wl = std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "wayland-1".into());
    let ydotool_sock = format!("{}/.ydotool_socket", xdg);
    vec![
        ("XDG_RUNTIME_DIR".into(), xdg),
        ("WAYLAND_DISPLAY".into(), wl),
        ("YDOTOOL_SOCKET".into(), ydotool_sock),
    ]
}

pub struct ExecOutput {
    pub status: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Roda `cmd args...` com timeout e env Wayland. Args passados como slice — sem shell quoting.
pub async fn run(cmd: &str, args: &[&str]) -> Result<ExecOutput> {
    run_with_timeout(cmd, args, DEFAULT_TIMEOUT).await
}

pub async fn run_with_timeout(cmd: &str, args: &[&str], dur: Duration) -> Result<ExecOutput> {
    let mut c = Command::new(cmd);
    c.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in wayland_env() {
        c.env(k, v);
    }
    tracing::debug!(target: "lumo_bridge::exec", "exec {} {:?}", cmd, args);
    let child = c.spawn().map_err(|e| anyhow!("spawn {}: {}", cmd, e))?;
    let fut = child.wait_with_output();
    let out = timeout(dur, fut)
        .await
        .map_err(|_| anyhow!("timeout ({:?}) running {}", dur, cmd))??;
    Ok(ExecOutput {
        status: out.status.code().unwrap_or(-1),
        stdout: out.stdout,
        stderr: out.stderr,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn run_echo_safe_quoting() {
        // arg com aspas/espacos -- nao expande, vai literal pro processo
        let out = run("/usr/bin/printf", &["%s", "hello; rm -rf /tmp/whatever"])
            .await
            .unwrap();
        assert_eq!(out.status, 0);
        assert_eq!(out.stdout, b"hello; rm -rf /tmp/whatever");
    }

    #[tokio::test]
    async fn run_timeout_kills() {
        let res = run_with_timeout("/usr/bin/sleep", &["10"], Duration::from_millis(100)).await;
        assert!(res.is_err());
    }
}
