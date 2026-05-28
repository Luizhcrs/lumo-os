//! crash_watcher.rs — UX1: watcher do diretorio ~/.local/state/lumo/crashes/
//!
//! Cada CrashReport JSON novo dispara notificacao toast (urgency=critical):
//!   summary = "lumo-X crashed"
//!   body    = "Codigo: WM-RENDER-PANIC\nthread main\n..."
//!
//! Implementacao polling (5s) em vez de inotify pra evitar dep nova.
//! Trade-off: latencia max 5s. Aceitavel pra crashes (raros, nao real-time).
//!
//! Filtra arquivos ja vistos via in-memory HashSet de filenames.
//! No restart do daemon, todos os crashes existentes sao re-notificados
//! ate o restart-budget vencer (4s timeout via cada toast).

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::dbus::NotifEvent;

const POLL_INTERVAL: Duration = Duration::from_secs(5);

pub fn crash_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".local/state/lumo/crashes")
}

pub async fn run(tx: mpsc::Sender<NotifEvent>) {
    let dir = crash_dir();
    let mut seen: HashSet<String> = HashSet::new();

    // Marca como ja vistos arquivos pre-existentes no startup (evita
    // notif storm se daemon reinicia com 50 crashes antigos).
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for ent in entries.flatten() {
            if let Some(name) = ent.file_name().to_str() {
                seen.insert(name.to_string());
            }
        }
    }

    let mut counter = 1_000_000u32; // espaco separado do dbus counter

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
                        summary,
                        body,
                        timeout_ms: 0, // sticky: critical
                        urgency: lumo_notif::urgency::Urgency::Critical,
                    })
                    .await;
            }
        }
    }
}

fn read_report(path: &std::path::Path) -> Option<(String, String)> {
    let content = std::fs::read_to_string(path).ok()?;
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

    fn tmp() -> PathBuf {
        std::env::temp_dir().join(format!(
            "crash-watcher-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    #[test]
    fn read_report_extracts_fields() {
        let dir = tmp();
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("crash.json");
        let json = serde_json::json!({
            "schema": 1,
            "binary": "lumo-files",
            "pid": 42,
            "ts_unix": 0,
            "domain": "app",
            "severity": "fatal",
            "code": "PANIC-UNCAUGHT-001",
            "msg": "boom",
            "thread": "main",
            "backtrace": [],
            "env_summary": {}
        });
        fs::write(&path, json.to_string()).unwrap();
        let (summary, body) = read_report(&path).expect("parse");
        assert_eq!(summary, "lumo-files crashed");
        assert!(body.contains("PANIC-UNCAUGHT-001"));
        assert!(body.contains("main"));
        assert!(body.contains("boom"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn read_report_returns_none_for_invalid_json() {
        let dir = tmp();
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.json");
        fs::write(&path, "not json").unwrap();
        assert!(read_report(&path).is_none());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn read_report_handles_missing_fields() {
        let dir = tmp();
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("partial.json");
        fs::write(&path, r#"{"binary":"x","code":"X-1"}"#).unwrap();
        let (summary, body) = read_report(&path).expect("partial ok");
        assert!(summary.contains("x"));
        assert!(body.contains("X-1"));
        fs::remove_dir_all(&dir).ok();
    }
}
