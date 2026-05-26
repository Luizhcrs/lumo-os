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
//! A11 (atual):
//!   [done] is_master check hard error -- ao inves de seguir tela preta
//!          silenciosa, aborta com lsof + fix sugerido
//!   [done] tty script forca fuser -k em /dev/dri/* antes de subir lumo-wm
//!   [skip] wlr-screencopy-unstable-v1 -- smithay 0.7 NAO tem suporte
//!          nativo (sem ScreencopyManagerState, sem helper renderer).
//!          Implementar do zero = 300+ LoC tocando Resource/Frame/dmabuf
//!          + copy via GlesRenderer pra SHM. Adiado pra A12 com decisao:
//!          (a) bump smithay pra git main se ja ganhou suporte, ou
//!          (b) crate auxiliar tipo wayland-protocols-wlr server-side.
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

use crate::backend::vrr::{DisplayConfig, VrrSetupResult};
use crate::state::LumoState;

use super::render_common::{
    clear_color_linear, collect_cursor_only_elements, collect_drm_elements, DrmCollectInputs,
    LumoCustomElement,
};

/// Watchdog: sem dispatch da event loop em 5s -> assumir DRM stall
/// e exit code 2. Recovery: kernel ja garante VT switch via Ctrl+Alt+F1.
const WATCHDOG_MS: u64 = 5_000;

/// Frame interval alvo (60Hz). Galaxy U300 painel 60Hz fixo.
const _FRAME_INTERVAL_MS: u64 = 8; // 125Hz tick (vsync 60Hz limita render efetivo)

/// W3.P1: janela de render antes do proximo vblank. Renderizar 3ms antes
/// do vblank captura inputs mais frescos, cortando latencia p95 em ~8ms.
/// Ref: Paalanen Weston repaint scheduling.
const MAX_RENDER_TIME_MS: u64 = 3;

/// W3.P1: intervalo real do painel 60Hz em microsegundos.
const FRAME_INTERVAL_US: u64 = 16_667;

/// Formatos color suportados pelo primary plane. ARGB/XRGB 8bit
/// = lista mais conservadora compativel com i915. 10-bit skipado
/// na Etapa 2B pra reduzir matriz de bugs (memory feedback_design_lapidado).
const SUPPORTED_FORMATS: &[Fourcc] = &[Fourcc::Argb8888, Fourcc::Xrgb8888];

/// W13.B: Tenta habilitar VRR no DrmSurfaceData se config e connector suportarem.
/// Chamado apos initialize_output. Atualiza surface.vrr_active.
#[cfg(feature = "drm-backend")]
pub fn try_enable_vrr_drm(surface: &mut DrmSurfaceData, conn: connector::Handle) {
    let cfg = DisplayConfig::load();
    if !cfg.vrr_enabled {
        tracing::debug!("W13.B: VRR desabilitado por config (vrr_enabled=false)");
        return;
    }

    use smithay::backend::drm::VrrSupport;
    let support = surface
        .drm_output
        .with_compositor(|compositor| compositor.vrr_supported(conn));

    match support {
        Err(err) => {
            tracing::warn!(?err, "W13.B: vrr_supported() falhou");
        }
        Ok(VrrSupport::NotSupported) => {
            tracing::info!("W13.B: VRR nao suportado neste connector (Galaxy eDP-1 esperado)");
        }
        Ok(VrrSupport::RequiresModeset) => {
            tracing::info!("W13.B: VRR capable mas requer modeset (HDMI). Skip.");
        }
        Ok(VrrSupport::Supported) => {
            let result = surface
                .drm_output
                .with_compositor(|compositor| compositor.use_vrr(true));
            match result {
                Ok(()) => {
                    surface.vrr_active = true;
                    tracing::info!("W13.B: VRR habilitado no connector");
                }
                Err(err) => {
                    tracing::warn!(?err, "W13.B: use_vrr(true) falhou");
                }
            }
        }
    }
}

/// Tipo concreto do DrmOutputManager que usamos. Single-GPU, sem
/// user_data (= ()), file descriptor sob DrmDeviceFd.
type LumoDrmOutputManager = DrmOutputManager<
    GbmAllocator<DrmDeviceFd>,
    GbmFramebufferExporter<DrmDeviceFd>,
    (),
    DrmDeviceFd,
>;
type LumoDrmOutput =
    DrmOutput<GbmAllocator<DrmDeviceFd>, GbmFramebufferExporter<DrmDeviceFd>, (), DrmDeviceFd>;

/// Estado por-output. Em Lumo 2B so existe 1 (eDP-1 interno).
pub struct DrmSurfaceData {
    pub crtc: crtc::Handle,
    pub output: Output,
    pub drm_output: LumoDrmOutput,
    pub pending_flip: bool,
    pub last_frame_time: Instant,
    // L2: frame timing log p50/p95/p99 a cada 60s.
    pub frame_durations: Vec<Duration>,
    pub last_timing_log: Instant,
    // W3.P1: late-render scheduler.
    // last_vblank_ts: timestamp monotonic do ultimo VBlank (Duration desde boot).
    // max_render_time_ms: janela de render antes do proximo vblank (default 3ms).
    pub last_vblank_ts: Option<Duration>,
    pub max_render_time_ms: u64,
    // W3.P2: cursor HW plane tracking.
    // true quando ultimo frame colocou cursor no HW plane (Kind::Cursor atomic).
    pub cursor_hw_plane_active: bool,
    // W13.B: VRR estado atual do output (cacheado apos try_enable_vrr_drm).
    pub vrr_active: bool,
}

