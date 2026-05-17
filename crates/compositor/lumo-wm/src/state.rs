//! LumoState - top-level compositor state for lumo-wm.
//!
//! Fase 5.4+: alem do esqueleto da 5.1/5.2/5.3:
//! - layer_shell_state, primary_selection_state, xdg_activation_state,
//!   fractional_scale_manager_state, cursor_shape_manager_state,
//!   xdg_toplevel_icon_manager
//! - PopupManager pra ciclo de vida dos popups xdg
//! - estado de input (pointer pos, foco, layout horizontal)
//! - cursor xcursor real (Adwaita/default) com MemoryRenderBuffer
//!
//! Fase 5.5 (A8):
//! - servidor IPC (`crate::ipc`) acoplado em `ipc: IpcServer`
//! - estado de workspace ativo (`active_workspace`)
//! - `handle_ipc_command` aplicando comandos do canal IPC
//! - `set_workspace` faz broadcast a cada troca

use std::sync::Arc;
use std::time::Instant;

use smithay::desktop::{PopupManager, Space, Window};
use smithay::input::keyboard::KeyboardHandle;
use smithay::input::pointer::PointerHandle;
use smithay::input::{Seat, SeatState};
use smithay::reexports::calloop::LoopHandle;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::utils::{Clock, Logical, Monotonic, Point};
use smithay::wayland::compositor::{CompositorClientState, CompositorState};
use smithay::wayland::cursor_shape::CursorShapeManagerState;
use smithay::wayland::fractional_scale::FractionalScaleManagerState;
use smithay::wayland::output::OutputManagerState;
use smithay::wayland::selection::data_device::DataDeviceState;
use smithay::wayland::selection::primary_selection::PrimarySelectionState;
use smithay::wayland::shell::wlr_layer::WlrLayerShellState;
use smithay::wayland::shell::xdg::XdgShellState;
use smithay::wayland::shm::ShmState;
use smithay::wayland::socket::ListeningSocketSource;
use smithay::wayland::xdg_activation::XdgActivationState;
use smithay::wayland::dmabuf::{DmabufGlobal, DmabufState};
use smithay::wayland::xdg_toplevel_icon::XdgToplevelIconManager;

use lumo_ipc::{LumoCommand, MAX_WORKSPACES};

use crate::ipc::IpcServer;

/// Estado raiz do Lumo WM.
pub struct LumoState {
    pub start_time: Instant,
    pub display_handle: DisplayHandle,
    pub loop_handle: LoopHandle<'static, LumoState>,
    pub socket_name: Option<String>,
    pub running: bool,

    pub clock: Clock<Monotonic>,

    // Core protocols
    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,
    pub seat_state: SeatState<Self>,
    pub data_device_state: DataDeviceState,

    // Fase 5.2: protocols extras
    pub layer_shell_state: WlrLayerShellState,
    pub primary_selection_state: PrimarySelectionState,
    pub xdg_activation_state: XdgActivationState,
    pub fractional_scale_state: FractionalScaleManagerState,
    pub cursor_shape_state: CursorShapeManagerState,
    #[allow(dead_code)]
    pub xdg_toplevel_icon_manager: XdgToplevelIconManager,

    // A10 frente 1: linux-dmabuf-v1.
    //
    // dmabuf_state existe sempre; dmabuf_global so eh criado quando o
    // backend (winit/drm) sobe um GlesRenderer e descobre o render-node
    // + format-set do EGL. Clients GPU (Firefox, Chrome, GPUI) precisam
    // do global; sem ele caem em wl_shm puro ou recusam.
    pub dmabuf_state: DmabufState,
    pub dmabuf_global: Option<DmabufGlobal>,

    /// Handle ao backend winit em Rc<RefCell<...>>. Setado pelo
    /// init de backend::winit; necessario pro DmabufHandler conseguir
    /// importar via GlesRenderer mantido la dentro. None no path DRM.
    pub winit_backend: Option<std::rc::Rc<std::cell::RefCell<
        smithay::backend::winit::WinitGraphicsBackend<
            smithay::backend::renderer::gles::GlesRenderer
        >>>>,

    // Input
    pub seat: Seat<Self>,
    pub keyboard: KeyboardHandle<Self>,
    pub pointer: PointerHandle<Self>,
    pub pointer_location: Point<f64, Logical>,

    // Desktop / window mgmt
    pub space: Space<Window>,
    pub popups: PopupManager,

    // Frame counter pra invalidar SolidColorRenderElements.
    pub frame_counter: u64,

    // Cursor xcursor real (fase 5.4).
    pub cursor: Option<crate::cursor::LoadedCursor>,
    pub cursor_buffer:
        Option<smithay::backend::renderer::element::memory::MemoryRenderBuffer>,

    // Fase 5.5 (A8): IPC + workspaces.
    pub ipc: IpcServer,
    /// Workspace ativo no instante atual. 1..=MAX_WORKSPACES.
    /// Default = 1 no startup.
    pub active_workspace: u8,

    // Fase 5.6 (A9): DRM session + watchdog.
    /// Sessao libseat (so existe no backend DRM). winit deixa None.
    /// Usado por handlers/input.rs pra change_vt (Ctrl+Alt+Fn).
    #[cfg(feature = "drm-backend")]
    pub session: Option<smithay::backend::session::libseat::LibSeatSession>,

    /// Backend data DRM (renderer, output_manager, surface). So no DRM path.
    /// Movido pra state pra que callbacks calloop (page-flip, frame timer)
    /// possam mutar via &mut state sem capturas Rc<RefCell>.
    #[cfg(feature = "drm-backend")]
    pub drm_backend: Option<crate::backend::drm::DrmBackendData>,

