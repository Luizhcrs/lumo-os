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
use smithay::input::pointer::CursorIcon;
use smithay::input::pointer::PointerHandle;
use smithay::input::{Seat, SeatState};
use smithay::reexports::calloop::LoopHandle;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::utils::{Clock, Logical, Monotonic, Point};
use smithay::wayland::compositor::{CompositorClientState, CompositorState};
use smithay::wayland::cursor_shape::CursorShapeManagerState;
use smithay::wayland::dmabuf::{DmabufGlobal, DmabufState};
use smithay::wayland::fractional_scale::FractionalScaleManagerState;
use smithay::wayland::idle_notify::IdleNotifierState;
use smithay::wayland::output::OutputManagerState;
use smithay::wayland::selection::data_device::DataDeviceState;
use smithay::wayland::selection::primary_selection::PrimarySelectionState;
use smithay::wayland::shell::wlr_layer::{Layer as WlrLayer, WlrLayerShellState};
use smithay::wayland::shell::xdg::decoration::XdgDecorationState;
use smithay::wayland::shell::xdg::{ToplevelSurface, XdgShellState};
use smithay::wayland::shm::ShmState;
use smithay::wayland::socket::ListeningSocketSource;
use smithay::wayland::xdg_activation::XdgActivationState;
use smithay::wayland::xdg_toplevel_icon::XdgToplevelIconManager;

use lumo_ipc::{LumoCommand, LumoEvent, MAX_WORKSPACES};

