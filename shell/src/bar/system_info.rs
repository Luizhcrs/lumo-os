//! bar/system_info.rs - Leitura de info do sistema (bateria, wifi, datetime).
//!
//! Helpers que tocam /sys/class/power_supply, /sys/class/net e processos
//! externos (iw, ip). Sem deps adicionais (nl80211 crate pesa demais pra
//! "iw dev <iface> link" curto).

use chrono::{Datelike, Local, Timelike};

use crate::bar::dropdowns::battery::BatteryInfo;
use crate::bar::dropdowns::datetime::{month_full_pt, weekday_full_pt, DateTimeInfo};
use crate::bar::dropdowns::wifi::{dbm_to_pct, find_wifi_iface, WifiInfo};

// ============================================================
// /sys helpers.
// ============================================================

pub fn sys_read_string(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn sys_read_u32(path: &str) -> Option<u32> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
}

// ============================================================
// BatteryInfo.
// ============================================================

pub fn read_battery_info() -> BatteryInfo {
    let mut dir = String::new();
    for bat in &["BAT0", "BAT1", "BAT2"] {
        let p = format!("/sys/class/power_supply/{}", bat);
        if std::path::Path::new(&format!("{}/capacity", p)).exists() {
            dir = p;
            break;
        }
    }
    if dir.is_empty() {
        return BatteryInfo {
            pct: 100,
            status: "Sem bateria".to_string(),
            ..Default::default()
        };
    }
    let voltage_uv = sys_read_u32(&format!("{}/voltage_now", dir));
    let voltage_mv = voltage_uv.map(|v| v / 1000);

    // Tenta energy_* (mWh, microWh dividido). Fallback charge_* (uAh)
    // convertendo via voltage_mv (mV) -> mWh = mAh * V = mAh * mV / 1000.
    let energy_now = sys_read_u32(&format!("{}/energy_now", dir));
    let energy_full = sys_read_u32(&format!("{}/energy_full", dir));
    let energy_full_design = sys_read_u32(&format!("{}/energy_full_design", dir));
    let power_now_uw = sys_read_u32(&format!("{}/power_now", dir));

    let charge_now = sys_read_u32(&format!("{}/charge_now", dir));
    let charge_full = sys_read_u32(&format!("{}/charge_full", dir));
    let charge_full_design = sys_read_u32(&format!("{}/charge_full_design", dir));
    let current_now_ua = sys_read_u32(&format!("{}/current_now", dir));

    // Convert uWh -> mWh (divide 1000); uAh + mV -> uWh -> mWh.
    fn uwh_to_mwh(uwh: Option<u32>) -> Option<u32> { uwh.map(|v| v / 1000) }
    fn uah_to_mwh(uah: Option<u32>, mv: Option<u32>) -> Option<u32> {
        let a = uah? as u64;
        let v = mv? as u64;
        Some(((a * v) / 1_000_000) as u32)
    }

    let full = uwh_to_mwh(energy_full).or_else(|| uah_to_mwh(charge_full, voltage_mv));
    let now = uwh_to_mwh(energy_now).or_else(|| uah_to_mwh(charge_now, voltage_mv));
    let full_design = uwh_to_mwh(energy_full_design)
        .or_else(|| uah_to_mwh(charge_full_design, voltage_mv));
    let power_now = uwh_to_mwh(power_now_uw)
        .or_else(|| uah_to_mwh(current_now_ua, voltage_mv));

    BatteryInfo {
        pct: sys_read_u32(&format!("{}/capacity", dir)).unwrap_or(100) as u8,
        status: sys_read_string(&format!("{}/status", dir)).unwrap_or_else(|| "?".into()),
        cycles: sys_read_u32(&format!("{}/cycle_count", dir)),
        full,
        now,
        full_design,
        power_now,
        voltage_now_mv: voltage_mv,
        model: sys_read_string(&format!("{}/model_name", dir)),
        manufacturer: sys_read_string(&format!("{}/manufacturer", dir)),
    }
}

// ============================================================
// WifiInfo.
// ============================================================

