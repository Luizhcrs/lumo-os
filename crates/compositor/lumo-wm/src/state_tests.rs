//! state_tests.rs — unit tests for LumoState logic.

use super::state::LumoState;
use smithay::reexports::wayland_server::Display;
use smithay::reexports::calloop::EventLoop;

fn setup() -> (LumoState, EventLoop<'static, LumoState>) {
    let display = Display::<LumoState>::new().expect("failed to create display");
    let event_loop = EventLoop::<LumoState>::try_new().expect("failed to create event loop");
    let state = LumoState::new(display.handle(), event_loop.handle(), None);
    (state, event_loop)
}

#[test]
fn test_state_initialization() {
    let (state, _) = setup();
    assert!(state.running);
    assert_eq!(state.active_workspace, 1);
    assert!(state.should_render);
}

#[test]
fn test_workspace_switch() {
    let (mut state, _) = setup();
    state.set_workspace(2);
    assert_eq!(state.active_workspace, 2);
    
    // Switch de volta
    state.set_workspace(1);
    assert_eq!(state.active_workspace, 1);
}

#[test]
fn test_workspace_invalid_switch_ignored() {
    let (mut state, _) = setup();
    state.set_workspace(99);
    assert_eq!(state.active_workspace, 1);
}

#[test]
fn test_next_tile_position_is_deterministic() {
    let (state, _) = setup();
    let p1 = state.next_tile_position();
    let p2 = state.next_tile_position();
    // Como nao mudamos o n_open (space.elements().count()), 
    // a posicao deve ser a mesma.
    assert_eq!(p1, p2);
}

#[test]
fn test_usable_geometry_fallback() {
    use crate::backend::render_common::{CARD_GAP, CARD_MARGIN};
    let (state, _) = setup();
    let geom = state.usable_geometry();
    // Sem outputs, fallback 1920x1080 RECUADO pelo card (margem lados/baixo
    // + gap topo). Janela do card vive recuada da borda.
    assert_eq!(geom.size.w, 1920 - 2 * CARD_MARGIN);
    assert_eq!(geom.size.h, 1080 - CARD_GAP - CARD_MARGIN);
    assert_eq!(geom.loc.x, CARD_MARGIN);
    assert_eq!(geom.loc.y, CARD_GAP);
}

#[test]
fn test_handle_ipc_command_switch() {
    let (mut state, _) = setup();
    use lumo_ipc::LumoCommand;
    state.handle_ipc_command(LumoCommand::Switch { to: 3 });
    assert_eq!(state.active_workspace, 3);
}

#[test]
fn test_tick_splash_cycle() {
    let (mut state, _) = setup();
    state.splash_phase = 0;
    state.splash_alpha = 0.0;
    
    // Fade in (rate 5.0/s) -> 0.1s deve dar 0.5
    super::state::tick_splash(&mut state, 0.1);
    assert!(state.splash_alpha > 0.4 && state.splash_alpha < 0.6);
    
    // Mais 0.2s -> deve completar fade in (phase 1)
    super::state::tick_splash(&mut state, 0.2);
    assert_eq!(state.splash_phase, 1);
    assert_eq!(state.splash_alpha, 1.0);
}

// Decoracao (regra Luiz): SSD so pra apps SEM decoracao propria (Lumo+term).
#[test]
fn lumo_and_terminals_get_ssd() {
    use super::state::app_should_have_ssd_with;
    let none: Vec<String> = vec![];
    assert!(app_should_have_ssd_with("lumo-calc", &none));
    assert!(app_should_have_ssd_with("lumo-files", &none));
    assert!(app_should_have_ssd_with("org.lumo.Editor", &none));
    assert!(app_should_have_ssd_with("foot", &none));
    assert!(app_should_have_ssd_with("alacritty", &none));
    assert!(app_should_have_ssd_with("kitty", &none));
}

#[test]
fn apps_with_own_decoration_no_ssd() {
    use super::state::app_should_have_ssd_with;
    let none: Vec<String> = vec![];
    // GTK3/GTK4/Qt/Chrome/Electron desenham a propria -> sem SSD Lumo.
    assert!(!app_should_have_ssd_with("org.xfce.mousepad", &none));
    assert!(!app_should_have_ssd_with("org.gnome.TextEditor", &none));
    assert!(!app_should_have_ssd_with("org.kde.konsole", &none));
    assert!(!app_should_have_ssd_with("chromium", &none));
    assert!(!app_should_have_ssd_with("code", &none));
}

#[test]
fn ssd_override_from_config_adds() {
    use super::state::app_should_have_ssd_with;
    let extra = vec!["com.example.bareapp".to_string()];
    assert!(app_should_have_ssd_with("com.example.BareApp", &extra));
    assert!(!app_should_have_ssd_with("com.example.other", &extra));
}
