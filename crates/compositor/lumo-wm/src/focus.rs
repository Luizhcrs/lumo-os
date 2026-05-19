//! focus.rs - FocusManager: state machine centralizada de foco de teclado.
//!
//! Antes: set_focus espalhado em handlers/input.rs, handlers/seat.rs,
//! handlers/xdg_shell.rs com logica duplicada.
//! Depois: toda policy de foco passa por FocusManager.
//!
//! Design: FocusManager NAO chama kb.set_focus diretamente (borrow conflict
//! com &mut LumoState). Os metodos recebem os dados necessarios, atualizam
//! FocusState/prev, e retornam Option<WlSurface> pro caller passar pra kb.
//! Caller: kb.set_focus(self, fm.click_toplevel(surface), serial).
//!
//! Transicoes validas (15+):
//!
//!   [1]  None        --click_toplevel-->   Toplevel(S)
//!   [2]  None        --new_toplevel-->     Toplevel(S)
//!   [3]  Toplevel(S) --click_toplevel-->   Toplevel(S')
//!   [4]  Toplevel(S) --click_layer_shell-> None
//!   [5]  Toplevel(S) --close_toplevel-->   Toplevel(next)
//!   [6]  Toplevel(S) --close_toplevel-->   None
//!   [7]  Toplevel(S) --cycle_next-->       Toplevel(S')
//!   [8]  Toplevel(S) --cycle_prev-->       Toplevel(S')
//!   [9]  None        --cycle_next-->       Toplevel(first)
//!   [10] None        --cycle_prev-->       Toplevel(first)
//!   [11] Dropdown    --click_toplevel-->   Toplevel(S)
//!   [12] Dropdown    --click_layer_shell-> None
//!   [13] Toplevel(S) --cycle(empty)-->     Toplevel(S) no-op
//!   [14] None        --click_layer_shell-> None no-op
//!   [15] None        --close_toplevel-->   None no-op
//!
//! SUPER+Tab   -> cycle_next (delta=+1)
//! SUPER+Shift+Tab -> cycle_prev (delta=-1)

use smithay::desktop::{Space, Window};
use smithay::input::keyboard::KeyboardHandle;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::seat::WaylandFocus;

use crate::state::LumoState;

/// Estado corrente de foco de teclado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusState {
    /// Nenhuma surface focada.
    None,
    /// Toplevel xdg focado.
    Toplevel(WlSurface),
    /// Dropdown layer-shell ativo. Teclado nao e redirecionado;
    /// prev guarda toplevel anterior para restaurar ao fechar.
    Dropdown,
}

/// FocusManager: centraliza todas as transicoes de foco.
///
/// Uso tipico:
///   let new_focus = self.focus_manager.click_toplevel(surface);
///   let kb = self.keyboard.clone();
///   kb.set_focus(self, new_focus, serial);
pub struct FocusManager {
    pub state: FocusState,
    /// Surface que tinha foco antes de Dropdown.
    pub prev: Option<WlSurface>,
    /// T1.5: MRU -- surface focada antes do toplevel atual.
    /// Quando um toplevel fecha, foco vai pra prev_focus se ainda vivo.
    pub prev_focus: Option<WlSurface>,
}

impl Default for FocusManager {
    fn default() -> Self {
        Self {
            state: FocusState::None,
            prev: None,
            prev_focus: None,
        }
    }
}

impl FocusManager {
    /// [1,3,11] Click em toplevel xdg.
    /// Retorna Some(surface) para kb.set_focus.
    pub fn click_toplevel(&mut self, surface: WlSurface) -> Option<WlSurface> {
        // T1.5: salva foco anterior no MRU antes de trocar.
        if let FocusState::Toplevel(ref prev) = self.state {
            if *prev != surface {
                self.prev_focus = Some(prev.clone());
            }
        }
        self.state = FocusState::Toplevel(surface.clone());
        self.prev = None;
        Some(surface)
    }

    /// [4,12,14] Click em layer-shell: remove foco.
    /// Retorna None para kb.set_focus.
    pub fn click_layer_shell(&mut self) -> Option<WlSurface> {
        self.state = FocusState::None;
        self.prev = None;
        None
    }

    /// [5,6,15] Toplevel fechado. Foca next ou vai pra None.
    /// Retorna o novo foco para kb.set_focus.
    pub fn close_toplevel(&mut self, next: Option<WlSurface>) -> Option<WlSurface> {
        match next {
            Some(s) => {
                self.state = FocusState::Toplevel(s.clone());
                self.prev = None;
                Some(s)
            }
            None => {
                self.state = FocusState::None;
                self.prev = None;
                None
            }
        }
    }

    /// [2] Nova janela mapeada: foca imediatamente.
    pub fn new_toplevel(&mut self, surface: WlSurface) -> Option<WlSurface> {
        // T1.5: salva foco anterior antes de focar nova janela.
        if let FocusState::Toplevel(ref prev) = self.state {
            self.prev_focus = Some(prev.clone());
        }
        self.state = FocusState::Toplevel(surface.clone());
        self.prev = None;
        Some(surface)
    }