    /// Forca repaint imediato no proximo render_drm tick. Setado em
    /// SessionEvent::ActivateSession (volta de VT switch) e em mudanca
    /// de mode/output. Reset apos render.
    #[cfg(feature = "drm-backend")]
    pub drm_force_repaint: bool,

    /// True quando outro VT esta ativo (SessionEvent::PauseSession).
    /// Watchdog ignora paused; render path skip enquanto paused.
    pub paused: bool,

    /// Deadline pra watchdog frame-timeout. None = sem watchdog (winit).
    pub watchdog_deadline: Option<std::time::Instant>,

    /// Exit code do processo. 0 = normal, 2 = watchdog DRM stall.
    pub exit_code: i32,
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
        let output_manager_state =
            OutputManagerState::new_with_xdg_output::<Self>(&display_handle);
        let mut seat_state = SeatState::new();
        let data_device_state = DataDeviceState::new::<Self>(&display_handle);

        let layer_shell_state = WlrLayerShellState::new::<Self>(&display_handle);
        let primary_selection_state = PrimarySelectionState::new::<Self>(&display_handle);
        let xdg_activation_state = XdgActivationState::new::<Self>(&display_handle);
        let fractional_scale_state =
            FractionalScaleManagerState::new::<Self>(&display_handle);
        let cursor_shape_state = CursorShapeManagerState::new::<Self>(&display_handle);
        let xdg_toplevel_icon_manager =
            XdgToplevelIconManager::new::<Self>(&display_handle);

        // DmabufState criado vazio. Global so registrado quando renderer
        // GPU sobe (winit::init OU drm::run).
        let dmabuf_state = DmabufState::new();

        let cursor = crate::cursor::try_load_first_available(24);
        let cursor_buffer = cursor.as_ref().map(|c| {
            use smithay::backend::allocator::Fourcc;
            use smithay::backend::renderer::element::memory::MemoryRenderBuffer;
            use smithay::utils::Transform;
            MemoryRenderBuffer::from_slice(
                &c.pixels,
                Fourcc::Abgr8888,
                (c.width as i32, c.height as i32),
                1,
                Transform::Normal,
                None,
            )
        });

        let mut seat = seat_state.new_wl_seat(&display_handle, "lumo-seat-0");
        let keyboard = seat
            .add_keyboard(Default::default(), 200, 25)
            .expect("falha ao adicionar keyboard");
        let pointer = seat.add_pointer();

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
            layer_shell_state,
            primary_selection_state,
            xdg_activation_state,
            fractional_scale_state,
            cursor_shape_state,
            xdg_toplevel_icon_manager,
            dmabuf_state,
            dmabuf_global: None,
            winit_backend: None,
            seat,
            keyboard,
            pointer,
            pointer_location: (0.0, 0.0).into(),
            space: Space::default(),
            popups: PopupManager::default(),
            frame_counter: 0,
            cursor,
            cursor_buffer,
            ipc: IpcServer::default(),
            active_workspace: 1,
            #[cfg(feature = "drm-backend")]
            session: None,
            #[cfg(feature = "drm-backend")]
            drm_backend: None,
            #[cfg(feature = "drm-backend")]
            drm_force_repaint: false,
            paused: false,
            watchdog_deadline: None,
            exit_code: 0,
        }
    }

    /// Salva sessao libseat no state. So chamado pelo backend DRM.
    /// Necessario pra handlers/input.rs intercept Ctrl+Alt+Fn -> change_vt.
    #[cfg(feature = "drm-backend")]
    pub fn set_session(
        &mut self,
        session: smithay::backend::session::libseat::LibSeatSession,
    ) {
        self.session = Some(session);
    }

    /// Encontra a surface sob a posicao global do ponteiro.
    pub fn surface_under(
        &self,
        pos: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<i32, Logical>)> {
        if let Some((window, win_loc)) = self.space.element_under(pos) {
            let rel = pos - win_loc.to_f64();
            if let Some((surface, surf_off)) =
                window.surface_under(rel, smithay::desktop::WindowSurfaceType::ALL)
            {
                return Some((surface, win_loc + surf_off));
            }
        }
        None
    }

    /// Calcula proxima posicao de tile horizontal. MVP.
    pub fn next_tile_position(&self) -> Point<i32, Logical> {
        let count = self.space.elements().count() as i32;
        ((count * 620).min(1280 - 600), 40).into()
    }

    /// Aplica comando recebido via IPC. Centraliza
    /// dispatch pra facilitar adicionar acoes novas
    /// (LumoCommand variantes).
    pub fn handle_ipc_command(&mut self, cmd: LumoCommand) {
        match cmd {
            LumoCommand::Switch { to } => {
                self.set_workspace(to);
            }
        }
    }

    /// Troca workspace ativo. Validacao + broadcast IPC.
    /// Memory feedback_input_feedback_imediato: aplicar no
    /// proximo frame (state muda; redraw da bar acontece no
    /// proprio compositor + lumo-bar reage ao broadcast).
    pub fn set_workspace(&mut self, to: u8) {
        if !(1..=MAX_WORKSPACES).contains(&to) {
            tracing::warn!(to, "workspace fora do range, ignorado");
            return;
        }
        if to == self.active_workspace {
            return;
        }
        let prev = self.active_workspace;
        self.active_workspace = to;
        tracing::info!(prev, current = to, "switch workspace");
        let ev = IpcServer::workspaces_event(self.active_workspace, MAX_WORKSPACES);
        self.ipc.broadcast(&ev);
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