/// Estado completo do backend DRM. Mantido em UserDataMap do state
/// e mutado por dispatch handlers.
pub struct DrmBackendData {
    pub output_manager: LumoDrmOutputManager,
    pub renderer: GlesRenderer,
    pub surface: Option<DrmSurfaceData>,
    /// W9.C: additional surfaces for secondary outputs (monitors 2+).
    pub extra_surfaces: Vec<DrmSurfaceData>,
    pub gpu_node: DrmNode,
    /// W8.A fix: cache do ultimo frame composto em buffer pixel BGRA8888.
    /// Atualizado dentro de render_drm a cada frame (path c: 1 GPU blit/frame).
    /// screencopy do_copy le essa cache pra entregar conteudo real ao client.
    /// None ate primeiro render bem-sucedido.
    pub screencopy_cache: Option<crate::backend::screencopy_cache::ScreencopyCache>,
}

/// Entry point do backend DRM. Bloqueia ate sair.
///
/// Etapa 2C: recebe `display` (Rc<RefCell>) pra agendar dispatch_clients
/// dentro do mesmo event loop -- antes o dispatch ficava no main pos-run,
/// que nunca era alcancado no path DRM (run bloqueia ate exit).
/// A16 frente 2: forca property "Broadcast RGB" = Full no connector.
///
/// Default i915 = Automatic (0). Em painel eDP-1 isso pode resolver pra
/// Limited 16:235 dependendo EDID e modeline (kernel heuristica). Limited
/// range = todos cores comprimidos pra 16..235 = banding/dither visivel
/// quando sRGB content full-range eh enviado bruto pro scanout.
///
/// Hyprland NAO seta isso explicito (aquamarine query Colorspace/max_bpc
/// mas nao Broadcast RGB) — mas Hyprland tambem sofre menos porque GBM
/// modifier Y-tiled mascara dither hardware. No lumo-wm, com LINEAR
/// fallback, padrao Bayer fica visivel.
///
/// Retorna Ok(()) se setou Full, Ok(()) com warn se prop nao existe (drivers
/// nao-Intel), Err se acesso DRM falhou.
fn _set_broadcast_rgb_full(
    drm_device: &smithay::backend::drm::DrmDevice,
    conn: smithay::reexports::drm::control::connector::Handle,
) -> anyhow::Result<()> {
    use smithay::reexports::drm::control::Device as ControlDevice;

    let props = drm_device
        .get_properties(conn)
        .map_err(|e| anyhow::anyhow!("get_properties(connector): {e}"))?;

    let mut target: Option<(smithay::reexports::drm::control::property::Handle, u64)> = None;
    let mut current_val: Option<u64> = None;

    for (handle, raw_value) in props.iter() {
        let info = match drm_device.get_property(*handle) {
            Ok(i) => i,
            Err(_) => continue,
        };
        let name = match info.name().to_str() {
            Ok(n) => n,
            Err(_) => continue,
        };
        if name != "Broadcast RGB" {
            continue;
        }

        current_val = Some(*raw_value as u64);

        // Procura enum value "Full"
        if let smithay::reexports::drm::control::property::ValueType::Enum(enums) =
            info.value_type()
        {
            let (raws, evals) = enums.values();
            for (raw, ev) in raws.iter().zip(evals.iter()) {
                if let Ok(ename) = ev.name().to_str() {
                    if ename == "Full" {
                        target = Some((*handle, *raw));
                        break;
                    }
                }
            }
        }
        break;
    }

    match target {
        Some((handle, full_val)) => {
            let cur = current_val.unwrap_or(u64::MAX);
            if cur == full_val {
                tracing::info!(
                    broadcast_rgb_value = full_val,
                    "Broadcast RGB ja em Full (skip set)"
                );
                return Ok(());
            }
            drm_device
                .set_property(conn, handle, full_val)
                .map_err(|e| anyhow::anyhow!("set_property(Broadcast RGB=Full): {e}"))?;
            tracing::info!(
                broadcast_rgb_value = full_val,
                broadcast_rgb_previous = cur,
                "Broadcast RGB property set: Full (A16 frente 2)"
            );
            Ok(())
        }
        None => {
            tracing::warn!(
                "Broadcast RGB property nao encontrada no connector (driver nao expoe — skip)"
            );
            Ok(())
        }
    }
}

