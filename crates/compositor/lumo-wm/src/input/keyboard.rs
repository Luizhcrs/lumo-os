//! Sistema de keybindings global do lumo-wm.
//!
//! B2: 16+ bindings Lumo-style, TOML remapeaveis em
//! ~/.config/lumo/keyboard.toml. Fallback para default_bindings()
//! se arquivo ausente ou invalido.

use serde::{Deserialize, Serialize};
use smithay::input::keyboard::Keysym;

/// Mask de modificadores.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ModifiersMask {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub logo: bool,
}

impl ModifiersMask {
    pub const fn super_only() -> Self {
        Self {
            ctrl: false,
            alt: false,
            shift: false,
            logo: true,
        }
    }
    pub const fn super_shift() -> Self {
        Self {
            ctrl: false,
            alt: false,
            shift: true,
            logo: true,
        }
    }
    pub const fn ctrl_alt() -> Self {
        Self {
            ctrl: true,
            alt: true,
            shift: false,
            logo: false,
        }
    }
}

/// Direcao para TileMove.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TileDir {
    Up,
    Down,
    Left,
    Right,
}

/// Acoes disparadas por keybind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyAction {
    /// F5: refresh compositor (re-render + re-init clients).
    Refresh,
    /// W12.A: cycle tiling mode (Floating->MasterStack->Spiral->Columns->Floating).
    TilingCycle,
    /// W12.A: rebalance/repaint tiling (SUPER+R).
    TilingRebalance,
    /// W12.A: cycle focus to previous master/tile (SUPER+H).
    TilingFocusPrev,
    /// W12.A: cycle focus to next master/tile (SUPER+L).
    TilingFocusNext,
    /// W12.B: toggle mission control overview (SUPER+UP).
    MissionControl,
    /// W12.C: open/cycle window stack picker (SUPER+TAB with visual).
    StackPicker,
    Spawn(String),
    CloseWindow,
    Lock,
    Launcher,
    Workspace(u8),
    MoveToWorkspace(u8),
    CycleWindow(i8),
    TileMove(TileDir),
    FullscreenToggle,
    Minimize,
    Quit,
    SwitchVt(i32),
    /// F1.5-D1: Hide window sem fechar (Mac Cmd+H equivalente).
    HideWindow,
    /// F1.5-D1: Show shortcut help overlay (Mac Cmd+/).
    ShortcutHelp,
    /// F1.5-D1: Jump pra N-th window do workspace (Super+1..9).
    /// N=0 reserva pra "show all" futuro.
    JumpToWindow(u8),
    /// F1.5-C2: Clipboard history picker (Super+Shift+V).
    ClipboardHistory,
    /// A2 review: invoca ShellApp resolvendo via ShellAppRegistry.
    /// Sobrepoe `Spawn(String)` hardcoded ao longo do tempo.
    InvokeApp(lumo_ipc::ShellApp),
}

/// Keysym como u32 pra serde.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeySym(pub u32);

impl KeySym {
    pub fn as_keysym(&self) -> Keysym {
        Keysym::from(self.0)
    }
}

impl From<Keysym> for KeySym {
    fn from(k: Keysym) -> Self {
        KeySym(k.raw())
    }
}

/// Um binding: mods + keysym -> acao.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBinding {
    pub mods: ModifiersMask,
    pub key: KeySym,
    pub action: KeyAction,
}

impl KeyBinding {
    fn new(mods: ModifiersMask, key: Keysym, action: KeyAction) -> Self {
        Self {
            mods,
            key: KeySym::from(key),
            action,
        }
    }
}

/// Configuracao completa de keybindings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyboardConfig {
    pub bindings: Vec<KeyBinding>,
}

impl Default for KeyboardConfig {
    fn default() -> Self {
        Self {
            bindings: default_bindings(),
        }
    }
}

