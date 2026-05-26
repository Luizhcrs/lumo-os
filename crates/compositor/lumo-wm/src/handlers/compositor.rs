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
        // W22: surface commit = render needed.
        self.should_render = true;

        if !is_sync_subsurface(surface) {
            let mut root = surface.clone();
            while let Some(parent) = get_parent(&root) {
                root = parent;
            }
            if let Some(window) = self
                .space
                .elements()
                .find(|w| w.wl_surface().map(|s| s.as_ref() == &root).unwrap_or(false))
                .cloned()
            {
                window.on_commit();

                // LIMITES DE GEOMETRIA: Respeite a usable_geometry() do compositor.
                // Janelas flutuantes nao devem abrir maiores que o espaco util da tela
                // nem transborde para fora dos limites.
                if self.tiling_mode == crate::tiling::TilingMode::Floating {
                    let usable = self.usable_geometry();
                    let geo = window.geometry();
                    let mut size_changed = false;
                    let mut new_w = geo.size.w;
                    let mut new_h = geo.size.h;

                    if new_w > usable.size.w {
                        new_w = usable.size.w;
                        size_changed = true;
                    }
                    if new_h > usable.size.h {
                        new_h = usable.size.h;
                        size_changed = true;
                    }

                    if size_changed {
                        if let Some(tl) = window.toplevel() {
                            tl.with_pending_state(|state| {
                                state.size = Some(smithay::utils::Size::from((new_w, new_h)));
                            });
                            let _ = tl.send_configure();
                        }
                    }

                    let current_loc = self.space.element_location(&window).unwrap_or_default();
                    let mut new_x = current_loc.x;
                    let mut new_y = current_loc.y;

                    const SSD_TITLEBAR_H: i32 = 30;
                    
                    let min_x = usable.loc.x;
                    let min_y = usable.loc.y + SSD_TITLEBAR_H;
                    let max_x = (usable.loc.x + usable.size.w - new_w).max(min_x);
                    let max_y = (usable.loc.y + usable.size.h - new_h).max(min_y);

                    new_x = new_x.clamp(min_x, max_x.max(min_x));
                    new_y = new_y.clamp(min_y, max_y.max(min_y));

                    if new_x != current_loc.x || new_y != current_loc.y {
                        self.space.map_element(window.clone(), (new_x, new_y), true);
                    }
                }
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