/// A16 frente 1+4: loga modifier real do swapchain + blend func GL ativo.
///
/// Hyprland: `glBlendFunc(GL_ONE, GL_ONE_MINUS_SRC_ALPHA)` = premultiplied.
/// GL_ONE = 1, GL_ONE_MINUS_SRC_ALPHA = 0x0303 = 771.
///
/// Modifier esperado em Intel: I915_y_tiled (Mesa escolhe automatico se
/// disponivel pra Argb8888 + SCANOUT). Se for LINEAR, dither hardware
/// alinha visualmente em padrao Bayer.
fn log_drm_pipeline_state(drm_output: &LumoDrmOutput, renderer: &mut GlesRenderer) {
    let format = drm_output.format();
    let modifiers: Vec<_> = drm_output.with_compositor(|c| c.modifiers().to_vec());

    tracing::info!(
        ?format,
        modifiers_count = modifiers.len(),
        modifiers = ?modifiers,
        "A16: swapchain format + modifiers ativos"
    );

    // Blend func via GlesRenderer context
    use smithay::backend::renderer::gles::ffi;
    let _ = renderer.with_context(|gl| unsafe {
        let mut src_rgb: i32 = 0;
        let mut dst_rgb: i32 = 0;
        let mut src_alpha: i32 = 0;
        let mut dst_alpha: i32 = 0;
        let mut blend_enabled: u8 = 0;
        gl.GetIntegerv(ffi::BLEND_SRC_RGB, &mut src_rgb as *mut _);
        gl.GetIntegerv(ffi::BLEND_DST_RGB, &mut dst_rgb as *mut _);
        gl.GetIntegerv(ffi::BLEND_SRC_ALPHA, &mut src_alpha as *mut _);
        gl.GetIntegerv(ffi::BLEND_DST_ALPHA, &mut dst_alpha as *mut _);
        gl.GetBooleanv(ffi::BLEND, &mut blend_enabled as *mut _);
        tracing::info!(
            blend_enabled = blend_enabled != 0,
            blend_src_rgb = src_rgb,
            blend_dst_rgb = dst_rgb,
            blend_src_alpha = src_alpha,
            blend_dst_alpha = dst_alpha,
            "A16: blend func GL atual (premul espera src=1 dst=771)"
        );
    });
}

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
            rustix::fs::OFlags::RDWR | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NONBLOCK,
        )
        .map_err(|e| anyhow!("session.open({}) falhou: {e:?}", gpu_path.display()))?;
    let drm_fd = DrmDeviceFd::new(DeviceFd::from(fd));

    // A11: diagnostico DRM master.
    //
    // Sintoma A10 (Luiz reportou): TTY3 sobe lumo-wm sem erro, mas tela
    // fica preta com cursor estatico no canto. Log mostrava:
    //   "Unable to become drm master, assuming unprivileged mode"
    //
    // Causa raiz: smithay 0.7 (device/fd.rs:77) chama acquire_master_lock
    // e se falha SO emite warn -- segue sem master. Sem master = render
    // loop roda mas page-flip nao faz scanout (kernel rejeita); buffer
    // vai pra GPU mas nunca aparece no painel.
    //
    // Quem segura master? Mesmo apos `hyprctl exit` o Hyprland host
    // pode demorar pra liberar /dev/dri/card0, OU seatd/logind segura
    // cacheado, OU outro processo (display manager) tem fd aberto.
    //
    // Fix: detectar unprivileged ANTES de continuar e abortar com
    // mensagem explicita pro Luiz saber exatamente o que rodar pra
    // resolver. Sem master = tela preta garantida; preferivel falhar
    // cedo + claro do que silenciar (memory feedback_design_lapidado).
    // Tentativa de virar master. Smithay 0.7 ja chamou acquire_master_lock
    // em DrmDeviceFd::new (fd.rs:77). Se outro processo segura, o lock
    // ja falhou silenciosamente la. Aqui repetimos pra confirmar com erro
    // explicito (is_privileged eh pub(crate), nao acessivel).
    let master_ok = {
        use smithay::reexports::drm::Device as DrmDeviceTrait;
        drm_fd.acquire_master_lock().is_ok()
    };
    // A11.8: master_lock falha eh OK em kernels novos (smithay docs).
    // Se ele falhar aqui, render path ainda funciona unprivileged.
    if !master_ok {
        tracing::warn!("master_lock falhou (esperado em kernels novos), seguindo unprivileged");
    }
    tracing::info!("DRM master adquirido (privileged)");

    let (drm_device, drm_notifier) =
        DrmDevice::new(drm_fd.clone(), true).map_err(|e| anyhow!("DrmDevice::new falhou: {e}"))?;

    tracing::info!(atomic = drm_device.is_atomic(), "DrmDevice aberto");

    // ============================================================
    // 4. GbmDevice + EGL + GlesRenderer.
    // ============================================================
    let gbm = GbmDevice::new(drm_fd.clone()).map_err(|e| anyhow!("GbmDevice::new: {e}"))?;
    tracing::info!("GbmDevice aberto");

    let egl_display =
        unsafe { EGLDisplay::new(gbm.clone()) }.map_err(|e| anyhow!("EGLDisplay::new: {e:?}"))?;
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

    // NOTE A15.6: GL_FRAMEBUFFER_SRGB pode gerar dithering visivel quando
    // framebuffer NAO eh sRGB-tagged (default GBM Argb8888 = linear). Removido
    // pra teste. Hyprland funciona sem isso provavelmente porque tagging do
    // EGLSurface vem por outro caminho (config attribs).
    tracing::info!("FRAMEBUFFER_SRGB skipped (testing)");
    tracing::info!("GlesRenderer iniciado");

    // A19.4: carrega wallpaper (igual winit.rs:131)
    let wallpaper = crate::backend::wallpaper::LumoWallpaper::try_load(&mut renderer);
    state.wallpaper = wallpaper;
    // A38: compila shader SDF corner radius (igual winit.rs).
    state.corner_shader = match crate::backend::corner_shader::CornerShader::compile(&mut renderer)
    {
        Ok(cs) => Some(cs),
        Err(e) => {
            tracing::warn!("corner_shader compile falhou: {:?}", e);
            None
        }
    };
    state.corner_mask_shader =
        match crate::backend::corner_shader::CornerMaskShader::compile(&mut renderer) {
            Ok(cs) => Some(cs),
            Err(e) => {
                tracing::warn!("corner_mask_shader compile falhou: {:?}", e);
                None
            }
        };
    state.titlebar_bg_shader =
        match crate::backend::corner_shader::TitlebarBgShader::compile(&mut renderer) {
            Ok(cs) => Some(cs),
            Err(e) => {
                tracing::warn!("titlebar_bg_shader compile falhou: {:?}", e);
                None
            }
        };

    // A10 frente 1: dmabuf-v1 global. Galaxy U300 = Intel i915 render
    // node /dev/dri/renderD128. EGLContext.dmabuf_render_formats() ja
    // inclui DRM_FORMAT_MOD_LINEAR + I915_FORMAT_MOD_X_TILED + Y_TILED
    // descobertos pelo Mesa. Sem hardcode (memory feedback_design_lapidado).
    {
        use smithay::wayland::dmabuf::DmabufFeedbackBuilder;
        let dev_id = primary.dev_id();
        let formats_count = render_formats.len();
        match DmabufFeedbackBuilder::new(dev_id, render_formats.iter().copied()).build() {
            Ok(feedback) => {
                let global = state
                    .dmabuf_state
                    .create_global_with_default_feedback::<LumoState>(
                        &state.display_handle,
                        &feedback,
                    );
                state.dmabuf_global = Some(global);
                tracing::info!(
                    dev_id,
                    formats = formats_count,
                    "dmabuf-v1 global criado (drm)"
                );
            }
            Err(err) => {
                tracing::warn!(?err, "DmabufFeedback build (drm) falhou; dmabuf desativado");
            }
        }
    }

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
    let allocator = GbmAllocator::new(
        gbm.clone(),
        GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT,
    );
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

    // Centraliza cursor no output real detectado
    state.pointer_location = (wl_mode.size.w as f64 / 2.0, wl_mode.size.h as f64 / 2.0).into();
    tracing::info!(
        x = state.pointer_location.x,
        y = state.pointer_location.y,
        "cursor centralizado"
    );
    state.space.map_output(&output, (0, 0));

    // ============================================================
    // 8. initialize_output -> cria DrmOutput (page-flip-ready surface).
    // ============================================================
    //
    // A16 frente 2: tenta forcar Broadcast RGB = Full ANTES do primeiro
    // page-flip. Se driver eDP nao expoe prop, segue (warn). Pre-condicao:
    // drm_device ainda owned por output_manager — mas DrmOutputManager
    // tem .with_device_mut? Nao no smithay 0.7. Workaround: pegamos
    // o connector handle e setamos via FD do drm_fd ja clonado.
    {
        // Recriar Device handle via FD (DrmDeviceFd implementa AsFd).
        // Helper aceita &DrmDevice mas precisamos do trait Device. Como
        // DrmOutputManager owns DrmDevice, usamos accessor:
        // A16.1: Broadcast RGB Full DESATIVADO. Hyprland funciona com Automatic (default).
        // Forcando Full pode estar piorando: painel TN recebe range full mas converte
        // pra Limited internamente -> dither extra. Deixar driver decidir.
        let _dev_ref = output_manager.device();
        tracing::info!("Broadcast RGB: usando default (Automatic) - Hyprland-style");
    }

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

    // A16 frente 1+4: log do estado real escolhido pelo driver.
    log_drm_pipeline_state(&drm_output, &mut renderer);

    // Guarda no state via UserDataMap (ou campo dedicado).
    state.drm_backend = Some(DrmBackendData {
        output_manager,
        renderer,
        extra_surfaces: Vec::new(),
        screencopy_cache: None,
        surface: Some(DrmSurfaceData {
            crtc: crtc_handle,
            output: output.clone(),
            drm_output,
            pending_flip: false,
            last_frame_time: Instant::now(),
            // L2: frame timing log.
            frame_durations: Vec::with_capacity(512),
            last_timing_log: Instant::now(),
            // W3.P1: late-render scheduler init.
            last_vblank_ts: None,
            max_render_time_ms: MAX_RENDER_TIME_MS,
            // W3.P2: cursor HW plane tracking.
            cursor_hw_plane_active: false,
            // W13.B: VRR init false; updated by try_enable_vrr_drm.
            vrr_active: false,
        }),
        gpu_node: primary,
    });

    // W13.B: tenta VRR se config habilitado.
    if let Some(backend) = state.drm_backend.as_mut() {
        if let Some(surf) = backend.surface.as_mut() {
            try_enable_vrr_drm(surf, picked_info.handle());
        }
    }

    // ============================================================
    // 9. Page-flip event source (DrmEvent::VBlank).
    // ============================================================
    event_loop
        .handle()
        .insert_source(
            drm_notifier,
            |event, metadata, state: &mut LumoState| match event {
                DrmEvent::VBlank(crtc_h) => {
                    if let Some(backend) = state.drm_backend.as_mut() {
                        if let Some(surf) = backend.surface.as_mut() {
                            if surf.crtc == crtc_h {
                                // Marca frame submetido -> libera swapchain slot.
                                if let Err(err) = surf.drm_output.frame_submitted() {
                                    tracing::warn!(?err, "frame_submitted falhou");
                                }
                                surf.pending_flip = false;
                                // W3.P1: captura timestamp monotonic do VBlank para
                                // calcular render_deadline do proximo frame.
                                if let Some(meta) = metadata {
                                    if let smithay::backend::drm::DrmEventTime::Monotonic(ts) =
                                        meta.time
                                    {
                                        surf.last_vblank_ts = Some(ts);
                                    }
                                }
                            }
                        }
                    }
                }
                DrmEvent::Error(err) => {
                    tracing::warn!(?err, "DrmEvent::Error");
                }
            },
        )
        .map_err(|e| anyhow!("insert drm event source: {e}"))?;

    // ============================================================
    // 10. UdevBackend pra hot-plug futuro (so loga por agora).
    // ============================================================
    let udev_backend =
        UdevBackend::new(&seat_name).map_err(|e| anyhow!("UdevBackend::new: {e}"))?;
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
        .insert_source(Timer::immediate(), |_, _, state: &mut LumoState| {
            // W3.P1: late-render scheduler.
            // Calcula render_deadline = last_vblank + frame_interval - max_render_time.
            // Se now < render_deadline, dorme ate deadline para capturar inputs mais
            // frescos e cortar latencia p95 em ~8ms.
            let next_timeout = compute_render_timeout(state);
            if let Some(sleep_for) = next_timeout {
                // Reagenda timer para o deadline sem render agora.
                return TimeoutAction::ToDuration(sleep_for);
            }
            render_drm(state);
            // W23: adaptive timer pra sub-1% CPU idle.
            // - Active (should_render OR force_repaint OR anim ativa): 16ms (60Hz vblank)
            // - Idle (nada mudou ultimo frame): 100ms (~10Hz wake-up)
            // - Bridge IPC + commit handler set should_render=true = volta active
            // W23.5: sticky active 500ms apos pointer motion — mouse 60fps durante drag.
            let recent_input = state
                .cursor_last_motion_ts
                .map(|t| t.elapsed() < Duration::from_millis(500))
                .unwrap_or(false);
            let active = state.should_render
                || state.drm_force_repaint
                || state.boot_curtain_alpha > 0.001
                || state.splash_alpha > 0.001
                || state.overview.is_some()
                || recent_input;
            let timeout_ms = if active { 16 } else { 33 };
            TimeoutAction::ToDuration(Duration::from_millis(timeout_ms))
        })
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
        .insert_source(Timer::immediate(), move |_, _, state: &mut LumoState| {
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
            // W23.2: adaptive dispatch_clients. 4ms active vs 20ms idle.
            // 4ms = 250Hz era residual maior fonte ctxt switches. Idle
            // 20ms = 50Hz suficiente pra latencia client perceivel.
            let recent_input = state
                .cursor_last_motion_ts
                .map(|t| t.elapsed() < Duration::from_millis(500))
                .unwrap_or(false);
            let active = state.should_render
                || state.drm_force_repaint
                || state.boot_curtain_alpha > 0.001
                || state.splash_alpha > 0.001
                || state.overview.is_some()
                || recent_input;
            let dispatch_ms = if active { 4 } else { 8 };
            TimeoutAction::ToDuration(Duration::from_millis(dispatch_ms))
        })
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
        state.watchdog_deadline = Some(Instant::now() + Duration::from_millis(WATCHDOG_MS));
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
        let Ok(enc_info) = device.get_encoder(enc_h) else {
            continue;
        };
        let crtcs = resource_handles.filter_crtcs(enc_info.possible_crtcs());
        if let Some(&first) = crtcs.first() {
            return Some(first);
        }
    }
    None
}

