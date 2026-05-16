//! wl_compositor delegate - aceita clientes Wayland, gerencia surfaces
//! e seu ciclo de commit. MVP: roteia commits pra Space pra serem
//! desenhados, sem damage tracking refinado.

use smithay::backend::renderer::utils::on_commit_buffer_handler;
use smithay::reexports::wayland_server::protocol::{wl_buffer::WlBuffer, wl_surface::WlSurface};
use smithay::reexports::wayland_server::Client;
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::compositor::{
    get_parent, is_sync_subsurface, CompositorClientState, CompositorHandler, CompositorState,
};

use crate::state::{ClientState, LumoState};

impl BufferHandler for LumoState {
    fn buffer_destroyed(&mut self, _buffer: &WlBuffer) {
        // MVP: nada a fazer. Buffers GLES sao tracked pelo renderer.
    }
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

        // Pula sub-surfaces sync: so processa quando o root commitar.
        if !is_sync_subsurface(surface) {
            let mut root = surface.clone();
            while let Some(parent) = get_parent(&root) {
                root = parent;
            }
            // Aqui entraria scheduling de redraw da janela que contem
            // o surface root. MVP: deixar Space::refresh no loop tick.
        }
        self.space.refresh();
    }
}

smithay::delegate_compositor!(LumoState);
