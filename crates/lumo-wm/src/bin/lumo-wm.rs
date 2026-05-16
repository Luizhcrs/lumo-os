//! lumo-wm entry point - inicia Wayland display, socket nested,
//! backend winit, registra handlers e roda event loop.
//!
//! Fase 5.3: dispatch_clients periodico via timer pra evitar
//! peer-reset em clientes idle. Antes o dispatch so rodava apos um
//! event source disparar - clientes que enviavam request enquanto o
//! loop estava em sleep ficavam pendentes ate o proximo evento.
//! Agora um Timer 4ms forca `dispatch_clients + flush_clients`,
//! cortando latencia de protocolo.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use anyhow::Result;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::calloop::EventLoop;
use smithay::reexports::wayland_server::Display;

use lumo_wm::{init_socket, LumoState};

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "lumo_wm=info,smithay=warn,wgpu=warn".into()),
        )
        .init();

    tracing::info!(
        "Lumo WM 0.1.0 - Fase 5.4 (cursor unico + SUPER keybinds + lumo-bar configure)"
    );

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
        tracing::info!(socket = %s, "Lumo WM aceitando clientes Wayland em WAYLAND_DISPLAY={}", s);
    }

    let mut state = LumoState::new(display_handle, event_loop.handle(), socket_name.clone());

    let _winit_data = lumo_wm::backend::winit::init(event_loop.handle(), &mut state)?;

    if let Some(s) = socket_name.as_ref() {
        std::env::set_var("WAYLAND_DISPLAY", s);
    }

    // Display em Rc<RefCell<...>> pra compartilhar entre o timer de
    // dispatch + o callback do event_loop.
    let display = Rc::new(RefCell::new(display));

    // Timer pra dispatch_clients periodico (4ms = 250Hz). Garante
    // que requests/responses Wayland fluem mesmo quando nao tem
    // events na fila (calloop em sleep aguardando timer maior).
    // Sem isso clientes idle podem disparar timeout interno e
    // bater "Connection reset by peer" no compositor.
    let display_for_timer = display.clone();
    event_loop
        .handle()
        .insert_source(
            Timer::from_duration(Duration::from_millis(4)),
            move |_, _, state: &mut LumoState| {
                if !state.running {
                    return TimeoutAction::Drop;
                }
                let mut d = display_for_timer.borrow_mut();
                if let Err(err) = d.dispatch_clients(state) {
                    tracing::warn!(?err, "dispatch_clients periodico falhou");
                }
                let _ = d.flush_clients();
                TimeoutAction::ToDuration(Duration::from_millis(4))
            },
        )
        .map_err(|e| anyhow::anyhow!("falha ao registrar timer dispatch: {e}"))?;

    let display_for_loop = display.clone();
    event_loop.run(None, &mut state, move |state| {
        if !state.running {
            return;
        }
        let mut d = display_for_loop.borrow_mut();
        let _ = d.dispatch_clients(state);
        let _ = d.flush_clients();
    })?;

    tracing::info!("Lumo WM saindo");
    Ok(())
}
