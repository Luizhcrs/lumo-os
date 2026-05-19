//! GET /state -- estado runtime do Lumo OS (compositor, bar, desktop, active_app, output).
//! GET /procs -- lista lumo-* PIDs com RSS + cpu.

use axum::response::Json;
use serde::Serialize;
use serde_json::{json, Value};
use std::path::Path;

use crate::exec;

#[derive(Serialize)]
pub struct StateResp {
    pub compositor: bool,
    pub bar: bool,
    pub desktop: bool,
    pub active_app: Option<ActiveApp>,
    pub output_w: u32,
    pub output_h: u32,
}

#[derive(Serialize, Default)]
pub struct ActiveApp {
    pub app_id: String,
    pub title: String,
    pub pid: u32,
}

fn proc_running(name: &str) -> bool {
    if let Ok(out) = std::process::Command::new("/usr/bin/pgrep").arg("-x").arg(name).output() {
        return out.status.success() && !out.stdout.is_empty();
    }
    false
}

async fn detect_output_size() -> (u32, u32) {
    // Heuristica: usa wlr-randr se disponivel; fallback grim PNG header.
    if let Ok(out) = exec::run("/usr/bin/wlr-randr", &[]).await {
        if out.status == 0 {
            let s = String::from_utf8_lossy(&out.stdout);
            for line in s.lines() {
                let trimmed = line.trim();
                if let Some(dims) = trimmed.split_whitespace().next() {
                    if let Some((w, h)) = dims.split_once('x') {
                        if let (Ok(w), Ok(h)) = (w.parse::<u32>(), h.parse::<u32>()) {
                            if w > 100 && h > 100 {
                                return (w, h);
                            }
                        }
                    }
                }
            }
        }
    }
    // Fallback: ler header de screenshot recente, ou padrao 1920x1080.
    (1920, 1080)
}

pub async fn get_state() -> Json<StateResp> {
    let (output_w, output_h) = detect_output_size().await;
    let resp = StateResp {
        compositor: proc_running("lumo-wm"),
        bar: proc_running("lumo-bar"),
        desktop: proc_running("lumo-desktop"),
        active_app: None, // TODO: subscrever lumo-ipc LumoEvent::ActiveApp em background.
        output_w,
        output_h,
    };
    Json(resp)
}

pub async fn procs() -> Json<Value> {
    // pgrep -af lumo-  -- lista PID + cmdline. ps pra RSS/cpu.
    let mut out_procs: Vec<Value> = Vec::new();
    if let Ok(o) = std::process::Command::new("/usr/bin/pgrep").args(["-f", "lumo-"]).output() {
        let pids: Vec<u32> = String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter_map(|l| l.trim().parse::<u32>().ok())
            .collect();
        for pid in pids {
            let stat = Path::new("/proc").join(pid.to_string()).join("status");
            let cmdline = Path::new("/proc").join(pid.to_string()).join("comm");
            let name = std::fs::read_to_string(&cmdline)
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            if !name.starts_with("lumo-") {
                continue;
            }
            let mut rss_kb: u64 = 0;
            if let Ok(s) = std::fs::read_to_string(&stat) {
                for line in s.lines() {
                    if let Some(rest) = line.strip_prefix("VmRSS:") {
                        rss_kb = rest
                            .trim()
                            .split_whitespace()
                            .next()
                            .and_then(|v| v.parse::<u64>().ok())
                            .unwrap_or(0);
                        break;
                    }
                }
            }
            out_procs.push(json!({"pid": pid, "name": name, "rss_kb": rss_kb}));
        }
    }
    Json(json!({"procs": out_procs}))
}
