//! wlr_layer_shell delegate - background/bar/overlay surfaces.

use smithay::desktop::{layer_map_for_output, LayerSurface as DesktopLayerSurface};
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::wayland::shell::wlr_layer::{
    Layer, LayerSurface, WlrLayerShellHandler, WlrLayerShellState,
};

use crate::state::LumoState;

impl WlrLayerShellHandler for LumoState {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.layer_shell_state
    }

    fn new_layer_surface(
        &mut self,
        surface: LayerSurface,
        _wl_output: Option<WlOutput>,
        _layer: Layer,
        namespace: String,
    ) {
        // MVP: 1 output unico (winit).
        let Some(output) = self.space.outputs().next().cloned() else {
            tracing::warn!("layer_surface sem output destino; ignorando");
            return;
        };

        let layer_surface = DesktopLayerSurface::new(surface, namespace);
        let mut map = layer_map_for_output(&output);
        if let Err(err) = map.map_layer(&layer_surface) {
            tracing::warn!(?err, "Falha ao mapear layer surface");
        }
    }

    fn layer_destroyed(&mut self, surface: LayerSurface) {
        let outputs: Vec<_> = self.space.outputs().cloned().collect();
        for output in outputs {
            let mut map = layer_map_for_output(&output);
            let target = map
                .layers()
                .find(|l| l.layer_surface() == &surface)
                .cloned();
            if let Some(layer) = target {
                map.unmap_layer(&layer);
            }
        }
    }
}

smithay::delegate_layer_shell!(LumoState);
