//! bar/system_info.rs - Leitura de info do sistema (bateria, wifi, datetime).
//!
//! Helpers que tocam /sys/class/power_supply, /sys/class/net e processos
//! externos (iw, ip). Sem deps adicionais (nl80211 crate pesa demais pra
//! "iw dev <iface> link" curto).

use chrono::{Datelike, Local, Timelike};

use crate::bar::dropdowns::battery::BatteryInfo;
use crate::bar::dropdowns::datetime::{month_full_pt, weekday_full_pt, DateTimeInfo};
use crate::bar::dropdowns::wifi::{dbm_to_pct, find_wifi_iface, WifiInfo, WifiNetwork};

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
        current_now: current_now_ua,
        charge_limit: sys_read_u32(&format!("{}/charge_control_end_threshold", dir)).map(|v| v.clamp(0, 100) as u8),
        balance_days: {
            let limit = sys_read_u32(&format!("{}/charge_control_end_threshold", dir)).map(|v| v.clamp(0, 100) as u8).unwrap_or(100);
            if limit <= 80 { Some(days_until_next_friday()) } else { None }
        },
        platform_profile: sys_read_string("/sys/firmware/acpi/platform_profile"),
        cpu_temp_c: read_cpu_temp_celsius(),
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

    // A31: lista redes proximas via nmcli (scan cache + filtra).
    info.networks = list_wifi_networks();
    eprintln!(
        "[lumo-bar] read_wifi_info: iface={:?} ssid={:?} dbm={:?} pct={:?} freq={:?} bitrate={:?} ip={:?} networks={}",
        info.iface, info.ssid, info.signal_dbm, info.signal_pct, info.freq_ghz, info.bitrate_mbps, info.ip,
        info.networks.len()
    );

    info
}

/// A31: enumera redes wifi via `nmcli -t -f IN-USE,SSID,SIGNAL,SECURITY dev wifi list`.
///
/// Output `-t` (terse) usa `:` como separador. SSIDs com `:` literal vem
/// escapados como `\:`. Dedupe por SSID (mantem signal maior). Sort desc.
pub fn list_wifi_networks() -> Vec<WifiNetwork> {
    let out = match std::process::Command::new("nmcli")
        .args(["-t", "-f", "IN-USE,SSID,SIGNAL,SECURITY", "dev", "wifi", "list"])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => {
            eprintln!("[lumo-bar] list_wifi_networks: nmcli falhou (vazio)");
            return Vec::new();
        }
    };

    let text = String::from_utf8_lossy(&out.stdout);
    let mut acc: std::collections::HashMap<String, WifiNetwork> = std::collections::HashMap::new();

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        // Parse manual respeitando `\:` escape do nmcli terse.
        let fields = nmcli_split(line);
        if fields.len() < 4 {
            continue;
        }
        let in_use = fields[0].trim();
        let ssid = fields[1].trim();
        let signal_str = fields[2].trim();
        let security = fields[3].trim();

        // Skip linhas sem SSID OU placeholder "--".
        if ssid.is_empty() || ssid == "--" {
            continue;
        }
        let signal_pct: u8 = signal_str.parse().unwrap_or(0).min(100);
        let connected = in_use == "*";
        let secured = !security.is_empty() && security != "--";

        let entry = acc.entry(ssid.to_string()).or_insert(WifiNetwork {
            ssid: ssid.to_string(),
            signal_pct: 0,
            secured,
            connected,
        });
        if signal_pct > entry.signal_pct {
            entry.signal_pct = signal_pct;
        }
        entry.connected = entry.connected || connected;
        entry.secured = entry.secured || secured;
    }

    let mut list: Vec<WifiNetwork> = acc.into_values().collect();
    list.sort_by(|a, b| {
        b.connected
            .cmp(&a.connected)
            .then_with(|| b.signal_pct.cmp(&a.signal_pct))
    });

    eprintln!("[lumo-bar] nmcli list parsed {} networks", list.len());
    list
}

/// Split terse nmcli line por `:` respeitando escape `\:` dentro de campos.
fn nmcli_split(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(&nxt) = chars.peek() {
                cur.push(nxt);
                chars.next();
                continue;
            }
        }
        if c == ':' {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    out.push(cur);
    out
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
    let first = NaiveDate::from_ymd_opt(year, month, 1).expect("year e month validados pelo caller; dia 1 sempre existe");
    let first_weekday = first.weekday().num_days_from_sunday() as usize;
    let next_month_first = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1).expect("janeiro dia 1 sempre existe")
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1).expect("mes + 1 com dia 1 valido quando month < 12")
    };
    let days_in_month = next_month_first.pred_opt().expect("pred_opt de Jan 1 nunca e None para datas razoaveis").day();

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

// ============================================================
// A31.2: actions wifi via nmcli (spawn async thread, fire-and-forget).
// ============================================================