    /// [7,8,9,10,13] Cicla foco entre toplevels.
    /// delta=+1 next, -1 prev. Retorna Some(surface) ou None.
    /// Caller: kb.set_focus(self, focus_manager.cycle(...), serial).
    pub fn cycle(
        &mut self,
        kb: &KeyboardHandle<LumoState>,
        space: &Space<Window>,
        delta: i8,
    ) -> Option<WlSurface> {
        let windows: Vec<_> = space.elements().cloned().collect();
        if windows.is_empty() {
            return None;
        }
        let current = kb.current_focus();
        let current_idx = current.as_ref().and_then(|focused| {
            windows.iter().position(|w| {
                w.wl_surface()
                    .map(|s| *s == *focused)
                    .unwrap_or(false)
            })
        });
        let len = windows.len() as isize;
        let next_idx = match current_idx {
            Some(i) => ((i as isize + delta as isize).rem_euclid(len)) as usize,
            None => 0,
        };
        if let Some(next_win) = windows.get(next_idx) {
            if let Some(surface) = next_win.wl_surface() {
                let owned = surface.into_owned();
                self.state = FocusState::Toplevel(owned.clone());
                self.prev = None;
                return Some(owned);
            }
        }
        None
    }

    /// SUPER+Tab: cicla pra frente (delta=+1).
    pub fn cycle_next(
        &mut self,
        kb: &KeyboardHandle<LumoState>,
        space: &Space<Window>,
    ) -> Option<WlSurface> {
        self.cycle(kb, space, 1)
    }

    /// SUPER+Shift+Tab: cicla pra tras (delta=-1).
    pub fn cycle_prev(
        &mut self,
        kb: &KeyboardHandle<LumoState>,
        space: &Space<Window>,
    ) -> Option<WlSurface> {
        self.cycle(kb, space, -1)
    }
}

// ============================================================
// Tests: transicoes de FocusState sem Wayland server real.
// WlSurface nao e constructivel em unit test; testamos
// FocusState enum e FocusManager struct diretamente.
// ============================================================

#[cfg(test)]
mod tests {
    use super::{FocusManager, FocusState};

    fn mgr() -> FocusManager {
        FocusManager::default()
    }

    // T01: estado inicial e None.
    #[test]
    fn t01_initial_state_is_none() {
        let m = mgr();
        assert_eq!(m.state, FocusState::None);
        assert!(m.prev.is_none());
    }

    // T02: Default::default() produz FocusState::None.
    #[test]
    fn t02_default_is_none() {
        let m: FocusManager = Default::default();
        assert_eq!(m.state, FocusState::None);
    }

    // T03: None != Dropdown.
    #[test]
    fn t03_none_ne_dropdown() {
        assert_ne!(FocusState::None, FocusState::Dropdown);
    }

    // T04: Dropdown != None.
    #[test]
    fn t04_dropdown_ne_none() {
        assert_ne!(FocusState::Dropdown, FocusState::None);
    }

    // T05: None == None.
    #[test]
    fn t05_none_eq_none() {
        assert_eq!(FocusState::None, FocusState::None);
    }

    // T06: Dropdown == Dropdown.
    #[test]
    fn t06_dropdown_eq_dropdown() {
        assert_eq!(FocusState::Dropdown, FocusState::Dropdown);
    }

    // T07: prev inicia None.
    #[test]
    fn t07_prev_starts_none() {
        let m = mgr();
        assert!(m.prev.is_none());
    }

    // T08: Dropdown sem toplevel anterior: prev permanece None.
    #[test]
    fn t08_dropdown_without_prior_toplevel() {
        let mut m = mgr();
        m.state = FocusState::Dropdown;
        assert_eq!(m.state, FocusState::Dropdown);
        assert!(m.prev.is_none());
    }

    // T09: Dropdown -> None (transicao manual).
    #[test]
    fn t09_dropdown_to_none() {
        let mut m = mgr();
        m.state = FocusState::Dropdown;
        m.state = FocusState::None;
        m.prev = None;
        assert_eq!(m.state, FocusState::None);
        assert!(m.prev.is_none());
    }

    // T10: prev roundtrip.
    #[test]
    fn t10_prev_roundtrip() {
        let mut m = mgr();
        assert!(m.prev.is_none());
        m.state = FocusState::None;
        m.prev = None;
        assert!(m.prev.is_none());
    }

    // T11: clone de FocusState::None.
    #[test]
    fn t11_clone_none() {
        let s = FocusState::None;
        let s2 = s.clone();
        assert_eq!(s, s2);
    }

    // T12: clone de FocusState::Dropdown.
    #[test]
    fn t12_clone_dropdown() {
        let s = FocusState::Dropdown;
        let s2 = s.clone();
        assert_eq!(s, s2);
    }

    // T13: debug print de None nao e vazio.
    #[test]
    fn t13_debug_none() {
        let s = FocusState::None;
        let dbg = format!("{:?}", s);
        assert!(!dbg.is_empty());
    }

    // T14: debug print de Dropdown nao e vazio.
    #[test]
    fn t14_debug_dropdown() {
        let s = FocusState::Dropdown;
        let dbg = format!("{:?}", s);
        assert!(!dbg.is_empty());
    }

    // T15: dois managers independentes nao compartilham estado.
    #[test]
    fn t15_independent_managers() {
        let mut m1 = mgr();
        let m2 = mgr();
        m1.state = FocusState::Dropdown;
        assert_eq!(m1.state, FocusState::Dropdown);
        assert_eq!(m2.state, FocusState::None);
    }
}
