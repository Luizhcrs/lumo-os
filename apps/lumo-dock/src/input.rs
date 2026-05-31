//! input.rs -- hit-test e focus-or-spawn de apps.

use crate::config::SlotConfig;
use std::collections::HashMap;

pub fn hit_test_slot(px: f32, slot_rects: &[(f32, f32)], trash_rect: Option<(f32, f32)>) -> i32 {
    for (i, &(x, w)) in slot_rects.iter().enumerate() {
        if px >= x && px < x + w {
            return i as i32;
        }
    }
    if let Some((x, w)) = trash_rect {
        if px >= x && px < x + w {
            return slot_rects.len() as i32;
        }
    }
    -1
}

/// F (auditoria 2026-05): clique num slot. Se o app ja esta rodando (dot verde),
/// pede ao compositor pra FOCAR a janela existente (single-instance) em vez de
/// dar spawn de uma 2a copia. So spawna quando nao ha instancia.
pub fn handle_click(hover_idx: i32, slots: &[SlotConfig], running: &HashMap<String, bool>) {
    if hover_idx < 0 {
        return;
    }
    let idx = hover_idx as usize;
    if idx >= slots.len() {
        // Trash: sempre abre o lumo-files na lixeira.
        spawn_app("lumo-files", &["--trash"]);
        return;
    }
    let slot = &slots[idx];
    let is_running =
        !slot.process.is_empty() && running.get(&slot.process).copied().unwrap_or(false);
    if is_running {
        let app_id = if slot.app_id.is_empty() {
            slot.process.as_str()
        } else {
            slot.app_id.as_str()
        };
        send_focus_app(app_id);
        return;
    }
    let parts: Vec<&str> = slot.exec.split_whitespace().collect();
    if parts.is_empty() {
        return;
    }
    spawn_app(parts[0], &parts[1..]);
}

/// F: envia FocusApp ao compositor via socket unix (fire-and-forget, igual ao
/// cliente do lumo-bridge). Erro = compositor offline -> log e segue.
fn send_focus_app(app_id: &str) {
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    let Some(path) = lumo_ipc::default_socket_path() else {
        eprintln!("[lumo-dock] focus: XDG_RUNTIME_DIR ausente");
        return;
    };
    let cmd = lumo_ipc::LumoCommand::FocusApp {
        app_id: app_id.to_string(),
    };
    let mut line = match serde_json::to_string(&cmd) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[lumo-dock] focus serialize falhou: {e}");
            return;
        }
    };
    line.push('\n');
    match UnixStream::connect(&path) {
        Ok(mut s) => {
            let _ = s.write_all(line.as_bytes());
            let _ = s.flush();
        }
        Err(e) => eprintln!("[lumo-dock] focus IPC connect falhou: {e}"),
    }
}

fn spawn_app(cmd: &str, args: &[&str]) {
    use std::process::Command;
    match Command::new(cmd).args(args).spawn() {
        Ok(child) => eprintln!("[lumo-dock] spawn {} pid={}", cmd, child.id()),
        Err(e) => eprintln!("[lumo-dock] spawn {} falhou: {}", cmd, e),
    }
}
