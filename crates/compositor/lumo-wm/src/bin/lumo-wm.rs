//! lumo-wm entry point - inicia Wayland display, socket nested,
//! IPC unix socket, backend (winit OU drm via env LUMO_WM_BACKEND),
//! registra handlers e roda event loop.
//!
//! Fase 5.5 (A8): seleciona backend por env. DRM precisa feature
//! `drm-backend` no build; winit eh sempre o default seguro.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use anyhow::Result;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::calloop::EventLoop;
use smithay::reexports::wayland_server::Display;

use lumo_wm::{init_socket, LumoState};

#[derive(Debug, Clone, Copy)]
enum BackendChoice {
    Winit,
    Drm,
}

fn pick_backend() -> BackendChoice {
    match std::env::var("LUMO_WM_BACKEND").as_deref() {
        Ok("drm") => BackendChoice::Drm,
        Ok("winit") | Err(_) => BackendChoice::Winit,
        Ok(other) => {
            tracing::warn!(value = other, "LUMO_WM_BACKEND desconhecido, usando winit");
            BackendChoice::Winit
        }
    }
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "lumo_wm=info,smithay=warn,wgpu=warn".into()),
        )
        .init();

    let backend = pick_backend();
    tracing::info!(
        "Lumo WM 0.1.0 - Fase 5.5 (A8: DRM backend + IPC + moldura) backend={:?}",
        backend
    );

    let mut event_loop: EventLoop<'static, LumoState> = EventLoop::try_new()?;
    let display: Display<LumoState> = Display::new()?;
    let display_handle = display.handle();

    let socket_name = match init_socket(&event_loop.handle(), &display_handle) {
        Ok(name) => Some(name),
        Err(err) => {
            tracing::warn!(?err, "Falha ao abrir socket Wayland; rodando sem");
            None
        }
    };
    if let Some(s) = socket_name.as_ref() {
        tracing::info!(socket = %s, "Wayland socket: WAYLAND_DISPLAY={}", s);
    }

    let mut state = LumoState::new(display_handle, event_loop.handle(), socket_name.clone());

    // IPC socket: best-effort. Falha = avisa e continua sem.
    match lumo_wm::ipc::init(event_loop.handle()) {
        Ok(ipc) => {
            state.ipc = ipc;
        }
        Err(err) => {
            tracing::warn!(?err, "IPC desativado");
        }
    }

    // Backend dispatch.
    match backend {
        BackendChoice::Winit => {
            let _winit_data = lumo_wm::backend::winit::init(event_loop.handle(), &mut state)?;
        }
        BackendChoice::Drm => {
            #[cfg(feature = "drm-backend")]
            {
                lumo_wm::backend::drm::run(&mut event_loop, &mut state)?;
                // run() bloqueia tudo; em DRM o proprio backend
                // orquestra o event loop. Saimos aqui depois.
                tracing::info!("Lumo WM saiu do backend DRM");
                return Ok(());
            }
            #[cfg(not(feature = "drm-backend"))]
            {
                anyhow::bail!(
                    "LUMO_WM_BACKEND=drm pediu DRM backend mas binario nao foi compilado \
                     com --features drm-backend. Rebuild: cargo build --release \
                     --features drm-backend --bin lumo-wm"
                );
            }
        }
    }

    if let Some(s) = socket_name.as_ref() {
        std::env::set_var("WAYLAND_DISPLAY", s);
    }

    let display = Rc::new(RefCell::new(display));

    // Timer 4ms: dispatch Wayland + tick IPC.
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
                drop(d);
                // Tick IPC (drain read+write de clients).
                lumo_wm::ipc::tick(state);
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

    // Cleanup socket IPC.
    if let Some(path) = state.ipc.socket_path.take() {
        let _ = std::fs::remove_file(path);
    }

    tracing::info!("Lumo WM saindo");
    Ok(())
}
