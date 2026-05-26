//! W8.B: Workspaces visual real.
//!
//! Gerencia vault de windows por workspace e animacao de slide.
//! LumoState usa WorkspaceVault para hide/show toplevels ao trocar workspace.

use smithay::desktop::Window;
use smithay::utils::{Logical, Point};
use std::collections::HashMap;

/// Posicao cached de uma janela ao ser ocultada.
#[derive(Debug, Clone)]
pub struct WindowEntry {
    pub window: Window,
    pub cached_pos: Point<i32, Logical>,
}

/// Vault de toplevels por workspace (1..=5).
/// Windows no workspace ativo ficam no space; os outros ficam aqui.
#[derive(Default)]
pub struct WorkspaceVault {
    pub vault: HashMap<u8, Vec<WindowEntry>>,
    /// Mapeamento window -> workspace de origem (para assign correto no new_toplevel).
    pub window_workspace: HashMap<u64, u8>,
}

impl WorkspaceVault {
    pub fn new() -> Self {
        Self::default()
    }

    /// Oculta todas as windows do workspace ws (move pro vault).
    pub fn hide_workspace(&mut self, ws: u8, entries: Vec<WindowEntry>) {
        self.vault.insert(ws, entries);
    }

    /// Restaura windows do workspace ws (remove do vault).
    pub fn show_workspace(&mut self, ws: u8) -> Vec<WindowEntry> {
        self.vault.remove(&ws).unwrap_or_default()
    }

    /// Quantas windows estao no vault do workspace ws.
    pub fn count(&self, ws: u8) -> usize {
        self.vault.get(&ws).map(|v| v.len()).unwrap_or(0)
    }

    /// Total de windows no vault (todos workspaces).
    pub fn total_vaulted(&self) -> usize {
        self.vault.values().map(|v| v.len()).sum()
    }

    /// Lista workspaces com windows no vault, ordenado.
    pub fn occupied_workspaces(&self) -> Vec<u8> {
        let mut keys: Vec<u8> = self.vault.keys().copied().collect();
        keys.sort();
        keys
    }
}

/// Estado de animacao de transicao entre workspaces.
#[derive(Debug, Clone)]
pub struct WorkspaceTransition {
    pub from: u8,
    pub to: u8,
    /// Progresso 0.0..=1.0.
    pub progress: f32,
    /// Duracao em segundos. 0 = instant (reduced_motion).
    pub duration: f32,
}

impl WorkspaceTransition {
    /// Cria transicao. duration=0 = instant.
    pub fn new(from: u8, to: u8, duration: f32) -> Self {
        Self {
            from,
            to,
            progress: if duration <= 0.0 { 1.0 } else { 0.0 },
            duration,
        }
    }

    /// Avanca animacao por dt segundos. Retorna true quando completo.
    pub fn tick(&mut self, dt: f32) -> bool {
        if self.duration <= 0.0 {
            self.progress = 1.0;
            return true;
        }
        self.progress = (self.progress + dt / self.duration).min(1.0);
        self.is_done()
    }

    pub fn is_done(&self) -> bool {
        self.progress >= 1.0
    }

    /// Calcula offset de slide em pixels logicos dado largura do output.
    /// Workspace maior = slide da direita; menor = slide da esquerda.
    pub fn slide_offset(&self, output_width: i32) -> i32 {
        let dir: i32 = if self.to > self.from { 1 } else { -1 };
        let t = ease_out_cubic(self.progress);
        let remaining = 1.0 - t;
        (remaining * output_width as f32 * dir as f32) as i32
    }
}

/// Easing ease-out-cubic: t^3.
fn ease_out_cubic(t: f32) -> f32 {
    let t1 = 1.0 - t;
    1.0 - t1 * t1 * t1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_vault_hide_show_roundtrip() {
        let mut v = WorkspaceVault::new();
        assert_eq!(v.count(1), 0);
        v.hide_workspace(1, vec![]);
        let r = v.show_workspace(1);
        assert_eq!(r.len(), 0);
    }

    #[test]
    fn workspace_vault_occupied_sorted() {
        let mut v = WorkspaceVault::new();
        v.hide_workspace(4, vec![]);
        v.hide_workspace(2, vec![]);
        assert_eq!(v.occupied_workspaces(), vec![2, 4]);
    }

    #[test]
    fn workspace_vault_show_missing_returns_empty() {
        let mut v = WorkspaceVault::new();
        assert!(v.show_workspace(99).is_empty());
    }

    #[test]
    fn workspace_transition_instant() {
        let mut tr = WorkspaceTransition::new(1, 2, 0.0);
        assert!(tr.is_done());
        assert!(tr.tick(0.016));
    }

    #[test]
    fn workspace_transition_animated_progress() {
        let mut tr = WorkspaceTransition::new(1, 2, 0.25);
        assert!(!tr.is_done());
        tr.tick(0.1);
        assert!(tr.progress > 0.0 && tr.progress < 1.0);
        tr.tick(0.2);
        assert!(tr.is_done());
    }

    #[test]
    fn workspace_transition_progress_clamps() {
        let mut tr = WorkspaceTransition::new(1, 3, 0.1);
        tr.tick(1.0);
        assert_eq!(tr.progress, 1.0);
    }

    #[test]
    fn workspace_slide_offset_zero_when_done() {
        let mut tr = WorkspaceTransition::new(1, 2, 0.25);
        tr.progress = 1.0;
        assert_eq!(tr.slide_offset(1920), 0);
    }

    #[test]
    fn workspace_slide_offset_direction() {
        let tr_right = WorkspaceTransition::new(1, 2, 0.25);
        let tr_left = WorkspaceTransition::new(2, 1, 0.25);
        // progress=0: offset deveria ser +/- output_width respectivamente.
        let off_r = tr_right.slide_offset(1920);
        let off_l = tr_left.slide_offset(1920);
        assert!(off_r > 0, "transicao pra direita offset positivo");
        assert!(off_l < 0, "transicao pra esquerda offset negativo");
    }

    #[test]
    fn workspace_vault_total_vaulted() {
        let mut v = WorkspaceVault::new();
        v.hide_workspace(1, vec![]);
        v.hide_workspace(2, vec![]);
        assert_eq!(v.total_vaulted(), 0); // ambos com vec vazio
    }

    #[test]
    fn workspace_transition_new_animated_progress_starts_zero() {
        let tr = WorkspaceTransition::new(1, 2, 0.3);
        assert_eq!(tr.progress, 0.0);
    }
}