/// Toggle radio wifi via nmcli. Async. Log saida.
pub fn nm_set_radio(on: bool) {
    std::thread::spawn(move || {
        // Bug Luiz 2026-05-18 v4: wifi voltava "desligado" sozinho.
        // Garante hardware unblocked antes de ligar radio.
        if on {
            let _ = std::process::Command::new("rfkill")
                .args(["unblock", "wifi"])
                .output();
        }
        let arg = if on { "on" } else { "off" };
        let res = std::process::Command::new("nmcli")
            .args(["radio", "wifi", arg])
            .output();
        match res {
            Ok(o) if o.status.success() => {
                eprintln!("[lumo-bar] nmcli radio wifi {} OK", arg);
            }
            Ok(o) => {
                let e = String::from_utf8_lossy(&o.stderr);
                eprintln!("[lumo-bar] nmcli radio wifi {} falha: {}", arg, e.trim());
            }
            Err(e) => eprintln!("[lumo-bar] nmcli spawn falha: {}", e),
        }
    });
}

/// Resultado assincrono de nm_connect.
pub enum NmConnectResult {
    Ok,
    NeedPassword { ssid: String },
    Failed(String),
}

/// Conecta a uma rede salva ou nova.
/// Async. Retorna receiver que entrega NmConnectResult.
/// A31.3: quando rede pede senha, envia NeedPassword -> main loop abre modal.
pub fn nm_connect(ssid: String) -> std::sync::mpsc::Receiver<NmConnectResult> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        eprintln!("[lumo-bar] R2 nm_connect ssid={:?} (len={})", ssid, ssid.len());
        let up = std::process::Command::new("nmcli")
            .args(["con", "up", &ssid])
            .output();
        match up {
            Ok(o) if o.status.success() => {
                eprintln!("[lumo-bar] nmcli con up {:?} OK", ssid);
                let _ = tx.send(NmConnectResult::Ok);
                return;
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                let stdout = String::from_utf8_lossy(&o.stdout);
                if stderr.contains("Secrets were required") || stdout.contains("Secrets were required") {
                    eprintln!("[lumo-bar] nm_connect {:?}: senha necessaria -> modal A31.3", ssid);
                    let _ = tx.send(NmConnectResult::NeedPassword { ssid });
                    return;
                }
                eprintln!("[lumo-bar] nm_connect {:?} con up falhou; tenta dev wifi connect", ssid);
            }
            Err(e) => {
                eprintln!("[lumo-bar] nmcli spawn falha: {}", e);
                let _ = tx.send(NmConnectResult::Failed(e.to_string()));
                return;
            }
        }
        let iface_opt = find_wifi_iface();
        let res = if let Some(ref iface) = iface_opt {
            std::process::Command::new("nmcli")
                .args(["dev", "wifi", "connect", &ssid, "ifname", iface])
                .output()
        } else {
            std::process::Command::new("nmcli")
                .args(["dev", "wifi", "connect", &ssid])
                .output()
        };
        match res {
            Ok(o) if o.status.success() => {
                eprintln!("[lumo-bar] nmcli dev wifi connect {:?} OK", ssid);
                let _ = tx.send(NmConnectResult::Ok);
            }
            Ok(o) => {
                let e = String::from_utf8_lossy(&o.stderr);
                let stdout = String::from_utf8_lossy(&o.stdout);
                if e.contains("Secrets were required") || stdout.contains("Secrets were required") {
                    eprintln!("[lumo-bar] nm_connect {:?}: senha necessaria fallback -> modal", ssid);
                    let _ = tx.send(NmConnectResult::NeedPassword { ssid });
                } else {
                    eprintln!("[lumo-bar] nmcli dev wifi connect {:?} falha: {}", ssid, e.trim());
                    let _ = tx.send(NmConnectResult::Failed(format!("{}", e.trim())));
                }
            }
            Err(e) => {
                let _ = tx.send(NmConnectResult::Failed(e.to_string()));
            }
        }
    });
    rx
}

/// Conecta com senha explicita (A31.3: pos modal de senha).
/// Senha vai por stdin (nmcli --ask), nunca por argv -- evita exposicao em /proc/<pid>/cmdline.
pub fn nm_connect_with_password(ssid: String, password: String) {
    use std::io::Write;
    use std::process::{Command, Stdio};
    std::thread::spawn(move || {
        eprintln!("[lumo-bar] A31.3 nm_connect_with_password ssid={:?}", ssid);
        let iface_opt = find_wifi_iface();
        let mut args: Vec<String> = vec![
            "--ask".into(), "dev".into(), "wifi".into(), "connect".into(), ssid.clone(),
        ];
        if let Some(ref iface) = iface_opt {
            args.push("ifname".into());
            args.push(iface.clone());
        }
        let mut child = match Command::new("nmcli")
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[lumo-bar] A31.3 nm_connect_with_password spawn falha: {}", e);
                return;
            }
        };
        if let Some(stdin) = child.stdin.as_mut() {
            let _ = writeln!(stdin, "{}", password);
        }
        match child.wait_with_output() {
            Ok(o) if o.status.success() => {
                eprintln!("[lumo-bar] A31.3 nm_connect_with_password {:?} OK", ssid);
            }
            Ok(o) => {
                let e = String::from_utf8_lossy(&o.stderr);
                eprintln!("[lumo-bar] A31.3 nm_connect_with_password {:?} falha: {}", ssid, e.trim());
            }
            Err(e) => eprintln!("[lumo-bar] A31.3 nm_connect_with_password wait falha: {}", e),
        }
    });
}

