//! # lumo-ipc
//!
//! Proposito: Tipos e helpers do canal IPC unix socket do Lumo OS. Protocolo line-delimited JSON.
//!
//! ## Invariantes
//! - Socket criado antes de clientes conectarem; clientes toleram ausencia com retry/standalone — ver I-02.
//! - Todos UnixStream IPC devem ter set_nonblocking(true) imediatamente apos connect/accept — ver I-06.
//! - active_workspace sempre em 1..=MAX_WORKSPACES; set_workspace() e cliente bar clampam — ver I-07.
//!
//! ## Memory refs
//! - [[feedback-design-lapidado]]
//! - [[project-lumo-os]]

use serde::{Deserialize, Serialize};

pub mod shell_app;
pub use shell_app::{ActivationKind, ShellApp, ShellAppEntry, ShellAppRegistry};

pub const SOCKET_BASENAME: &str = "lumo-wm.sock";

pub const MAX_WORKSPACES: u8 = 5;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OsdIcon {
    Keyboard,
    Volume,
    Brightness,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LumoEvent {
    Workspaces {
        active: u8,
        total: u8,
    },
    CloseDropdowns,
    CloseDesktopMenu,
    DesktopOpenSelected,
    ActiveApp {
        app_id: String,
        title: String,
        pid: u32,
    },
    /// W34.11: explicit clear (todas janelas fecharam). Bar reseta pills.
    /// Diferente de ActiveApp{app_id:""} que e transient focus_changed.
    ActiveAppCleared,
    ShowOsd {
        text: String,
        icon: OsdIcon,
        duration_ms: u32,
    },
    ThemeReloaded {
        mode: ThemeMode,
    },
    /// W9.C: novo output conectado ou adicionado em hot-plug.
    /// name = connector name (ex: "eDP-1", "HDMI-A-1").
    /// index = indice do output no compositor (0-based).
    OutputAdded {
        name: String,
        index: u32,
        width: u32,
        height: u32,
    },
    /// W9.C: output removido (hot-unplug ou shutdown).
    OutputRemoved {
        name: String,
        index: u32,
    },
    /// UX2: subsystem entrou em modo degradado. Bar mostra pill warning.
    /// Compositor emite quando feature off (vsync off, color mgmt off, etc).
    DegradedFeature {
        code: String,
        label: String,
    },
    /// UX2: subsystem voltou ao normal. Bar remove pill.
    DegradedFeatureCleared {
        code: String,
    },
    /// UX3: app freeze detectado (sem Pong em 2s). Bar marca cursor wait.
    AppFreeze {
        pid: u32,
        app_id: String,
    },
    /// UX3: app respondeu, freeze cleared.
    AppFreezeCleared {
        pid: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LumoCommand {
    Switch {
        to: u8,
    },
    CloseDropdowns,
    CloseDesktopMenu,
    ReloadTheme,
    CloseFocusedToplevel,
    /// SI.1: input sintetico -- ponteiro absoluto em pixels logicos.
    /// Bridge HTTP usa pra remotar input sem libinput/ydotool.
    SyntheticPointerMove {
        x: f64,
        y: f64,
    },
    /// SI.1: input sintetico -- botao do ponteiro.
    /// `button` = codigo linux/input-event-codes (BTN_LEFT=0x110, BTN_RIGHT=0x111, BTN_MIDDLE=0x112).
    SyntheticPointerButton {
        button: u32,
        pressed: bool,
    },
    /// SI.1: input sintetico -- scroll/axis.
    /// `dx` = eixo horizontal, `dy` = eixo vertical (positivo = down/right).
    SyntheticPointerScroll {
        dx: f64,
        dy: f64,
    },
    /// SI.1: input sintetico -- tecla individual.
    /// `keycode` = evdev KEY_* (mesma tabela do bridge/ydotool).
    /// O compositor traduz pra xkb Keycode internamente (evdev + 8).
    /// Conhecido: nao usa keysym; layout atual aplicado pelo xkb state do compositor.
    SyntheticKey {
        keycode: u32,
        pressed: bool,
    },
    /// SI.1: input sintetico -- atalho. Pressiona `keys` em ordem,
    /// pequena pausa, libera em ordem reversa. Codigos evdev KEY_*.
    SyntheticKeyCombo {
        keys: Vec<u32>,
    },
    /// W17.1: toggle maximize/fullscreen no toplevel com foco.
    /// Sem identifier de surface: usa focused toplevel (mesma logica de
    /// `CloseFocusedToplevel`). Bridge HTTP usa pra remotar a acao.
    ToggleMaximize,
    /// W17.1: minimize/iconify (stub) no toplevel com foco. Sem Wayland
    /// iconify protocol estavel; loga info e nao altera estado.
    MinimizeFocused,
    /// F (auditoria 2026-05): dock pede pra FOCAR/levantar a janela de um app ja
    /// aberto, em vez de dar spawn de uma 2a instancia. `app_id` = xdg app_id OU
    /// nome do binario; o WM faz match case-insensitive contra os toplevels
    /// mapeados e, se a janela estiver minimizada, restaura. No-op se nao houver
    /// janela do app (o dock entao da spawn normal).
    FocusApp {
        app_id: String,
    },
    /// W34.10: lumo-appsd notifica WM que abriu janela com app_id conhecido.
    /// WM faz broadcast LumoEvent::ActiveApp com esses dados pro bar popular pills.
    /// Bypass Iced 0.13 que nao emite xdg_toplevel.set_app_id antes do focus_changed.
    AppActivated {
        app_id: String,
        title: String,
        pid: u32,
    },
    /// W34.11: lumo-appsd notifica WM que fechou todas janelas. Limpar pills bar.
    AppDeactivated,
}

/// F (auditoria 2026-05): predicado de match do FocusApp. O dock manda o nome do
/// binario (ex "lumo-files"); o xdg app_id da janela pode ser igual, reverse-DNS
/// (ex "org.alacritty.Alacritty") ou prefixado. Case-insensitive. Vazio nunca
/// casa. Funcao pura -> testavel sem Wayland.
///
/// Casamento (apertado de proposito pra NAO dar falso-positivo em slots custom,
/// ex "qt" nao deve casar "qt-creator"):
///   1. exato (cobre apps Lumo: app_id == binario; e chromium/Alacritty ci);
///   2. ultimo segmento '.'-separado do app_id == want (reverse-DNS, ex
///      "org.alacritty.Alacritty" ~ "alacritty").
pub fn app_id_matches(window_app_id: &str, want: &str) -> bool {
    let a = window_app_id.trim().to_ascii_lowercase();
    let w = want.trim().to_ascii_lowercase();
    if a.is_empty() || w.is_empty() {
        return false;
    }
    a == w || a.rsplit('.').next() == Some(w.as_str())
}

pub fn default_socket_path() -> Option<std::path::PathBuf> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")?;
    let mut p = std::path::PathBuf::from(dir);
    p.push(SOCKET_BASENAME);
    Some(p)
}

pub fn encode_event(ev: &LumoEvent) -> String {
    // IPC-FRAME-001: serializacao so falha se LumoEvent for malformado
    // (variant nao-serializavel), o que e bug nosso. Em prod, evento e
    // descartado e log warn em vez de panic.
    match serde_json::to_string(ev) {
        Ok(mut s) => {
            s.push('\n');
            s
        }
        Err(e) => {
            tracing::error!(code = "IPC-FRAME-001", err = %e, ?ev, "encode_event falhou; dropando evento");
            String::new()
        }
    }
}

/// Variant fallible de encode_event. Use quando caller quer detectar erro.
pub fn try_encode_event(ev: &LumoEvent) -> Result<String, serde_json::Error> {
    let mut s = serde_json::to_string(ev)?;
    s.push('\n');
    Ok(s)
}

pub fn parse_command(line: &str) -> Option<Result<LumoCommand, serde_json::Error>> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(serde_json::from_str(trimmed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_roundtrip() {
        let ev = LumoEvent::Workspaces {
            active: 3,
            total: 5,
        };
        let line = encode_event(&ev);
        assert!(line.ends_with('\n'));
        let parsed: LumoEvent = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(parsed, ev);
    }

    #[test]
    fn switch_command_parses() {
        let raw = r#"{"type":"switch","to":2}"#;
        let cmd = parse_command(raw).unwrap().unwrap();
        assert_eq!(cmd, LumoCommand::Switch { to: 2 });
    }

    #[test]
    fn blank_line_is_skipped() {
        assert!(parse_command("").is_none());
        assert!(parse_command("   \n").is_none());
    }

    #[test]
    fn theme_reloaded_roundtrip() {
        let ev = LumoEvent::ThemeReloaded {
            mode: ThemeMode::Dark,
        };
        let line = encode_event(&ev);
        let parsed: LumoEvent = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(parsed, ev);
    }

    #[test]
    fn reload_theme_command_roundtrip() {
        let cmd = LumoCommand::ReloadTheme;
        let json = serde_json::to_string(&cmd).unwrap();
        let parsed: LumoCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, cmd);
    }

    /// W9.C: OutputAdded serializa e desserializa corretamente.
    #[test]
    fn output_added_roundtrip() {
        let ev = LumoEvent::OutputAdded {
            name: "HDMI-A-1".to_string(),
            index: 1,
            width: 1920,
            height: 1080,
        };
        let line = encode_event(&ev);
        let parsed: LumoEvent = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(parsed, ev);
    }

    /// W9.C: OutputRemoved serializa e desserializa corretamente.
    #[test]
    fn output_removed_roundtrip() {
        let ev = LumoEvent::OutputRemoved {
            name: "HDMI-A-1".to_string(),
            index: 1,
        };
        let line = encode_event(&ev);
        let parsed: LumoEvent = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(parsed, ev);
    }

    /// SI.1: SyntheticPointerMove serializa/desserializa e o campo `type`
    /// vai em snake_case (`synthetic_pointer_move`).
    #[test]
    fn synthetic_pointer_move_roundtrip() {
        let cmd = LumoCommand::SyntheticPointerMove {
            x: 960.5,
            y: 540.25,
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("synthetic_pointer_move"), "json={json}");
        let parsed: LumoCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, cmd);
    }

    /// SI.1: SyntheticPointerButton, SyntheticKey, SyntheticKeyCombo
    /// fazem roundtrip JSON line-delimited.
    #[test]
    fn synthetic_input_variants_roundtrip() {
        // button
        let raw = r#"{"type":"synthetic_pointer_button","button":272,"pressed":true}"#;
        let cmd = parse_command(raw).unwrap().unwrap();
        assert_eq!(
            cmd,
            LumoCommand::SyntheticPointerButton {
                button: 272,
                pressed: true
            }
        );

        // scroll
        let scroll = LumoCommand::SyntheticPointerScroll { dx: 0.0, dy: 15.0 };
        let s = serde_json::to_string(&scroll).unwrap();
        let back: LumoCommand = serde_json::from_str(&s).unwrap();
        assert_eq!(back, scroll);

        // single key
        let key = LumoCommand::SyntheticKey {
            keycode: 28,
            pressed: false,
        };
        let back: LumoCommand =
            serde_json::from_str(&serde_json::to_string(&key).unwrap()).unwrap();
        assert_eq!(back, key);

        // combo
        let combo = LumoCommand::SyntheticKeyCombo {
            keys: vec![29, 56, 20],
        };
        let back: LumoCommand =
            serde_json::from_str(&serde_json::to_string(&combo).unwrap()).unwrap();
        assert_eq!(back, combo);
    }

    #[test]
    fn output_added_type_field() {
        let ev = LumoEvent::OutputAdded {
            name: "eDP-1".to_string(),
            index: 0,
            width: 2560,
            height: 1600,
        };
        let json = serde_json::to_string(&ev).unwrap();
        assert!(json.contains("output_added"), "json={json}");
    }

    // --- SEGURANCA E ROBUSTEZ ---

    #[test]
    fn malicious_invalid_json_fails_gracefully() {
        let bad_json = r#"{"type":"switch", "to": "MWAHAHA"}"#; // "to" deve ser numero
        let result = parse_command(bad_json).unwrap();
        assert!(result.is_err());
    }

    #[test]
    fn malicious_unknown_command_ignored() {
        let unknown = r#"{"type":"execute_format_c", "confirm": true}"#;
        let result = parse_command(unknown).unwrap();
        assert!(result.is_err()); // Serde falha ao nao encontrar variante no enum
    }

    #[test]
    fn robustness_large_payload_ignored() {
        let mut large = String::from(r#"{"type":"switch", "to":1, "junk":""#);
        for _ in 0..1000 { large.push('A'); }
        large.push_str(r#""}"#);
        // JSON com campo extra eh aceito pelo Serde se nao configurado deny_unknown_fields,
        // mas aqui testamos que ele pelo menos nao causa pânico.
        let result = parse_command(&large).unwrap();
        assert!(result.is_ok());
    }

    #[test]
    fn try_encode_event_returns_ok_for_valid() {
        let ev = LumoEvent::Workspaces { active: 1, total: 5 };
        let out = try_encode_event(&ev).expect("encode ok");
        assert!(out.ends_with('\n'));
        assert!(out.contains("workspaces") || out.contains("Workspaces"));
    }

    #[test]
    fn encode_event_always_returns_string_or_empty() {
        // encode_event nunca panica mesmo em payload normal.
        let ev = LumoEvent::Workspaces { active: 0, total: 0 };
        let s = encode_event(&ev);
        assert!(!s.is_empty());
        assert!(s.ends_with('\n'));
    }

    #[test]
    fn degraded_feature_event_roundtrip() {
        let ev = LumoEvent::DegradedFeature {
            code: "WM-RENDER-002".into(),
            label: "Vsync off".into(),
        };
        let s = try_encode_event(&ev).unwrap();
        let line = s.trim();
        let back: LumoEvent = serde_json::from_str(line).unwrap();
        assert!(matches!(back, LumoEvent::DegradedFeature { .. }));
    }

    /// F (auditoria 2026-05): FocusApp faz roundtrip JSON line-delimited e o
    /// campo `type` vai em snake_case (`focus_app`).
    #[test]
    fn focus_app_command_roundtrip() {
        let cmd = LumoCommand::FocusApp {
            app_id: "lumo-files".into(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("focus_app"), "json={json}");
        let parsed = parse_command(&json).unwrap().unwrap();
        assert_eq!(parsed, cmd);
    }

    /// F: app_id_matches -- exato, reverse-DNS (ultimo segmento), prefixo-com-
    /// separador; rejeita vazio e NAO da falso-positivo por substring solta.
    #[test]
    fn app_id_matches_cases() {
        // 1. exato (case-insensitive) -- caso comum dos apps Lumo.
        assert!(app_id_matches("Alacritty", "alacritty"));
        assert!(app_id_matches("lumo-files", "lumo-files"));
        assert!(app_id_matches("chromium", "chromium"));
        // 2. reverse-DNS: ultimo segmento == want.
        assert!(app_id_matches("org.alacritty.Alacritty", "alacritty"));
        assert!(app_id_matches("org.chromium.Chromium", "chromium"));
        // NAO casa: substring solta (regressao do matcher antigo bidirecional).
        assert!(!app_id_matches("qt-creator", "qt"));
        assert!(!app_id_matches("qtcreator", "qt"));
        assert!(!app_id_matches("lumo-files", "lumo-calc"));
        assert!(!app_id_matches("chromium-browser", "chromium")); // sem prefixo-loose
        // vazio nunca casa.
        assert!(!app_id_matches("", "lumo-files"));
        assert!(!app_id_matches("lumo-files", ""));
        assert!(!app_id_matches("  ", "lumo-files"));
    }

    #[test]
    fn app_freeze_event_roundtrip() {
        let ev = LumoEvent::AppFreeze {
            pid: 42,
            app_id: "lumo-files".into(),
        };
        let s = try_encode_event(&ev).unwrap();
        let line = s.trim();
        let back: LumoEvent = serde_json::from_str(line).unwrap();
        assert!(matches!(back, LumoEvent::AppFreeze { pid: 42, .. }));
    }
}
