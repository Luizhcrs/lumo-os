//! Backend winit - roda lumo-wm como cliente Wayland dentro do Hyprland.
//!
//! Cria janela 1280x720 (o "monitor virtual" do Lumo WM), inicializa
//! GlesRenderer e registra um Output Smithay. O proprio WinitEventLoop
//! implementa `calloop::EventSource`, entao basta `insert_source`.
//!
//! Em Fase 5.1 nao desenhamos clientes ainda - render real entra na 5.3
//! quando lumo-gfx-core for plugado.

use anyhow::{anyhow, Result};
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::winit::{self, WinitEvent, WinitGraphicsBackend};
use smithay::output::{Mode, Output, PhysicalProperties, Subpixel};
use smithay::reexports::calloop::LoopHandle;
use smithay::utils::Transform;

use crate::state::LumoState;

const OUTPUT_NAME: &str = "lumo-winit-0";
const REFRESH_MHZ: i32 = 60_000;
/// Clear color (futuro): emerald-600 (#10b981) ~linear.
#[allow(dead_code)]
const CLEAR: [f32; 4] = [0.06, 0.73, 0.51, 1.0];

pub struct WinitData {
    pub backend: WinitGraphicsBackend<GlesRenderer>,
    pub damage_tracker: OutputDamageTracker,
    pub output: Output,
}

/// Inicializa backend winit, registra output em LumoState, conecta
/// WinitEventLoop ao calloop.
pub fn init(
    loop_handle: LoopHandle<'static, LumoState>,
    state: &mut LumoState,
) -> Result<WinitData> {
    let (backend, winit_loop) = winit::init::<GlesRenderer>()
        .map_err(|e| anyhow!("falha init winit backend: {e:?}"))?;

    let size = backend.window_size();
    let mode = Mode {
        size,
        refresh: REFRESH_MHZ,
    };

    let output = Output::new(
        OUTPUT_NAME.to_string(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: "Lumo".into(),
            model: "Winit".into(),
        },
    );
    let _global = output.create_global::<LumoState>(&state.display_handle);
    output.change_current_state(
        Some(mode),
        Some(Transform::Flipped180),
        None,
        Some((0, 0).into()),
    );
    output.set_preferred(mode);
    state.space.map_output(&output, (0, 0));

    let damage_tracker = OutputDamageTracker::from_output(&output);

    // WinitEventLoop implementa calloop::EventSource - drop direto no loop.
    loop_handle
        .insert_source(winit_loop, move |event, _, state| match event {
            WinitEvent::Resized { size, .. } => {
                tracing::debug!(?size, "winit resized");
            }
            WinitEvent::Input(_ev) => {
                // Input dispatch entra na Fase 5.2.
            }
            WinitEvent::CloseRequested => {
                tracing::info!("winit close requested");
                state.running = false;
            }
            _ => {}
        })
        .map_err(|e| anyhow!("falha ao registrar winit event source: {e}"))?;

    Ok(WinitData {
        backend,
        damage_tracker,
        output,
    })
}
