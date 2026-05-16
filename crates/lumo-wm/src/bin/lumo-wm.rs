//! lumo-wm entry point - inicia Wayland display, socket nested,
//! backend winit, registra handlers e roda event loop.

use anyhow::Result;
use smithay::reexports::calloop::EventLoop;
use smithay::reexports::wayland_server::Display;

use lumo_wm::{init_socket, LumoState};

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "lumo_wm=debug,smithay=info".into()),
        )
        .init();

    tracing::info!("Lumo WM 0.1.0 - Fase 5.1 (nested winit)");

    let mut event_loop: EventLoop<'static, LumoState> = EventLoop::try_new()?;
    let display: Display<LumoState> = Display::new()?;
    let display_handle = display.handle();

    let socket_name = match init_socket(&event_loop.handle(), &display_handle) {
        Ok(name) => Some(name),
        Err(err) => {
            tracing::warn!(?err, "Falha ao abrir socket; rodando sem socket publico");
            None
        }
    };

    if let Some(s) = socket_name.as_ref() {
        tracing::info!(socket = %s, "Lumo WM aceitando clientes Wayland");
        std::env::set_var("WAYLAND_DISPLAY", s);
    }

    let mut state = LumoState::new(display_handle, event_loop.handle(), socket_name);

    let _winit_data = lumo_wm::backend::winit::init(event_loop.handle(), &mut state)?;

    let display = std::cell::RefCell::new(display);
    event_loop.run(None, &mut state, move |state| {
        if !state.running {
            // Sair do loop suavemente.
            return;
        }
        // Flush display - garante que respostas Wayland saem.
        let mut d = display.borrow_mut();
        let _ = d.dispatch_clients(state);
        let _ = d.flush_clients();
    })?;

    tracing::info!("Lumo WM saindo");
    Ok(())
}
