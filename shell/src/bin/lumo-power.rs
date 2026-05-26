//! lumo-power daemon: charge limit 80% + weekly cell balance (P5).
//!
//! Reads ~/.config/lumo/power.toml. On startup applies set_charge_limit.
//! Minute-tick loop: on Fri 22h triggers balance cycle (100% for N hours, then 80%).
//! Notifies via notify-send (OSD bridge for P5; full IPC ShowOsd in P6+).

use std::time::Duration;

use chrono::{Datelike, Local, Timelike, Weekday};
use serde::Deserialize;

use lumo_sensors::{Battery, SensorError};

// ------------------------------------------------------------
// Config
// ------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct PowerConfig {
    #[serde(default = "def_limit")]
    limit_percent: u8,
    #[serde(default = "def_cron")]
    balance_schedule_cron: String,
    #[serde(default = "def_target")]
    balance_target: u8,
    #[serde(default = "def_hours")]
    balance_duration_hours: u8,
    #[serde(default = "def_enabled")]
    enabled: bool,
}

fn def_limit() -> u8 {
    80
}
fn def_cron() -> String {
    "0 22 * * 5".to_string()
}
fn def_target() -> u8 {
    100
}
fn def_hours() -> u8 {
    12
}
fn def_enabled() -> bool {
    true
}

impl Default for PowerConfig {
    fn default() -> Self {
        Self {
            limit_percent: def_limit(),
            balance_schedule_cron: def_cron(),
            balance_target: def_target(),
            balance_duration_hours: def_hours(),
            enabled: def_enabled(),
        }
    }
}

fn config_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let xdg = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{home}/.config"));
    std::path::PathBuf::from(xdg)
        .join("lumo")
        .join("power.toml")
}

fn load_config() -> PowerConfig {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(s) => match toml::from_str::<PowerConfig>(&s) {
            Ok(c) => {
                eprintln!(
                    "[lumo-power] config: limit={}% balance={}% hours={}h enabled={}",
                    c.limit_percent, c.balance_target, c.balance_duration_hours, c.enabled
                );
                c
            }
            Err(e) => {
                eprintln!("[lumo-power] toml parse error: {e}; defaults");
                PowerConfig::default()
            }
        },
        Err(_) => {
            eprintln!(
                "[lumo-power] no config at {}; defaults (limit=80%)",
                path.display()
            );
            PowerConfig::default()
        }
    }
}

// ------------------------------------------------------------
// OSD notify
// ------------------------------------------------------------

fn notify(text: &str) {
    eprintln!("[lumo-power] notify: {text}");
    let _ = std::process::Command::new("notify-send")
        .args(["--urgency=normal", "--expire-time=4000", "Lumo Power", text])
        .spawn();
}

// ------------------------------------------------------------
// Schedule helpers
// ------------------------------------------------------------

/// Parse minimal cron "MIN HOUR * * WEEKDAY" and match against now.
/// Weekday: cron 5 = Friday.
fn is_balance_time(cfg: &PowerConfig) -> bool {
    let parts: Vec<&str> = cfg.balance_schedule_cron.split_whitespace().collect();
    if parts.len() != 5 {
        return false;
    }
    let cron_hour: u32 = parts[1].parse().unwrap_or(22);
    let cron_wd: u32 = parts[4].parse().unwrap_or(5);
    let now = Local::now();
    let is_target_wd = match cron_wd {
        0 => now.weekday() == Weekday::Sun,
        1 => now.weekday() == Weekday::Mon,
        2 => now.weekday() == Weekday::Tue,
        3 => now.weekday() == Weekday::Wed,
        4 => now.weekday() == Weekday::Thu,
        5 => now.weekday() == Weekday::Fri,
        6 => now.weekday() == Weekday::Sat,
        _ => false,
    };
    is_target_wd && now.hour() == cron_hour
}

/// Returns the number of days until the next Friday, from today.
pub fn days_until_next_friday() -> u32 {
    use chrono::Weekday;
    let wd = Local::now().weekday();
    let today_num = wd.num_days_from_monday(); // Mon=0 .. Sun=6
    let fri_num = Weekday::Fri.num_days_from_monday(); // 4
    if today_num <= fri_num {
        fri_num - today_num
    } else {
        7 - today_num + fri_num
    }
}

// ------------------------------------------------------------
// Balance cycle
// ------------------------------------------------------------

fn run_balance_cycle(battery: &Battery, cfg: &PowerConfig) {
    eprintln!(
        "[lumo-power] balance cycle: raising limit to {}%",
        cfg.balance_target
    );
    notify("Bateria balanceando");
    if let Err(e) = battery.set_charge_limit(cfg.balance_target) {
        eprintln!(
            "[lumo-power] set_charge_limit({}) failed: {e}",
            cfg.balance_target
        );
        return;
    }
    let total_secs = cfg.balance_duration_hours as u64 * 3600;
    eprintln!(
        "[lumo-power] balance cycle: holding {}h",
        cfg.balance_duration_hours
    );
    std::thread::sleep(Duration::from_secs(total_secs));
    eprintln!(
        "[lumo-power] balance cycle: restoring to {}%",
        cfg.limit_percent
    );
    match battery.set_charge_limit(cfg.limit_percent) {
        Ok(()) => notify("Bateria 80% (otimizado)"),
        Err(e) => eprintln!(
            "[lumo-power] restore set_charge_limit({}) failed: {e}",
            cfg.limit_percent
        ),
    }
}

// ------------------------------------------------------------
// Main
// ------------------------------------------------------------

fn main() {
    eprintln!("[lumo-power] starting");
    let battery = match Battery::discover() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[lumo-power] battery discover failed: {e}");
            std::process::exit(1);
        }
    };

    let cfg = load_config();
    if cfg.enabled {
        match battery.set_charge_limit(cfg.limit_percent) {
            Ok(()) => eprintln!(
                "[lumo-power] startup: charge limit -> {}%",
                cfg.limit_percent
            ),
            Err(SensorError::NotSupported(msg)) => {
                eprintln!("[lumo-power] charge limit not supported: {msg}; continuing for balance schedule");
            }
            Err(e) => eprintln!("[lumo-power] set_charge_limit failed: {e}"),
        }
    }

    let mut last_balance_day: Option<chrono::NaiveDate> = None;

    loop {
        std::thread::sleep(Duration::from_secs(60));
        let cfg = load_config();
        if !cfg.enabled {
            continue;
        }
        let today = Local::now().date_naive();
        if last_balance_day != Some(today) && is_balance_time(&cfg) {
            last_balance_day = Some(today);
            eprintln!("[lumo-power] balance schedule matched");
            run_balance_cycle(&battery, &cfg);
        }
    }
}
