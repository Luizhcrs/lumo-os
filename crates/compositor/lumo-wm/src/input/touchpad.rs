//! Touchpad UX Lumo-like.
//!
//! Config serde-able para ~/.config/lumo/touchpad.toml.
//! apply_to_device() chamado em DeviceAdded no backend libinput.
//! Gesture state acumula deltas de swipe para threshold workspace switch.

#[cfg(feature = "drm-backend")]
use smithay::reexports::input as li;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// Threshold horizontal acumulado (px norm 1000dpi) para disparar
// workspace switch. ~30% de swipe de 3 dedos.
const SWIPE_H_THRESHOLD: f64 = 60.0;

// Threshold vertical para missao control / desktop reveal.
const SWIPE_V_THRESHOLD: f64 = 60.0;

/// Configuracao completa do touchpad. Serializada em TOML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TouchpadConfig {
    /// Tap to click (1 dedo = botao esquerdo).
    pub tap_enabled: bool,
    /// Tap and drag (segurar drag entre toques).
    pub tap_drag: bool,
    /// Drag lock: mantem drag apos lift do dedo ate proximo tap.
    /// Lumo default = false (industry default). Quando true, atrapalha
    /// rubber-band selection no desktop (rect demora a sumir).
    #[serde(default)]
    pub tap_drag_lock: bool,
    /// Natural scroll (direcao conteudo, Mac default).
    pub natural_scroll: bool,
    /// Two-finger scroll habilitado.
    pub two_finger_scroll: bool,
    /// Gestures multi-dedo habilitados (swipe, pinch).
    pub gestures_enabled: bool,
    /// Perfil de aceleracao: "adaptive" ou "flat".
    pub accel_profile: AccelProfileCfg,
    /// Velocidade de aceleracao [-1.0, 1.0]. 0.0 = valor padrao.
    pub accel_speed: f64,
    /// Desabilitar touchpad enquanto digitando.
    pub disable_while_typing: bool,
    /// Metodo de click: "clickfinger" (1=L 2=R 3=M) ou "button_areas".
    pub click_method: ClickMethodCfg,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AccelProfileCfg {
    Adaptive,
    Flat,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ClickMethodCfg {
    Clickfinger,
    ButtonAreas,
}

impl Default for TouchpadConfig {
    fn default() -> Self {
        Self {
            tap_enabled: true,
            tap_drag: true,
            tap_drag_lock: false,
            natural_scroll: true,
            two_finger_scroll: true,
            gestures_enabled: true,
            accel_profile: AccelProfileCfg::Adaptive,
            accel_speed: 0.0,
            disable_while_typing: true,
            click_method: ClickMethodCfg::ButtonAreas,
        }
    }
}

impl TouchpadConfig {
    /// Carrega de ~/.config/lumo/touchpad.toml.
    /// Fallback silencioso para Default se arquivo ausente ou invalido.
    pub fn load() -> Self {
        match Self::load_inner() {
            Ok(cfg) => cfg,
            Err(err) => {
                tracing::debug!(?err, "touchpad.toml nao carregado, usando defaults industry");
                Self::default()
            }
        }
    }

    fn load_inner() -> anyhow::Result<Self> {
        let path = config_path()?;
        let raw = std::fs::read_to_string(&path)?;
        let cfg: Self = toml::from_str(&raw)?;
        tracing::info!(?path, "touchpad.toml carregado");
        Ok(cfg)
    }

    /// Persiste configuracao atual em ~/.config/lumo/touchpad.toml.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let toml_str = toml::to_string_pretty(self)?;
        std::fs::write(&path, toml_str)?;
        tracing::info!(?path, "touchpad.toml salvo");
        Ok(())
    }

    /// Aplica configuracao a um device libinput em DeviceAdded.
    /// So disponivel no drm-backend (libinput real).
    #[cfg(feature = "drm-backend")]
    pub fn apply_to_device(&self, device: &mut li::Device) {
        // So aplica se for touchpad (tem config de tap).
        if device.config_tap_finger_count() == 0 {
            return;
        }

        // Aceleracao.
        let profile = match self.accel_profile {
            AccelProfileCfg::Adaptive => li::AccelProfile::Adaptive,
            AccelProfileCfg::Flat => li::AccelProfile::Flat,
        };
        let _ = device.config_accel_set_profile(profile);
        let _ = device.config_accel_set_speed(self.accel_speed);

        // Tap to click + button map.
        let _ = device.config_tap_set_enabled(self.tap_enabled);
        if self.tap_enabled {
            let _ = device.config_tap_set_button_map(li::TapButtonMap::LeftRightMiddle);
            let _ = device.config_tap_set_drag_enabled(self.tap_drag);
            let _ = device.config_tap_set_drag_lock_enabled(self.tap_drag_lock);
        }

        // Natural scroll.
        if device.config_scroll_has_natural_scroll() {
            let _ = device.config_scroll_set_natural_scroll_enabled(self.natural_scroll);
        }

        // Two-finger scroll.
        if self.two_finger_scroll {
            let _ = device.config_scroll_set_method(li::ScrollMethod::TwoFinger);
        }

        // Disable while typing.
        let _ = device.config_dwt_set_enabled(self.disable_while_typing);

        // Click method.
        let methods = device.config_click_methods();
        let want = match self.click_method {
            ClickMethodCfg::Clickfinger => li::ClickMethod::Clickfinger,
            ClickMethodCfg::ButtonAreas => li::ClickMethod::ButtonAreas,
        };
        if methods.contains(&want) {
            let _ = device.config_click_set_method(want);
        }

        tracing::info!(
            name = ?device.name(),
            tap = self.tap_enabled,
            natural_scroll = self.natural_scroll,
            click_method = ?self.click_method,
            accel_speed = self.accel_speed,
            "touchpad config aplicada"
        );
    }
}

