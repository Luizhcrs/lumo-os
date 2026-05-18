//! lumo-wm entry point - inicia Wayland display, socket nested,
//! IPC unix socket, backend (winit OU drm via env LUMO_WM_BACKEND),
//! registra handlers e roda event loop.
//!
//! Fase 5.5 (A8): seleciona backend por env. DRM precisa feature
//! `drm-backend` no build; winit eh sempre o default seguro.
//!
//! A9.2C: Display agora vira Rc<RefCell> antes do dispatch de backend,
//! pra que o path DRM possa registrar seu proprio timer dispatch_clients
//! dentro do event loop que ele controla (run() bloqueia ate exit).
//! Antes, o timer dispatch_clients ficava no main pos-backend init, mas
//! em DRM esse main nunca chega no path -- run() segura o thread.
//!
//! A12 Frente 1: auto-spawn lumo-bar no path DRM (TTY standalone).
//! Sem isso, Luiz precisa abrir SSH externo so pra subir a bar manualmente.
//! Winit nested NAO faz auto-spawn (host ja tem barra; evita 2 bars na demo).
//! Memory feedback_design_lapidado: justificar -- spawn so quando backend
//! eh DRM E binario lumo-bar existe ao lado de lumo-wm; log explicito em
//! qualquer falha pra Luiz diagnosticar via journalctl.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use anyhow::Result;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::calloop::EventLoop;
use smithay::reexports::wayland_server::Display;