impl KeyboardConfig {
    /// Carrega de ~/.config/lumo/keyboard.toml ou usa default.
    pub fn load() -> Self {
        match Self::try_load() {
            Ok(cfg) => {
                tracing::info!("keybindings: carregado de ~/.config/lumo/keyboard.toml");
                cfg
            }
            Err(err) => {
                tracing::debug!(?err, "keybindings: usando default");
                Self::default()
            }
        }
    }

    fn try_load() -> anyhow::Result<Self> {
        let home = std::env::var("HOME")?;
        let xdg_cfg =
            std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{home}/.config"));
        let path = format!("{xdg_cfg}/lumo/keyboard.toml");
        let raw = std::fs::read_to_string(&path)?;
        let cfg: Self = toml::from_str(&raw)?;
        Ok(cfg)
    }

    /// Salva em ~/.config/lumo/keyboard.toml.
    pub fn save(&self) -> anyhow::Result<()> {
        let home = std::env::var("HOME")?;
        let xdg_cfg =
            std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{home}/.config"));
        let dir = format!("{xdg_cfg}/lumo");
        std::fs::create_dir_all(&dir)?;
        let path = format!("{dir}/keyboard.toml");
        let raw = toml::to_string_pretty(self)?;
        std::fs::write(&path, raw)?;
        Ok(())
    }

    /// Procura o primeiro binding que casa com mods+sym.
    pub fn match_binding(
        &self,
        mods: &smithay::input::keyboard::ModifiersState,
        sym: Keysym,
    ) -> Option<&KeyAction> {
        let pressed = ModifiersMask {
            ctrl: mods.ctrl,
            alt: mods.alt,
            shift: mods.shift,
            logo: mods.logo,
        };
        self.bindings
            .iter()
            .find(|b| b.mods == pressed && b.key.as_keysym() == sym)
            .map(|b| &b.action)
    }
}

