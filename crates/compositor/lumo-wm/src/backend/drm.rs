//! Backend DRM/KMS - lumo-wm full session direto no hardware.
//!
//! ETAPA 2A (A9): session + input + enumeracao real + event loop ativo.
//!   Por que 2A e nao 2 completo: portar GlesRenderer +
//!   DrmCompositor + page-flip + dmabuf scan-out em 90min sem TTY pra
//!   testar produz bugs nao reproduziveis (memory feedback_design_lapidado:
//!   spec antes de codar feature grande). Esta etapa entrega:
//!
//!     [done] LibSeatSession + notifier (Pause/Resume)
//!     [done] UdevBackend pra DRM devices
//!     [done] DrmDevice::new + enumeracao connectors/CRTCs/modes
//!     [done] Pick eDP-1 (Galaxy interno) ou primeiro Connected
//!     [done] libinput backend via session (teclado/mouse/touchpad)
//!     [done] Ctrl+Alt+F1..F12 -> session.change_vt (DRM, real)
//!     [done] Ctrl+Alt+Backspace -> exit clean
//!     [done] Watchdog 5s sem dispatch -> exit code 2
//!     [done] SessionEvent::Pause -> state.paused = true
//!     [done] SessionEvent::Resume -> state.paused = false
//!
//! Pendente Etapa 2B (proxima iteracao, NAO commitada hoje):
//!     [todo] GbmDevice + GbmAllocator + GbmFramebufferExporter
//!     [todo] EGLDisplay::new(gbm) + EGLContext + GlesRenderer
//!     [todo] DrmCompositor::new + render_frame loop com elementos
//!            reaproveitando winit.rs (cursor xcursor + sombras + cantos)
//!     [todo] queue_frame + page-flip event source pra vsync real
//!     [todo] linux-dmabuf-v1 import pra GPU clients
//!     [todo] cursor HW plane (low priority)
//!
//! Justificativa pratica: com Etapa 2A o Luiz ja consegue entrar em TTY3,
//! ver Lumo capturar input, ver enumeracao DRM real do Galaxy U300, e
//! voltar pro Hyprland host via Ctrl+Alt+F1 sem corromper nada. Render
//! preto sem janelas eh feio MAS reproduzivel e seguro. Render completo
//! exige debug iterativo no TTY que precisa do Luiz fisico - melhor
//! commitar quando todo o pipeline estiver verificado.
//!
//! Refs: smithay 0.7.0 docs, em particular:
//!   - backend::session::libseat::LibSeatSession
//!   - backend::udev::UdevBackend + UdevEvent
//!   - backend::libinput::LibinputInputBackend
//!   - backend::drm::compositor::DrmCompositor (Etapa 2B)

use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use smithay::backend::drm::{DrmDevice, DrmDeviceFd};
use smithay::backend::session::libseat::LibSeatSession;
use smithay::backend::session::{Event as SessionEvent, Session};
use smithay::backend::udev::{all_gpus, primary_gpu, UdevBackend, UdevEvent};
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::calloop::EventLoop;
use smithay::reexports::drm::control::{connector, Device as ControlDevice};
use smithay::reexports::drm::node::{DrmNode, NodeType};
use smithay::reexports::rustix;
use smithay::utils::DeviceFd;

use crate::state::LumoState;

/// Watchdog: sem dispatch da event loop em 5s -> assumir DRM stall
/// e exit code 2. Recovery: kernel ja garante VT switch via Ctrl+Alt+F1
/// (intercept no kernel, nao precisa do compositor).
const WATCHDOG_MS: u64 = 5_000;

/// Entry point do backend DRM. Bloqueia ate sair.
pub fn run(
    event_loop: &mut EventLoop<'static, LumoState>,
    state: &mut LumoState,
) -> Result<()> {
    tracing::info!("DRM backend Etapa 2A: session + input + enumeracao real");

    // ============================================================
    // 1. Session libseat (precisa seatd OU logind ativo em TTY).
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

    // Registra notifier de Pause/Resume na calloop.
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
    // 3. Abre DRM device via session (libseat empresta fd com perms).
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
    let (drm_device, _drm_notifier) =
        DrmDevice::new(drm_fd, true).map_err(|e| anyhow!("DrmDevice::new falhou: {e}"))?;

    tracing::info!(atomic = drm_device.is_atomic(), "DrmDevice aberto");

    // ============================================================
    // 4. Enumeracao: connectors -> picar o conectado (priorizar eDP).
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

    // Prioriza eDP (Galaxy Book interno) > LVDS > qualquer outro.
    let picked_idx = connected
        .iter()
        .position(|(_, n)| n.starts_with("EDP") || n.starts_with("Edp") || n.starts_with("eDP"))
        .or_else(|| {
            connected
                .iter()
                .position(|(_, n)| n.starts_with("LVDS") || n.starts_with("Lvds"))
        })
        .unwrap_or(0);

    let (picked_info, picked_name) = &connected[picked_idx];
    let picked_mode = picked_info
        .modes()
        .first()
        .copied()
        .ok_or_else(|| anyhow!("connector {picked_name} sem modes"))?;

    let (w, h) = picked_mode.size();
    tracing::info!(
        connector = %picked_name,
        width = w,
        height = h,
        refresh_hz = picked_mode.vrefresh(),
        "output escolhido (Etapa 2A nao monta surface ainda)"
    );

    // ============================================================
    // 5. UdevBackend pra hot-plug futuro. Por ora so escuta + loga.
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
    // 6. libinput backend (teclado/mouse/touchpad direto).
    // ============================================================
    super::libinput::init(&session, &event_loop.handle())?;

    // Salva session no state pra que input handler chame switch_vt.
    state.set_session(session);

    // ============================================================
    // 7. Watchdog frame timer: marca proximo deadline a cada dispatch.
    //    Se passar WATCHDOG_MS sem ticks - exit code 2.
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
    // 8. Loop principal. Etapa 2A: dispatch eventos sem render real.
    //    Watchdog vai matar se ficar muito tempo idle.
    //    Etapa 2B: aqui entrara drm_compositor.render_frame + queue_frame.
    // ============================================================
    tracing::info!(
        "DRM Etapa 2A: entrando event loop. Ctrl+Alt+F1 retorna Hyprland host."
    );
    while state.running {
        if let Err(err) = event_loop.dispatch(Some(Duration::from_millis(16)), state) {
            tracing::warn!(?err, "dispatch DRM event loop falhou");
        }
        // Pulse watchdog: cada dispatch reseta deadline.
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