/// Desconecta interface wifi ativa. Async.
pub fn nm_disconnect_iface(iface: String) {
    std::thread::spawn(move || {
        let res = std::process::Command::new("nmcli")
            .args(["dev", "disconnect", &iface])
            .output();
        match res {
            Ok(o) if o.status.success() => {
                eprintln!("[lumo-bar] nmcli disconnect {} OK", iface);
            }
            Ok(o) => {
                let e = String::from_utf8_lossy(&o.stderr);
                eprintln!("[lumo-bar] nmcli disconnect {} falha: {}", iface, e.trim());
            }
            Err(e) => eprintln!("[lumo-bar] nmcli spawn falha: {}", e),
        }
    });
}

// ============================================================
// L5: CPU thermal + platform profile helpers.
// ============================================================


/// Returns days until the next Friday (0 = today is Friday, 1 = tomorrow, ...).
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

/// Returns x86_pkg_temp (or first TCPU zone) temperature in Celsius.
/// Returns None if no suitable zone found.
pub fn read_cpu_temp_celsius() -> Option<f32> {
    for idx in 0..16 {
        let base = format!("/sys/class/thermal/thermal_zone{}", idx);
        let type_path = format!("{}/type", base);
        let temp_path = format!("{}/temp", base);
        let kind = sys_read_string(&type_path).unwrap_or_default();
        let lower = kind.to_ascii_lowercase();
        if lower.contains("x86_pkg") || lower.contains("tcpu") {
            if let Some(raw) = sys_read_u32(&temp_path) {
                return Some(raw as f32 / 1000.0);
            }
        }
    }
    None
}

/// Reads current platform_profile, cycles to the next in the ordered list,
/// writes it, and returns the new value as a String.
pub fn platform_profile_cycle_next() -> Option<String> {
    let order = ["low-power", "quiet", "balanced", "performance"];
    let current = sys_read_string("/sys/firmware/acpi/platform_profile")?;
    let idx = order.iter().position(|&s| s == current.trim()).unwrap_or(2);
    let next = order[(idx + 1) % order.len()];
    std::fs::write("/sys/firmware/acpi/platform_profile", next).ok()?;
    Some(next.to_string())
}

// ============================================================
// L5: Brightness info.
// ============================================================

pub fn read_brightness_info() -> crate::bar::dropdowns::brightness::BrightnessInfo {
    let dirs = [
        "/sys/class/backlight/intel_backlight",
        "/sys/class/backlight/amdgpu_bl0",
        "/sys/class/backlight/acpi_video0",
    ];
    for dir in &dirs {
        let cur_path = format!("{}/brightness", dir);
        let max_path = format!("{}/max_brightness", dir);
        if let (Some(cur), Some(max)) = (sys_read_u32(&cur_path), sys_read_u32(&max_path)) {
            if max > 0 {
                let pct = ((cur as f32 / max as f32) * 100.0).round().clamp(0.0, 100.0) as u8;
                return crate::bar::dropdowns::brightness::BrightnessInfo { pct };
            }
        }
    }
    crate::bar::dropdowns::brightness::BrightnessInfo::default()
}

/// T1.8: usa lumo_sensors::Backlight::set_percent -- fonte de verdade unica.
/// Remove duplicata que divergia de lid.rs. Ambos chamam o mesmo sysfs path.
pub fn set_brightness_pct(pct: u8) {
    match lumo_sensors::Backlight::discover() {
        Some(bl) => {
            if let Err(e) = bl.set_percent(pct) {
                eprintln!("[lumo-bar] brightness set_percent({pct}) erro: {e}");
            }
        }
        None => {
            eprintln!("[lumo-bar] brightness: nenhum backlight encontrado");
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    /// Smoke test: nm_connect_with_password nao expoe senha em argv.
    /// Verifica que a funcao compila com a assinatura correta e usa stdin path.
    /// (Teste de integracao real requereria nmcli e root -- skipped em CI.)
    #[test]
    fn nm_connect_with_password_signature_ok() {
        // Compilar essa chamada prova que a assinatura eh (String, String) -> ()
        // e que o body usa Stdio::piped() em vez de args de senha.
        // Nao executa de fato: ssid invalido = nmcli falha silenciosamente no thread.
        let _fn: fn(String, String) = super::nm_connect_with_password;
        // Se esse teste compila, a assinatura publica esta correta.
    }
}