use lumo_wm::{init_socket, LumoState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// A12 Frente 1: spawna lumo-bar (e opcionalmente foot) na inicializacao
/// do compositor full-session.
///
/// So roda em backend DRM. Em winit nested o host (Hyprland) ja tem a sua
/// propria bar; subir uma segunda dentro da janela nested polui a demo.
///
/// `socket_name` tem que estar binded antes desta funcao -- filhos herdam
/// WAYLAND_DISPLAY do env e conectam imediato. O dispatch_clients timer do
/// backend DRM (drm.rs step 13b, 4ms) responde os primeiros bind requests
/// em < 1 frame, entao o filho nao trava no connect.
///
/// Falha de spawn = warn no log + segue. lumo-wm nao para de funcionar
/// porque a bar nao subiu (Luiz ainda consegue SUPER+Q -> foot manual).
fn spawn_autostart(socket_name: &str) {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_owned()))
        .unwrap_or_else(|| std::path::PathBuf::from("./target/release"));

    let bar_path = exe_dir.join("lumo-bar");
    let desktop_path = exe_dir.join("lumo-desktop");
    let home = std::env::var("HOME").unwrap_or_default();
    let xdg = std::env::var("XDG_CONFIG_HOME")
        .unwrap_or_else(|_| format!("{home}/.config"));
    let xdg_runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_default();

    // A21: lumo-desktop ANTES de lumo-bar.
    // Background layer eh atras de tudo = primeiro a desenhar; subir
    // antes garante que o compositor ja tem a surface registrada quando
    // bar/toplevels chegarem por cima. Falha de spawn = warn + continua.
    if desktop_path.exists() {
        let mut cmd = std::process::Command::new(&desktop_path);
        cmd.env("WAYLAND_DISPLAY", socket_name);
        cmd.env("HOME", &home);
        cmd.env("XDG_CONFIG_HOME", &xdg);
        cmd.env("XDG_RUNTIME_DIR", &xdg_runtime);
        match cmd.spawn() {
            Ok(child) => tracing::info!(
                pid = child.id(),
                desktop = ?desktop_path,
                "autostart lumo-desktop"
            ),
            Err(err) => tracing::warn!(?err, "autostart lumo-desktop falhou"),
        }
    } else {
        tracing::warn!(desktop = ?desktop_path, "lumo-desktop binary nao encontrado, skip autostart");
    }

    // lumo-bar
    if bar_path.exists() {
        let mut cmd = std::process::Command::new(&bar_path);
        cmd.env("WAYLAND_DISPLAY", socket_name);
        cmd.env("HOME", &home);
        cmd.env("XDG_CONFIG_HOME", &xdg);
        cmd.env("XDG_RUNTIME_DIR", &xdg_runtime);
        match cmd.spawn() {
            Ok(child) => tracing::info!(
                pid = child.id(),
                bar = ?bar_path,
                "autostart lumo-bar"
            ),
            Err(err) => tracing::warn!(?err, "autostart lumo-bar falhou"),
        }
    } else {
        tracing::warn!(bar = ?bar_path, "lumo-bar binary nao encontrado, skip autostart");
    }

    // Opcional: terminal foot se LUMO_AUTOSTART_FOOT=1.
    // Padrao OFF porque Luiz pode preferir desktop limpo no boot.
    if std::env::var("LUMO_AUTOSTART_FOOT").is_ok() {
        let mut cmd = std::process::Command::new("foot");
        cmd.env("WAYLAND_DISPLAY", socket_name);
        cmd.env("HOME", &home);
        cmd.env("XDG_CONFIG_HOME", &xdg);
        cmd.env("XDG_RUNTIME_DIR", &xdg_runtime);
        match cmd.spawn() {
            Ok(child) => tracing::info!(pid = child.id(), "autostart foot"),
            Err(err) => tracing::warn!(?err, "autostart foot falhou"),
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

    // L5: lid watcher (Galaxy Book 4) - best-effort, no-op on other HW.
    lumo_wm::handlers::lid::register_lid_watcher(&event_loop.handle());

    // IPC socket: best-effort. Falha = avisa e continua sem.
    match lumo_wm::ipc::init(event_loop.handle()) {
        Ok(ipc) => {
            state.ipc = ipc;
        }
        Err(err) => {
            tracing::warn!(?err, "IPC desativado");
        }
    }

    // WAYLAND_DISPLAY exportado pra clients filhos (foot, lumo-bar) que
    // herdam env. Setado antes do dispatch porque DRM bloqueia.
    if let Some(s) = socket_name.as_ref() {
        std::env::set_var("WAYLAND_DISPLAY", s);
    }

    // A12 Frente 1: autostart so no path DRM (full TTY session).
    // Em winit nested, host ja tem a bar -- evitar duplicar.
    if backend == BackendChoice::Drm {
        if let Some(s) = socket_name.as_deref() {
            spawn_autostart(s);
        } else {
            tracing::warn!("autostart skip: socket Wayland nao foi criado");
        }
    }

    // Display em Rc<RefCell> -- compartilhado entre path winit (timer
    // do main pos-init) e path DRM (timer dentro do run).
    let display = Rc::new(RefCell::new(display));

    // Backend dispatch.
    match backend {
        BackendChoice::Winit => {
            let _winit_data = lumo_wm::backend::winit::init(event_loop.handle(), &mut state)?;
        }
        BackendChoice::Drm => {
            #[cfg(feature = "drm-backend")]
            {
                lumo_wm::backend::drm::run(&mut event_loop, &mut state, display.clone())?;
                // run() bloqueia tudo; em DRM o proprio backend
                // orquestra o event loop. Saimos aqui depois.
                let hw = lumo_wm::hardware::HardwareTarget::detect();
    tracing::info!(hardware = ?hw, "Hardware detectado: {}", hw.label());
    if hw == lumo_wm::hardware::HardwareTarget::GenericLinux {
        tracing::warn!("Lumo otimizado pra Samsung Galaxy Book 4. Outro hardware: defaults genericos, alguns visuais podem diferir.");
    }
    tracing::info!("Lumo WM saiu do backend DRM");
                if let Some(path) = state.ipc.socket_path.take() {
                    let _ = std::fs::remove_file(path);
                }
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

    // Timer 4ms: dispatch Wayland + tick IPC (so winit path -- DRM tem
    // o seu proprio dentro de drm::run).
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

    let hw = lumo_wm::hardware::HardwareTarget::detect();
    tracing::info!(hardware = ?hw, "Hardware detectado: {}", hw.label());
    if hw == lumo_wm::hardware::HardwareTarget::GenericLinux {
        tracing::warn!("Lumo otimizado pra Samsung Galaxy Book 4. Outro hardware: defaults genericos, alguns visuais podem diferir.");
    }
    tracing::info!("Lumo WM saindo");
    Ok(())
}
