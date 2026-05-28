//! crash_watcher.rs — UX1: watcher do diretorio ~/.local/state/lumo/crashes/
//!
//! Security (review H3 + M1):
//! - H3: markup_escape no body antes de mandar pra toast (body-markup
//!   capability anunciada -> attacker que escreve crash json poderia
//!   forjar Pango markup / HTML injection).
//! - M1: open com O_NOFOLLOW + size cap 64 KiB pra evitar symlink
//!   ataque (~/.ssh/id_rsa) e DoS read GB.
//! - L2: HOME unset -> abort watcher (nao /tmp fallback).
//!
//! Filtra arquivos ja vistos via in-memory HashSet de filenames.

use lumo_notif::sanitize::{clamp, markup_escape};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::dbus::NotifEvent;

const POLL_INTERVAL: Duration = Duration::from_secs(5);
const MAX_CRASH_FILE_BYTES: u64 = 64 * 1024;
const MAX_SUMMARY_CHARS: usize = 256;
const MAX_BODY_CHARS: usize = 1024;

pub fn try_crash_dir() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    if home.is_empty() {
        return None;
    }
    Some(PathBuf::from(home).join(".local/state/lumo/crashes"))
}

pub async fn run(tx: mpsc::Sender<NotifEvent>) {
    let Some(dir) = try_crash_dir() else {
        eprintln!("[lumo-notif] crash_watcher: HOME nao definido, watcher off");
        return;
    };
    let mut seen: HashSet<String> = HashSet::new();

    if let Ok(entries) = std::fs::read_dir(&dir) {
        for ent in entries.flatten() {
            if let Some(name) = ent.file_name().to_str() {
                seen.insert(name.to_string());
            }
        }
    }

    let mut counter = 1_000_000u32;

    loop {
        tokio::time::sleep(POLL_INTERVAL).await;
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for ent in entries.flatten() {
            let Some(name) = ent.file_name().to_str().map(String::from) else {
                continue;
            };
            if seen.contains(&name) {
                continue;
            }
            if !name.ends_with(".json") {
                seen.insert(name);
                continue;
            }
            let path = ent.path();
            seen.insert(name);
            if let Some((summary, body)) = read_report(&path) {
                counter += 1;
                let _ = tx
                    .send(NotifEvent::Notify {
                        id: counter,
                        app_name: "lumo-crash".into(),
                        summary: clamp(&summary, MAX_SUMMARY_CHARS),
                        body: clamp(&markup_escape(&body), MAX_BODY_CHARS),
                        timeout_ms: 0,
                        urgency: lumo_notif::urgency::Urgency::Critical,
                    })
                    .await;
            }
        }
    }
}

/// M1: read seguro com O_NOFOLLOW + size cap.
fn read_safe(path: &std::path::Path) -> Option<String> {
    #[cfg(unix)]
    {
        use std::io::Read;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .ok()?;
        let meta = f.metadata().ok()?;
        if !meta.is_file() {
            return None;
        }
        if meta.len() > MAX_CRASH_FILE_BYTES {
            return None;
        }
        let mut buf = String::new();
        f.read_to_string(&mut buf).ok()?;
        return Some(buf);
    }
    #[cfg(not(unix))]
    {
        let meta = std::fs::metadata(path).ok()?;
        if !meta.is_file() {
            return None;
        }
        if meta.len() > MAX_CRASH_FILE_BYTES {
            return None;
        }
        std::fs::read_to_string(path).ok()
    }
}

fn read_report(path: &std::path::Path) -> Option<(String, String)> {
    let content = read_safe(path)?;
    let v: serde_json::Value = serde_json::from_str(&content).ok()?;
    let binary = v.get("binary")?.as_str().unwrap_or("?");
    let code = v.get("code")?.as_str().unwrap_or("?");
    let msg = v.get("msg").and_then(|m| m.as_str()).unwrap_or("");
    let thread = v
        .get("thread")
        .and_then(|t| t.as_str())
        .unwrap_or("<unknown>");
    let summary = format!("{} crashed", binary);
    let body = format!("{}\nthread: {}\n{}", code, thread, msg);
    Some((summary, body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "crash-watcher-{}-{}-{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    #[test]
    fn read_report_extracts_fields() {
        let dir = tmp("extract");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("crash.json");
        let json = serde_json::json!({
            "binary": "lumo-files",
            "code": "PANIC-001",
            "msg": "boom",
            "thread": "main",
        });
        fs::write(&path, json.to_string()).unwrap();
        let (summary, body) = read_report(&path).expect("parse");
        assert_eq!(summary, "lumo-files crashed");
        assert!(body.contains("PANIC-001"));
        assert!(body.contains("main"));
        assert!(body.contains("boom"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn read_report_invalid_json_returns_none() {
        let dir = tmp("invalid");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.json");
        fs::write(&path, "not json").unwrap();
        assert!(read_report(&path).is_none());
        fs::remove_dir_all(&dir).ok();
    }

    // M1: oversize file rejeitado
    #[test]
    fn read_safe_rejects_oversize_file() {
        let dir = tmp("oversize");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("big.json");
        let huge = vec![b'a'; (MAX_CRASH_FILE_BYTES + 1) as usize];
        fs::write(&path, &huge).unwrap();
        assert!(read_safe(&path).is_none(), "arquivo grande deve ser rejeitado");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn read_safe_accepts_normal_file() {
        let dir = tmp("normal");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ok.json");
        fs::write(&path, b"{}").unwrap();
        assert!(read_safe(&path).is_some());
        fs::remove_dir_all(&dir).ok();
    }

    // L2: try_crash_dir respeita HOME unset
    #[test]
    fn try_crash_dir_none_without_home() {
        let old = std::env::var("HOME").ok();
        std::env::remove_var("HOME");
        assert!(try_crash_dir().is_none());
        if let Some(h) = old {
            std::env::set_var("HOME", h);
        }
    }

    // M1: symlink rejeitado em Unix
    #[cfg(unix)]
    #[test]
    fn read_safe_refuses_symlink() {
        use std::os::unix::fs::symlink;
        let dir = tmp("symlink");
        fs::create_dir_all(&dir).unwrap();
        let real = dir.join("real.json");
        fs::write(&real, b"{}").unwrap();
        let link = dir.join("link.json");
        symlink(&real, &link).unwrap();
        // O_NOFOLLOW: abrir link deve falhar.
        assert!(read_safe(&link).is_none());
        fs::remove_dir_all(&dir).ok();
    }
}