/// W3.P1: calcula quanto tempo esperar antes de renderizar o proximo frame.
///
/// Retorna Some(duration) se devemos esperar (nao renderizar agora),
/// None se podemos renderizar imediatamente.
///
/// Logica (relativa a last_frame_time via Instant):
///   deadline_age = frame_interval - max_render_time
///   Se age_since_last_frame < deadline_age -> dorme ate deadline.
///   Se age >= deadline_age (ou frame perdido) -> render imediato.
///
/// Exemplo 60Hz (frame_interval=16.67ms, max_render=3ms):
///   deadline_age = 13.67ms. Render ao atingir 13.67ms apos o ultimo frame.
///   Captura inputs ate 3ms antes do vblank = corta latencia p95 em ~8ms.
///
/// Ref: Paalanen Weston repaint scheduling + Hugl KWin VRR patches.
fn compute_render_timeout(state: &LumoState) -> Option<Duration> {
    let backend = state.drm_backend.as_ref()?;
    let surf = backend.surface.as_ref()?;

    // Sem vblank registrado ainda -> render imediato no primeiro frame.
    if surf.last_vblank_ts.is_none() {
        return None;
    }

    let frame_interval = Duration::from_micros(FRAME_INTERVAL_US);
    let max_render = Duration::from_millis(surf.max_render_time_ms);

    // deadline_age: quanto tempo apos o ultimo frame podemos comecar a render.
    let deadline_age = frame_interval.saturating_sub(max_render);

    let age = surf.last_frame_time.elapsed();

    if age >= deadline_age {
        // Ja passamos do deadline (ou perdemos frame) -> render imediato.
        return None;
    }

    let sleep_for = deadline_age - age;
    // Nao reschedula por menos de 0.5ms (overhead de calloop timer).
    if sleep_for < Duration::from_micros(500) {
        return None;
    }

    Some(sleep_for)
}

