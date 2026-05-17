//! Backend DRM/KMS - lumo-wm full session direto no hardware.
//!
//! ETAPA 2B (A9): pipeline de render real ligado.
//!   [done] LibSeatSession + Pause/Resume
//!   [done] UdevBackend + DrmDevice + enumeracao connectors
//!   [done] Pick eDP-1 (Galaxy interno) ou primeiro Connected
//!   [done] libinput direto via session
//!   [done] Ctrl+Alt+F1..F12 -> session.change_vt
//!   [done] Ctrl+Alt+Backspace -> exit clean
//!   [done] Watchdog 5s sem dispatch -> exit code 2
//!   [done] GbmDevice + GbmAllocator + GbmFramebufferExporter
//!   [done] EGLDisplay + EGLContext + GlesRenderer
//!   [done] DrmOutputManager + initialize_output (single connector/crtc)
//!   [done] render_frame + queue_frame loop por timer 16ms (60Hz)
//!   [done] Page-flip event -> frame_submitted
//!   [done] DRM master attempt + error code claro se falha
//!   [done] reuse render_common (cursor/cantos/sombras) -- mesmo visual
//!          que winit, sem duplicar codigo (memory feedback_design_lapidado)
//!
//! ETAPA 2C (A9): toplevels reais + dispatch clients.
//!   [done] timer dispatch_clients 4ms dentro do event loop DRM
//!          (antes ficava no main loop pos-run() que NUNCA chega no
//!          path DRM, deixando socket Wayland sem dispatch)
//!   [done] collect_drm_elements -> SpaceRenderElements pra toplevels +
//!          layer-shell na ordem certa
//!   [done] LumoCustomElement::Space variant cobrindo xdg/layer
//!
//! Pendente Etapa 2D (futuro):
//!   [todo] cursor HW plane (overlay scan-out direct)
//!   [todo] linux-dmabuf-v1 pra clients
//!   [todo] hot-plug real (atualmente so loga)
//!   [todo] VRR (Galaxy U300 nao tem painel VRR; skipped)
//!   [todo] HiDPI scale tracking (output_scale != 1.0)
//!   [todo] damage tracker -> skip queue_frame quando damage vazio
//!
//! Memory feedback_input_feedback_imediato: frame loop 60Hz +
//! libinput dispatch entre frames + dispatch_clients 4ms = lag total
//! ficar < 16ms.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use smithay::backend::allocator::gbm::{GbmAllocator, GbmBufferFlags, GbmDevice};
use smithay::backend::allocator::Fourcc;
use smithay::backend::drm::compositor::FrameFlags;
use smithay::backend::drm::exporter::gbm::GbmFramebufferExporter;
use smithay::backend::drm::output::{DrmOutput, DrmOutputManager, DrmOutputRenderElements};
use smithay::backend::drm::{DrmDevice, DrmDeviceFd, DrmEvent};
use smithay::backend::egl::{EGLContext, EGLDisplay};
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::renderer::Color32F;
use smithay::backend::session::libseat::LibSeatSession;
use smithay::backend::session::{Event as SessionEvent, Session};
use smithay::backend::udev::{all_gpus, primary_gpu, UdevBackend, UdevEvent};
use smithay::output::{Mode as WlMode, Output, PhysicalProperties, Subpixel};
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::calloop::EventLoop;
use smithay::reexports::drm::control::{connector, crtc, Device as ControlDevice, ModeTypeFlags};
use smithay::reexports::drm::node::{DrmNode, NodeType};
use smithay::reexports::rustix;
use smithay::reexports::wayland_server::Display;
use smithay::utils::DeviceFd;

use crate::state::LumoState;

use super::render_common::{collect_drm_elements, DrmCollectInputs, LumoCustomElement, CLEAR_INK_DEEP};

/// Watchdog: sem dispatch da event loop em 5s -> assumir DRM stall
/// e exit code 2. Recovery: kernel ja garante VT switch via Ctrl+Alt+F1.
const WATCHDOG_MS: u64 = 5_000;

/// Frame interval alvo (60Hz). Galaxy U300 painel 60Hz fixo.
const FRAME_INTERVAL_MS: u64 = 16;

