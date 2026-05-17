//! Backend DRM/KMS - lumo-wm full session direto no hardware.
//!
//! ETAPA 1 (A8 atual): scaffolding + bring-up de session.
//!   - LibSeatSession::new (seatd/logind)
//!   - udev::Enumerator pra encontrar primary GPU
//!   - DrmNode::from_path + abrir DrmDevice
//!   - log de connectors/CRTCs disponiveis
//!   - sai cleanly se nao tem TTY (smoke test friendly)
//!
//! ETAPA 2 (proxima): page-flip loop completo
//!   - GbmDevice + DrmCompositor + GlesRenderer
//!   - render_output igual ao winit.rs
//!   - libinput backend pra teclado/mouse direto via /dev/input
//!   - timer fallback frame
//!
//! ETAPA 3: hot-plug, multi-output, dmabuf import.
//!
//! Justificativa do gating em etapas (memory feedback_design_lapidado):
//! - portar 1500 linhas de anvil/udev.rs num soco vira festival de
//!   bugs que so aparecem em TTY fisico (sem repro via SSH);
//! - Etapa 1 isola tudo que da pra testar via SSH (session bring-up)
//!   antes de empilhar render path. Falha aqui = bug em camada simples.
//!
//! Refs: smithay anvil src/udev.rs (commit master de 2026-05),
//! smithay 0.7.0 docs.

use anyhow::{anyhow, Result};
use smithay::backend::session::libseat::LibSeatSession;
use smithay::backend::session::Session;
use smithay::backend::udev::{all_gpus, primary_gpu};
use smithay::reexports::calloop::EventLoop;
use smithay::reexports::drm::node::{DrmNode, NodeType};

use crate::state::LumoState;

/// Entry point do backend DRM. Bloqueia ate sair.
pub fn run(
    _event_loop: &mut EventLoop<'static, LumoState>,
    _state: &mut LumoState,
) -> Result<()> {
    tracing::info!("DRM backend Etapa 1: bring-up de session + enumeracao de GPU");

    // 1. LibSeatSession - precisa de seatd rodando OU logind ativo.
    //    Fora de TTY (ex: SSH em sessao desktop GUI) tipicamente
    //    erra com "no compatible seat".
    let (session, _notifier) = LibSeatSession::new()
        .map_err(|e| anyhow!("LibSeatSession::new falhou: {e}. Possiveis causas: nao esta em TTY (precisa Ctrl+Alt+F3), seatd nao instalado, ou XDG_SESSION_TYPE incorreto."))?;
    let seat_name = session.seat();
    tracing::info!(seat = %seat_name, "session libseat ok");

    // 2. Localiza GPU primaria.
    let primary = primary_gpu(&seat_name)
        .map_err(|e| anyhow!("primary_gpu falhou: {e}"))?
        .and_then(|p| DrmNode::from_path(&p).ok())
        .and_then(|node| node.node_with_type(NodeType::Render).and_then(|r| r.ok()))
        .or_else(|| {
            // Fallback: pega primeira GPU disponivel.
            all_gpus(&seat_name)
                .ok()?
                .into_iter()
                .find_map(|p| DrmNode::from_path(p).ok())
        })
        .ok_or_else(|| anyhow!("nenhuma GPU achada via udev"))?;

    tracing::info!(
        primary_gpu = %primary,
        "primary GPU localizada via udev"
    );

    // 3. Anuncia o que viria a seguir (Etapa 2).
    tracing::warn!(
        "DRM backend Etapa 1 completou bring-up. Render path (Etapa 2) ainda nao \
         implementado. Saindo cleanly com codigo 0. Use LUMO_WM_BACKEND=winit pra \
         sessao usavel."
    );

    // Cleanup explicito antes de sair pra liberar libseat.
    drop(session);
    Ok(())
}