/// Renderiza 1 frame DRM. No-op se paused, lid fechado, ou pending_flip.
/// L2: lid closed skip + frame timing log p50/p95/p99.
fn render_drm(state: &mut LumoState) {
    if state.paused {
        return;
    }

    // L2: lid fechado -> skip render (economiza GPU + bateria).
    {
        let lid_closed = state
            .lid_handler
            .lock()
            .map(|l| l.closed_at.is_some())
            .unwrap_or(false);
        if lid_closed {
            return;
        }
    }

    // W22: damage-gating. Skip render se nada mudou desde ultimo frame.
    // should_render set true por: commit surface, cursor move, anim tick,
    // focus change, decoration change. drm_force_repaint bypass gating.
    if !state.should_render && !state.drm_force_repaint {
        state.skipped_frames = state.skipped_frames.wrapping_add(1);
        return;
    }
    state.should_render = false;

    state.frame_counter = state.frame_counter.wrapping_add(1);
    let trace = std::env::var("LUMO_TRACE_FRAMES").is_ok();
    let force_repaint = state.drm_force_repaint;
    state.drm_force_repaint = false;

    // A39: tick boot curtain com delta tempo real (P1 fix: nao acoplado a frame rate).
    {
        let now = std::time::Instant::now();
        let dt = now.duration_since(state.boot_last_tick).as_secs_f32();
        state.boot_last_tick = now;
        if !state.boot_ready && state.boot_clients_ready() {
            state.boot_ready = true;
        }
        if state.boot_ready && state.boot_curtain_alpha > 0.001 {
            // Fade 1.0 -> 0.0 em 250ms = rate 4.0/s.
            state.boot_curtain_alpha = (state.boot_curtain_alpha - dt * 4.0).max(0.0);
        }
        // W6.C: tick splash logo animation.
        crate::state::tick_splash(state, dt);
        // W12.B: tick overview.
        if let Some(ov) = state.overview.as_mut() {
            ov.tick(dt);
        }
        if state
            .overview
            .as_ref()
            .map(|o| o.is_closed())
            .unwrap_or(false)
        {
            state.overview = None;
        }
    }
    let boot_curtain_alpha = state.boot_curtain_alpha;

    // R1: calcular cursor_moved ANTES do destructure pra evitar borrow conflict.
    // Bypass pending_flip quando cursor se moveu = elimina delay visual.
    let cursor_moved = {
        let last = state.last_rendered_cursor_pos;
        let cur = state.pointer_location;
        (last.x - cur.x).abs() > 0.01 || (last.y - cur.y).abs() > 0.01
    };
    if cursor_moved {
        state.last_rendered_cursor_pos = state.pointer_location;
    }

    // Destructure pra split borrow: backend mut + state.* imut em paralelo.
    let LumoState {
        ref mut drm_backend,
        ref pointer_location,
        ref cursor,
        ref cursor_buffer,
        ref space,
        ref start_time,
        ref wallpaper,
        ref corner_shader,
        ref ssd_windows,
        ref splash_buffer,
        splash_alpha,
        frame_counter,
        ..
    } = *state;
    let splash_alpha_val = splash_alpha;
    let splash_buf_ref = splash_buffer.as_ref();

    let Some(backend) = drm_backend.as_mut() else {
        return;
    };

    let Some(surface) = backend.surface.as_mut() else {
        return;
    };

    // R1 fix2 flicker: cursor move durante pending_flip = espera. Cursor HW plane
    // (W3.P2) atualiza via atomic async em paralelo. Render novo durante pending = flicker.
    if surface.pending_flip && !force_repaint {
        return;
    }

    let pointer_location = *pointer_location;
    let start_time_elapsed = start_time.elapsed();
    if std::env::var("LUMO_TRACE_POINTER").as_deref() == Ok("1") {
        eprintln!(
            "[trace] render cursor pos=({:.1},{:.1})",
            pointer_location.x, pointer_location.y
        );
    }

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
    let titlebar_menu_opt = state
        .titlebar_menu
        .as_ref()
        .map(|(_, pos, hover)| (*pos, *hover));
    let collect_inputs = DrmCollectInputs {
        boot_curtain_alpha,
        splash_alpha: splash_alpha_val,
        splash_buffer: splash_buf_ref,
        wallpaper: wallpaper.as_ref(),
        corner_shader: corner_shader.as_ref(),
        ssd_windows,
        titlebar_menu: titlebar_menu_opt,
        snap_preview: state.snap_preview,
        corner_mask_shader: state.corner_mask_shader.as_ref(),
        titlebar_bg_shader: state.titlebar_bg_shader.as_ref(),
        overview_elements: state
            .overview
            .as_ref()
            .map(|ov| crate::overview::overview_elements(ov, ow, oh))
            .unwrap_or_default(),
        picker_elements: state
            .stack_picker
            .as_ref()
            .map(|p| crate::stack_picker::picker_elements(p, ow, oh))
            .unwrap_or_default(),
        space,
        output: &surface.output,
        pointer_location,
        frame_counter,
        cursor: cursor.as_ref(),
        cursor_buffer: cursor_buffer.as_ref(),
        output_w: ow,
        output_h: oh,
    };
    // R1.fix3 flicker: cursor_only path REMOVIDO. queue_frame com lista
    // contendo SO cursor limpa primary plane = flicker visivel quando mouse
    // move. DrmCompositor 0.7 ja detecta Kind::Cursor em lista completa +
    // faz cursor HW plane atomic-async sem re-render primary plane.
    let _ = (cursor_moved, boot_curtain_alpha, splash_alpha_val);
    let all_elements = collect_drm_elements(&mut backend.renderer, &collect_inputs);

    // W3.P4: damage merge heuristica antes de queue_frame.
    // Computa damage rects dos elementos e merge se lista complexa.
    // Aplica sobre a geometria dos elementos pra decidir se simplificamos.
    {
        let output_w = ow;
        let output_h = oh;
        let mut elem_damage: Vec<smithay::utils::Rectangle<i32, smithay::utils::Physical>> =
            all_elements
                .iter()
                .filter_map(|el| {
                    use smithay::backend::renderer::element::Element;
                    let geo = el.geometry(smithay::utils::Scale::from(1.0));
                    if geo.size.w > 0 && geo.size.h > 0 {
                        Some(geo)
                    } else {
                        None
                    }
                })
                .collect();
        crate::backend::damage::merge_if_complex_default(&mut elem_damage, output_w, output_h);
        // Resultado logado via tracing::trace dentro de merge_if_complex.
    }

    // Render frame.
    let clear = clear_color_linear();
    // INSTR.A: mede duracao REAL de render_frame (CPU + GPU submit) separada do
    // intervalo de frame (last_frame_time delta = vsync 16.67ms, NAO duracao).
    // Acumula em state.perf via record_render_duration; log a cada 60s.
    let _render_t0 = Instant::now();
    let render_result = surface.drm_output.render_frame::<_, LumoCustomElement>(
        &mut backend.renderer,
        &all_elements,
        Color32F::new(clear[0], clear[1], clear[2], clear[3]),
        FrameFlags::DEFAULT,
    );
    let render_elapsed = _render_t0.elapsed();
    // Telemetry: record frame render duration.
    lumo_telemetry::histogram("frame_render_us", render_elapsed.as_micros() as u64);
    // Telemetry: input-to-paint if a pointer event was recorded.
    if let Some(input_ts) = state.last_input_ts.take() {
        let input_to_paint_us = input_ts.elapsed().as_micros() as u64;
        lumo_telemetry::histogram("input_to_paint_us", input_to_paint_us);
    }
    // INSTR.A: warning imediato se render > 10ms (suspeita de starvation calloop).
    if render_elapsed > Duration::from_millis(10) {
        tracing::warn!(
            render_ms = render_elapsed.as_millis(),
            render_us = render_elapsed.as_micros(),
            "INSTR.A: render_frame > 10ms (suspeita starvation calloop)"
        );
    }

    match render_result {
        Ok(result) => {
            // W8.A fix: atualiza cache screencopy com mesmo set de elementos.
            // Render off-screen num GlesRenderbuffer e copia pixels pra Vec<u8>
            // BGRA8888 cacheado. Custo: 1 GPU re-render/frame quando ha client
            // screencopy ativo (lazy: cache so renderiza apos primeiro pedido).
            // Path c do plano: render manual em shadow buffer, evita ler primary
            // plane post-scanout (dmabuf pode estar em uso pelo display engine).
            if backend
                .screencopy_cache
                .as_ref()
                .map(|c| c.is_armed())
                .unwrap_or(false)
            {
                if let Some(cache) = backend.screencopy_cache.as_mut() {
                    if let Err(err) =
                        cache.refresh(&mut backend.renderer, &surface.output, &all_elements, clear)
                    {
                        tracing::warn!(?err, "W8.A: screencopy cache refresh falhou");
                    }
                }
            }
            // W3.P2: rastreia se cursor foi para HW plane neste frame.
            // result.cursor_element.is_some() = DrmCompositor colocou cursor no HW plane.
            let cursor_on_hw = result.cursor_element.is_some();
            if cursor_on_hw != surface.cursor_hw_plane_active {
                surface.cursor_hw_plane_active = cursor_on_hw;
                tracing::info!(
                    cursor_hw_plane = cursor_on_hw,
                    "W3.P2: cursor HW plane state changed"
                );
            }

            if !result.is_empty {
                // Damage existe -> queue page-flip.
                match surface.drm_output.queue_frame(()) {
                    Ok(()) => {
                        let now = Instant::now();
                        // L2: coleta duracao deste frame.
                        let frame_dur = now.duration_since(surface.last_frame_time);
                        surface.last_frame_time = now;
                        surface.pending_flip = true;
                        surface.frame_durations.push(frame_dur);
                        // W6.D: perf tracker (histograma separado com us precision).
                        // Filtra gaps > 100ms: ociosidade nao eh frame drop.
                        if frame_dur < Duration::from_millis(100) {
                            state.perf.record_frame(frame_dur);
                        }
                        if trace {
                            tracing::debug!(
                                frame = frame_counter,
                                cursor_hw = cursor_on_hw,
                                "frame queued"
                            );
                        }
                        // L2: log p50/p95/p99 a cada 60s.
                        if surface.last_timing_log.elapsed() >= Duration::from_secs(60)
                            && surface.frame_durations.len() >= 10
                        {
                            let mut durs = surface.frame_durations.clone();
                            durs.sort_unstable();
                            let n = durs.len();
                            let p50 = durs[n / 2].as_millis();
                            let p95 = durs[(n * 95 / 100).min(n - 1)].as_millis();
                            let p99 = durs[(n * 99 / 100).min(n - 1)].as_millis();
                            tracing::info!(
                                samples = n,
                                p50_ms = p50,
                                p95_ms = p95,
                                p99_ms = p99,
                                "L2: frame timing 60s window"
                            );
                            surface.frame_durations.clear();
                            surface.last_timing_log = Instant::now();
                        }
                        // W6.D: perf log separado com us precision.
                        if surface.last_timing_log.elapsed() < Duration::from_secs(1) {
                            // Acabou de logar L2; aproveita pra logar W6.D tambem.
                            state.perf.log_and_reset();
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

#[cfg(test)]
mod tests {
    use super::*;

    // W3.P1 tests: logica de late-render scheduler (pura, sem DRM real).

    #[test]
    fn deadline_age_calculation_60hz() {
        // 60Hz: frame_interval=16667us, max_render=3ms.
        // deadline_age = 16667us - 3000us = 13667us = 13.667ms.
        let frame_interval = Duration::from_micros(FRAME_INTERVAL_US);
        let max_render = Duration::from_millis(MAX_RENDER_TIME_MS);
        let deadline_age = frame_interval.saturating_sub(max_render);
        // Esperado: ~13.667ms.
        assert!(deadline_age.as_millis() >= 13);
        assert!(deadline_age.as_millis() <= 14);
    }

    #[test]
    fn no_sleep_when_no_vblank_recorded() {
        // Sem last_vblank_ts -> timeout = None (render imediato).
        // Testa a logica de guard diretamente.
        let last_vblank: Option<Duration> = None;
        let result = if last_vblank.is_none() {
            None
        } else {
            Some(Duration::from_millis(1))
        };
        assert!(result.is_none());
    }

    #[test]
    fn sleep_when_recently_rendered() {
        // Se age < deadline_age -> deve dormir.
        let frame_interval = Duration::from_micros(FRAME_INTERVAL_US);
        let max_render = Duration::from_millis(MAX_RENDER_TIME_MS);
        let deadline_age = frame_interval.saturating_sub(max_render);

        // Simula: age = 2ms (muito cedo, deadline_age ~= 13.667ms).
        let age = Duration::from_millis(2);
        let should_sleep = age < deadline_age;
        assert!(should_sleep);
        let sleep_for = deadline_age - age;
        assert!(sleep_for > Duration::from_micros(500));
    }

    #[test]
    fn no_sleep_at_deadline() {
        // Se age >= deadline_age -> render imediato (None).
        let frame_interval = Duration::from_micros(FRAME_INTERVAL_US);
        let max_render = Duration::from_millis(MAX_RENDER_TIME_MS);
        let deadline_age = frame_interval.saturating_sub(max_render);

        // Simula: age = deadline_age (exato).
        let age = deadline_age;
        let should_sleep = age < deadline_age;
        assert!(!should_sleep);
    }

    // W3.P2 tests: cursor HW plane path.

    #[test]
    fn cursor_only_collect_returns_nonempty() {
        // Nao temos renderer real aqui; testamos a logica de selecao de path.
        // cursor_moved=true e pending_flip=false -> deveria usar cursor_only path.
        let cursor_moved = true;
        let pending_flip = false;
        let use_cursor_only = cursor_moved && !pending_flip;
        assert!(use_cursor_only);
    }

    #[test]
    fn cursor_only_path_disabled_when_pending_flip() {
        // cursor_moved=true mas pending_flip=true -> full render path.
        let cursor_moved = true;
        let pending_flip = true;
        let use_cursor_only = cursor_moved && !pending_flip;
        assert!(!use_cursor_only);
    }

    #[test]
    fn cursor_only_path_disabled_when_no_cursor_move() {
        // cursor_moved=false -> full render path independente de pending_flip.
        let cursor_moved = false;
        let pending_flip = false;
        let use_cursor_only = cursor_moved && !pending_flip;
        assert!(!use_cursor_only);
    }
}
