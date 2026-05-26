//! lumo-lock auth module W10.A PAM 0.8 or su fallback.
pub fn authenticate(password: &str) -> Result<(), String> {
    #[cfg(feature = "pam-auth")]
    return authenticate_pam(password);
    #[allow(unreachable_code)]
    authenticate_su(password)
}

#[cfg(feature = "pam-auth")]
fn authenticate_pam(password: &str) -> Result<(), String> {
    use pam::Client;
    let username = get_username();
    let mut client = Client::with_password("login").map_err(|e| format!("PAM init: {e:?}"))?;
    client
        .conversation_mut()
        .set_credentials(username.as_str(), password);
    client
        .authenticate()
        .map_err(|e| format!("Senha incorreta ({e:?})"))?;
    Ok(())
}

fn authenticate_su(password: &str) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let username = get_username();
    let mut child = Command::new("su")
        .args(["-c", "true", &username])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("spawn su: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(password.as_bytes());
        let _ = stdin.write_all(b"\n");
    }
    let status = child.wait().map_err(|e| format!("wait: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("Senha incorreta".to_string())
    }
}

fn get_username() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "root".to_string())
}
