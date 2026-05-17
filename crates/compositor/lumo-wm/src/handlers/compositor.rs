//! wl_compositor delegate.
//!
//! Fase 5.4: alem do commit padrao, dispara initial-configure pros
//! layer-shell surfaces (ex: lumo-bar) e mantem layer_map arranjado.
//! Sem isso o cliente layer-shell nunca recebia configure com size,
//! committeava tamanho 0x0 e a barra so renderizava o brand-dot.

use smithay::backend::renderer::utils::on_commit_buffer_handler;
use smithay::desktop::layer_map_for_output;
use smithay::reexports::wayland_server::protocol::{wl_buffer::WlBuffer, wl_surface::WlSurface};
use smithay::reexports::wayland_server::Client;
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::compositor::{
    get_parent, is_sync_subsurface, CompositorClientState, CompositorHandler, CompositorState,
};
use smithay::wayland::seat::WaylandFocus;

use crate::state::{ClientState, LumoState};

impl BufferHandler for LumoState {
    fn buffer_destroyed(&mut self, _buffer: &WlBuffer) {}
}

impl CompositorHandler for LumoState {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client
            .get_data::<ClientState>()
            .expect("ClientState ausente; cliente nao foi inserido com ClientState")
            .compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);

        if !is_sync_subsurface(surface) {
            let mut root = surface.clone();
            while let Some(parent) = get_parent(&root) {
                root = parent;
            }
            if let Some(window) = self
                .space
                .elements()
                .find(|w| {
                    w.wl_surface()
                        .map(|s| s.as_ref() == &root)
                        .unwrap_or(false)
                })
                .cloned()
            {
                window.on_commit();
            }

            // Fix 4 (5.4): initial-configure pra layer-shell surfaces
            // (ex: lumo-bar). `map_layer` em new_layer_surface ja
            // chamou `arrange()` e fixou pending size, mas
            // `send_pending_configure` so dispara automatico apos o
            // initial. Forcamos `send_configure` no primeiro commit
            // que ainda tem `initial_configure_sent == false`
            // (`has_pending_changes` retorna true nesse caso).
            let wlr_layer = self
                .layer_shell_state
                .layer_surfaces()
                .find(|l| l.wl_surface() == &root);
            if let Some(wlr_layer) = wlr_layer {
                // Re-arranja layer_map por output pra atualizar size
                // caso a anchor/exclusive_zone tenha mudado no commit.
                let outputs: Vec<_> = self.space.outputs().cloned().collect();
                for output in outputs {
                    let mut map = layer_map_for_output(&output);
                    map.arrange();
                }
                if wlr_layer.has_pending_changes() {
                    let _ = wlr_layer.send_configure();
                }
            }
        }
        self.popups.commit(surface);
        self.space.refresh();
    }
}

smithay::delegate_compositor!(LumoState);