/// Le info real do wifi via `iw dev <iface> link` + `ip -4 -o addr show <iface>`.
pub fn read_wifi_info() -> WifiInfo {
    let iface = match find_wifi_iface() {
        Some(n) => n,
        None => return WifiInfo::default(),
    };

    let mut info = WifiInfo {
        up: true,
        iface: Some(iface.clone()),
        ..Default::default()
    };

    // ---- iw dev <iface> link ----
    if let Ok(out) = std::process::Command::new("iw")
        .args(["dev", &iface, "link"])
        .output()
    {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout);
            // Se "Not connected" => up mas sem rede ativa.
            if s.contains("Not connected") {
                info.up = false;
            } else {
                for raw in s.lines() {
                    let line = raw.trim();
                    if let Some(v) = line.strip_prefix("SSID:") {
                        info.ssid = Some(v.trim().to_string());
                    } else if let Some(v) = line.strip_prefix("freq:") {
                        let mhz: f32 = v.trim().parse().unwrap_or(0.0);
                        if mhz > 0.0 {
                            info.freq_ghz = Some((mhz / 1000.0 * 10.0).round() / 10.0);
                        }
                    } else if let Some(v) = line.strip_prefix("signal:") {
                        let tok = v.trim().split_whitespace().next().unwrap_or("");
                        if let Ok(d) = tok.parse::<i32>() {
                            info.signal_dbm = Some(d);
                            info.signal_pct = Some(dbm_to_pct(d));
                        }
                    } else if let Some(v) = line.strip_prefix("tx bitrate:") {
                        let tok = v.trim().split_whitespace().next().unwrap_or("");
                        if let Ok(f) = tok.parse::<f32>() {
                            info.bitrate_mbps = Some(f.round() as u32);
                        }
                    }
                }
            }
        }
    }

    // ---- ip -4 -o addr show <iface> ----
    if let Ok(out) = std::process::Command::new("ip")
        .args(["-4", "-o", "addr", "show", &iface])
        .output()
    {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout);
            for line in s.lines() {
                if let Some(pos) = line.find("inet ") {
                    let rest = &line[pos + 5..];
                    let ip = rest
                        .split_whitespace()
                        .next()
                        .unwrap_or("")
                        .split('/')
                        .next()
                        .unwrap_or("")
                        .to_string();
                    if !ip.is_empty() {
                        info.ip = Some(ip);
                        break;
                    }
                }
            }
        }
    }

    eprintln!(
        "[lumo-bar] read_wifi_info: iface={:?} ssid={:?} dbm={:?} pct={:?} freq={:?} bitrate={:?} ip={:?}",
        info.iface, info.ssid, info.signal_dbm, info.signal_pct, info.freq_ghz, info.bitrate_mbps, info.ip
    );

    info
}

/// Check rapido se ha interface wireless up. Usado pelo render do icone wifi.
pub fn read_wifi() -> bool {
    if let Ok(entries) = std::fs::read_dir("/sys/class/net") {
        for e in entries.flatten() {
            let name = e.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("wl") {
                let s =
                    std::fs::read_to_string(e.path().join("operstate")).unwrap_or_default();
                if s.trim() == "up" {
                    return true;
                }
            }
        }
    }
    false
}

// ============================================================
// DateTime helpers.
// ============================================================

/// Constroi grid 6x7 do mes. Coluna 0 = Domingo (padrao PT-BR D S T Q Q S S).
pub fn month_grid_for(year: i32, month: u32) -> Vec<Vec<Option<u32>>> {
    use chrono::NaiveDate;
    let first = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let first_weekday = first.weekday().num_days_from_sunday() as usize;
    let next_month_first = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap()
    };
    let days_in_month = next_month_first.pred_opt().unwrap().day();

    let mut grid = vec![vec![None; 7]; 6];
    let mut day = 1u32;
    for week in 0..6 {
        for col in 0..7 {
            if (week == 0 && col < first_weekday) || day > days_in_month {
                grid[week][col] = None;
            } else {
                grid[week][col] = Some(day);
                day += 1;
            }
        }
    }
    grid
}

pub fn read_datetime_info(viewed_year: i32, viewed_month: u32, selected_day: Option<u32>) -> DateTimeInfo {
    let now = Local::now();
    DateTimeInfo {
        weekday_full: weekday_full_pt(now.weekday()).to_string(),
        day: now.day(),
        month_full: month_full_pt(now.month()).to_string(),
        year: now.year(),
        hour: now.hour() as u8,
        minute: now.minute() as u8,
        second: now.second() as u8,
        month_grid: month_grid_for(viewed_year, viewed_month),
        today_day: now.day(),
        today_month: now.month(),
        today_year: now.year(),
        viewed_year,
        viewed_month,
        viewed_month_full: month_full_pt(viewed_month).to_string(),
        selected_day,
    }
}

// ============================================================
// Format helpers (date PT-BR pra bar pill direita).
// ============================================================

pub fn weekday_abbr_pt(d: chrono::Weekday) -> &'static str {
    use chrono::Weekday::*;
    match d {
        Mon => "seg", Tue => "ter", Wed => "qua", Thu => "qui",
        Fri => "sex", Sat => "sab", Sun => "dom",
    }
}

pub fn month_abbr_pt(m: u32) -> &'static str {
    match m {
        1 => "jan", 2 => "fev", 3 => "mar", 4 => "abr",
        5 => "mai", 6 => "jun", 7 => "jul", 8 => "ago",
        9 => "set", 10 => "out", 11 => "nov", 12 => "dez",
        _ => "?",
    }
}

pub fn format_date_pt(dt: &chrono::DateTime<Local>) -> String {
    format!("{} {} {}", weekday_abbr_pt(dt.weekday()), dt.day(), month_abbr_pt(dt.month()))
}