/// Formatos color suportados pelo primary plane. ARGB/XRGB 8bit
/// = lista mais conservadora compativel com i915. 10-bit skipado
/// na Etapa 2B pra reduzir matriz de bugs (memory feedback_design_lapidado).
const SUPPORTED_FORMATS: &[Fourcc] = &[Fourcc::Argb8888, Fourcc::Xrgb8888];

/// Tipo concreto do DrmOutputManager que usamos. Single-GPU, sem
/// user_data (= ()), file descriptor sob DrmDeviceFd.
type LumoDrmOutputManager =
    DrmOutputManager<GbmAllocator<DrmDeviceFd>, GbmFramebufferExporter<DrmDeviceFd>, (), DrmDeviceFd>;
type LumoDrmOutput =
    DrmOutput<GbmAllocator<DrmDeviceFd>, GbmFramebufferExporter<DrmDeviceFd>, (), DrmDeviceFd>;

/// Estado por-output. Em Lumo 2B so existe 1 (eDP-1 interno).
pub struct DrmSurfaceData {
    pub crtc: crtc::Handle,
    pub output: Output,
    pub drm_output: LumoDrmOutput,
    pub pending_flip: bool,
    pub last_frame_time: Instant,
}

/// Estado completo do backend DRM. Mantido em UserDataMap do state
/// e mutado por dispatch handlers.
pub struct DrmBackendData {
    pub output_manager: LumoDrmOutputManager,
    pub renderer: GlesRenderer,
    pub surface: Option<DrmSurfaceData>,
    pub gpu_node: DrmNode,
}

