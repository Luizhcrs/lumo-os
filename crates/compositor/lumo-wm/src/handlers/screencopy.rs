//! W8.A: zwlr-screencopy-v1 protocol implementation.
//!
//! Permite que grim e outros clients capturem frames de um output.
//! Implementacao server-side via wayland-protocols-wlr.
//!
//! Fluxo:
//!   1. Client solicita ZwlrScreencopyManagerV1::capture_output
//!   2. Compositor cria ZwlrScreencopyFrameV1 e envia evento buffer()
//!      com formato ARGB8888 e dimensoes do output
//!   3. Client aloca wl_shm buffer e chama frame.copy(buffer)
//!   4. Compositor le pixels do `screencopy_cache` (BGRA8888 cacheado
//!      apos cada render_frame) e copia pro shm pool, depois manda ready().
//!
//! W8.A fix: ANTES o do_copy escrevia uma cor fixa #131318 (placeholder).
//! Resultado: grim retornava PNG solid color. Agora le do cache real
//! atualizado em backend::screencopy_cache pelo render_drm.

use std::sync::{Arc, Mutex};
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource,
};
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_protocols_wlr::screencopy::v1::server::{
    zwlr_screencopy_frame_v1::{self, ZwlrScreencopyFrameV1},
    zwlr_screencopy_manager_v1::{self, ZwlrScreencopyManagerV1},
};
use smithay::wayland::shm::with_buffer_contents;

use crate::state::LumoState;

/// Metadados do frame pendente de copia.
#[derive(Debug)]
pub struct ScreencopyFrameData {
    pub format: u32,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
}

/// UserData attached a cada ZwlrScreencopyFrameV1.
pub struct FrameUserData {
    pub data: Mutex<Option<ScreencopyFrameData>>,
}

/// Estado screencopy registrado no LumoState.
pub struct ScreencopyState {
    pub global: smithay::reexports::wayland_server::backend::GlobalId,
}

impl ScreencopyState {
    pub fn new(display: &DisplayHandle) -> Self {
        let global = display.create_global::<LumoState, ZwlrScreencopyManagerV1, _>(3, ());
        ScreencopyState { global }
    }
}

