//! W8.A fix: cache de pixels do ultimo frame composto para zwlr-screencopy-v1.
//!
//! Problema: a implementacao anterior do screencopy escrevia uma cor fixa
//! (#131318) no buffer shm do client, ignorando o conteudo real renderizado.
//! O codigo placeholder estava em handlers/screencopy.rs `do_copy`.
//!
//! Fix: depois de `render_frame` ter sucesso no `render_drm`, renderizamos
//! o MESMO conjunto de elementos num GlesRenderbuffer off-screen, lemos os
//! pixels de volta via `copy_framebuffer` + `map_texture`, e armazenamos
//! Vec<u8> BGRA8888 nesse cache. O handler screencopy entao serve esse
//! buffer pro client sem precisar tocar no pipeline DRM/scanout.
//!
//! Custo: 1 GPU re-render + 1 PBO readback por frame APENAS quando ha
//! cliente screencopy ativo. Modo "armado" liga via `arm()` na primeira
//! requisicao do client; render_drm verifica `is_armed()` antes de pagar
//! o custo. Apos N segundos sem novo `arm()` o cache desarma sozinho.
//!
//! Path do plano original = "c" (render manual em shadow buffer). Mais
//! simples que (a) porque reutiliza a mesma `OutputDamageTracker` +
//! lista de elementos ja preparada pra `render_frame`. Evita (b) que
//! depende de comportamento especifico do DrmCompositor multi-plane.

use std::time::{Duration, Instant};

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::gles::{GlesRenderbuffer, GlesRenderer};
use smithay::backend::renderer::{Bind, Color32F, ExportMem, Offscreen};
use smithay::output::Output;
use smithay::utils::{Buffer as BufferCoord, Physical, Rectangle, Size, Transform};

use crate::backend::render_common::LumoCustomElement;

/// Tempo apos ultimo arm() pra desarmar o cache (economiza GPU).
const ARM_TTL: Duration = Duration::from_secs(3);

/// Cache off-screen de pixels do framebuffer composto.
pub struct ScreencopyCache {
    renderbuffer: GlesRenderbuffer,
    damage: OutputDamageTracker,
    /// BGRA8888 pixels do ultimo refresh. stride = width * 4.
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Quando ha ao menos um client ativo, armed = true.
    /// Quando expira, render_drm para de pagar custo de re-render.
    armed_until: Option<Instant>,
}

impl ScreencopyCache {
    /// Cria cache novo dimensionado pra output atual. Aloca renderbuffer
    /// GBM Argb8888.
    pub fn new(
        renderer: &mut GlesRenderer,
        width: u32,
        height: u32,
    ) -> Result<Self, smithay::backend::renderer::gles::GlesError> {
        let size_buf: Size<i32, BufferCoord> = (width as i32, height as i32).into();
        let rb = renderer.create_buffer(Fourcc::Argb8888, size_buf)?;
        let size_phys: Size<i32, Physical> = (width as i32, height as i32).into();
        let damage = OutputDamageTracker::new(size_phys, 1.0, Transform::Normal);
        Ok(Self {
            renderbuffer: rb,
            damage,
            pixels: vec![0u8; (width * height * 4) as usize],
            width,
            height,
            armed_until: None,
        })
    }

    /// Marca cache como "ativo": render_drm vai atualizar a cada frame.
    pub fn arm(&mut self) {
        self.armed_until = Some(Instant::now() + ARM_TTL);
    }

    /// True se ainda deve pagar custo de re-render. Auto-desarma apos TTL.
    pub fn is_armed(&self) -> bool {
        match self.armed_until {
            Some(until) => Instant::now() < until,
            None => false,
        }
    }

    /// Renderiza elementos no renderbuffer off-screen e copia pixels pra
    /// `self.pixels` em BGRA8888. Chamado dentro de render_drm depois de
    /// render_frame succeed (mesma lista de elementos).
    pub fn refresh(
        &mut self,
        renderer: &mut GlesRenderer,
        _output: &Output,
        elements: &[LumoCustomElement],
        clear: [f32; 4],
    ) -> Result<(), CacheError> {
        // 1. Bind renderbuffer como framebuffer ativo.
        let mut fb = renderer
            .bind(&mut self.renderbuffer)
            .map_err(CacheError::Bind)?;

        // 2. Render full-damage (queremos buffer completo todo frame). Como
        // o cache eh consumido raramente (grim 1x), reaproveitar damage
        // tracker daria parcial damage que truncaria conteudo. Reset damage.
        let size_phys: Size<i32, Physical> = (self.width as i32, self.height as i32).into();
        self.damage = OutputDamageTracker::new(size_phys, 1.0, Transform::Normal);

        let _result = self
            .damage
            .render_output::<LumoCustomElement, GlesRenderer>(
                renderer,
                &mut fb,
                0,
                elements,
                Color32F::new(clear[0], clear[1], clear[2], clear[3]),
            )
            .map_err(|e| CacheError::Render(format!("{e:?}")))?;

        // 3. Copy framebuffer pixels pra mapping (PBO readback).
        let region: Rectangle<i32, BufferCoord> =
            Rectangle::from_size((self.width as i32, self.height as i32).into());
        let mapping = renderer
            .copy_framebuffer(&fb, region, Fourcc::Argb8888)
            .map_err(CacheError::Copy)?;

        // Drop fb (release bind) ANTES de map_texture: map_texture muda bind state.
        drop(fb);

        let bytes = renderer.map_texture(&mapping).map_err(CacheError::Map)?;

        let need = (self.width * self.height * 4) as usize;
        if bytes.len() < need {
            return Err(CacheError::ShortRead {
                got: bytes.len(),
                need,
            });
        }
        if self.pixels.len() != need {
            self.pixels.resize(need, 0);
        }
        self.pixels.copy_from_slice(&bytes[..need]);
        Ok(())
    }
}

#[derive(Debug)]
pub enum CacheError {
    Bind(smithay::backend::renderer::gles::GlesError),
    Render(String),
    Copy(smithay::backend::renderer::gles::GlesError),
    Map(smithay::backend::renderer::gles::GlesError),
    ShortRead { got: usize, need: usize },
}

impl std::fmt::Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheError::Bind(e) => write!(f, "bind renderbuffer: {e:?}"),
            CacheError::Render(e) => write!(f, "render_output: {e}"),
            CacheError::Copy(e) => write!(f, "copy_framebuffer: {e:?}"),
            CacheError::Map(e) => write!(f, "map_texture: {e:?}"),
            CacheError::ShortRead { got, need } => {
                write!(f, "map_texture short read: got {got} need {need}")
            }
        }
    }
}
impl std::error::Error for CacheError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arm_then_armed_true() {
        // Cant build a real cache without GLES; testa logica armed_until.
        let mut c = StubArmed { until: None };
        assert!(!c.is_armed());
        c.arm();
        assert!(c.is_armed());
    }

    struct StubArmed {
        until: Option<Instant>,
    }
    impl StubArmed {
        fn arm(&mut self) {
            self.until = Some(Instant::now() + ARM_TTL);
        }
        fn is_armed(&self) -> bool {
            match self.until {
                Some(u) => Instant::now() < u,
                None => false,
            }
        }
    }

    #[test]
    fn arm_expires_after_ttl_logic() {
        let past = Instant::now() - Duration::from_secs(10);
        let c = StubArmed { until: Some(past) };
        assert!(!c.is_armed());
    }
}