/// Entry point do backend DRM. Bloqueia ate sair.
///
/// Etapa 2C: recebe `display` (Rc<RefCell>) pra agendar dispatch_clients
/// dentro do mesmo event loop -- antes o dispatch ficava no main pos-run,
/// que nunca era alcancado no path DRM (run bloqueia ate exit).
pub fn run(
    event_loop: &mut EventLoop<'static, LumoState>,
    state: &mut LumoState,
    display: Rc<RefCell<Display<LumoState>>>,
) -> Result<()> {
    tracing::info!("DRM backend Etapa 2C: render toplevels ativo");

    // ============================================================
    // 1. Session libseat.
    // ============================================================
    let (mut session, notifier) = LibSeatSession::new().map_err(|e| {
        anyhow!(
            "LibSeatSession::new falhou: {e}. Causas: nao esta em TTY \
             (precisa Ctrl+Alt+F3), seatd nao rodando, ou XDG_SESSION_TYPE \
             incorreto."
        )
    })?;
    let seat_name = session.seat();
    tracing::info!(seat = %seat_name, "session libseat ok");

    event_loop
        .handle()
        .insert_source(notifier, |event, _, state| match event {
            SessionEvent::PauseSession => {
                tracing::info!("SessionEvent::PauseSession (VT switch out)");
                state.paused = true;
            }
            SessionEvent::ActivateSession => {
                tracing::info!("SessionEvent::ActivateSession (VT switch in)");
                state.paused = false;
                // Forca repaint imediato apos resume.
                state.drm_force_repaint = true;
            }
        })
        .map_err(|e| anyhow!("insert session notifier: {e}"))?;

    // ============================================================
    // 2. GPU primaria via udev.
    // ============================================================
    let primary = primary_gpu(&seat_name)
        .map_err(|e| anyhow!("primary_gpu falhou: {e}"))?
        .and_then(|p| DrmNode::from_path(&p).ok())
        .and_then(|node| node.node_with_type(NodeType::Primary).and_then(|r| r.ok()))
        .or_else(|| {
            all_gpus(&seat_name)
                .ok()?
                .into_iter()
                .find_map(|p| DrmNode::from_path(p).ok())
        })
        .ok_or_else(|| anyhow!("nenhuma GPU achada via udev"))?;

    let gpu_path = primary
        .dev_path()
        .ok_or_else(|| anyhow!("GPU sem dev path"))?;
    tracing::info!(gpu = %gpu_path.display(), "primary GPU");

    // ============================================================
    // 3. Abre DRM device via session.
    // ============================================================
    let fd = session
        .open(
            &gpu_path,
            rustix::fs::OFlags::RDWR
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::NONBLOCK,
        )
        .map_err(|e| anyhow!("session.open({}) falhou: {e:?}", gpu_path.display()))?;
    let drm_fd = DrmDeviceFd::new(DeviceFd::from(fd));
    let (drm_device, drm_notifier) = DrmDevice::new(drm_fd.clone(), true)
        .map_err(|e| anyhow!("DrmDevice::new falhou: {e}"))?;

    tracing::info!(atomic = drm_device.is_atomic(), "DrmDevice aberto");

    // Nota: DrmDevice nao expoe set_master direto. Libseat ja entrega
    // fd com master quando rodamos em TTY ativo. Se outro compositor
    // (Hyprland host) segura, initialize_output mais abaixo vai falhar
    // com erro DRM e propagamos o anyhow. Pra teste em TTY3 com
    // Hyprland ainda vivo, use scripts/lumo-tty.sh que mata Hyprland
    // antes de subir Lumo.

    // ============================================================
    // 4. GbmDevice + EGL + GlesRenderer.
    // ============================================================
    let gbm = GbmDevice::new(drm_fd.clone()).map_err(|e| anyhow!("GbmDevice::new: {e}"))?;
    tracing::info!("GbmDevice aberto");

    let egl_display = unsafe { EGLDisplay::new(gbm.clone()) }
        .map_err(|e| anyhow!("EGLDisplay::new: {e:?}"))?;
    let egl_context =
        EGLContext::new(&egl_display).map_err(|e| anyhow!("EGLContext::new: {e:?}"))?;
    let render_formats = egl_context
        .dmabuf_render_formats()
        .iter()
        .copied()
        .collect::<Vec<_>>();
    tracing::info!(
        render_formats_count = render_formats.len(),
        "EGL context pronto"
    );

    let mut renderer = unsafe { GlesRenderer::new(egl_context) }
        .map_err(|e| anyhow!("GlesRenderer::new: {e:?}"))?;
    tracing::info!("GlesRenderer iniciado");

    // ============================================================
    // 5. Enumeracao connectors -> pick eDP -> achar CRTC.
    // ============================================================
    let resource_handles = drm_device
        .resource_handles()
        .map_err(|e| anyhow!("resource_handles: {e}"))?;

    let mut connected: Vec<(connector::Info, String)> = Vec::new();
    for &handle in resource_handles.connectors() {
        match drm_device.get_connector(handle, false) {
            Ok(info) => {
                let name = format!("{:?}-{}", info.interface(), info.interface_id());
                tracing::info!(
                    connector = %name,
                    state = ?info.state(),
                    modes = info.modes().len(),
                    "connector"
                );
                if info.state() == connector::State::Connected {
                    connected.push((info, name));
                }
            }
            Err(err) => {
                tracing::warn!(?handle, ?err, "get_connector falhou (skip)");
            }
        }
    }

    if connected.is_empty() {
        return Err(anyhow!(
            "nenhum connector DRM conectado. Display desligado ou GPU sem outputs."
        ));
    }

    let picked_idx = connected
        .iter()
        .position(|(_, n)| n.starts_with("EDP") || n.starts_with("Edp") || n.starts_with("eDP"))
        .or_else(|| {
            connected
                .iter()
                .position(|(_, n)| n.starts_with("LVDS") || n.starts_with("Lvds"))
        })
        .unwrap_or(0);

    let (picked_info, picked_name) = connected.swap_remove(picked_idx);
    tracing::info!(connector = %picked_name, "output escolhido");

    // Mode preferido OU primeiro.
    let picked_mode = picked_info
        .modes()
        .iter()
        .find(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED))
        .copied()
        .or_else(|| picked_info.modes().first().copied())
        .ok_or_else(|| anyhow!("connector {picked_name} sem modes"))?;

    let (mode_w, mode_h) = picked_mode.size();
    tracing::info!(
        connector = %picked_name,
        width = mode_w,
        height = mode_h,
        refresh_hz = picked_mode.vrefresh(),
        "mode selecionado"
    );

    // Acha CRTC compativel: current encoder.crtc, OU primeiro
    // encoder.possible_crtcs com bit set.
    let crtc_handle = pick_crtc_for_connector(&drm_device, &picked_info, &resource_handles)
        .ok_or_else(|| anyhow!("nenhuma CRTC compativel pra connector {picked_name}"))?;
    tracing::info!(?crtc_handle, "crtc compativel achada");

    // ============================================================
    // 6. DrmOutputManager: monta exporter+allocator+device.
    // ============================================================
    let allocator =
        GbmAllocator::new(gbm.clone(), GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT);
    let exporter = GbmFramebufferExporter::new(gbm.clone(), None);

    let mut output_manager = DrmOutputManager::new(
        drm_device,
        allocator,
        exporter,
        Some(gbm.clone()),
        SUPPORTED_FORMATS.iter().copied(),
        render_formats.iter().copied(),
    );
    tracing::info!("DrmOutputManager pronto");

    // ============================================================
    // 7. Output wayland + map no Space.
    // ============================================================
    let wl_mode = WlMode::from(picked_mode);
    let (phys_w, phys_h) = picked_info.size().unwrap_or((0, 0));
    let output = Output::new(
        picked_name.clone(),
        PhysicalProperties {
            size: (phys_w as i32, phys_h as i32).into(),
            subpixel: Subpixel::Unknown,
            make: "Lumo".into(),
            model: picked_name.clone(),
        },
    );
    let _global = output.create_global::<LumoState>(&state.display_handle);
    output.set_preferred(wl_mode);
    output.change_current_state(Some(wl_mode), None, None, Some((0, 0).into()));
    state.space.map_output(&output, (0, 0));

    // ============================================================
    // 8. initialize_output -> cria DrmOutput (page-flip-ready surface).
    // ============================================================
    let drm_output = output_manager
        .initialize_output::<_, LumoCustomElement>(
            crtc_handle,
            picked_mode,
            &[picked_info.handle()],
            &output,
            None, // planes None = aceita todos os planes que driver permite
            &mut renderer,
            &DrmOutputRenderElements::default(),
        )
        .map_err(|e| anyhow!("initialize_output: {e:?}"))?;
    tracing::info!("DrmOutput surface ativa");

    // Guarda no state via UserDataMap (ou campo dedicado).
    state.drm_backend = Some(DrmBackendData {
        output_manager,
        renderer,
        surface: Some(DrmSurfaceData {
            crtc: crtc_handle,
            output: output.clone(),
            drm_output,
            pending_flip: false,
            last_frame_time: Instant::now(),
        }),
        gpu_node: primary,
    });

    // ============================================================
    // 9. Page-flip event source (DrmEvent::VBlank).
    // ============================================================
    event_loop
        .handle()
        .insert_source(drm_notifier, |event, _metadata, state: &mut LumoState| match event {
            DrmEvent::VBlank(crtc_h) => {
                if let Some(backend) = state.drm_backend.as_mut() {
                    if let Some(surf) = backend.surface.as_mut() {
                        if surf.crtc == crtc_h {
                            // Marca frame submetido -> libera swapchain slot.
                            if let Err(err) = surf.drm_output.frame_submitted() {
                                tracing::warn!(?err, "frame_submitted falhou");
                            }
                            surf.pending_flip = false;
                        }
                    }
                }
            }
            DrmEvent::Error(err) => {
                tracing::warn!(?err, "DrmEvent::Error");
            }
        })
        .map_err(|e| anyhow!("insert drm event source: {e}"))?;

    // ============================================================
    // 10. UdevBackend pra hot-plug futuro (so loga por agora).
    // ============================================================
    let udev_backend = UdevBackend::new(&seat_name)
        .map_err(|e| anyhow!("UdevBackend::new: {e}"))?;
    event_loop
        .handle()
        .insert_source(udev_backend, |event, _, _state| match event {
            UdevEvent::Added { device_id, path } => {
                tracing::info!(?device_id, path = %path.display(), "udev DRM device added");
            }
            UdevEvent::Changed { device_id } => {
                tracing::debug!(?device_id, "udev DRM device changed");
            }
            UdevEvent::Removed { device_id } => {
                tracing::warn!(?device_id, "udev DRM device removed");
            }
        })
        .map_err(|e| anyhow!("insert udev source: {e}"))?;

    // ============================================================
    // 11. libinput.
    // ============================================================
    super::libinput::init(&session, &event_loop.handle())?;

    state.set_session(session);

    // ============================================================
    // 12. Watchdog.
    // ============================================================
    state.watchdog_deadline = Some(Instant::now() + Duration::from_millis(WATCHDOG_MS));
    event_loop
        .handle()
        .insert_source(
            Timer::from_duration(Duration::from_millis(500)),
            |_, _, state: &mut LumoState| {
                let now = Instant::now();
                if let Some(deadline) = state.watchdog_deadline {
                    if now > deadline && !state.paused {
                        tracing::error!(
                            "DRM watchdog: nenhum dispatch em {}ms, exit code 2",
                            WATCHDOG_MS
                        );
                        state.running = false;
                        state.exit_code = 2;
                        return TimeoutAction::Drop;
                    }
                }
                TimeoutAction::ToDuration(Duration::from_millis(500))
            },
        )
        .map_err(|e| anyhow!("insert watchdog timer: {e}"))?;

    // ============================================================
    // 13. Frame timer 60Hz: chama render_drm a cada 16ms.
    //     Memory feedback_input_feedback_imediato: libinput dispatch
    //     entre frames mantem lag baixo.
    // ============================================================
    event_loop
        .handle()
        .insert_source(
            Timer::immediate(),
            |_, _, state: &mut LumoState| {
                render_drm(state);
                TimeoutAction::ToDuration(Duration::from_millis(FRAME_INTERVAL_MS))
            },
        )
        .map_err(|e| anyhow!("insert frame timer: {e}"))?;

    // ============================================================
    // 13b. Display dispatch_clients timer 4ms (Etapa 2C).
    //      Sem isso, clients Wayland (foot, lumo-bar) conectam socket
    //      mas nunca recebem eventos -- compositor responde so
    //      enquanto pixels rolam mas protocolo fica mudo.
    //      4ms = mesmo intervalo do path winit.
    // ============================================================
    let display_for_dispatch = display.clone();
    event_loop
        .handle()
        .insert_source(
            Timer::immediate(),
            move |_, _, state: &mut LumoState| {
                if !state.running {
                    return TimeoutAction::Drop;
                }
                let mut d = display_for_dispatch.borrow_mut();
                if let Err(err) = d.dispatch_clients(state) {
                    tracing::warn!(?err, "DRM dispatch_clients falhou");
                }
                let _ = d.flush_clients();
                drop(d);
                // Tick IPC pra workspaces (broadcast pra lumo-bar quando
                // Super+1..9 chega via libinput).
                crate::ipc::tick(state);
                TimeoutAction::ToDuration(Duration::from_millis(4))
            },
        )
        .map_err(|e| anyhow!("insert DRM dispatch timer: {e}"))?;

    // ============================================================
    // 14. Event loop principal.
    // ============================================================
    tracing::info!("DRM Etapa 2C: entrando event loop com toplevels reais");
    while state.running {
        if let Err(err) = event_loop.dispatch(Some(Duration::from_millis(16)), state) {
            tracing::warn!(?err, "dispatch DRM event loop falhou");
        }
        // Flush extra pos-dispatch garante que respostas synchrnous geradas
        // por handlers (ex: configure events em xdg_shell) vao pro wire
        // antes do proximo tick.
        let _ = display.borrow_mut().flush_clients();
        state.watchdog_deadline =
            Some(Instant::now() + Duration::from_millis(WATCHDOG_MS));
    }

    tracing::info!(exit_code = state.exit_code, "DRM backend saindo");
    let code = state.exit_code;
    if code != 0 {
        std::process::exit(code);
    }
    Ok(())
}

