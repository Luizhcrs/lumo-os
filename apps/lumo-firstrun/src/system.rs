//! system.rs -- chamadas de sistema: useradd, nmcli, flag first-run.

use std::process::Command;

/// Cria usuario via `useradd`. Retorna Err se o comando falhar.
pub fn create_user(username: &str, password: &str) -> Result<(), String> {
    // useradd -m -s /bin/bash <username>
    let status = Command::new("useradd")
        .args(["-m", "-s", "/bin/bash", username])
        .status()
        .map_err(|e| format!("useradd falhou: {e}"))?;

    if !status.success() {
        return Err(format!("useradd retornou codigo {}", status.code().unwrap_or(-1)));
    }

    // Definir senha via `chpasswd`
    let mut child = Command::new("chpasswd")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("chpasswd falhou: {e}"))?;

    if let Some(stdin) = child.stdin.as_mut() {
        use std::io::Write;
        writeln!(stdin, "{username}:{password}")
            .map_err(|e| format!("chpasswd stdin: {e}"))?;
    }

    let out = child.wait().map_err(|e| format!("chpasswd wait: {e}"))?;
    if !out.success() {
        return Err(format!("chpasswd retornou {}", out.code().unwrap_or(-1)));
    }

    Ok(())
}

/// Lista redes Wi-Fi via `nmcli -t -f SSID,SIGNAL,SECURITY device wifi list`.
/// Retorna lista de (ssid, signal, secured).
pub fn list_wifi() -> Vec<(String, u8, bool)> {
    let out = Command::new("nmcli")
        .args(["-t", "-f", "SSID,SIGNAL,SECURITY", "device", "wifi", "list"])
        .output();

    let out = match out {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    if !out.status.success() {
        return Vec::new();
    }

    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(3, ':').collect();
            if parts.len() < 3 {
                return None;
            }
            let ssid = parts[0].trim().to_string();
            if ssid.is_empty() {
                return None;
            }
            let signal: u8 = parts[1].trim().parse().unwrap_or(0);
            let secured = !parts[2].trim().eq_ignore_ascii_case("--");
            Some((ssid, signal, secured))
        })
        .collect()
}

/// Conecta a rede Wi-Fi via nmcli.
pub fn connect_wifi(ssid: &str, password: Option<&str>) -> Result<(), String> {
    let mut cmd = Command::new("nmcli");
    cmd.args(["device", "wifi", "connect", ssid]);
    if let Some(pw) = password {
        cmd.args(["password", pw]);
    }
    let status = cmd.status().map_err(|e| format!("nmcli connect: {e}"))?;
    if !status.success() {
        return Err(format!("nmcli retornou {}", status.code().unwrap_or(-1)));
    }
    Ok(())
}

/// Cria o arquivo flag indicando que o first-run foi concluido.
pub fn mark_first_run_done(flag_path: &str) -> std::io::Result<()> {
    if let Some(parent) = std::path::Path::new(flag_path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(flag_path, "")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn mark_first_run_creates_file() {
        let dir = tempdir().unwrap();
        let flag = dir.path().join("first-run-done");
        mark_first_run_done(flag.to_str().unwrap()).unwrap();
        assert!(flag.exists());
    }

    #[test]
    fn mark_first_run_idempotent() {
        let dir = tempdir().unwrap();
        let flag = dir.path().join("first-run-done");
        mark_first_run_done(flag.to_str().unwrap()).unwrap();
        mark_first_run_done(flag.to_str().unwrap()).unwrap(); // segunda chamada nao falha
        assert!(flag.exists());
    }

    #[test]
    fn list_wifi_returns_vec() {
        // nmcli pode nao estar disponivel em CI, so verificamos que nao panicar
        let nets = list_wifi();
        // resultado pode ser vazio; tipo correto
        let _: Vec<(String, u8, bool)> = nets;
    }
}
