//! Integration test cross-crate pra ShellAppRegistry.
//!
//! Verifica que o registry default tem todos apps shell esperados pelos
//! KeyAction handlers em lumo-wm, e que serde roundtrip preserva semantica
//! pra leitura de config user (`~/.config/lumo/shell-apps.toml`).

use lumo_ipc::{ActivationKind, ShellApp, ShellAppRegistry};

#[test]
fn default_registry_resolves_clipboard_to_lumo_clip() {
    let r = ShellAppRegistry::default();
    match r.lookup(ShellApp::Clipboard) {
        Some(ActivationKind::Spawn { command }) => assert_eq!(command, "lumo-clip"),
        other => panic!("clipboard != Spawn lumo-clip: {:?}", other),
    }
}

#[test]
fn default_registry_resolves_launcher_to_lumo_launcher() {
    let r = ShellAppRegistry::default();
    match r.lookup(ShellApp::Launcher) {
        Some(ActivationKind::Spawn { command }) => assert_eq!(command, "lumo-launcher"),
        other => panic!("launcher != Spawn lumo-launcher: {:?}", other),
    }
}

#[test]
fn override_then_serde_preserves_override() {
    let mut r = ShellAppRegistry::default();
    r.set(
        ShellApp::Launcher,
        ActivationKind::Spawn {
            command: "rofi".into(),
        },
    );
    let toml_str = serde_json::to_string(&r).expect("serialize");
    let restored: ShellAppRegistry = serde_json::from_str(&toml_str).expect("deserialize");
    match restored.lookup(ShellApp::Launcher) {
        Some(ActivationKind::Spawn { command }) => assert_eq!(command, "rofi"),
        _ => panic!("override perdido"),
    }
}

#[test]
fn signal_activation_roundtrips() {
    let mut r = ShellAppRegistry::default();
    r.set(
        ShellApp::Clipboard,
        ActivationKind::Signal {
            pidfile: "/run/lumo/clip.pid".into(),
            signal: 10,
        },
    );
    let s = serde_json::to_string(&r).unwrap();
    let r2: ShellAppRegistry = serde_json::from_str(&s).unwrap();
    match r2.lookup(ShellApp::Clipboard) {
        Some(ActivationKind::Signal { pidfile, signal }) => {
            assert_eq!(pidfile, "/run/lumo/clip.pid");
            assert_eq!(*signal, 10);
        }
        _ => panic!("signal activation perdido"),
    }
}

#[test]
fn all_default_apps_have_spawn_command() {
    let r = ShellAppRegistry::default();
    for app in [
        ShellApp::Launcher,
        ShellApp::Clipboard,
        ShellApp::Settings,
        ShellApp::Files,
        ShellApp::Terminal,
        ShellApp::Lock,
    ] {
        match r.lookup(app) {
            Some(ActivationKind::Spawn { command }) => {
                assert!(!command.is_empty(), "{:?} command vazio", app);
            }
            other => panic!("{:?} sem Spawn default: {:?}", app, other),
        }
    }
}

#[test]
fn dbus_activation_roundtrips() {
    let mut r = ShellAppRegistry::default();
    r.set(
        ShellApp::Settings,
        ActivationKind::DBus {
            bus_name: "org.lumo.Settings".into(),
            object_path: "/org/lumo/Settings".into(),
            interface: "org.lumo.Settings1".into(),
            method: "Open".into(),
        },
    );
    let s = serde_json::to_string(&r).unwrap();
    let r2: ShellAppRegistry = serde_json::from_str(&s).unwrap();
    assert!(matches!(
        r2.lookup(ShellApp::Settings),
        Some(ActivationKind::DBus { .. })
    ));
}
