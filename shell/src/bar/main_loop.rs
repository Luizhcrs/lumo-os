//! bar/main_loop.rs - Entry point `run()` da bar. Inicializa Wayland
//! connection, layer surface, pool e roda o event loop (poll + dispatch +
//! IPC tick + clock tick).
//!
//! DEPS.md A20.9: precisa `prepare_read + poll + read` antes de
//! `dispatch_pending`; sem isso seat capabilities events nunca chegam.

use std::sync::{atomic::{AtomicBool, AtomicU8, Ordering as AtomOrd}, Arc};
use std::time::{Duration, Instant};

use chrono::{Datelike, Local};
use smithay_client_toolkit::{
    compositor::CompositorState,
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm,
    output::OutputState,
    registry::RegistryState,
    seat::SeatState,
    shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell},
    shm::{slot::SlotPool, Shm},
};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::reexports::client::{
    globals::registry_queue_init,
    Connection,
};

use crate::bar::dropdowns::battery::BatteryInfo;
use crate::bar::dropdowns::wifi::WifiInfo;
use crate::bar::dropdowns::DropdownActive;
use crate::bar::fonts::{font_system, swash_cache};
use crate::bar::ipc::{connect_ipc, drain_ipc};
use lumo_animation::{AnimCurve, LAAnimator, LACurve, Spring};
use crate::bar::state::LumoBar;
use crate::bar::tokens::*;
use crate::menu;

// Delegate macros (precisam ver LumoBar + handlers no mesmo escopo).
delegate_compositor!(LumoBar);
delegate_output!(LumoBar);
delegate_shm!(LumoBar);
delegate_layer!(LumoBar);
delegate_seat!(LumoBar);
delegate_keyboard!(LumoBar);
delegate_pointer!(LumoBar);
delegate_registry!(LumoBar);

