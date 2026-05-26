//! desktop/main_loop.rs - Entry point `run()` do lumo-desktop.

use std::time::{Duration, Instant};

use smithay_client_toolkit::reexports::client::{globals::registry_queue_init, Connection};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::{
    compositor::CompositorState,
    delegate_compositor, delegate_layer, delegate_output, delegate_pointer, delegate_registry,
    delegate_seat, delegate_shm,
    output::OutputState,
    registry::RegistryState,
    seat::SeatState,
    shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell},
    shm::{slot::SlotPool, Shm},
};

use lumo_foundation::current_colors;
use lumo_ipc;

use crate::desktop::state::{
    connect_ipc, drain_ipc_events, font_system, swash_cache, LumoDesktop, MenuActive, OUTPUT_H,
    OUTPUT_W,
};

// Delegate macros (precisam ver LumoDesktop + handlers no mesmo escopo).
delegate_compositor!(LumoDesktop);
delegate_output!(LumoDesktop);
delegate_shm!(LumoDesktop);
delegate_layer!(LumoDesktop);
delegate_seat!(LumoDesktop);
delegate_pointer!(LumoDesktop);
delegate_registry!(LumoDesktop);

/// Entry point do binario `lumo-desktop`.
pub fn run() {
    let _ = font_system();
    let _ = swash_cache();

    let conn = Connection::connect_to_env().expect("conectar wayland");
    let (globals, mut queue) = registry_queue_init::<LumoDesktop>(&conn).expect("registry init");
    let qh = queue.handle();

    let compositor = CompositorState::bind(&globals, &qh).expect("wl_compositor nao disponivel");
    let layer_shell = LayerShell::bind(&globals, &qh).expect("wlr_layer_shell nao disponivel");
    let shm = Shm::bind(&globals, &qh).expect("wl_shm nao disponivel");

    let surface = compositor.create_surface(&qh);
    let layer = layer_shell.create_layer_surface(
        &qh,
        surface,
        Layer::Background,
        Some("lumo-desktop"),
        None,
    );
    layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
    layer.set_size(0, 0);
    layer.set_exclusive_zone(-1);
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer.commit();

    let pool =
        SlotPool::new(OUTPUT_W as usize * OUTPUT_H as usize * 4 * 2, &shm).expect("SlotPool init");

    let mut state = LumoDesktop {
        registry: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        shm,
        seat_state: SeatState::new(&globals, &qh),
        layer,
        pool,
        width: OUTPUT_W,
        height: OUTPUT_H,
        running: true,
        first_configured: false,
        pointer: None,
        pointer_pos: None,
        menu: MenuActive {
            visible: false,
            x: 0.0,
            y: 0.0,
            hover_idx: usize::MAX,
        },
        ipc_stream: connect_ipc(),
        ipc_rx_buf: Vec::with_capacity(256),
        last_click_at: None,
        palette: current_colors(),
        need_redraw: false,
        icons: crate::desktop::icons::IconsState::new(),
        rubber_band: crate::desktop::rubber_band::RubberBand::new(),
    };

    eprintln!("[lumo-desktop] A27: menu desktop + CloseDropdowns IPC");

    let mut last_ipc_tick = Instant::now();
    while state.running {
        conn.flush().ok();
        if let Some(guard) = queue.prepare_read() {
            use std::os::fd::AsFd;
            let fd = conn.as_fd();
            let mut pfd = [nix::poll::PollFd::new(fd, nix::poll::PollFlags::POLLIN)];
            let _ = nix::poll::poll(
                &mut pfd,
                nix::poll::PollTimeout::try_from(50i32)
                    .expect("50 e literal valido para PollTimeout"),
            );
            let _ = guard.read();
        }
        if let Err(e) = queue.dispatch_pending(&mut state) {
            let msg = format!("{e:?}");
            if msg.contains("ConnectionReset")
                || msg.contains("BrokenPipe")
                || msg.contains("InvalidObject")
            {
                eprintln!("[lumo-desktop] compositor desconectou ({e:?}), saindo");
                break;
            }
            eprintln!("[lumo-desktop] dispatch_pending warn: {e:?}");
        }
        if conn.flush().is_err() {
            eprintln!("[lumo-desktop] flush falhou, saindo");
            break;
        }

        // A40: drena IPC events do compositor. Tick 8ms = ~120Hz.
        if last_ipc_tick.elapsed() >= Duration::from_millis(8) {
            last_ipc_tick = Instant::now();
            if let Some(mut s) = state.ipc_stream.take() {
                let (alive, close_menu, open_selected, theme_mode) =
                    drain_ipc_events(&mut s, &mut state.ipc_rx_buf);
                if alive {
                    state.ipc_stream = Some(s);
                } else {
                    eprintln!("[lumo-desktop] IPC peer fechou; desktop continua passivo");
                }
                if close_menu {
                    // D2: fecha menu contextual de area vazia e ctx_menu de icone.
                    if state.menu.visible {
                        state.menu.visible = false;
                        state.menu.hover_idx = usize::MAX;
                        state.need_redraw = true;
                        eprintln!("[lumo-desktop] D2: menu overlay fechado por IPC");
                    }
                    if state.icons.ctx_menu.is_some() {
                        state.icons.ctx_menu = None;
                        state.icons.ctx_hover = usize::MAX;
                        state.need_redraw = true;
                        eprintln!("[lumo-desktop] D2: ctx_menu icone fechado por IPC");
                    }
                }
                if open_selected {
                    state.icons.open_selected();
                }
                // L6: ThemeReloaded -> recarrega palette e redesenha.
                if let Some(mode) = theme_mode {
                    let tokens = lumo_foundation::LumoTokens::load_from_disk();
                    state.palette = tokens.resolve();
                    eprintln!("[lumo-desktop] L6: ThemeReloaded {:?} -> redraw", mode);
                    state.need_redraw = true;
                }
            }
        }
        state.icons.tick();
        if state.need_redraw {
            state.need_redraw = false;
            state.redraw(&qh);
        }
    }
}
