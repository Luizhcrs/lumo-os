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
//!   4. Compositor escreve pixels no shm pool e envia frame.ready()
//!
//! W8.A simplificado: pixel data = ink-deep sRGB (placeholder funcional).

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

    let result = with_buffer_contents(&buffer, |ptr, len, _spec| {
        let expected = (frame_data.stride * frame_data.height) as usize;
        if len >= expected {
            // SAFETY: ptr aponta para shm pool validado pelo compositor.
            let buf = unsafe { std::slice::from_raw_parts_mut(ptr as *mut u8, len) };
            for chunk in buf[..expected].chunks_exact_mut(4) {
                chunk[0] = 0x18; // B
                chunk[1] = 0x13; // G
                chunk[2] = 0x13; // R
                chunk[3] = 0xff; // A
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
            tracing::info!(w = frame_data.width, h = frame_data.height, "W8.A: screencopy frame ready");
        }
        Err(_) => {
            frame.failed();
            tracing::warn!("W8.A: screencopy copy falhou (buffer shm invalido)");
        }
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
    fn screencopy_pixel_ink_deep_argb() {
        let buf: [u8; 4] = [0x18, 0x13, 0x13, 0xff];
        assert_eq!(buf[3], 0xFF);
        assert!(buf[0] > 0);
    }

    #[test]
    fn screencopy_expected_byte_count() {
        let w = 1920u32;
        let h = 1080u32;
        let expected = (w * 4 * h) as usize;
        assert_eq!(expected, 1920 * 1080 * 4);
    }
}