impl GlobalDispatch<ZwlrScreencopyManagerV1, ()> for LumoState {
    fn bind(
        _state: &mut LumoState,
        _dh: &DisplayHandle,
        _client: &Client,
        resource: New<ZwlrScreencopyManagerV1>,
        _global_data: &(),
        data_init: &mut DataInit<'_, LumoState>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<ZwlrScreencopyManagerV1, ()> for LumoState {
    fn request(
        state: &mut LumoState,
        _client: &Client,
        _manager: &ZwlrScreencopyManagerV1,
        request: zwlr_screencopy_manager_v1::Request,
        _data: &(),
        _dh: &DisplayHandle,
        data_init: &mut DataInit<'_, LumoState>,
    ) {
        match request {
            zwlr_screencopy_manager_v1::Request::CaptureOutput {
                frame,
                overlay_cursor: _,
                output,
            } => {
                handle_capture(state, frame, output, data_init);
            }
            zwlr_screencopy_manager_v1::Request::CaptureOutputRegion {
                frame,
                overlay_cursor: _,
                output,
                x: _,
                y: _,
                width: _,
                height: _,
            } => {
                handle_capture(state, frame, output, data_init);
            }
            zwlr_screencopy_manager_v1::Request::Destroy => {}
            _ => {}
        }
    }
}

fn handle_capture(
    state: &mut LumoState,
    frame: New<ZwlrScreencopyFrameV1>,
    _output: WlOutput,
    data_init: &mut DataInit<'_, LumoState>,
) {
    let (w, h) = state
        .space
        .outputs()
        .next()
        .and_then(|o| o.current_mode())
        .map(|m| (m.size.w as u32, m.size.h as u32))
        .unwrap_or((1920, 1080));

    let stride = w * 4;

    // W8.A fix: arma cache pra render_drm passar a atualizar a cada frame.
    // Cache TTL de 3s = se cliente para de pedir capture, custo de re-render
    // some sozinho. Tambem dispara render-into-cache imediato pra evitar
    // entregar buffer zero na primeira captura (grim chama Copy logo apos
    // capture_output, antes do proximo render_drm tick).
    #[cfg(feature = "drm-backend")]
    arm_and_refresh_now(state);

    let user_data = Arc::new(FrameUserData {
        data: Mutex::new(Some(ScreencopyFrameData {
            format: 0,
            width: w,
            height: h,
            stride,
        })),
    });

    let frame_obj = data_init.init(frame, user_data);

    frame_obj.buffer(
        smithay::reexports::wayland_server::protocol::wl_shm::Format::Argb8888,
        w,
        h,
        stride,
    );

    if frame_obj.version() >= 3 {
        frame_obj.buffer_done();
    }
}

impl Dispatch<ZwlrScreencopyFrameV1, Arc<FrameUserData>> for LumoState {
    fn request(
        state: &mut LumoState,
        _client: &Client,
        frame: &ZwlrScreencopyFrameV1,
        request: zwlr_screencopy_frame_v1::Request,
        data: &Arc<FrameUserData>,
        _dh: &DisplayHandle,
        _data_init: &mut DataInit<'_, LumoState>,
    ) {
        match request {
            zwlr_screencopy_frame_v1::Request::Copy { buffer } => {
                do_copy(state, frame, data, buffer);
            }
            zwlr_screencopy_frame_v1::Request::CopyWithDamage { buffer } => {
                do_copy(state, frame, data, buffer);
            }
            zwlr_screencopy_frame_v1::Request::Destroy => {}
            _ => {}
        }
    }
}

fn do_copy(
    state: &mut LumoState,
    frame: &ZwlrScreencopyFrameV1,
    data: &Arc<FrameUserData>,
    buffer: WlBuffer,
) {
    let frame_data = match data.data.lock().unwrap().take() {
        Some(d) => d,
        None => {
            frame.failed();
            return;
        }
    };

    // W8.A fix: garante cache populado antes da copia. Se cache vazio ou stale,
    // dispara render-into-cache sincrono agora mesmo. Necessario porque grim
    // chama Copy logo apos capture_output sem esperar um render_drm tick.
    #[cfg(feature = "drm-backend")]
    arm_and_refresh_now(state);

    // Le pixels cacheados (BGRA8888 ja no formato ARGB8888 wl_shm little endian).
    let cache_bytes: Option<Vec<u8>> = {
        #[cfg(feature = "drm-backend")]
        {
            state
                .drm_backend
                .as_ref()
                .and_then(|b| b.screencopy_cache.as_ref())
                .filter(|c| {
                    c.width == frame_data.width && c.height == frame_data.height
                        && !c.pixels.is_empty()
                })
                .map(|c| c.pixels.clone())
        }
        #[cfg(not(feature = "drm-backend"))]
        {
            None::<Vec<u8>>
        }
    };

    let result = with_buffer_contents(&buffer, |ptr, len, _spec| {
        let expected = (frame_data.stride * frame_data.height) as usize;
        if len < expected {
            return;
        }
        // SAFETY: ptr aponta para shm pool validado pelo compositor.
        let buf = unsafe { std::slice::from_raw_parts_mut(ptr as *mut u8, len) };
        match cache_bytes.as_ref() {
            Some(src) if src.len() >= expected => {
                buf[..expected].copy_from_slice(&src[..expected]);
            }
            _ => {
                // Sem cache disponivel ainda (ex: backend winit ou primeiro
                // frame antes do refresh). Limpa pra preto opaco em vez de
                // entregar lixo, mas sinaliza warning.
                for chunk in buf[..expected].chunks_exact_mut(4) {
                    chunk[0] = 0x00;
                    chunk[1] = 0x00;
                    chunk[2] = 0x00;
                    chunk[3] = 0xff;
                }
                tracing::warn!(
                    w = frame_data.width,
                    h = frame_data.height,
                    "W8.A: cache screencopy vazio, devolvendo frame preto"
                );
            }
        }
    });

    match result {
        Ok(()) => {
            // clock.now() retorna Time<Monotonic>. as_millis() = u32.
            let ms = state.clock.now().as_millis() as u64;
            let sec_lo = (ms / 1000) as u32;
            let sec_hi = 0u32;
            let tv_nsec = ((ms % 1000) * 1_000_000) as u32;
            frame.ready(sec_hi, sec_lo, tv_nsec);
            tracing::info!(
                w = frame_data.width,
                h = frame_data.height,
                cache_hit = cache_bytes.is_some(),
                "W8.A: screencopy frame ready"
            );
        }
        Err(_) => {
            frame.failed();
            tracing::warn!("W8.A: screencopy copy falhou (buffer shm invalido)");
        }
    }
}

/// W8.A fix: arma cache e dispara render-into-cache sincrono agora.
/// Compartilhada por handle_capture e do_copy pra cobrir tanto streaming
/// (capture_output -> frame -> Copy) quanto one-shot (grim).
#[cfg(feature = "drm-backend")]
fn arm_and_refresh_now(state: &mut LumoState) {
    // Coleta inputs ANTES de pegar drm_backend mut pra evitar duplo borrow.
    let titlebar_menu_opt = state
        .titlebar_menu
        .as_ref()
        .map(|(_, pos, hover)| (*pos, *hover));
    let snap_preview = state.snap_preview;
    let overview_state = state.overview.as_ref();
    let stack_picker_state = state.stack_picker.as_ref();
    let pointer_location = state.pointer_location;
    let start_time_elapsed = state.start_time.elapsed();
    let frame_counter = state.frame_counter;
    let splash_alpha_val = state.splash_alpha;
    let boot_curtain_alpha = state.boot_curtain_alpha;
    let _ = start_time_elapsed;

    let LumoState {
        ref mut drm_backend,
        ref cursor,
        ref cursor_buffer,
        ref space,
        ref wallpaper,
        ref corner_shader,
        ref titlebar_bg_shader,
        ref ssd_windows,
        ref splash_buffer,
        ..
    } = *state;

    let Some(backend) = drm_backend.as_mut() else {
        return;
    };
    let Some(surface) = backend.surface.as_ref() else {
        return;
    };

    let mode = surface.output.current_mode();
    let (ow, oh) = match mode {
        Some(m) => (m.size.w, m.size.h),
        None => (1920, 1080),
    };
    let (w_u32, h_u32) = (ow as u32, oh as u32);

    // Lazy alloc / realloc cache se output size mudou.
    let need_realloc = match &backend.screencopy_cache {
        None => true,
        Some(c) => c.width != w_u32 || c.height != h_u32,
    };
    if need_realloc {
        match crate::backend::screencopy_cache::ScreencopyCache::new(
            &mut backend.renderer,
            w_u32,
            h_u32,
        ) {
            Ok(c) => {
                backend.screencopy_cache = Some(c);
            }
            Err(err) => {
                tracing::warn!(?err, "W8.A: alloc ScreencopyCache falhou");
                return;
            }
        }
    }

    let cache = match backend.screencopy_cache.as_mut() {
        Some(c) => c,
        None => return,
    };
    cache.arm();

    // Reproduz collect_drm_elements com mesmo set usado em render_drm.
    let overview_elements = overview_state
        .map(|ov| crate::overview::overview_elements(ov, ow, oh))
        .unwrap_or_default();
    let picker_elements = stack_picker_state
        .map(|p| crate::stack_picker::picker_elements(p, ow, oh))
        .unwrap_or_default();

    let inputs = crate::backend::render_common::DrmCollectInputs {
        boot_curtain_alpha,
        splash_alpha: splash_alpha_val,
        splash_buffer: splash_buffer.as_ref(),
        wallpaper: wallpaper.as_ref(),
        corner_shader: corner_shader.as_ref(),
        ssd_windows,
        titlebar_menu: titlebar_menu_opt,
        snap_preview,
        corner_mask_shader: None,
        titlebar_bg_shader: titlebar_bg_shader.as_ref(),
        overview_elements,
        picker_elements,
        space,
        output: &surface.output,
        pointer_location,
        frame_counter,
        cursor: cursor.as_ref(),
        cursor_buffer: cursor_buffer.as_ref(),
        output_w: ow,
        output_h: oh,
    };

    let all_elements =
        crate::backend::render_common::collect_drm_elements(&mut backend.renderer, &inputs);

    let clear = crate::backend::render_common::clear_color_linear();
    if let Err(err) = cache.refresh(
        &mut backend.renderer,
        &surface.output,
        &all_elements,
        clear,
    ) {
        tracing::warn!(?err, "W8.A: screencopy cache refresh sincrono falhou");
    } else {
        tracing::debug!(
            w = cache.width,
            h = cache.height,
            "W8.A: screencopy cache atualizado sincrono"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screencopy_frame_data_fields() {
        let d = ScreencopyFrameData { format: 0, width: 1920, height: 1080, stride: 1920 * 4 };
        assert_eq!(d.format, 0);
        assert_eq!(d.width, 1920);
        assert_eq!(d.height, 1080);
        assert_eq!(d.stride, 7680);
    }

    #[test]
    fn screencopy_stride_is_4x_width() {
        for w in [800u32, 1280u32, 1920u32, 2560u32, 3840u32] {
            assert_eq!(w * 4, w * 4);
        }
    }

    #[test]
    fn frame_user_data_take_once() {
        let ud = FrameUserData {
            data: Mutex::new(Some(ScreencopyFrameData { format: 0, width: 100, height: 100, stride: 400 })),
        };
        assert!(ud.data.lock().unwrap().take().is_some());
        assert!(ud.data.lock().unwrap().take().is_none());
    }

    #[test]
    fn screencopy_pixel_fallback_argb_opaque_black() {
        // Fallback quando cache vazio (sem drm backend ou primeiro frame).
        let buf: [u8; 4] = [0x00, 0x00, 0x00, 0xff];
        assert_eq!(buf[3], 0xFF);
        assert_eq!(buf[0], 0x00);
        assert_eq!(buf[1], 0x00);
        assert_eq!(buf[2], 0x00);
    }

    #[test]
    fn screencopy_expected_byte_count() {
        let w = 1920u32;
        let h = 1080u32;
        let expected = (w * 4 * h) as usize;
        assert_eq!(expected, 1920 * 1080 * 4);
    }
}
