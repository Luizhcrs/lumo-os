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

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use smithay::desktop::{layer_map_for_output, PopupManager, Space, Window, WindowSurfaceType};
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
use smithay::wayland::shell::wlr_layer::{Layer as WlrLayer, WlrLayerShellState};
use smithay::wayland::shell::xdg::{ToplevelSurface, XdgShellState};
use smithay::wayland::shell::xdg::decoration::XdgDecorationState;
use smithay::wayland::shm::ShmState;
use smithay::wayland::socket::ListeningSocketSource;
use smithay::wayland::xdg_activation::XdgActivationState;
use smithay::wayland::dmabuf::{DmabufGlobal, DmabufState};
use smithay::wayland::xdg_toplevel_icon::XdgToplevelIconManager;

use lumo_ipc::{LumoCommand, LumoEvent, MAX_WORKSPACES};

use crate::ipc::IpcServer;
use crate::input::keyboard::KeyboardConfig;

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
    pub xdg_decoration_state: Option<XdgDecorationState>,
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

    // B2: keybindings configuracao carregada de TOML.
    pub keyboard_config: KeyboardConfig,
    /// Bug Luiz 2026-05-18 v3: estado caps/num lock sync com /sys/class/leds.
    /// xkb led mapping nao funcionou via SeatHandler — fallback direto.
    pub caps_lock_on: bool,
    pub num_lock_on: bool,


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

    /// A19: wallpaper opcional carregado pelo backend (winit OU drm)
    /// apos o GlesRenderer estar pronto. None = clear color de fundo.
    pub wallpaper: Option<crate::backend::wallpaper::LumoWallpaper>,
    /// A38: programa SDF corner radius. None ate renderer iniciado.
    pub corner_shader: Option<crate::backend::corner_shader::CornerShader>,

    /// L1: focus state machine centralizada.
    pub focus_manager: crate::focus::FocusManager,
    /// M1: surfaces que aceitaram SSD via xdg-decoration protocol.
    pub ssd_windows: HashSet<WlSurface>,
    /// B1: gesture state acumulado (swipe + pinch).
    pub gesture: crate::input::TouchpadGestureState,
    /// L5: lid switch handler state.
    pub lid_handler: std::sync::Arc<std::sync::Mutex<crate::handlers::lid::LidHandlerState>>,

    // A39: boot curtain. Tela preta inicial ate lumo-bar estar mapeada.
    // boot_ready: lumo-bar detectada pelo menos 1x via layer_map.
    // boot_curtain_alpha: 1.0 inicial; decrementa com delta real (4.0/s) apos ready.
    // boot_last_tick: timestamp do ultimo frame para calculo de dt.
    pub boot_ready: bool,
    pub boot_curtain_alpha: f32,
    pub boot_last_tick: std::time::Instant,
    /// R1: posicao do cursor no ultimo frame renderizado. Bypass pending_flip
    /// quando cursor se moveu pra eliminar delay visual.
    #[cfg(feature = "drm-backend")]
    pub last_rendered_cursor_pos: smithay::utils::Point<f64, smithay::utils::Logical>,
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
            xdg_decoration_state: None,
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
            pointer_location: (960.0, 540.0).into(), // centro 1920x1080, ajustado dinamico ao detectar output real
            space: Space::default(),
            popups: PopupManager::default(),
            frame_counter: 0,
            cursor,
            cursor_buffer,
            ipc: IpcServer::default(),
            active_workspace: 1,
            keyboard_config: KeyboardConfig::load(),
            caps_lock_on: false,
            num_lock_on: false,
            #[cfg(feature = "drm-backend")]
            session: None,
            #[cfg(feature = "drm-backend")]
            drm_backend: None,
            #[cfg(feature = "drm-backend")]
            drm_force_repaint: false,
            paused: false,
            watchdog_deadline: None,
            exit_code: 0,
            wallpaper: None,
            corner_shader: None,
            focus_manager: Default::default(),
            ssd_windows: HashSet::new(),
            gesture: Default::default(),
            lid_handler: std::sync::Arc::new(std::sync::Mutex::new(Default::default())),
            boot_ready: false,
            boot_curtain_alpha: 1.0,
            boot_last_tick: std::time::Instant::now(),
            #[cfg(feature = "drm-backend")]
            last_rendered_cursor_pos: (0.0, 0.0).into(),
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
        let trace = std::env::var("LUMO_TRACE_POINTER").is_ok();
        if trace { eprintln!("[trace] surface_under pos=({:.1},{:.1})", pos.x, pos.y); }
        // A20.2: layer-shell PRIMEIRO (bar/dock/notif).
        // Z-order: Overlay > Top > Window > Bottom > Background.
        let outputs: Vec<_> = self.space.outputs().cloned().collect();
        for output in outputs.iter() {
            let map = layer_map_for_output(output);
            for layer in map.layers_on(WlrLayer::Overlay).chain(map.layers_on(WlrLayer::Top)) {
                let geo = map.layer_geometry(layer).unwrap_or_default();
                if trace { eprintln!("[trace] layer Top geo={:?} contains={}", geo, geo.to_f64().contains(pos)); }
                if geo.to_f64().contains(pos) {
                    let rel = pos - geo.loc.to_f64();
                    if let Some((surface, surf_off)) =
                        layer.surface_under(rel, WindowSurfaceType::ALL)
                    {
                        if trace { eprintln!("[trace] FOUND layer namespace={:?} surface_alive={}", layer.namespace(), true); }
                        return Some((surface, geo.loc + surf_off));
                    } else if trace {
                        eprintln!("[trace] layer Top contains pos mas surface_under retornou None namespace={:?}", layer.namespace());
                    }
                }
            }
        }
        // Toplevels
        if let Some((window, win_loc)) = self.space.element_under(pos) {
            let rel = pos - win_loc.to_f64();
            if let Some((surface, surf_off)) =
                window.surface_under(rel, WindowSurfaceType::ALL)
            {
                return Some((surface, win_loc + surf_off));
            }
        }
        // Layer-shell Bottom/Background (atras dos toplevels)
        for output in outputs.iter() {
            let map = layer_map_for_output(output);
            for layer in map.layers_on(WlrLayer::Bottom).chain(map.layers_on(WlrLayer::Background)) {
                let geo = map.layer_geometry(layer).unwrap_or_default();
                if geo.to_f64().contains(pos) {
                    let rel = pos - geo.loc.to_f64();
                    if let Some((surface, surf_off)) =
                        layer.surface_under(rel, WindowSurfaceType::ALL)
                    {
                        return Some((surface, geo.loc + surf_off));
                    }
                }
            }
        }
        None
    }

    /// Calcula posicao pra nova janela. Estrategia: cursor center se nao
    /// colide com bar/desktop, fallback centro tela. Mac/Windows-like.
    pub fn next_tile_position(&self) -> Point<i32, Logical> {
        // Assume 1920x1080 output (Galaxy Book 4 nativo).
        // Bar topo = 32px. Janela default ~800x600. Centro logico (560, 240).
        const OUT_W: i32 = 1920;
        const OUT_H: i32 = 1080;
        const DEFAULT_W: i32 = 800;
        const DEFAULT_H: i32 = 600;
        const BAR_H: i32 = 40;
        let cx = self.pointer_location.x as i32;
        let cy = self.pointer_location.y as i32;
        // Posiciona window CENTRADA no cursor.
        let mut x = cx - DEFAULT_W / 2;
        let mut y = cy - DEFAULT_H / 2;
        // Clamp dentro output, respeita bar topo.
        x = x.clamp(8, OUT_W - DEFAULT_W - 8);
        y = y.clamp(BAR_H + 8, OUT_H - DEFAULT_H - 8);
        (x, y).into()
    }

    /// Aplica comando recebido via IPC. Centraliza
    /// dispatch pra facilitar adicionar acoes novas
    /// (LumoCommand variantes).
    pub fn handle_ipc_command(&mut self, cmd: LumoCommand) {
        match cmd {
            LumoCommand::Switch { to } => {
                self.set_workspace(to);
            }
            LumoCommand::CloseDropdowns => {
                // A21: compositor nao tem dropdown proprio; so propaga pra
                // clients (lumo-bar) decidirem o que fechar. Idempotente —
                // clients sem dropdown ativo ignoram.
                self.ipc.broadcast(&LumoEvent::CloseDropdowns);
            }
            LumoCommand::CloseDesktopMenu => {
                // A26: mutex de popups. Bar abriu dropdown -> pede pra
                // lumo-desktop fechar menu contextual (e vice-versa).
                // Idempotente — clients sem menu ativo ignoram.
                self.ipc.broadcast(&LumoEvent::CloseDesktopMenu);
            }
            LumoCommand::ReloadTheme => {
                // L6: lumoctl pediu reload de theme.toml.
                // Le tokens atualizados e broadcast ThemeReloaded pros clients.
                let tokens = lumo_foundation::LumoTokens::load_from_disk();
                let mode = match tokens.mode {
                    lumo_foundation::LumoTheme::Light => lumo_ipc::ThemeMode::Light,
                    lumo_foundation::LumoTheme::Dark => lumo_ipc::ThemeMode::Dark,
                };
                tracing::info!(?mode, "L6: ThemeReloaded broadcast");
                self.ipc.broadcast(&LumoEvent::ThemeReloaded { mode });
            }
        }
    }

    /// A39: retorna true quando lumo-bar esta mapeada no layer_map.
    /// Detecta pelo namespace lumo-bar em qualquer output e qualquer layer.
    pub fn boot_clients_ready(&self) -> bool {
        for output in self.space.outputs() {
            let map = layer_map_for_output(output);
            for layer in map.layers() {
                if layer.namespace() == "lumo-bar" {
                    return true;
                }
            }
        }
        false
    }

    /// A39: broadcast DesktopOpenSelected pra lumo-desktop abrir
    /// o icone selecionado via xdg-open.
    pub fn broadcast_desktop_open_selected(&mut self) {
        self.ipc.broadcast(&LumoEvent::DesktopOpenSelected);
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

use smithay::wayland::shell::xdg::decoration::XdgDecorationHandler;
use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode;

impl XdgDecorationHandler for LumoState {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(Mode::ServerSide);
        });
        self.ssd_windows.insert(toplevel.wl_surface().clone());
        toplevel.send_configure();
        tracing::debug!("xdg_decoration: new_decoration -> ServerSide");
    }

    fn request_mode(&mut self, toplevel: ToplevelSurface, _mode: Mode) {
        // P0: forca ServerSide sempre. Apps que ignoram (GTK4) continuam CSD client-side.
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(Mode::ServerSide);
        });
        toplevel.send_configure();
        let surf = toplevel.wl_surface().clone();
        self.ssd_windows.insert(surf);
        tracing::debug!("xdg_decoration: request_mode -> forced ServerSide");
    }

    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(Mode::ServerSide);
        });
        self.ssd_windows.insert(toplevel.wl_surface().clone());
        toplevel.send_configure();
        tracing::debug!("xdg_decoration: unset_mode -> ServerSide");
    }
}


smithay::delegate_xdg_decoration!(LumoState);

/// Inicializa o global xdg-decoration-unstable-v1 no compositor.
/// Deve ser chamado apos LumoState::new() pra que as delegate impls
/// geradas em handlers::xdg_decoration estejam em scope.
pub fn init_xdg_decoration(state: &mut LumoState) {
    use smithay::wayland::shell::xdg::decoration::XdgDecorationState;
    state.xdg_decoration_state = Some(XdgDecorationState::new::<LumoState>(&state.display_handle));
    tracing::info!("M1: xdg_decoration global registrado");
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
