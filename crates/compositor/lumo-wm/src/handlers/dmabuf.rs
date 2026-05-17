//! linux-dmabuf-v1 - importa buffers GPU de clients (Firefox, Chrome,
//! GPUI apps). Sem isso clients EGL/Vulkan recusam abrir porque so
//! oferecemos wl_shm.
//!
//! A10 frente 1.
//!
//! Fluxo:
//! - DmabufState criado no LumoState::new (sem global ainda).
//! - Backend (winit/drm) cria GlesRenderer + DmabufGlobal apos EGL up.
//! - dmabuf_imported chamado quando cliente envia DMABUF -> tentamos
//!   importar via GlesRenderer ativo (winit_backend ou drm_backend).
//! - Sucesso = client recebe wl_buffer; falha = client cai pra wl_shm.
//!
//! Memory feedback_design_lapidado: formato + modifiers vem direto do
//! EGLContext.dmabuf_render_formats() (Mesa decide o que driver
//! Intel/i915 aceita: LINEAR + X_TILED + Y_TILED auto). Sem hardcode.

use smithay::{
    backend::{allocator::dmabuf::Dmabuf, renderer::ImportDma},
    delegate_dmabuf,
    wayland::dmabuf::{DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier},
};

use crate::state::LumoState;

impl DmabufHandler for LumoState {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.dmabuf_state
    }

    fn dmabuf_imported(
        &mut self,
        _global: &DmabufGlobal,
        dmabuf: Dmabuf,
        notifier: ImportNotifier,
    ) {
        // Tenta importar via renderer ativo. winit_backend tem
        // prioridade quando ambos existem (nao acontece na pratica,
        // mas defensivo).
        let result: Result<(), String> = {
            // Path 1: winit (nested).
            if let Some(backend_rc) = self.winit_backend.clone() {
                let mut bk = backend_rc.borrow_mut();
                let renderer = bk.renderer();
                match renderer.import_dmabuf(&dmabuf, None) {
                    Ok(_tex) => Ok(()),
                    Err(e) => Err(format!("winit import_dmabuf: {e:?}")),
                }
            } else {
                #[cfg(feature = "drm-backend")]
                {
                    if let Some(drm) = self.drm_backend.as_mut() {
                        match drm.renderer.import_dmabuf(&dmabuf, None) {
                            Ok(_tex) => Ok(()),
                            Err(e) => Err(format!("drm import_dmabuf: {e:?}")),
                        }
                    } else {
                        Err("sem renderer ativo (drm_backend None)".into())
                    }
                }
                #[cfg(not(feature = "drm-backend"))]
                {
                    Err("sem renderer ativo".into())
                }
            }
        };

        match result {
            Ok(()) => {
                tracing::debug!("dmabuf import ok");
                let _ = notifier.successful::<LumoState>();
            }
            Err(e) => {
                tracing::warn!(error = %e, "dmabuf import falhou");
                notifier.failed();
            }
        }
    }
}

// BufferHandler ja implementado em handlers/compositor.rs; delegate_dmabuf
// pede que LumoState implemente BufferHandler -- isso ja existe.
// Nao re-implementamos aqui (gera conflict).

delegate_dmabuf!(LumoState);