use crate::handlers::color_management::ColorManagerState;
use crate::handlers::idle::LumoIdleManager;
use crate::handlers::screencopy::ScreencopyState;
use crate::input::keyboard::KeyboardConfig;
use crate::ipc::IpcServer;
use crate::workspace::{WorkspaceTransition, WorkspaceVault};
use smithay::wayland::commit_timing::CommitTimingManagerState;
use smithay::wayland::fifo::FifoManagerState;

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
    pub xdg_toplevel_icon_manager: Option<XdgToplevelIconManager>,

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
    pub winit_backend: Option<
        std::rc::Rc<
            std::cell::RefCell<
                smithay::backend::winit::WinitGraphicsBackend<
                    smithay::backend::renderer::gles::GlesRenderer,
                >,
            >,
        >,
    >,

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
    pub cursor_buffer: Option<smithay::backend::renderer::element::memory::MemoryRenderBuffer>,

    // Fase 5.5 (A8): IPC + workspaces.
    pub ipc: IpcServer,
    /// Workspace ativo no instante atual. 1..=MAX_WORKSPACES.
    /// Default = 1 no startup.
    pub active_workspace: u8,
    /// W34.4: ultimo ActiveApp broadcast pra re-enviar ao bar quando reconecta.
    pub last_active_app: Option<(String, String, u32)>,
    /// W34.13: cache pid -> (app_id, title) populado por AppActivated.
    /// focus_changed resolve app_id vazio (Iced lifecycle bug) via lookup.
    pub pid_app_cache: std::collections::HashMap<u32, (String, String)>,

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

    /// W22: damage-gating render. True quando ha mudanca de estado que
    /// exige render (commit surface, cursor move, anim tick, focus change).
    /// Frame timer skip render quando false = CPU idle <1%.
    /// Reset apos render. Sempre true se drm_force_repaint=true.
    pub should_render: bool,

    /// W22: contador de skip render consecutivos (debug).
    pub skipped_frames: u64,

    /// True quando outro VT esta ativo (SessionEvent::PauseSession).
    /// Watchdog ignora paused; render path skip enquanto paused.
    pub paused: bool,

    /// Deadline pra watchdog frame-timeout. None = sem watchdog (winit).
    pub watchdog_deadline: Option<std::time::Instant>,

    /// Exit code do processo. 0 = normal, 2 = watchdog DRM stall.
    pub exit_code: i32,

    // W8.A: screencopy global state.
    pub screencopy: Option<ScreencopyState>,
    // W13.A: color management global.
    pub color_manager: Option<ColorManagerState>,
    // W13.C: wp-fifo-v1 + wp-commit-timing-v1.
    pub fifo_manager_state: FifoManagerState,
    pub commit_timing_manager_state: CommitTimingManagerState,
    // W8.B: workspace vault (windows ocultas por workspace).
    pub workspace_vault: WorkspaceVault,
    // W8.B: animacao ativa de troca de workspace (None quando idle).
    pub workspace_transition: Option<WorkspaceTransition>,

    /// A19: wallpaper opcional carregado pelo backend (winit OU drm)
    /// apos o GlesRenderer estar pronto. None = clear color de fundo.
    pub wallpaper: Option<crate::backend::wallpaper::LumoWallpaper>,
    /// A38: programa SDF corner radius. None ate renderer iniciado.
    pub corner_shader: Option<crate::backend::corner_shader::CornerShader>,

    /// L1: focus state machine centralizada.
    pub focus_manager: crate::focus::FocusManager,
    /// M1: surfaces que aceitaram SSD via xdg-decoration protocol.
    pub ssd_windows: HashSet<WlSurface>,
    /// T1.1: menu popup de titlebar SSD ativo.
    /// None = sem menu. Some((window, cursor_pos, hover_idx)).
    pub titlebar_menu: Option<(
        smithay::desktop::Window,
        smithay::utils::Point<i32, smithay::utils::Logical>,
        usize,
    )>,
    /// B1: gesture state acumulado (swipe + pinch).
    pub gesture: crate::input::TouchpadGestureState,
    /// W9.A: per-window open/close spring animation registry.
    pub window_anim: crate::window_anim::WindowAnimRegistry,
    /// W9.B: active snap zone preview during window drag. None = no preview.
    pub snap_preview: Option<crate::input::move_grab::SnapZone>,
    // W12.A: tiling layout mode. Default = Floating.
    pub tiling_mode: crate::tiling::TilingMode,
    // W12.B: mission control overview. None = inactive.
    pub overview: Option<crate::overview::OverviewState>,
    // W12.C: window stack picker (SUPER+TAB visual). None = inactive.
    pub stack_picker: Option<crate::stack_picker::StackPickerState>,
    /// L5: lid switch handler state.
    pub lid_handler: std::sync::Arc<std::sync::Mutex<crate::handlers::lid::LidHandlerState>>,

    // W10.B: idle management state.
    pub idle_manager: LumoIdleManager,
    pub idle_notifier_state: IdleNotifierState<Self>,

    // W10.C: active cursor icon requested via wp-cursor-shape-v1.
    // Default is CursorIcon::Default (arrow). Swapped when client requests shape.
    pub active_cursor_icon: CursorIcon,

    // A39: boot curtain. Tela preta inicial ate lumo-bar estar mapeada.
    // boot_ready: lumo-bar detectada pelo menos 1x via layer_map.
    // boot_curtain_alpha: 1.0 inicial; decrementa com delta real (4.0/s) apos ready.
    // boot_last_tick: timestamp do ultimo frame para calculo de dt.
    pub boot_ready: bool,
    pub boot_curtain_alpha: f32,
    pub boot_last_tick: std::time::Instant,
    // W6.D: perf tracker.
    pub perf: crate::perf::PerfTracker,
    // W6.C: splash boot logo.
    // splash_alpha: 1.0 (fade-in) -> 1.0 (hold) -> 0.0 (fade-out).
    // splash_phase: 0=fade-in, 1=hold, 2=fade-out, 3=done.
    pub splash_alpha: f32,
    pub splash_phase: u8,
    pub splash_timer: f32,
    pub splash_buffer: Option<smithay::backend::renderer::element::memory::MemoryRenderBuffer>,
    /// W28: 12x12 pixmap top-left corner round + 3 quadrants squared. Flipped on right side.
    pub corner_mask_shader: Option<crate::backend::corner_shader::CornerMaskShader>,
    pub titlebar_bg_shader: Option<crate::backend::corner_shader::TitlebarBgShader>,
    /// Telemetry: timestamp of last pointer button press for input-to-paint measurement.
    pub last_input_ts: Option<std::time::Instant>,
    /// W23.5: timestamp ultimo PointerMotion/Button. Sticky 200ms = active mode.
    pub cursor_last_motion_ts: Option<std::time::Instant>,
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
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&display_handle);
        let mut seat_state = SeatState::new();
        let data_device_state = DataDeviceState::new::<Self>(&display_handle);

        let layer_shell_state = WlrLayerShellState::new::<Self>(&display_handle);
        let primary_selection_state = PrimarySelectionState::new::<Self>(&display_handle);
        let xdg_activation_state = XdgActivationState::new::<Self>(&display_handle);
        let fractional_scale_state = FractionalScaleManagerState::new::<Self>(&display_handle);
        let cursor_shape_state = CursorShapeManagerState::new::<Self>(&display_handle);
        // W37.18: xdg_toplevel_icon_manager_v1 DESABILITADO por default.
        // Smithay 0.7.0 tem bug em register_buffer_destruction_hook que NAO
        // desregistra ao destruir icon. Chromium sequencia (icon.destroy ->
        // buffer.destroy) e SPEC-COMPLIANT mas smithay emite protocol error
        // "buffer destroyed before icon" -> Chromium fecha conexao = broken
        // pipe. Sem global, Chromium pula icon support gracefully.
        let xdg_toplevel_icon_manager: Option<XdgToplevelIconManager> = if should_enable_toplevel_icon_manager(
            std::env::var("LUMO_ENABLE_TOPLEVEL_ICON").ok().as_deref(),
        ) {
            Some(XdgToplevelIconManager::new::<Self>(&display_handle))
        } else {
            None
        };

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

        let corner_mask_shader: Option<crate::backend::corner_shader::CornerMaskShader> = None;
        let titlebar_bg_shader: Option<crate::backend::corner_shader::TitlebarBgShader> = None;

        let mut seat = seat_state.new_wl_seat(&display_handle, "lumo-seat-0");
        let keyboard = seat
            .add_keyboard(Default::default(), 200, 25)
            .expect("falha ao adicionar keyboard");
        let pointer = seat.add_pointer();
        // W10.B: must create before display_handle moves into Self.
        let idle_notifier_state = IdleNotifierState::new(&display_handle, loop_handle.clone());
        // W13.A: color management global.
        // W37.15: DESABILITADO por default ate fix completo do bug Chromium
        // broken pipe pos info events. Apps Lumo nao usam color_management.
        // Reativar via env LUMO_ENABLE_COLOR_MGMT=1 quando bug resolvido.
        let color_manager =
            if should_enable_color_manager(std::env::var("LUMO_ENABLE_COLOR_MGMT").ok().as_deref()) {
                tracing::info!("W37.15: wp_color_manager_v1 habilitado via env");
                Some(ColorManagerState::new(&display_handle))
            } else {
                tracing::info!(
                    "W37.15: wp_color_manager_v1 desabilitado (default, workaround Chromium)"
                );
                None
            };
        // W13.C: fifo + commit-timing.
        let fifo_manager_state = FifoManagerState::new::<Self>(&display_handle);
        let commit_timing_manager_state = CommitTimingManagerState::new::<Self>(&display_handle);

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
            last_active_app: None,
            pid_app_cache: std::collections::HashMap::new(),
            screencopy: None,
            color_manager,
            fifo_manager_state,
            commit_timing_manager_state,
            workspace_vault: WorkspaceVault::new(),
            workspace_transition: None,
            keyboard_config: KeyboardConfig::load(),
            caps_lock_on: false,
            num_lock_on: false,
            #[cfg(feature = "drm-backend")]
            session: None,
            #[cfg(feature = "drm-backend")]
            drm_backend: None,
            #[cfg(feature = "drm-backend")]
            drm_force_repaint: false,
            should_render: true,
            skipped_frames: 0,
            paused: false,
            watchdog_deadline: None,
            exit_code: 0,
            wallpaper: None,
            corner_shader: None,
            focus_manager: Default::default(),
            ssd_windows: HashSet::new(),
            titlebar_menu: None,
            gesture: Default::default(),
            window_anim: crate::window_anim::WindowAnimRegistry::new(),
            snap_preview: None,
            tiling_mode: crate::tiling::TilingMode::Floating,
            overview: None,
            stack_picker: None,
            lid_handler: std::sync::Arc::new(std::sync::Mutex::new(Default::default())),
            idle_manager: LumoIdleManager::new(),
            idle_notifier_state,
            active_cursor_icon: CursorIcon::Default,
            boot_ready: false,
            boot_curtain_alpha: 1.0,
            boot_last_tick: std::time::Instant::now(),
            perf: crate::perf::PerfTracker::new(),
            splash_alpha: 0.0,
            splash_phase: 0,
            splash_timer: 0.0,
            splash_buffer: crate::backend::wallpaper::load_splash_buffer(),
            corner_mask_shader,
            titlebar_bg_shader,
            last_input_ts: None,
            cursor_last_motion_ts: None,
            #[cfg(feature = "drm-backend")]
            last_rendered_cursor_pos: (0.0, 0.0).into(),
        }
    }

    /// Salva sessao libseat no state. So chamado pelo backend DRM.
    /// Necessario pra handlers/input.rs intercept Ctrl+Alt+Fn -> change_vt.
    #[cfg(feature = "drm-backend")]
    pub fn set_session(&mut self, session: smithay::backend::session::libseat::LibSeatSession) {
        self.session = Some(session);
    }

    /// Encontra a surface sob a posicao global do ponteiro.
    pub fn surface_under(
        &self,
        pos: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<i32, Logical>)> {
        let trace = std::env::var("LUMO_TRACE_POINTER").as_deref() == Ok("1");
        if trace {
            eprintln!("[trace] surface_under pos=({:.1},{:.1})", pos.x, pos.y);
        }
        // A20.2: layer-shell PRIMEIRO (bar/dock/notif).
        // Z-order: Overlay > Top > Window > Bottom > Background.
        let outputs: Vec<_> = self.space.outputs().cloned().collect();
        for output in outputs.iter() {
            let map = layer_map_for_output(output);
            for layer in map
                .layers_on(WlrLayer::Overlay)
                .chain(map.layers_on(WlrLayer::Top))
            {
                let geo = map.layer_geometry(layer).unwrap_or_default();
                if trace {
                    eprintln!(
                        "[trace] layer Top geo={:?} contains={}",
                        geo,
                        geo.to_f64().contains(pos)
                    );
                }
                if geo.to_f64().contains(pos) {
                    let rel = pos - geo.loc.to_f64();
                    if let Some((surface, surf_off)) =
                        layer.surface_under(rel, WindowSurfaceType::ALL)
                    {
                        // INSTR.D: log input_region status quando layer surface eh hit.
                        let region_status =
                            smithay::wayland::compositor::with_states(&surface, |states| {
                                let mut cs = states
                                    .cached_state
                                    .get::<smithay::wayland::compositor::SurfaceAttributes>(
                                );
                                cs.current().input_region.is_some()
                            });
                        tracing::info!(
                            namespace = %layer.namespace(),
                            pos = ?(pos.x as i32, pos.y as i32),
                            rel = ?(rel.x as i32, rel.y as i32),
                            input_region_set = region_status,
                            "INSTR.D layer surface_under hit"
                        );
                        if trace {
                            eprintln!(
                                "[trace] FOUND layer namespace={:?} surface_alive={}",
                                layer.namespace(),
                                true
                            );
                        }
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
            // INSTR.E (W19): log toplevel hit + app_id + surface delivery,
            // pra investigar pq clicks em apps Iced (Y > bar) nao registram.
            use smithay::wayland::seat::WaylandFocus;
            use smithay::wayland::shell::xdg::XdgToplevelSurfaceData;
            let app_id_for_log: String = window
                .wl_surface()
                .and_then(|s| {
                    smithay::wayland::compositor::with_states(&s, |states| {
                        states
                            .data_map
                            .get::<XdgToplevelSurfaceData>()
                            .and_then(|d| d.lock().ok().and_then(|g| g.app_id.clone()))
                    })
                })
                .unwrap_or_else(|| String::from("<no-app-id>"));
            let hit = window.surface_under(rel, WindowSurfaceType::ALL);
            tracing::info!(
                pos = ?(pos.x as i32, pos.y as i32),
                rel = ?(rel.x as i32, rel.y as i32),
                win_loc = ?(win_loc.x, win_loc.y),
                app_id = %app_id_for_log,
                surface_hit = hit.is_some(),
                "INSTR.E toplevel surface_under"
            );
            if let Some((surface, surf_off)) = hit {
                return Some((surface, win_loc + surf_off));
            }
        }
        // Layer-shell Bottom/Background (atras dos toplevels)
        for output in outputs.iter() {
            let map = layer_map_for_output(output);
            for layer in map
                .layers_on(WlrLayer::Bottom)
                .chain(map.layers_on(WlrLayer::Background))
            {
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
    /// W24: rect onde apps xdg-shell podem mapear/mover.
    /// Excludes layer-shell exclusive zones (bar Top, dock Bottom, etc).
    /// Fallback (1920x1080) se sem output.
    pub fn usable_geometry(&self) -> smithay::utils::Rectangle<i32, smithay::utils::Logical> {
        use smithay::desktop::layer_map_for_output;
        if let Some(output) = self.space.outputs().next() {
            let map = layer_map_for_output(output);
            let zone = map.non_exclusive_zone();
            // W24.5: Garante que a zona util nao seja negativa ou absurda.
            smithay::utils::Rectangle::new(
                zone.loc,
                smithay::utils::Size::from((
                    zone.size.w.clamp(64, 4096),
                    zone.size.h.clamp(64, 4096),
                )),
            )
        } else {
            smithay::utils::Rectangle::new(
                smithay::utils::Point::from((0, 0)),
                smithay::utils::Size::from((1920, 1080)),
            )
        }
    }

    pub fn next_tile_position(&self) -> Point<i32, Logical> {
        // W32: Janelas novas abrem CENTRADAS na tela (estilo Windows/macOS).
        // Resoluçao fixa padrao: 1024x768.
        const FIXED_W: i32 = 1024;
        const FIXED_H: i32 = 768;
        const SSD_TITLEBAR_H: i32 = 30;
        
        let usable = self.usable_geometry();
        
        // Calcula o centro da area util
        let cx = usable.loc.x + (usable.size.w / 2);
        let cy = usable.loc.y + (usable.size.h / 2);
        
        // Posicao top-left para que o centro da janela coincida com o centro da tela
        let mut x = cx - (FIXED_W / 2);
        let mut y = cy - (FIXED_H / 2);
        
        // W24.5: Protecao de bordas (garante que nao abra fora da tela se o monitor for pequeno)
        let min_x = usable.loc.x + 8;
        let max_x = (usable.loc.x + usable.size.w - FIXED_W - 8).max(min_x);
        x = x.clamp(min_x, max_x.max(min_x));
        
        let min_y = usable.loc.y + SSD_TITLEBAR_H + 8;
        let max_y = (usable.loc.y + usable.size.h - FIXED_H - 8).max(min_y);
        y = y.clamp(min_y, max_y.max(min_y));
        
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
            LumoCommand::CloseFocusedToplevel => {
                eprintln!("[wm] CloseFocusedToplevel recv");
                // W32.6: snap close instantaneo (igual btn X).
                // W34.20: fallback last xdg_toplevel se kb sem focus (click via bar layer-shell
                // pode shiftar keyboard focus pra bar surface, quebrando current_focus()).
                use smithay::wayland::seat::WaylandFocus;
                let kb = self.keyboard.clone();
                let win = if let Some(focused) = kb.current_focus() {
                    self.space
                        .elements()
                        .find(|w| w.wl_surface().map(|s| *s == focused).unwrap_or(false))
                        .cloned()
                } else {
                    // Sem keyboard focus: pega top-most toplevel mapeado.
                    self.space.elements().last().cloned()
                };
                if let Some(w) = win {
                    eprintln!("[wm] CloseFocusedToplevel -> close window");
                    if let Some(tl) = w.toplevel() {
                        tl.send_close();
                    }
                    if let Some(s) = w.wl_surface() {
                        self.ssd_windows.remove(&*s);
                    }
                    self.space.unmap_elem(&w);
                    self.should_render = true;
                } else {
                    eprintln!("[wm] CloseFocusedToplevel: nenhuma janela pra fechar");
                }
            }
            LumoCommand::SyntheticPointerMove { x, y } => {
                self.handle_synthetic_pointer_move(x, y);
            }
            LumoCommand::SyntheticPointerButton { button, pressed } => {
                self.handle_synthetic_pointer_button(button, pressed);
            }
            LumoCommand::SyntheticPointerScroll { dx, dy } => {
                self.handle_synthetic_pointer_scroll(dx, dy);
            }
            LumoCommand::SyntheticKey { keycode, pressed } => {
                self.handle_synthetic_key(keycode, pressed);
            }
            LumoCommand::SyntheticKeyCombo { keys } => {
                self.handle_synthetic_key_combo(&keys);
            }
            LumoCommand::ToggleMaximize => {
                // W17.1: toggle fullscreen no toplevel focado.
                use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State as XdgState;
                use smithay::wayland::seat::WaylandFocus;
                let kb = self.keyboard.clone();
                if let Some(focused) = kb.current_focus() {
                    let win = self
                        .space
                        .elements()
                        .find(|w| w.wl_surface().map(|s| *s == focused).unwrap_or(false))
                        .cloned();
                    if let Some(w) = win {
                        if let Some(tl) = w.toplevel() {
                            let is_fs = tl.current_state().states.contains(XdgState::Fullscreen);
                            tl.with_pending_state(|st| {
                                if is_fs {
                                    st.states.unset(XdgState::Fullscreen);
                                } else {
                                    st.states.set(XdgState::Fullscreen);
                                }
                            });
                            tl.send_configure();
                            tracing::info!(was_fs = is_fs, "W17.1: ToggleMaximize IPC");
                        }
                    }
                }
            }
            LumoCommand::MinimizeFocused => {
                // W17.1: stub. Sem iconify protocol Wayland estavel; loga apenas.
                tracing::info!("W17.1: MinimizeFocused IPC (stub, no protocol)");
            }
            LumoCommand::AppActivated { app_id, title, pid } => {
                // W34.10: lumo-appsd notificou abertura de app Lumo. Iced 0.13 nao
                // propaga xdg_toplevel.set_app_id a tempo do focus_changed; este
                // path bypassa e seta cache + broadcast pro bar render pills.
                tracing::info!(%app_id, %title, pid, "W34.10: AppActivated IPC");
                eprintln!(
                    "[wm] W34.10 AppActivated app_id={:?} title={:?} pid={}",
                    app_id, title, pid
                );
                if !app_id.is_empty() {
                    self.last_active_app = Some((app_id.clone(), title.clone(), pid));
                    // W34.13: cache pid -> app_id pra resolver focus_changed empty later.
                    self.pid_app_cache
                        .insert(pid, (app_id.clone(), title.clone()));
                }
                self.ipc
                    .broadcast(&LumoEvent::ActiveApp { app_id, title, pid });
            }
            LumoCommand::AppDeactivated => {
                // W34.11: appsd fechou todas janelas. Bar limpa pills.
                tracing::info!("W34.11: AppDeactivated IPC");
                eprintln!("[wm] W34.11 AppDeactivated -> clear");
                self.last_active_app = None;
                self.ipc.broadcast(&LumoEvent::ActiveAppCleared);
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

    /// D2: retorna true se a posicao esta sobre a surface da bar (namespace lumo-bar).
    /// Usado para evitar broadcast CloseDropdowns quando click e dentro da bar.
    pub fn pos_is_on_bar(&self, pos: smithay::utils::Point<f64, smithay::utils::Logical>) -> bool {
        for output in self.space.outputs() {
            let map = layer_map_for_output(output);
            for layer in map.layers() {
                if layer.namespace() == "lumo-bar" {
                    let geo = map.layer_geometry(layer).unwrap_or_default();
                    if geo.to_f64().contains(pos) {
                        return true;
                    }
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

        // W8.B: oculta toplevels do workspace atual movendo pro vault.
        // reduced_motion: duracao 0 (instant); normal: 250ms slide.
        // W8.C: reduced_motion=true -> duracao 0 (instant).
        let a11y = lumo_foundation::A11yTokens::load_from_disk();
        let duration = if a11y.reduced_motion { 0.0f32 } else { 0.25f32 };
        use crate::workspace::WindowEntry;
        let current_windows: Vec<WindowEntry> = self
            .space
            .elements()
            .map(|w| {
                let pos = self.space.element_location(w).unwrap_or_default();
                WindowEntry {
                    window: w.clone(),
                    cached_pos: pos,
                }
            })
            .collect();
        for entry in &current_windows {
            self.space.unmap_elem(&entry.window);
        }
        self.workspace_vault.hide_workspace(prev, current_windows);

        // Restaura toplevels do workspace destino.
        let to_restore = self.workspace_vault.show_workspace(to);
        for entry in to_restore {
            self.space
                .map_element(entry.window, entry.cached_pos, false);
        }

        self.active_workspace = to;
        tracing::info!(prev, current = to, "switch workspace W8.B");

        // Inicia animacao de slide (W8.B).
        self.workspace_transition = Some(crate::workspace::WorkspaceTransition::new(
            prev, to, duration,
        ));

        let ev = IpcServer::workspaces_event(self.active_workspace, MAX_WORKSPACES);
        self.ipc.broadcast(&ev);
    }

    /// W12.A: returns output dimensions (w, h) in logical pixels.
    /// Falls back to 1920x1080 if no output is registered.
    pub fn output_dimensions(&self) -> (i32, i32) {
        self.space
            .outputs()
            .next()
            .and_then(|o| {
                let mode = o.current_mode()?;
                Some((mode.size.w, mode.size.h))
            })
            .unwrap_or((1920, 1080))
    }

    /// W8.B: move toplevel focado para workspace `to`.
    pub fn move_focused_to_workspace(&mut self, to: u8) {
        use smithay::wayland::seat::WaylandFocus;
        if !(1..=MAX_WORKSPACES).contains(&to) {
            return;
        }
        let kb = self.keyboard.clone();
        let focused_surf = kb.current_focus();
        let window = focused_surf.and_then(|s| {
            self.space
                .elements()
                .find(|w| w.wl_surface().map(|ws| *ws == s).unwrap_or(false))
                .cloned()
        });
        let window = match window {
            Some(w) => w,
            None => return,
        };
        let pos = self.space.element_location(&window).unwrap_or_default();
        self.space.unmap_elem(&window);
        use crate::workspace::WindowEntry;
        let entry = WindowEntry {
            window,
            cached_pos: pos,
        };
        self.workspace_vault
            .vault
            .entry(to)
            .or_default()
            .push(entry);
        tracing::info!(to, "W8.B: toplevel movido para workspace");
    }
}

use smithay::wayland::shell::xdg::decoration::XdgDecorationHandler;
use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode;

/// W37.8: decisao pura - dado mode pedido pelo cliente, retorna mode escolhido.
/// ClientSide respeita; default ServerSide.
pub fn decide_decoration_mode(requested: Mode) -> Mode {
    match requested {
        Mode::ClientSide => Mode::ClientSide,
        _ => Mode::ServerSide,
    }
}

impl XdgDecorationHandler for LumoState {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        // W37.8: default ServerSide. Cliente pode pedir ClientSide via request_mode.
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(Mode::ServerSide);
        });
        self.ssd_windows.insert(toplevel.wl_surface().clone());
        toplevel.send_configure();
        tracing::debug!("xdg_decoration: new_decoration -> ServerSide (default)");
    }

    fn request_mode(&mut self, toplevel: ToplevelSurface, mode: Mode) {
        // W37.8: respeita pedido do cliente para evitar 2 titlebars empilhadas
        // quando GTK3/Xfce4 (Mousepad) insistem em CSD mesmo com ServerSide
        // setado. Antes: forcava ServerSide sempre + cliente desenhava CSD
        // mesmo assim = 2 titlebars.
        let chosen = decide_decoration_mode(mode);
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(chosen);
        });
        toplevel.send_configure();
        let surf = toplevel.wl_surface().clone();
        if chosen == Mode::ServerSide {
            self.ssd_windows.insert(surf);
        } else {
            self.ssd_windows.remove(&surf);
        }
        tracing::debug!(
            "xdg_decoration: request_mode {:?} -> chosen {:?}",
            mode,
            chosen
        );
    }

    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        // Cliente "desliga" preferencia -> volta pro default ServerSide.
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(Mode::ServerSide);
        });
        self.ssd_windows.insert(toplevel.wl_surface().clone());
        toplevel.send_configure();
        tracing::debug!("xdg_decoration: unset_mode -> ServerSide");
    }
}

smithay::delegate_xdg_decoration!(LumoState);

// W37.11: protocols modernos pra apps Chromium/Kate/Firefox.
// Deps: wp_viewporter (HiDPI surface scaling),
// wp_single_pixel_buffer_v1 (Chromium 113+ hard requires),
// wp_presentation_time (vsync/frame callbacks).
smithay::delegate_viewporter!(LumoState);
smithay::delegate_single_pixel_buffer!(LumoState);
smithay::delegate_presentation!(LumoState);

/// W37.18: helper testavel - decide se xdg_toplevel_icon_manager_v1 sera
/// criado. Default: NAO (workaround bug smithay 0.7.0). Opt-in via env
/// aceita apenas valores truthy ("1", "true", "yes" case-insensitive).
pub fn should_enable_toplevel_icon_manager(env_var: Option<&str>) -> bool {
    is_env_truthy(env_var)
}

/// W37.15: helper testavel - decide se wp_color_manager_v1 sera criado.
/// Default: NAO. Opt-in via env aceita apenas valores truthy.
pub fn should_enable_color_manager(env_var: Option<&str>) -> bool {
    is_env_truthy(env_var)
}

/// W37.19: parsing comum de env vars boolean.
/// Aceita: "1", "true", "yes", "on" (case-insensitive).
/// Rejeita: None, "", "0", "false", "no", "off", outros.
fn is_env_truthy(env_var: Option<&str>) -> bool {
    match env_var {
        Some(v) => matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        None => false,
    }
}

#[cfg(test)]
mod w37_protocol_gating_tests {
    use super::{is_env_truthy, should_enable_color_manager, should_enable_toplevel_icon_manager};

    #[test]
    fn w37_18_toplevel_icon_disabled_by_default() {
        assert!(!should_enable_toplevel_icon_manager(None));
    }

    #[test]
    fn w37_18_toplevel_icon_enabled_via_env() {
        assert!(should_enable_toplevel_icon_manager(Some("1")));
        assert!(should_enable_toplevel_icon_manager(Some("yes")));
        assert!(should_enable_toplevel_icon_manager(Some("true")));
    }

    #[test]
    fn w37_18_toplevel_icon_falsy_strings_stay_disabled() {
        // Bugfix W37.19: env vazia ou "0" nao deve ativar.
        assert!(!should_enable_toplevel_icon_manager(Some("")));
        assert!(!should_enable_toplevel_icon_manager(Some("0")));
        assert!(!should_enable_toplevel_icon_manager(Some("false")));
        assert!(!should_enable_toplevel_icon_manager(Some("no")));
    }

    #[test]
    fn w37_15_color_manager_disabled_by_default() {
        assert!(!should_enable_color_manager(None));
    }

    #[test]
    fn w37_15_color_manager_enabled_via_env() {
        assert!(should_enable_color_manager(Some("1")));
    }

    #[test]
    fn w37_15_color_manager_falsy_strings_stay_disabled() {
        assert!(!should_enable_color_manager(Some("")));
        assert!(!should_enable_color_manager(Some("0")));
    }

    #[test]
    fn w37_19_is_env_truthy_case_insensitive() {
        assert!(is_env_truthy(Some("YES")));
        assert!(is_env_truthy(Some("True")));
        assert!(is_env_truthy(Some("ON")));
    }

    #[test]
    fn w37_19_is_env_truthy_trim_whitespace() {
        assert!(is_env_truthy(Some(" 1 ")));
        assert!(is_env_truthy(Some("\tyes\n")));
    }
}

#[cfg(test)]
mod decoration_decision_tests {
    use super::decide_decoration_mode;
    use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode;

    #[test]
    fn w37_8_client_side_respeitado() {
        // GTK3/Xfce4 que insistem em CSD nao tem 2 titlebars.
        assert_eq!(decide_decoration_mode(Mode::ClientSide), Mode::ClientSide);
    }

    #[test]
    fn w37_8_server_side_default() {
        assert_eq!(decide_decoration_mode(Mode::ServerSide), Mode::ServerSide);
    }
}
// W13.C: delegate fifo + commit_timing
smithay::delegate_fifo!(LumoState);
smithay::delegate_commit_timing!(LumoState);

/// Inicializa o global xdg-decoration-unstable-v1 no compositor.
/// Deve ser chamado apos LumoState::new() pra que as delegate impls
/// geradas em handlers::xdg_decoration estejam em scope.
pub fn init_xdg_decoration(state: &mut LumoState) {
    use smithay::wayland::shell::xdg::decoration::XdgDecorationState;
    state.xdg_decoration_state = Some(XdgDecorationState::new::<LumoState>(&state.display_handle));
    tracing::info!("M1: xdg_decoration global registrado");
}

/// W37.11: registra globals dos protocols modernos no compositor.
/// Destrava Chromium/Kate/Firefox + apps que dependem de HiDPI scaling,
/// single-pixel buffers e presentation feedback.
pub fn init_modern_protocols(state: &mut LumoState) {
    use smithay::wayland::presentation::PresentationState;
    use smithay::wayland::single_pixel_buffer::SinglePixelBufferState;
    use smithay::wayland::viewporter::ViewporterState;
    let dh = &state.display_handle;
    let _ = ViewporterState::new::<LumoState>(dh);
    let _ = SinglePixelBufferState::new::<LumoState>(dh);
    // CLOCK_MONOTONIC = 1 (POSIX). Smithay usa pra timestamps de frame.
    let _ = PresentationState::new::<LumoState>(dh, 1);
    tracing::info!("W37.11: viewporter + single_pixel_buffer + presentation registrados");
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

// ============================================================
// W6.C: splash boot animation
// ============================================================

/// Ticka a maquina de estados do splash logo.
///
/// Sequencia (total ~1.25s):
///   phase 0 (fade-in):  0 -> 1.0 em 200ms (rate 5.0/s)
///   phase 1 (hold):     1.0 por 800ms
///   phase 2 (fade-out): junto com boot_curtain (boot_curtain_alpha ja faz isso)
///                       -- splash_alpha vai de 1.0 -> 0.0 em 250ms (rate 4.0/s)
///   phase 3 (done):     splash_alpha = 0.0, sem render
///
/// Chamado a cada frame do backend (winit/drm) com dt em segundos.
pub fn tick_splash(state: &mut LumoState, dt: f32) {
    match state.splash_phase {
        0 => {
            // Fade-in: 200ms
            state.splash_alpha = (state.splash_alpha + dt * 5.0).min(1.0);
            state.splash_timer += dt;
            if state.splash_alpha >= 1.0 {
                state.splash_phase = 1;
                state.splash_timer = 0.0;
            }
        }
        1 => {
            // Hold: 800ms
            state.splash_timer += dt;
            if state.splash_timer >= 0.8 {
                state.splash_phase = 2;
                state.splash_timer = 0.0;
            }
        }
        2 => {
            // Fade-out: em paralelo com boot_curtain (rate 4.0/s = 250ms).
            state.splash_alpha = (state.splash_alpha - dt * 4.0).max(0.0);
            if state.splash_alpha <= 0.001 {
                state.splash_alpha = 0.0;
                state.splash_phase = 3;
            }
        }
        _ => {
            // done: nada a fazer.
        }
    }
}
