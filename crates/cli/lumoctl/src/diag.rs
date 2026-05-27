//! diag.rs — subcomando lumoctl diag
//!
//! Coleta estado pra share com support:
//! - ultimos N crash dumps
//! - GPU info (lspci grep VGA, /sys/class/drm/card*)
//! - IPC sockets ativos em XDG_RUNTIME_DIR
//! - Lumo binaries em PATH + versoes
//!
//! Output texto humano por default. --json pra estruturado.

use std::path::PathBuf;
use std::process::Command;

pub fn run(args: &[String]) {
    let json_mode = args.iter().any(|a| a == "--json");
    let report = collect();
    if json_mode {
        println!("{}", serde_json::to_string_pretty(&report).unwrap_or_default());
    } else {
        print_text(&report);
    }
}

#[derive(serde::Serialize)]
pub struct DiagReport {
    pub hostname: Option<String>,
    pub wayland_display: Option<String>,
    pub xdg_session_type: Option<String>,
    pub crash_count: usize,
    pub crash_latest: Vec<String>,
    pub gpu_lines: Vec<String>,
    pub drm_cards: Vec<String>,
    pub sockets: Vec<String>,
}

pub fn collect() -> DiagReport {
    DiagReport {
        hostname: std::env::var("HOSTNAME").ok().or_else(|| {
            std::fs::read_to_string("/etc/hostname")
                .ok()
                .map(|s| s.trim().to_string())
        }),
        wayland_display: std::env::var("WAYLAND_DISPLAY").ok(),
        xdg_session_type: std::env::var("XDG_SESSION_TYPE").ok(),
        crash_count: count_crashes(),
        crash_latest: list_recent_crashes(5),
        gpu_lines: gpu_lspci(),
        drm_cards: drm_cards(),
        sockets: ipc_sockets(),
    }
}

fn crash_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".local/state/lumo/crashes")
}

fn count_crashes() -> usize {
    std::fs::read_dir(crash_dir())
        .map(|d| {
            d.flatten()
                .filter(|e| {
                    e.file_name()
                        .to_str()
                        .map(|s| s.ends_with(".json"))
                        .unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0)
}

fn list_recent_crashes(n: usize) -> Vec<String> {
    let dir = crash_dir();
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .map(|d| {
            d.flatten()
                .filter_map(|e| e.file_name().to_str().map(String::from))
                .filter(|s| s.ends_with(".json"))
                .collect()
        })
        .unwrap_or_default();
    names.sort_by(|a, b| b.cmp(a));
    names.into_iter().take(n).collect()
}

fn gpu_lspci() -> Vec<String> {
    let out = Command::new("lspci").arg("-mm").output().ok();
    let Some(out) = out else {
        return vec![];
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| l.contains("VGA") || l.contains("3D"))
        .map(String::from)
        .collect()
}

fn drm_cards() -> Vec<String> {
    let Ok(entries) = std::fs::read_dir("/sys/class/drm") else {
        return vec![];
    };
    entries
        .flatten()
        .filter_map(|e| e.file_name().to_str().map(String::from))
        .filter(|s| s.starts_with("card"))
        .collect()
}

fn ipc_sockets() -> Vec<String> {
    let Ok(rt) = std::env::var("XDG_RUNTIME_DIR") else {
        return vec![];
    };
    std::fs::read_dir(rt)
        .map(|d| {
            d.flatten()
                .filter_map(|e| e.file_name().to_str().map(String::from))
                .filter(|s| s.contains("lumo") || s == "wayland-0" || s == "wayland-1")
                .collect()
        })
        .unwrap_or_default()
}

fn print_text(r: &DiagReport) {
    println!("=== Lumo OS Diagnostico ===");
    if let Some(h) = &r.hostname {
        println!("hostname: {h}");
    }
    if let Some(w) = &r.wayland_display {
        println!("WAYLAND_DISPLAY: {w}");
    }
    if let Some(s) = &r.xdg_session_type {
        println!("XDG_SESSION_TYPE: {s}");
    }
    println!();
    println!("Crashes total: {}", r.crash_count);
    if !r.crash_latest.is_empty() {
        println!("Recentes:");
        for c in &r.crash_latest {
            println!("  {}", c);
        }
    }
    println!();
    if !r.gpu_lines.is_empty() {
        println!("GPU:");
        for g in &r.gpu_lines {
            println!("  {}", g);
        }
    }
    if !r.drm_cards.is_empty() {
        println!("DRM cards: {}", r.drm_cards.join(", "));
    }
    if !r.sockets.is_empty() {
        println!("IPC sockets:");
        for s in &r.sockets {
            println!("  {}", s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diag_report_serializes() {
        let r = DiagReport {
            hostname: Some("test".into()),
            wayland_display: None,
            xdg_session_type: None,
            crash_count: 0,
            crash_latest: vec![],
            gpu_lines: vec![],
            drm_cards: vec![],
            sockets: vec![],
        };
        let s = serde_json::to_string(&r).expect("ser");
        assert!(s.contains("hostname"));
        assert!(s.contains("crash_count"));
    }

    #[test]
    fn list_recent_crashes_returns_at_most_n() {
        let v = list_recent_crashes(3);
        assert!(v.len() <= 3);
    }
}