/// 16+ bindings padrao Lumo-style.
pub fn default_bindings() -> Vec<KeyBinding> {
    let s = ModifiersMask::super_only;
    let ss = ModifiersMask::super_shift;
    let ca = ModifiersMask::ctrl_alt;

    vec![
        // SUPER bindings
        KeyBinding::new(s(), Keysym::l, KeyAction::Lock),
        KeyBinding::new(s(), Keysym::L, KeyAction::Lock),
        KeyBinding::new(s(), Keysym::q, KeyAction::CloseWindow),
        KeyBinding::new(s(), Keysym::Q, KeyAction::CloseWindow),
        // F1.5-D1: Mac-style Cmd+W close window (alias Super+Q).
        KeyBinding::new(s(), Keysym::w, KeyAction::CloseWindow),
        KeyBinding::new(s(), Keysym::W, KeyAction::CloseWindow),
        // F1.5-D1: Cmd+/ shortcut help overlay.
        KeyBinding::new(s(), Keysym::slash, KeyAction::ShortcutHelp),
        KeyBinding::new(s(), Keysym::question, KeyAction::ShortcutHelp),
        // Super+1..9 = trocar workspace (padrao GNOME/KDE/i3). Os bindings
        // JumpToWindow(1..9) foram REMOVIDOS: colidiam com Workspace(1..9)
        // (mesmo mods+key) e match_binding pegava o primeiro -> JumpToWindow
        // vencia e a troca de workspace por teclado ficava morta. Workspace
        // definido mais abaixo (mantido). JumpToWindow continua acessivel
        // via codigo/IPC se necessario, sem acelerador conflitante.
        KeyBinding::new(s(), Keysym::space, KeyAction::Launcher),
        KeyBinding::new(s(), Keysym::Return, KeyAction::Spawn("foot".to_string())),
        KeyBinding::new(s(), Keysym::Tab, KeyAction::StackPicker),
        // W12.A: tiling
        KeyBinding::new(s(), Keysym::t, KeyAction::TilingCycle),
        KeyBinding::new(s(), Keysym::T, KeyAction::TilingCycle),
        KeyBinding::new(s(), Keysym::r, KeyAction::TilingRebalance),
        KeyBinding::new(s(), Keysym::R, KeyAction::TilingRebalance),
        KeyBinding::new(s(), Keysym::h, KeyAction::TilingFocusPrev),
        KeyBinding::new(s(), Keysym::H, KeyAction::TilingFocusPrev),
        KeyBinding::new(s(), Keysym::semicolon, KeyAction::TilingFocusNext),
        // W12.B: mission control
        KeyBinding::new(s(), Keysym::Up, KeyAction::MissionControl),
        KeyBinding::new(ModifiersMask::default(), Keysym::F5, KeyAction::Refresh),
        KeyBinding::new(s(), Keysym::f, KeyAction::FullscreenToggle),
        KeyBinding::new(s(), Keysym::F, KeyAction::FullscreenToggle),
        KeyBinding::new(s(), Keysym::m, KeyAction::Minimize),
        KeyBinding::new(s(), Keysym::M, KeyAction::Minimize),
        // SUPER+Arrow -> TileMove
        // W12.B: SUPER+Up -> MissionControl (replaces TileMove::Up stub).
        KeyBinding::new(s(), Keysym::Down, KeyAction::TileMove(TileDir::Down)),
        KeyBinding::new(s(), Keysym::Left, KeyAction::TileMove(TileDir::Left)),
        KeyBinding::new(s(), Keysym::Right, KeyAction::TileMove(TileDir::Right)),
        // SUPER+1..9 -> Switch workspace
        KeyBinding::new(s(), Keysym::_1, KeyAction::Workspace(1)),
        KeyBinding::new(s(), Keysym::_2, KeyAction::Workspace(2)),
        KeyBinding::new(s(), Keysym::_3, KeyAction::Workspace(3)),
        KeyBinding::new(s(), Keysym::_4, KeyAction::Workspace(4)),
        KeyBinding::new(s(), Keysym::_5, KeyAction::Workspace(5)),
        KeyBinding::new(s(), Keysym::_6, KeyAction::Workspace(6)),
        KeyBinding::new(s(), Keysym::_7, KeyAction::Workspace(7)),
        KeyBinding::new(s(), Keysym::_8, KeyAction::Workspace(8)),
        KeyBinding::new(s(), Keysym::_9, KeyAction::Workspace(9)),
        // SUPER+Shift+1..9 -> Move janela pra workspace
        KeyBinding::new(ss(), Keysym::_1, KeyAction::MoveToWorkspace(1)),
        KeyBinding::new(ss(), Keysym::_2, KeyAction::MoveToWorkspace(2)),
        KeyBinding::new(ss(), Keysym::_3, KeyAction::MoveToWorkspace(3)),
        KeyBinding::new(ss(), Keysym::_4, KeyAction::MoveToWorkspace(4)),
        KeyBinding::new(ss(), Keysym::_5, KeyAction::MoveToWorkspace(5)),
        KeyBinding::new(ss(), Keysym::_6, KeyAction::MoveToWorkspace(6)),
        KeyBinding::new(ss(), Keysym::_7, KeyAction::MoveToWorkspace(7)),
        KeyBinding::new(ss(), Keysym::_8, KeyAction::MoveToWorkspace(8)),
        KeyBinding::new(ss(), Keysym::_9, KeyAction::MoveToWorkspace(9)),
        // SUPER+Shift+Tab -> Cycle anterior
        KeyBinding::new(ss(), Keysym::Tab, KeyAction::CycleWindow(-1)),
        // F1.5-C2: SUPER+Shift+V -> clipboard history picker (lumo-clip).
        KeyBinding::new(ss(), Keysym::v, KeyAction::ClipboardHistory),
        KeyBinding::new(ss(), Keysym::V, KeyAction::ClipboardHistory),
        // Ctrl+Alt+Backspace -> Quit
        KeyBinding::new(ca(), Keysym::BackSpace, KeyAction::Quit),
        // Ctrl+Alt+F1..F6 -> VT switch
        KeyBinding::new(ca(), Keysym::F1, KeyAction::SwitchVt(1)),
        KeyBinding::new(ca(), Keysym::F2, KeyAction::SwitchVt(2)),
        KeyBinding::new(ca(), Keysym::F3, KeyAction::SwitchVt(3)),
        KeyBinding::new(ca(), Keysym::F4, KeyAction::SwitchVt(4)),
        KeyBinding::new(ca(), Keysym::F5, KeyAction::SwitchVt(5)),
        KeyBinding::new(ca(), Keysym::F6, KeyAction::SwitchVt(6)),
        // XF86_Switch_VT_* keysyms
        KeyBinding::new(ca(), Keysym::XF86_Switch_VT_1, KeyAction::SwitchVt(1)),
        KeyBinding::new(ca(), Keysym::XF86_Switch_VT_2, KeyAction::SwitchVt(2)),
        KeyBinding::new(ca(), Keysym::XF86_Switch_VT_3, KeyAction::SwitchVt(3)),
        KeyBinding::new(ca(), Keysym::XF86_Switch_VT_4, KeyAction::SwitchVt(4)),
        KeyBinding::new(ca(), Keysym::XF86_Switch_VT_5, KeyAction::SwitchVt(5)),
        KeyBinding::new(ca(), Keysym::XF86_Switch_VT_6, KeyAction::SwitchVt(6)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup(cfg: &KeyboardConfig, mods: ModifiersMask, sym: Keysym) -> Option<KeyAction> {
        cfg.bindings
            .iter()
            .find(|b| b.mods == mods && b.key.as_keysym() == sym)
            .map(|b| b.action.clone())
    }

    #[test]
    fn super_shift_v_bound_to_clipboard_history() {
        let cfg = KeyboardConfig::default();
        let ss = ModifiersMask::super_shift();
        assert_eq!(
            lookup(&cfg, ss, Keysym::v),
            Some(KeyAction::ClipboardHistory)
        );
    }

    #[test]
    fn super_shift_v_uppercase_also_bound() {
        let cfg = KeyboardConfig::default();
        let ss = ModifiersMask::super_shift();
        assert_eq!(
            lookup(&cfg, ss, Keysym::V),
            Some(KeyAction::ClipboardHistory)
        );
    }

    #[test]
    fn plain_super_v_not_clipboard_history() {
        let cfg = KeyboardConfig::default();
        let s = ModifiersMask::super_only();
        assert_ne!(lookup(&cfg, s, Keysym::v), Some(KeyAction::ClipboardHistory));
    }

    // Super+1..9 deve trocar workspace (nao JumpToWindow). Regressao do bug
    // de binding duplicado onde JumpToWindow vencia o match.
    #[test]
    fn super_digits_bound_to_workspace() {
        let cfg = KeyboardConfig::default();
        let s = ModifiersMask::super_only();
        let digits = [
            Keysym::_1,
            Keysym::_2,
            Keysym::_3,
            Keysym::_4,
            Keysym::_5,
            Keysym::_6,
            Keysym::_7,
            Keysym::_8,
            Keysym::_9,
        ];
        for (i, k) in digits.iter().enumerate() {
            let n = (i + 1) as u8;
            assert_eq!(
                lookup(&cfg, s.clone(), *k),
                Some(KeyAction::Workspace(n)),
                "Super+{n} deve ser Workspace({n}), nao JumpToWindow"
            );
        }
    }

    // Blindagem: nenhum (mods,key) pode aparecer 2x em default_bindings.
    // match_binding pega o primeiro -> duplicata = binding morto silencioso.
    #[test]
    fn no_duplicate_accelerators() {
        use std::collections::HashSet;
        let cfg = KeyboardConfig::default();
        let mut seen: HashSet<(bool, bool, bool, bool, u32)> = HashSet::new();
        for b in &cfg.bindings {
            let key = (
                b.mods.ctrl,
                b.mods.alt,
                b.mods.shift,
                b.mods.logo,
                b.key.0,
            );
            assert!(
                seen.insert(key),
                "acelerador duplicado: mods={:?} key={:?}",
                b.mods,
                b.key.0
            );
        }
    }
}