fn config_path() -> anyhow::Result<PathBuf> {
    let home = std::env::var("HOME")
        .map_err(|_| anyhow::anyhow!("HOME nao definido"))?;
    Ok(PathBuf::from(home).join(".config").join("lumo").join("touchpad.toml"))
}

// ---- Gesture State --------------------------------------------------------

/// Sentido do swipe de 3 dedos horizontal resolvido ao End.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SwipeDirection {
    Left,
    Right,
    Up,
    Down,
}

/// Estado acumulado de gestures em andamento.
/// Vive em LumoState, resetado a cada Begin/End.
#[derive(Debug, Default)]
pub struct TouchpadGestureState {
    /// Fingers do swipe corrente (0 = nenhum em andamento).
    pub swipe_fingers: u32,
    /// Delta X acumulado desde Begin.
    pub swipe_dx: f64,
    /// Delta Y acumulado desde Begin.
    pub swipe_dy: f64,
    /// Scale corrente do pinch (1.0 = neutro).
    pub pinch_scale: f64,
    /// Fingers do pinch corrente.
    pub pinch_fingers: u32,
}

impl TouchpadGestureState {
    pub fn on_swipe_begin(&mut self, fingers: u32) {
        self.swipe_fingers = fingers;
        self.swipe_dx = 0.0;
        self.swipe_dy = 0.0;
    }

    pub fn on_swipe_update(&mut self, dx: f64, dy: f64) {
        self.swipe_dx += dx;
        self.swipe_dy += dy;
    }

    /// Retorna direcao resolvida se threshold atingido, None se cancelado
    /// ou delta insuficiente.
    pub fn on_swipe_end(&mut self, cancelled: bool) -> Option<(u32, SwipeDirection)> {
        let fingers = self.swipe_fingers;
        let dx = self.swipe_dx;
        let dy = self.swipe_dy;
        self.swipe_fingers = 0;
        self.swipe_dx = 0.0;
        self.swipe_dy = 0.0;

        if cancelled || fingers == 0 {
            return None;
        }

        // Determina eixo dominante.
        if dx.abs() >= dy.abs() {
            if dx.abs() >= SWIPE_H_THRESHOLD {
                let dir = if dx > 0.0 { SwipeDirection::Right } else { SwipeDirection::Left };
                return Some((fingers, dir));
            }
        } else if dy.abs() >= SWIPE_V_THRESHOLD {
            let dir = if dy > 0.0 { SwipeDirection::Down } else { SwipeDirection::Up };
            return Some((fingers, dir));
        }

        None
    }

    pub fn on_pinch_begin(&mut self, fingers: u32) {
        self.pinch_fingers = fingers;
        self.pinch_scale = 1.0;
    }

    pub fn on_pinch_update(&mut self, scale: f64) {
        self.pinch_scale = scale;
    }

    pub fn on_pinch_end(&mut self, cancelled: bool) -> Option<f64> {
        let scale = self.pinch_scale;
        let fingers = self.pinch_fingers;
        self.pinch_fingers = 0;
        self.pinch_scale = 1.0;

        if cancelled || fingers == 0 {
            return None;
        }
        Some(scale)
    }
}