/// Entry point do binario `lumo-bar`. `src/bin/lumo-bar.rs` so chama esta funcao.
pub fn run() {
    // W19 BUG-FIX: i18n init no startup (lia EN antes).
    lumo_foundation::i18n::I18n::init();
    let _ = font_system();
    let _ = swash_cache();

    // F1: flag compartilhado com thread watch_layout.
    let layout_reload_flag = Arc::new(AtomicBool::new(false));
    {
        let flag = Arc::clone(&layout_reload_flag);
        lumo_foundation::watch_layout(move |_layout| {
            flag.store(true, AtomOrd::Release);
        });
    }

    let conn = Connection::connect_to_env().expect("conectar wayland");
    let (globals, mut queue) = registry_queue_init::<LumoBar>(&conn).expect("registry init");
    let qh = queue.handle();

    let compositor =
        CompositorState::bind(&globals, &qh).expect("wl_compositor nao disponivel");
    let layer_shell = LayerShell::bind(&globals, &qh).expect("wlr_layer_shell nao disponivel");
    let shm = Shm::bind(&globals, &qh).expect("wl_shm nao disponivel");

    let surface = compositor.create_surface(&qh);
    let layer =
        layer_shell.create_layer_surface(&qh, surface, Layer::Top, Some("lumo-bar"), None);
    layer.set_anchor(Anchor::TOP | Anchor::LEFT | Anchor::RIGHT);
    // A24+A27: altura max cobre maior dropdown.
    let lumo_menu_h_main = menu::menu_height(MENU_LUMO_ITEMS) as u32;
    let surface_max_h = BAR_HEIGHT
        + DROPDOWN_GAP as u32
        + DROPDOWN_H
            .max(DROPDOWN_DATETIME_H)
            .max(lumo_menu_h_main as f32) as u32
        + 8;
    layer.set_size(1920, surface_max_h);
    layer.set_exclusive_zone(BAR_HEIGHT as i32);
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer.commit();

    // A20/A24: pool dimensionado pra acomodar bar EXPANDIDA com maior dropdown.
    let max_height = surface_max_h as usize;
    let pool = SlotPool::new(1920 * max_height * 4 * 2, &shm)
        .expect("SlotPool init");

    let active_workspace = Arc::new(AtomicU8::new(1));
    let theme = lumo_foundation::current_theme();
    let palette = lumo_foundation::current_colors();
    eprintln!(
        "[lumo-bar] A18: pill-style activated; tema = {:?}, pill_bg = #{:06X}, alpha = 0x{:02X}",
        theme, palette.pill_bg, palette.pill_bg_alpha
    );

    let mut state = LumoBar {
        registry: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        shm,
        seat_state: SeatState::new(&globals, &qh),
        compositor_state: compositor, // A29: pra set_input_region
        current_input_region: None, // W18.fix
        layer,
        pool,
        width: 1920, // A19.18 default = output Galaxy nativo
        height: BAR_HEIGHT,
        active_workspace: active_workspace.clone(),
        battery_pct: 100,
        battery_info: BatteryInfo::default(),
        wifi_on: true,
        wifi_info: WifiInfo::default(), // A23
        wifi_refresh_due: None,
        running: true,
        first_configured: false,
        pointer: None,
        keyboard: None,
        pointer_x: 0.0,
        pointer_pos: None,
        bat_hit_rect: None,
        wifi_hit_rect: None,     // A23
        datetime_hit_rect: None, // A24
        lumo_hit_rect: None,     // A27
        lumo_menu_hover_idx: usize::MAX, // A27
        cal_prev_rect: None,
        cal_next_rect: None,
        cal_today_rect: None,
        cal_day_rects: Vec::new(),
        // L5: battery interactive hit-rects.
        bat_charge_limit_toggle_rect: None,
        bat_profile_cycle_rect: None,
        // L5: brightness.
        brightness_info: crate::bar::dropdowns::brightness::BrightnessInfo::default(),
        brightness_hit_rect: None,
        brightness_slider_rect: None,
        brightness_preset_day_rect: None,
        brightness_preset_night_rect: None,
        brightness_dragging: false,
        brightness_drag_last_y: 0.0,
        // A31.2: wifi hit-rects.
        wifi_toggle_rect: None,
        wifi_disconnect_rect: None,
        wifi_connect_rects: Vec::new(),
        last_click_at: None,
        dropdown: DropdownActive::None,
        viewed_year: Local::now().year(),
        viewed_month: Local::now().month(),
        selected_day: None,
        // C5.1: registrar DBus server handle.
        registrar_handle: {
            let h = crate::bar::registrar::new_handle();
            crate::bar::registrar::spawn_registrar(h.clone());
            h
        },
        // C5: appmenu.
        appmenu: crate::bar::appmenu::AppMenuState::default(),
        appmenu_open_idx: None,
        appmenu_submenu: Vec::new(),
        appmenu_pill_rects: Vec::new(),
        appmenu_submenu_rects: Vec::new(),
        ipc_stream: connect_ipc(),
        ipc_rx_buf: Vec::with_capacity(256),
        ipc_reconnect_at: None,
        ipc_reconnect_delay: Duration::from_secs(1),
        theme,
        palette,
        // B4: animadores dropdown. Iniciam em done (scale=1, alpha=1) = nenhuma animacao.
        dropdown_scale_anim: {
            let mut a = LAAnimator::new(1.0f32, 1.0f32,
                AnimCurve::Bezier { curve: LACurve::ease_in_out(), duration: 0.28 });
            a.elapsed = 1.0; // marca como done
            a
        },
        dropdown_alpha_anim: {
            let mut a = LAAnimator::new(1.0f32, 1.0f32,
                AnimCurve::Bezier { curve: LACurve::ease_out_cubic(), duration: 0.22 });
            a.elapsed = 1.0;
            a
        },
        dropdown_closing: false,
        dropdown_closing_which: crate::bar::dropdowns::DropdownActive::None,
        // M2: animador de fade da bar no F5. Inicia done (sem animacao).
        refresh_anim: {
            let mut a = LAAnimator::new(0.7f32, 1.0f32,
                AnimCurve::Bezier { curve: LACurve::ease_out_cubic(), duration: 0.25 });
            a.elapsed = 1.0; // done = nenhuma animacao inicial
            a
        },
        refresh_animating: false,
        // S2: fallback pill state.
        appmenu_fallback_rect: None,
        appmenu_fallback_dropdown_rects: Vec::new(),
        appmenu_fallback_hover_idx: None,
        // A31.3: modal de senha wifi.
        password_modal: crate::bar::password_modal::PasswordModalState::default(),
        pwd_confirm_rect: None,
        pwd_cancel_rect: None,
        nm_connect_rx: None,
    };

    let mut last_tick = Instant::now();
    let mut last_clock_tick = Instant::now();
    let mut last_ipc_tick = Instant::now();
    // B4: tick de animacao 60Hz quando ativa.
    let mut last_anim_tick = Instant::now();
    while state.running {
        // Ticks PRIMEIRO (antes do dispatch nao bloquear demais)
        // B4: tick animacao dropdown a 60Hz quando ativa.
        {
            let anim_dt = last_anim_tick.elapsed().as_secs_f32();
            let scale_done = state.dropdown_scale_anim.is_done();
            let alpha_done = state.dropdown_alpha_anim.is_done();
            let refresh_done = !state.refresh_animating;
            let animating = !scale_done || !alpha_done || state.dropdown_closing || !refresh_done;
            if animating && anim_dt >= 1.0 / 62.0 {
                last_anim_tick = Instant::now();
                // Avanca animadores dropdown.
                let _s = state.dropdown_scale_anim.tick(anim_dt);
                let _a = state.dropdown_alpha_anim.tick(anim_dt);
                // M2: avanca animador de refresh da bar.
                if state.refresh_animating {
                    let _ = state.refresh_anim.tick(anim_dt);
                    if state.refresh_anim.is_done() {
                        state.refresh_animating = false;
                    }
                }
                // Quando fechando: ao terminar animacao, limpa dropdown.
                if state.dropdown_closing {
                    if state.dropdown_scale_anim.is_done() && state.dropdown_alpha_anim.is_done() {
                        state.dropdown = crate::bar::dropdowns::DropdownActive::None;
                        state.dropdown_closing = false;
                        state.dropdown_closing_which = crate::bar::dropdowns::DropdownActive::None;
                        eprintln!("[lumo-bar] B4: dropdown fechou (anim done)");
                    }
                }
                state.redraw(&qh);
            }
        }
        if last_clock_tick.elapsed() >= Duration::from_secs(1) {
            last_clock_tick = Instant::now();
            state.redraw(&qh);
        }
        // A31.2.fix: deferred wifi refresh apos action click (nao bloqueia handler).
        if let Some(due) = state.wifi_refresh_due {
            if Instant::now() >= due {
                state.refresh();
                state.redraw(&qh);
                state.wifi_refresh_due = None;
            }
        }
        // A31.3: poll nm_connect receiver -> se NeedPassword, abre modal de senha.
        if let Some(ref rx) = state.nm_connect_rx {
            match rx.try_recv() {
                Ok(crate::bar::system_info::NmConnectResult::NeedPassword { ssid }) => {
                    eprintln!("[lumo-bar] A31.3 NeedPassword ssid={:?} -> abrindo modal", ssid);
                    state.password_modal.open(ssid);
                    state.nm_connect_rx = None;
                    // Solicita foco de teclado exclusivo enquanto modal ativo.
                    state.layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
                    state.layer.wl_surface().commit();
                    state.redraw(&qh);
                }
                Ok(_) => {
                    state.nm_connect_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    state.nm_connect_rx = None;
                }
            }
        }

        if last_tick.elapsed() >= Duration::from_secs(30) {
            state.refresh();
            state.redraw(&qh);
            last_tick = Instant::now();
        }

        conn.flush().ok();
        // A20.9: poll com timeout 50ms = events processados sem bloqueio infinito
        if let Some(guard) = queue.prepare_read() {
            use std::os::fd::AsFd;
            let fd = conn.as_fd();
            let mut pfd = [nix::poll::PollFd::new(fd, nix::poll::PollFlags::POLLIN)];
            let _ = nix::poll::poll(&mut pfd, nix::poll::PollTimeout::try_from(50i32).expect("50 e literal valido para PollTimeout"));
            let _ = guard.read();
        }
        if let Err(e) = queue.dispatch_pending(&mut state) {
            let msg = format!("{e:?}");
            // A20.14: connection reset / broken pipe = compositor saiu, sair limpo
            if msg.contains("ConnectionReset") || msg.contains("BrokenPipe") || msg.contains("InvalidObject") {
                eprintln!("[lumo-bar] compositor desconectou ({e:?}), saindo");
                break;
            }
            eprintln!("[lumo-bar] dispatch_pending warn: {e:?}");
        }
        // Detectar disconnect via flush tambem
        if conn.flush().is_err() {
            eprintln!("[lumo-bar] flush falhou, compositor encerrou - saindo");
            break;
        }

        // IPC reconnect: tenta reconectar quando backoff expira e stream esta None.
        if state.ipc_stream.is_none() {
            if let Some(due) = state.ipc_reconnect_at {
                if Instant::now() >= due {
                    state.ipc_reconnect_at = None;
                    match crate::bar::ipc::connect_ipc() {
                        Some(s) => {
                            eprintln!("[lumo-bar] IPC reconectado");
                            state.ipc_stream = Some(s);
                            state.ipc_reconnect_delay = Duration::from_secs(1);
                        }
                        None => {
                            // ainda nao disponivel; proximo backoff
                            eprintln!("[lumo-bar] IPC reconnect falhou; proximo em {:?}", state.ipc_reconnect_delay);
                            state.ipc_reconnect_at = Some(Instant::now() + state.ipc_reconnect_delay);
                            state.ipc_reconnect_delay = (state.ipc_reconnect_delay * 2).min(Duration::from_secs(30));
                        }
                    }
                }
            }
        }
        if last_ipc_tick.elapsed() >= Duration::from_millis(8) {
            last_ipc_tick = Instant::now();
            if let Some(mut s) = state.ipc_stream.take() {
                let res = drain_ipc(&mut s, &mut state.ipc_rx_buf, &state.active_workspace);
                let alive = res.alive;
                let close_dropdowns = res.close_dropdowns;
                if alive {
                    state.ipc_stream = Some(s);
                } else {
                    eprintln!("[lumo-bar] IPC peer fechou; reconnect em {:?}", state.ipc_reconnect_delay);
                    state.ipc_reconnect_at = Some(Instant::now() + state.ipc_reconnect_delay);
                    // exponential backoff, cap at 30s
                    state.ipc_reconnect_delay = (state.ipc_reconnect_delay * 2).min(Duration::from_secs(30));
                }
                // A25/D2: CloseDropdowns IPC -> fecha dropdown ativo e appmenu submenu.
                if close_dropdowns {
                    let mut changed = false;
                    if state.dropdown != DropdownActive::None {
                        state.dropdown = DropdownActive::None;
                        state.lumo_menu_hover_idx = usize::MAX;
                        changed = true;
                        eprintln!("[lumo-bar] D2: dropdown fechado por CloseDropdowns IPC");
                    }
                    if state.appmenu_open_idx.is_some() {
                        state.appmenu_open_idx = None;
                        state.appmenu_submenu.clear();
                        changed = true;
                        eprintln!("[lumo-bar] D2: appmenu submenu fechado por CloseDropdowns IPC");
                    }
                    if changed {
                        state.update_size_and_redraw(&qh);
                    }
                }
                // C5.1: ActiveApp -> fetch appmenu via Registrar.
                if let Some((app_id, title, pid)) = res.active_app {
                    let new_menu = crate::bar::appmenu::AppMenuState::fetch(pid, &app_id, &title);
                    state.appmenu = new_menu;
                    state.appmenu_open_idx = None;
                    state.appmenu_submenu.clear();
                    state.redraw(&qh);
                }
                // M2: ThemeReloaded -> inicia fade bar 0.7->1.0 em 250ms.
                if res.theme_reloaded {
                    state.refresh_anim = LAAnimator::new(0.7f32, 1.0f32,
                        AnimCurve::Bezier { curve: LACurve::ease_out_cubic(), duration: 0.25 });
                    state.refresh_animating = true;
                    eprintln!("[lumo-bar] M2: ThemeReloaded -> bar fade iniciado");
                }
            }
        }

        // F1: layout.toml mudou -> redraw.
        if layout_reload_flag.swap(false, AtomOrd::AcqRel) {
            eprintln!("[lumo-bar] F1: layout.toml recarregado -> redraw");
            state.redraw(&qh);
        }
    }

    let _ = active_workspace;
}
