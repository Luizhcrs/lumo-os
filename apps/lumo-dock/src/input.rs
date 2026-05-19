//! input.rs -- hit-test e spawn de apps.

use crate::config::SlotConfig;

pub fn hit_test_slot(px: f32, slot_rects: &[(f32, f32)], trash_rect: Option<(f32, f32)>) -> i32 {
    for (i, &(x, w)) in slot_rects.iter().enumerate() {
        if px >= x && px < x + w { return i as i32; }
    }
    if let Some((x, w)) = trash_rect {
        if px >= x && px < x + w { return slot_rects.len() as i32; }
    }
    -1
}

pub fn handle_click(hover_idx: i32, slots: &[SlotConfig]) {
    if hover_idx < 0 { return; }
    let idx = hover_idx as usize;
    if idx >= slots.len() {
        spawn_app("lumo-files", &["--trash"]);
        return;
    }
    let slot = &slots[idx];
    let parts: Vec<&str> = slot.exec.split_whitespace().collect();
    if parts.is_empty() { return; }
    spawn_app(parts[0], &parts[1..]);
}

fn spawn_app(cmd: &str, args: &[&str]) {
    use std::process::Command;
    match Command::new(cmd).args(args).spawn() {
        Ok(child) => eprintln!("[lumo-dock] spawn {} pid={}", cmd, child.id()),
        Err(e) => eprintln!("[lumo-dock] spawn {} falhou: {}", cmd, e),
    }
}
