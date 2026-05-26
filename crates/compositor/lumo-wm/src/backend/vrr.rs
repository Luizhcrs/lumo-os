//! W13.B: VRR (Variable Refresh Rate) adaptive sync config + types.
//!
//! Ref: Hugl 2021 KWin VRR Wayland.
//!
//! Politica Galaxy Book 4:
//!   - Painel TN 6-bit + FRC eDP-1: provavel nao VRR capable.
//!   - HDMI externo: pode ser VRR capable (HDMI 2.1 display).
//!   - Padrao: vrr_enabled = false (TOML ~/.config/lumo/display.toml).
//!   - Quando vrr_enabled = true E output capable: seta VRR_ENABLED atomic.
//!
//! A funcao try_enable_vrr() esta em backend/drm.rs (acessa LumoDrmOutput).
//! Este modulo expoe apenas: DisplayConfig, VrrSetupResult, VrrError.

use serde::{Deserialize, Serialize};

// ============================================================
// DisplayConfig -- ~/.config/lumo/display.toml
// ============================================================

/// Configuracao de display carregada de ~/.config/lumo/display.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
    /// Habilita VRR (adaptive sync) se o output suportar.
    /// Padrao: false. Galaxy Book 4 eDP-1 provavelmente nao suporta.
    #[serde(default)]
    pub vrr_enabled: bool,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self { vrr_enabled: false }
    }
}

impl DisplayConfig {
    /// Carrega de ~/.config/lumo/display.toml. Retorna Default se ausente.
    pub fn load() -> Self {
        match Self::try_load() {
            Ok(cfg) => {
                tracing::info!(
                    vrr_enabled = cfg.vrr_enabled,
                    "W13.B: display.toml carregado"
                );
                cfg
            }
            Err(err) => {
                tracing::debug!(
                    ?err,
                    "W13.B: display.toml ausente/invalido, usando default (vrr=false)"
                );
                Self::default()
            }
        }
    }

    fn try_load() -> anyhow::Result<Self> {
        let home = std::env::var("HOME")?;
        let xdg = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{home}/.config"));
        let path = format!("{xdg}/lumo/display.toml");
        let raw = std::fs::read_to_string(&path)?;
        let cfg: Self = toml::from_str(&raw)?;
        Ok(cfg)
    }
}

// ============================================================
// VRR result types
// ============================================================

/// Resultado da tentativa de habilitar VRR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VrrSetupResult {
    /// VRR habilitado com sucesso.
    Enabled,
    /// VRR nao suportado neste connector/CRTC.
    NotSupported,
    /// VRR suportado mas requer modeset (HDMI -- kernel issue AMD).
    RequiresModeset,
    /// VRR desabilitado por config (vrr_enabled = false).
    Disabled,
    /// Erro ao verificar/setar VRR.
    Error(VrrError),
}

/// Erro durante setup VRR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VrrError {
    /// Connector handle nao disponivel.
    NoConnector,
    /// DRM ioctl falhou.
    DrmError,
}

// ============================================================
// Tests -- pure logic, sem DRM
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_config_default_vrr_disabled() {
        let cfg = DisplayConfig::default();
        assert!(!cfg.vrr_enabled);
    }

    #[test]
    fn display_config_toml_parse_vrr_true() {
        let raw = "vrr_enabled = true\n";
        let cfg: DisplayConfig = toml::from_str(raw).unwrap();
        assert!(cfg.vrr_enabled);
    }

    #[test]
    fn display_config_toml_parse_vrr_false() {
        let raw = "vrr_enabled = false\n";
        let cfg: DisplayConfig = toml::from_str(raw).unwrap();
        assert!(!cfg.vrr_enabled);
    }

    #[test]
    fn display_config_toml_parse_empty_is_default() {
        let raw = "";
        let cfg: DisplayConfig = toml::from_str(raw).unwrap();
        assert!(!cfg.vrr_enabled);
    }

    #[test]
    fn vrr_setup_result_disabled_variant_distinct() {
        assert_ne!(VrrSetupResult::Disabled, VrrSetupResult::Enabled);
        assert_ne!(VrrSetupResult::Disabled, VrrSetupResult::NotSupported);
    }

    #[test]
    fn vrr_error_variants_distinct() {
        assert_ne!(VrrError::NoConnector, VrrError::DrmError);
    }

    #[test]
    fn vrr_setup_result_all_variants_self_eq() {
        assert_eq!(VrrSetupResult::Enabled, VrrSetupResult::Enabled);
        assert_eq!(VrrSetupResult::NotSupported, VrrSetupResult::NotSupported);
        assert_eq!(
            VrrSetupResult::RequiresModeset,
            VrrSetupResult::RequiresModeset
        );
        assert_eq!(VrrSetupResult::Disabled, VrrSetupResult::Disabled);
    }
}
