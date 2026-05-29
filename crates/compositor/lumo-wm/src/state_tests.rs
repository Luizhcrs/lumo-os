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
    let (state, _) = setup();
    let geom = state.usable_geometry();
    // Sem outputs, deve retornar fallback 1920x1080.
    assert_eq!(geom.size.w, 1920);
    assert_eq!(geom.size.h, 1080);
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

// Decoracao: SO GTK4/libadwaita suprime SSD. GTK3 (nocsd) mantem.
#[test]
fn gtk4_libadwaita_apps_detected_csd() {
    use super::state::app_prefers_csd_with;
    let none: Vec<String> = vec![];
    assert!(app_prefers_csd_with("org.gnome.TextEditor", &none));
    assert!(app_prefers_csd_with("org.gnome.Nautilus", &none));
}

#[test]
fn gtk3_and_lumo_apps_keep_ssd() {
    use super::state::app_prefers_csd_with;
    let none: Vec<String> = vec![];
    // GTK3 (mousepad): gtk3-nocsd tira a CSD -> PRECISA do SSD Lumo (senao
    // fica so com um X). Por isso NAO esta na lista de supressao.
    assert!(!app_prefers_csd_with("org.xfce.mousepad", &none));
    assert!(!app_prefers_csd_with("Mousepad", &none));
    // Apps Lumo (Iced) + term + Qt + Chrome (negocia separado) mantem SSD.
    assert!(!app_prefers_csd_with("lumo-calc", &none));
    assert!(!app_prefers_csd_with("foot", &none));
    assert!(!app_prefers_csd_with("org.kde.konsole", &none));
    assert!(!app_prefers_csd_with("", &none));
}

#[test]
fn csd_override_from_config_matches() {
    use super::state::app_prefers_csd_with;
    let extra = vec!["com.example.weirdapp".to_string()];
    assert!(app_prefers_csd_with("com.example.WeirdApp", &extra));
    assert!(!app_prefers_csd_with("com.example.other", &extra));
}

#[test]
fn app_should_have_ssd_inverse() {
    use super::state::app_should_have_ssd;
    // Sem config override (env HOME pode nao ter o arquivo) os defaults valem.
    assert!(!app_should_have_ssd("org.gnome.TextEditor"));
    assert!(app_should_have_ssd("lumo-files"));
}
