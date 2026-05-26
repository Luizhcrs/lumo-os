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
    if let Ok(out) = std::process::Command::new("/usr/bin/pgrep")
        .arg("-x")
        .arg(name)
        .output()
    {
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
    if let Ok(o) = std::process::Command::new("/usr/bin/pgrep")
        .args(["-f", "lumo-"])
        .output()
    {
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

/// GET /state/dump — snapshot completo Lumo OS pra debug + observability.
/// Agrega: procs lumo-*, samsung-galaxybook sysfs, telemetry socket, env, logs tail.
pub async fn dump() -> Result<Json<Value>, (axum::http::StatusCode, String)> {
    let mut out = serde_json::Map::new();

    // 1. Procs lumo-*
    let procs = collect_procs().await;
    out.insert("procs".into(), procs);

    // 2. samsung-galaxybook sysfs
    out.insert("samsung_galaxybook".into(), collect_galaxybook());

    // 3. Telemetry snapshot
    out.insert("telemetry".into(), collect_telemetry().await);

    // 4. Env critico
    out.insert("env".into(), collect_env());

    // 5. Logs tail
    out.insert("logs".into(), collect_logs_tail());

    // 6. System (load, mem, disk)
    out.insert("system".into(), collect_system().await);

    // 7. Timestamp
    out.insert(
        "ts".into(),
        Value::String(format!("{:?}", std::time::SystemTime::now())),
    );

    Ok(Json(Value::Object(out)))
}

async fn collect_procs() -> Value {
    let mut arr = vec![];
    if let Ok(out) = std::process::Command::new("/usr/bin/ps")
        .args(["-eo", "pid,pcpu,pmem,rss,comm"])
        .output()
    {
        let s = String::from_utf8_lossy(&out.stdout);
        for line in s.lines().skip(1) {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() < 5 {
                continue;
            }
            let comm = cols[4];
            if !comm.starts_with("lumo-") {
                continue;
            }
            arr.push(json!({
                "pid": cols[0].parse::<u32>().unwrap_or(0),
                "cpu_pct": cols[1].parse::<f64>().unwrap_or(0.0),
                "mem_pct": cols[2].parse::<f64>().unwrap_or(0.0),
                "rss_kb": cols[3].parse::<u64>().unwrap_or(0),
                "comm": comm,
            }));
        }
    }
    Value::Array(arr)
}

fn read_sysfs(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
}

fn collect_galaxybook() -> Value {
    json!({
        "platform_profile": read_sysfs("/sys/firmware/acpi/platform_profile"),
        "platform_profile_choices": read_sysfs("/sys/firmware/acpi/platform_profile_choices"),
        "battery_charge_end": read_sysfs("/sys/class/power_supply/BAT1/charge_control_end_threshold"),
        "battery_capacity": read_sysfs("/sys/class/power_supply/BAT1/capacity"),
        "battery_status": read_sysfs("/sys/class/power_supply/BAT1/status"),
        "battery_energy_full": read_sysfs("/sys/class/power_supply/BAT1/energy_full"),
        "battery_energy_full_design": read_sysfs("/sys/class/power_supply/BAT1/energy_full_design"),
        "battery_energy_now": read_sysfs("/sys/class/power_supply/BAT1/energy_now"),
        "battery_power_now": read_sysfs("/sys/class/power_supply/BAT1/power_now"),
        "kbd_backlight": read_sysfs("/sys/class/leds/samsung-galaxybook::kbd_backlight/brightness"),
    })
}

async fn collect_telemetry() -> Value {
    use std::io::Read;
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    let sock = "/run/user/1000/lumo-metrics.sock";
    if let Ok(mut stream) = UnixStream::connect(sock) {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let mut buf = vec![0u8; 8192];
        if let Ok(n) = stream.read(&mut buf) {
            if let Ok(s) = std::str::from_utf8(&buf[..n]) {
                if let Ok(v) = serde_json::from_str::<Value>(s.trim()) {
                    return v;
                }
            }
        }
    }
    json!({ "error": "telemetry socket unavailable" })
}

fn collect_env() -> Value {
    let keys = [
        "LUMO_THEME",
        "LUMO_TRACE_POINTER",
        "WAYLAND_DISPLAY",
        "XDG_RUNTIME_DIR",
        "XDG_SESSION_TYPE",
        "XDG_CURRENT_DESKTOP",
    ];
    let mut m = serde_json::Map::new();
    for k in keys {
        m.insert(
            k.into(),
            Value::String(std::env::var(k).unwrap_or_default()),
        );
    }
    Value::Object(m)
}

fn collect_logs_tail() -> Value {
    let paths = [
        ("lumo_wm_tty", "/tmp/lumo-wm-tty.log"),
        ("lumo_bar", "/tmp/lumo-bar.log"),
        ("lumo_bridge", "/tmp/lumo-bridge.log"),
    ];
    let mut m = serde_json::Map::new();
    for (key, path) in paths {
        if let Ok(s) = std::fs::read_to_string(path) {
            let lines: Vec<&str> = s.lines().rev().take(10).collect();
            let last = lines.into_iter().rev().collect::<Vec<_>>().join("\n");
            m.insert(key.into(), Value::String(last));
        } else {
            m.insert(key.into(), Value::Null);
        }
    }
    Value::Object(m)
}

async fn collect_system() -> Value {
    let mut m = serde_json::Map::new();
    if let Ok(out) = std::process::Command::new("/usr/bin/uptime").output() {
        m.insert(
            "uptime".into(),
            Value::String(String::from_utf8_lossy(&out.stdout).trim().to_string()),
        );
    }
    if let Ok(s) = std::fs::read_to_string("/proc/meminfo") {
        let mut mem = serde_json::Map::new();
        for line in s.lines().take(5) {
            if let Some((k, v)) = line.split_once(':') {
                mem.insert(k.trim().into(), Value::String(v.trim().to_string()));
            }
        }
        m.insert("meminfo".into(), Value::Object(mem));
    }
    if let Ok(s) = std::fs::read_to_string("/proc/loadavg") {
        m.insert("loadavg".into(), Value::String(s.trim().to_string()));
    }
    Value::Object(m)
}
