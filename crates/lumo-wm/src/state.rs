//! LumoState - top-level compositor state for lumo-wm MVP.
//!
//! Fase 5.1: estrutura minima necessaria pra Smithay despachar requests
//! Wayland em um event loop calloop. Sem renderer custom, sem tiling
//! avancado, sem layer-shell ainda. So o esqueleto que compila e aceita
//! clientes via socket nested.

use std::sync::Arc;
use std::time::Instant;

use smithay::desktop::{Space, Window};
use smithay::input::{Seat, SeatState};
use smithay::reexports::calloop::LoopHandle;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::utils::{Clock, Monotonic};
use smithay::wayland::compositor::{CompositorClientState, CompositorState};
use smithay::wayland::output::OutputManagerState;
use smithay::wayland::selection::data_device::DataDeviceState;
use smithay::wayland::shell::xdg::XdgShellState;
use smithay::wayland::shm::ShmState;
use smithay::wayland::socket::ListeningSocketSource;

/// Estado raiz do Lumo WM.
///
/// Smithay nao impoe uma struct fixa; voce escolhe quais protocols
/// implementar e cola via `delegate_*!` macros. Esses sao os minimos
/// pra um nested compositor MVP rodar.
pub struct LumoState {
    pub start_time: Instant,
    pub display_handle: DisplayHandle,
    pub loop_handle: LoopHandle<'static, LumoState>,
    pub socket_name: Option<String>,
    pub running: bool,

    // Clocks
    pub clock: Clock<Monotonic>,

    // Smithay state pieces
    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,
    pub seat_state: SeatState<Self>,
    pub data_device_state: DataDeviceState,

    // Input
    pub seat: Seat<Self>,

    // Desktop / window mgmt
    pub space: Space<Window>,
}

impl LumoState {
    pub fn new(
        display_handle: DisplayHandle,
        loop_handle: LoopHandle<'static, LumoState>,
        socket_name: Option<String>,
    ) -> Self {
        let clock = Clock::new();

        let compositor_state = CompositorState::new::<Self>(&display_handle);
        let xdg_shell_state = XdgShellState::new::<Self>(&display_handle);
        let shm_state = ShmState::new::<Self>(&display_handle, vec![]);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&display_handle);
        let mut seat_state = SeatState::new();
        let data_device_state = DataDeviceState::new::<Self>(&display_handle);

        let mut seat = seat_state.new_wl_seat(&display_handle, "lumo-seat-0");
        // Keyboard + pointer mais tarde (Fase 5.1 entrega o esqueleto).
        let _ = seat.add_keyboard(Default::default(), 200, 25);
        let _ = seat.add_pointer();

        Self {
            start_time: Instant::now(),
            display_handle,
            loop_handle,
            socket_name,
            running: true,
            clock,
            compositor_state,
            xdg_shell_state,
            shm_state,
            output_manager_state,
            seat_state,
            data_device_state,
            seat,
            space: Space::default(),
        }
    }
}

/// Estado por-cliente exigido pelo CompositorHandler.
#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
}

impl smithay::reexports::wayland_server::backend::ClientData for ClientState {
    fn initialized(&self, _client_id: smithay::reexports::wayland_server::backend::ClientId) {}
    fn disconnected(
        &self,
        _client_id: smithay::reexports::wayland_server::backend::ClientId,
        _reason: smithay::reexports::wayland_server::backend::DisconnectReason,
    ) {
    }
}

/// Wrapper helper pra criar o socket Wayland publico.
pub fn init_socket(
    loop_handle: &LoopHandle<'static, LumoState>,
    _display_handle: &DisplayHandle,
) -> anyhow::Result<String> {
    let source = ListeningSocketSource::new_auto()?;
    let socket_name = source.socket_name().to_string_lossy().into_owned();

    loop_handle
        .insert_source(source, move |client_stream, _, state| {
            if let Err(err) = state
                .display_handle
                .insert_client(client_stream, Arc::new(ClientState::default()))
            {
                tracing::warn!(?err, "Falha ao inserir cliente Wayland");
            }
        })
        .map_err(|e| anyhow::anyhow!("falha ao registrar socket: {e}"))?;

    Ok(socket_name)
}