/// Acha CRTC compativel com o connector escolhido. Primeiro tenta
/// current_encoder().crtc(); se nao, varre possible_crtcs do primeiro
/// encoder valido.
fn pick_crtc_for_connector(
    device: &DrmDevice,
    connector_info: &connector::Info,
    resource_handles: &smithay::reexports::drm::control::ResourceHandles,
) -> Option<crtc::Handle> {
    // 1. Encoder atualmente atribuido.
    if let Some(enc_h) = connector_info.current_encoder() {
        if let Ok(enc_info) = device.get_encoder(enc_h) {
            if let Some(crtc_h) = enc_info.crtc() {
                return Some(crtc_h);
            }
        }
    }
    // 2. Varre encoders possiveis.
    for &enc_h in connector_info.encoders() {
        let Ok(enc_info) = device.get_encoder(enc_h) else { continue };
        let crtcs = resource_handles.filter_crtcs(enc_info.possible_crtcs());
        if let Some(&first) = crtcs.first() {
            return Some(first);
        }
    }
    None
}

/// Renderiza 1 frame DRM. No-op se paused (VT switched) ou pending_flip.
/// Chamado pelo frame timer a cada 16ms.
fn render_drm(state: &mut LumoState) {
    if state.paused {
        return;
    }

    state.frame_counter = state.frame_counter.wrapping_add(1);
    let trace = std::env::var("LUMO_TRACE_FRAMES").is_ok();
    let force_repaint = state.drm_force_repaint;
    state.drm_force_repaint = false;

    // Destructure pra split borrow: backend mut + state.* imut em paralelo.
    let LumoState {
        ref mut drm_backend,
        ref pointer_location,
        ref cursor,
        ref cursor_buffer,
        ref space,
        ref start_time,
        frame_counter,
        ..
    } = *state;

    let Some(backend) = drm_backend.as_mut() else {
        return;
    };

    let Some(surface) = backend.surface.as_mut() else {
        return;
    };

    if surface.pending_flip && !force_repaint {
        return;
    }

    let pointer_location = *pointer_location;
    let start_time_elapsed = start_time.elapsed();

    // Output size pra mascara de cantos.
    let mode = surface.output.current_mode().unwrap_or(WlMode {
        size: (1920, 1080).into(),
        refresh: 60_000,
    });
    let (ow, oh) = (mode.size.w, mode.size.h);

    // Etapa 2C: coleta chrome (cursor/cantos/sombras) + Space (toplevels +
    // layer-shell) em uma lista unica. collect_drm_elements ja respeita
    // ordem de stack -- cursor primeiro (front), cantos, sombras, depois
    // SpaceRenderElements vindos do smithay com z-order interno correto.
    let collect_inputs = DrmCollectInputs {
        space,
        output: &surface.output,
        pointer_location,
        frame_counter,
        cursor: cursor.as_ref(),
        cursor_buffer: cursor_buffer.as_ref(),
        output_w: ow,
        output_h: oh,
    };
    let all_elements = collect_drm_elements(&mut backend.renderer, &collect_inputs);

    // Render frame.
    let render_result = surface.drm_output.render_frame::<_, LumoCustomElement>(
        &mut backend.renderer,
        &all_elements,
        Color32F::new(
            CLEAR_INK_DEEP[0],
            CLEAR_INK_DEEP[1],
            CLEAR_INK_DEEP[2],
            CLEAR_INK_DEEP[3],
        ),
        FrameFlags::DEFAULT,
    );

    match render_result {
        Ok(result) => {
            if !result.is_empty {
                // Damage existe -> queue page-flip.
                match surface.drm_output.queue_frame(()) {
                    Ok(()) => {
                        surface.pending_flip = true;
                        surface.last_frame_time = Instant::now();
                        if trace {
                            tracing::debug!(frame = frame_counter, "frame queued");
                        }
                    }
                    Err(err) => {
                        tracing::warn!(?err, "queue_frame falhou");
                    }
                }
            } else if trace {
                tracing::trace!(frame = frame_counter, "no damage skip");
            }

            // Envia frame callbacks pros toplevels (mesmo sem render).
            let throttle = Some(Duration::from_millis(16));
            for window in space.elements() {
                window.send_frame(&surface.output, start_time_elapsed, throttle, |_, _| {
                    Some(surface.output.clone())
                });
            }
        }
        Err(err) => {
            tracing::warn!(?err, "render_frame falhou");
        }
    }
}
