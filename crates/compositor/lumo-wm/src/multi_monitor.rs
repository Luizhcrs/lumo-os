//! W9.C: Multi-monitor scaffolding.
//!
//! Extends DRM backend to detect all connected connectors and create
//! a DrmSurfaceData per output. Render loop iterates each surface.
//! IPC broadcasts OutputAdded/OutputRemoved on change.
//!
//! Design:
//! - Primary output (eDP-1 or first connected) -> DrmBackendData.surface
//! - Extra outputs -> DrmBackendData.extra_surfaces Vec
//! - Per-output: space.map_output at x_offset (tiled horizontally)
//! - Bar/dock layer-shell: clients handle multi-output via wlr-layer-shell
//!   per-output binding; compositor maps layer_map per output already.
//! - IPC: OutputAdded broadcast after each surface init.
//!
//! Workspaces per-output: deferred to M2. For now all workspaces share
//! the same vault regardless of output.

/// Tiled layout for N outputs: place outputs side by side horizontally.
/// Returns (x_offset, y_offset) for output at index `idx`.
pub fn tile_output_position(idx: usize, width_per_output: i32) -> (i32, i32) {
    ((idx as i32) * width_per_output, 0)
}

/// Selects the primary connector index from a list of connector names.
/// Prefers eDP/LVDS (laptop internal panel). Falls back to index 0.
pub fn pick_primary_connector(names: &[String]) -> usize {
    names.iter()
        .position(|n| {
            let upper = n.to_uppercase();
            upper.starts_with("EDP") || upper.starts_with("LVDS")
        })
        .unwrap_or(0)
}

/// Summary of a detected output, for IPC broadcast.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputInfo {
    pub name: String,
    pub index: u32,
    pub width: u32,
    pub height: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_position_first_is_zero() {
        assert_eq!(tile_output_position(0, 1920), (0, 0));
    }

    #[test]
    fn tile_position_second_is_offset() {
        assert_eq!(tile_output_position(1, 1920), (1920, 0));
    }

    #[test]
    fn tile_position_third() {
        assert_eq!(tile_output_position(2, 1920), (3840, 0));
    }

    #[test]
    fn pick_primary_edp_preferred() {
        let names = vec![
            "HDMI-A-1".to_string(),
            "eDP-1".to_string(),
            "DP-1".to_string(),
        ];
        assert_eq!(pick_primary_connector(&names), 1);
    }

    #[test]
    fn pick_primary_lvds_preferred() {
        let names = vec![
            "HDMI-A-1".to_string(),
            "LVDS-1".to_string(),
        ];
        assert_eq!(pick_primary_connector(&names), 1);
    }

    #[test]
    fn pick_primary_fallback_to_zero() {
        let names = vec![
            "HDMI-A-1".to_string(),
            "DP-1".to_string(),
        ];
        assert_eq!(pick_primary_connector(&names), 0);
    }

    #[test]
    fn pick_primary_edp_uppercase() {
        let names = vec![
            "DP-1".to_string(),
            "EDP-1".to_string(),
        ];
        assert_eq!(pick_primary_connector(&names), 1);
    }

    #[test]
    fn pick_primary_single_connector() {
        let names = vec!["HDMI-A-1".to_string()];
        assert_eq!(pick_primary_connector(&names), 0);
    }

    #[test]
    fn output_info_fields() {
        let info = OutputInfo {
            name: "eDP-1".to_string(),
            index: 0,
            width: 1920,
            height: 1080,
        };
        assert_eq!(info.name, "eDP-1");
        assert_eq!(info.index, 0);
        assert_eq!(info.width, 1920);
        assert_eq!(info.height, 1080);
    }

    #[test]
    fn tile_many_outputs() {
        for i in 0..5usize {
            let (x, y) = tile_output_position(i, 2560);
            assert_eq!(x, i as i32 * 2560);
            assert_eq!(y, 0);
        }
    }
}
