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
    Workspaces { active: u8, total: u8 },
    CloseDropdowns,
    CloseDesktopMenu,
    DesktopOpenSelected,
    ActiveApp { app_id: String, title: String, pid: u32 },
    ShowOsd { text: String, icon: OsdIcon, duration_ms: u32 },
    ThemeReloaded { mode: ThemeMode },
    /// W9.C: novo output conectado ou adicionado em hot-plug.
    /// name = connector name (ex: "eDP-1", "HDMI-A-1").
    /// index = indice do output no compositor (0-based).
    OutputAdded { name: String, index: u32, width: u32, height: u32 },
    /// W9.C: output removido (hot-unplug ou shutdown).
    OutputRemoved { name: String, index: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LumoCommand {
    Switch { to: u8 },
    CloseDropdowns,
    CloseDesktopMenu,
    ReloadTheme,
    CloseFocusedToplevel,
    /// SI.1: input sintetico -- ponteiro absoluto em pixels logicos.
    /// Bridge HTTP usa pra remotar input sem libinput/ydotool.
    SyntheticPointerMove { x: f64, y: f64 },
    /// SI.1: input sintetico -- botao do ponteiro.
    /// `button` = codigo linux/input-event-codes (BTN_LEFT=0x110, BTN_RIGHT=0x111, BTN_MIDDLE=0x112).
    SyntheticPointerButton { button: u32, pressed: bool },
    /// SI.1: input sintetico -- scroll/axis.
    /// `dx` = eixo horizontal, `dy` = eixo vertical (positivo = down/right).
    SyntheticPointerScroll { dx: f64, dy: f64 },
    /// SI.1: input sintetico -- tecla individual.
    /// `keycode` = evdev KEY_* (mesma tabela do bridge/ydotool).
    /// O compositor traduz pra xkb Keycode internamente (evdev + 8).
    /// Conhecido: nao usa keysym; layout atual aplicado pelo xkb state do compositor.
    SyntheticKey { keycode: u32, pressed: bool },
    /// SI.1: input sintetico -- atalho. Pressiona `keys` em ordem,
    /// pequena pausa, libera em ordem reversa. Codigos evdev KEY_*.
    SyntheticKeyCombo { keys: Vec<u32> },
    /// W17.1: toggle maximize/fullscreen no toplevel com foco.
    /// Sem identifier de surface: usa focused toplevel (mesma logica de
    /// `CloseFocusedToplevel`). Bridge HTTP usa pra remotar a acao.
    ToggleMaximize,
    /// W17.1: minimize/iconify (stub) no toplevel com foco. Sem Wayland
    /// iconify protocol estavel; loga info e nao altera estado.
    MinimizeFocused,
}

pub fn default_socket_path() -> Option<std::path::PathBuf> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")?;
    let mut p = std::path::PathBuf::from(dir);
    p.push(SOCKET_BASENAME);
    Some(p)
}

pub fn encode_event(ev: &LumoEvent) -> String {
    let mut s = serde_json::to_string(ev).expect("LumoEvent sempre serializa");
    s.push('\n');
    s
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
        let ev = LumoEvent::Workspaces { active: 3, total: 5 };
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
        let ev = LumoEvent::ThemeReloaded { mode: ThemeMode::Dark };
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
        let cmd = LumoCommand::SyntheticPointerMove { x: 960.5, y: 540.25 };
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
            LumoCommand::SyntheticPointerButton { button: 272, pressed: true }
        );

        // scroll
        let scroll = LumoCommand::SyntheticPointerScroll { dx: 0.0, dy: 15.0 };
        let s = serde_json::to_string(&scroll).unwrap();
        let back: LumoCommand = serde_json::from_str(&s).unwrap();
        assert_eq!(back, scroll);

        // single key
        let key = LumoCommand::SyntheticKey { keycode: 28, pressed: false };
        let back: LumoCommand =
            serde_json::from_str(&serde_json::to_string(&key).unwrap()).unwrap();
        assert_eq!(back, key);

        // combo
        let combo = LumoCommand::SyntheticKeyCombo { keys: vec![29, 56, 20] };
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
}
