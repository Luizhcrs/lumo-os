//! shell_app.rs — A2 review: registry de apps shell que o compositor pode
//! invocar (lumo-launcher, lumo-clip, lumo-settings, etc).
//!
//! Substitui hardcoded `Spawn("lumo-clip")` em handlers/input.rs por
//! `KeyAction::InvokeApp(ShellApp::Clipboard)`. WM resolve via tabela
//! carregada de `~/.config/lumo/shell-apps.toml` (override) com defaults
//! compilados in.

use serde::{Deserialize, Serialize};

/// Apps shell built-in que o compositor invoca.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShellApp {
    /// Spotlight launcher (Super+Space).
    Launcher,
    /// Clipboard history picker (Super+Shift+V).
    Clipboard,
    /// Settings panel.
    Settings,
    /// File manager.
    Files,
    /// Terminal emulator default.
    Terminal,
    /// Lock screen.
    Lock,
}

/// Como ativar um ShellApp.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ActivationKind {
    /// Spawn binario novo a cada invocacao.
    Spawn { command: String },
    /// Enviar signal (SIGUSR1) pra daemon ja rodando.
    Signal { pidfile: String, signal: u8 },
    /// Chamada DBus.
    DBus {
        bus_name: String,
        object_path: String,
        interface: String,
        method: String,
    },
}

/// Entry no registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellAppEntry {
    pub app: ShellApp,
    pub activation: ActivationKind,
}

/// Registry. Iniciado com defaults; user pode override via TOML.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellAppRegistry {
    pub entries: Vec<ShellAppEntry>,
}

impl Default for ShellAppRegistry {
    fn default() -> Self {
        use ActivationKind::Spawn;
        Self {
            entries: vec![
                ShellAppEntry {
                    app: ShellApp::Launcher,
                    activation: Spawn {
                        command: "lumo-launcher".into(),
                    },
                },
                ShellAppEntry {
                    app: ShellApp::Clipboard,
                    activation: Spawn {
                        command: "lumo-clip".into(),
                    },
                },
                ShellAppEntry {
                    app: ShellApp::Settings,
                    activation: Spawn {
                        command: "lumo-settings".into(),
                    },
                },
                ShellAppEntry {
                    app: ShellApp::Files,
                    activation: Spawn {
                        command: "lumo-files".into(),
                    },
                },
                ShellAppEntry {
                    app: ShellApp::Terminal,
                    activation: Spawn {
                        command: "foot".into(),
                    },
                },
                ShellAppEntry {
                    app: ShellApp::Lock,
                    activation: Spawn {
                        command: "lumo-lock".into(),
                    },
                },
            ],
        }
    }
}

impl ShellAppRegistry {
    pub fn lookup(&self, app: ShellApp) -> Option<&ActivationKind> {
        self.entries
            .iter()
            .find(|e| e.app == app)
            .map(|e| &e.activation)
    }

    /// Override de um app preservando os demais defaults.
    pub fn set(&mut self, app: ShellApp, activation: ActivationKind) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.app == app) {
            e.activation = activation;
            return;
        }
        self.entries.push(ShellAppEntry { app, activation });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_all_builtin_apps() {
        let r = ShellAppRegistry::default();
        for app in [
            ShellApp::Launcher,
            ShellApp::Clipboard,
            ShellApp::Settings,
            ShellApp::Files,
            ShellApp::Terminal,
            ShellApp::Lock,
        ] {
            assert!(r.lookup(app).is_some(), "missing {:?}", app);
        }
    }

    #[test]
    fn default_clipboard_is_lumo_clip() {
        let r = ShellAppRegistry::default();
        match r.lookup(ShellApp::Clipboard) {
            Some(ActivationKind::Spawn { command }) => assert_eq!(command, "lumo-clip"),
            _ => panic!("clipboard deve ser Spawn lumo-clip"),
        }
    }

    #[test]
    fn set_overrides_existing() {
        let mut r = ShellAppRegistry::default();
        r.set(
            ShellApp::Launcher,
            ActivationKind::Spawn {
                command: "rofi".into(),
            },
        );
        match r.lookup(ShellApp::Launcher) {
            Some(ActivationKind::Spawn { command }) => assert_eq!(command, "rofi"),
            _ => panic!(),
        }
    }

    #[test]
    fn set_preserves_others() {
        let mut r = ShellAppRegistry::default();
        r.set(
            ShellApp::Launcher,
            ActivationKind::Spawn {
                command: "rofi".into(),
            },
        );
        assert!(r.lookup(ShellApp::Clipboard).is_some());
    }

    #[test]
    fn serde_roundtrip() {
        let r = ShellAppRegistry::default();
        let s = serde_json::to_string(&r).unwrap();
        let r2: ShellAppRegistry = serde_json::from_str(&s).unwrap();
        assert_eq!(r, r2);
    }

    #[test]
    fn signal_activation_serde() {
        let act = ActivationKind::Signal {
            pidfile: "/run/lumo/foo.pid".into(),
            signal: 10, // SIGUSR1
        };
        let s = serde_json::to_string(&act).unwrap();
        let a2: ActivationKind = serde_json::from_str(&s).unwrap();
        assert_eq!(act, a2);
    }

    #[test]
    fn dbus_activation_serde() {
        let act = ActivationKind::DBus {
            bus_name: "org.lumo.Clip".into(),
            object_path: "/org/lumo/Clip".into(),
            interface: "org.lumo.Clip1".into(),
            method: "Open".into(),
        };
        let s = serde_json::to_string(&act).unwrap();
        let a2: ActivationKind = serde_json::from_str(&s).unwrap();
        assert_eq!(act, a2);
    }

    #[test]
    fn lookup_nonexistent_after_clear() {
        let r = ShellAppRegistry { entries: vec![] };
        assert!(r.lookup(ShellApp::Launcher).is_none());
    }
}
