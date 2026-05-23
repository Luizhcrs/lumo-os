//! bar/main_loop.rs - Entry point `run()` da bar.

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

// Delegate macros
delegate_compositor!(LumoBar);
delegate_output!(LumoBar);
delegate_shm!(LumoBar);
delegate_layer!(LumoBar);
delegate_seat!(LumoBar);
delegate_keyboard!(LumoBar);
delegate_pointer!(LumoBar);
delegate_registry!(LumoBar);

pub fn run() {
    // A31.7: Instancia unica via lock file.
    let uid = unsafe { libc::getuid() };
    let lock_path = format!("/run/user/{}/lumo-bar.lock", uid);
    let lock_file = std::fs::OpenOptions::new()
        .read(true).write(true).create(true).open(&lock_path)
        .expect("Falha ao abrir lock file da bar");

    use std::os::unix::io::AsRawFd;
    let fd = lock_file.as_raw_fd();
    let locked = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
    if locked != 0 {
        eprintln!("[lumo-bar] ERRO: Outra instancia ja esta rodando (lock em {}).", lock_path);
        std::process::exit(1);
    }

    lumo_foundation::i18n::I18n::init();
    let _ = font_system();
    let _ = swash_cache();

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

    let compositor = CompositorState::bind(&globals, &qh).expect("wl_compositor nao disponivel");
    let layer_shell = LayerShell::bind(&globals, &qh).expect("wlr_layer_shell nao disponivel");
    let shm = Shm::bind(&globals, &qh).expect("wl_shm nao disponivel");

    let surface = compositor.create_surface(&qh);
    let layer = layer_shell.create_layer_surface(&qh, surface, Layer::Top, Some("lumo-bar"), None);
    layer.set_anchor(Anchor::TOP | Anchor::LEFT | Anchor::RIGHT);
    
    let lumo_menu_h_main = menu::menu_height(MENU_LUMO_ITEMS) as u32;
    let surface_max_h = BAR_HEIGHT + DROPDOWN_GAP as u32 + DROPDOWN_H.max(DROPDOWN_DATETIME_H).max(lumo_menu_h_main as f32) as u32 + 8;
    layer.set_size(1920, surface_max_h);
    layer.set_exclusive_zone(BAR_HEIGHT as i32);
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer.commit();

    let pool = SlotPool::new(1920 * surface_max_h as usize * 4 * 2, &shm).expect("SlotPool init");
    let active_workspace = Arc::new(AtomicU8::new(1));
    let theme = lumo_foundation::current_theme();
    let palette = lumo_foundation::current_colors();

    let mut state = LumoBar {
        registry: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        shm,
        seat_state: SeatState::new(&globals, &qh),
        compositor_state: compositor,
        current_input_region: None,
        layer,
        pool,
        width: 1920,
        height: BAR_HEIGHT,
        active_workspace: active_workspace.clone(),
        battery_pct: 100,
        battery_info: BatteryInfo::default(),
        wifi_on: true,
        wifi_info: WifiInfo::default(),
        wifi_refresh_due: None,
        running: true,
        first_configured: false,
        pointer: None,
        keyboard: None,
        pointer_x: 0.0,
        pointer_pos: None,
        bat_hit_rect: None,
        wifi_hit_rect: None,
        datetime_hit_rect: None,
        lumo_hit_rect: None,
        lumo_menu_hover_idx: usize::MAX,
        cal_prev_rect: None,
        cal_next_rect: None,
        cal_today_rect: None,
        cal_day_rects: Vec::new(),
        bat_charge_limit_toggle_rect: None,
        bat_profile_cycle_rect: None,
        brightness_info: crate::bar::dropdowns::brightness::BrightnessInfo::default(),
        brightness_hit_rect: None,
        brightness_slider_rect: None,
        brightness_preset_day_rect: None,
        brightness_preset_night_rect: None,
        brightness_dragging: false,
        brightness_drag_last_y: 0.0,
        wifi_toggle_rect: None,
        wifi_disconnect_rect: None,
        wifi_connect_rects: Vec::new(),
        last_click_at: None,
        dropdown: DropdownActive::None,
        dropdown_rect: None,
        dropdown_h_final: 0.0,
        viewed_year: Local::now().year(),
        viewed_month: Local::now().month(),
        selected_day: None,
        registrar_handle: {
            let h = crate::bar::registrar::new_handle();
            crate::bar::registrar::spawn_registrar(h.clone());
            h
        },
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
        brightness_dragging_slider: false,
        dropdown_scale_anim: {
            let mut a = LAAnimator::new(1.0f32, 1.0f32, AnimCurve::Bezier { curve: LACurve::ease_in_out(), duration: 0.28 });
            a.elapsed = 1.0;
            a
        },
        dropdown_alpha_anim: {
            let mut a = LAAnimator::new(1.0f32, 1.0f32, AnimCurve::Bezier { curve: LACurve::ease_out_cubic(), duration: 0.22 });
            a.elapsed = 1.0;
            a
        },
        dropdown_closing: false,
        dropdown_closing_which: DropdownActive::None,
        refresh_anim: {
            let mut a = LAAnimator::new(0.7f32, 1.0f32, AnimCurve::Bezier { curve: LACurve::ease_out_cubic(), duration: 0.25 });
            a.elapsed = 1.0;
            a
        },
        refresh_animating: false,
        appmenu_fallback_rect: None,
        appmenu_fallback_dropdown_rects: Vec::new(),
        appmenu_fallback_hover_idx: None,
        password_modal: crate::bar::password_modal::PasswordModalState::default(),
        pwd_confirm_rect: None,
        pwd_cancel_rect: None,
        nm_connect_rx: None,
    };

    let mut last_tick = Instant::now();
    let mut last_clock_tick = Instant::now();
    let mut last_ipc_tick = Instant::now();
    let mut last_anim_tick = Instant::now();
    while state.running {
        {
            let anim_dt = last_anim_tick.elapsed().as_secs_f32();
            let animating = !state.dropdown_scale_anim.is_done() || !state.dropdown_alpha_anim.is_done() || state.dropdown_closing || state.refresh_animating;
            if animating && anim_dt >= 1.0 / 62.0 {
                last_anim_tick = Instant::now();
                state.dropdown_scale_anim.tick(anim_dt);
                state.dropdown_alpha_anim.tick(anim_dt);
                if state.refresh_animating {
                    state.refresh_anim.tick(anim_dt);
                    if state.refresh_anim.is_done() { state.refresh_animating = false; }
                }
                if state.dropdown_closing {
                    if state.dropdown_scale_anim.is_done() && state.dropdown_alpha_anim.is_done() {
                        state.dropdown = DropdownActive::None;
                        state.dropdown_closing = false;
                        state.dropdown_closing_which = DropdownActive::None;
                    }
                }
                state.redraw(&qh);
            }
        }
        if last_clock_tick.elapsed() >= Duration::from_secs(1) {
            last_clock_tick = Instant::now();
            state.redraw(&qh);
        }
        if let Some(due) = state.wifi_refresh_due {
            if Instant::now() >= due {
                state.refresh();
                state.redraw(&qh);
                state.wifi_refresh_due = None;
            }
        }
        if let Some(ref rx) = state.nm_connect_rx {
            if let Ok(crate::bar::system_info::NmConnectResult::NeedPassword { ssid }) = rx.try_recv() {
                state.password_modal.open(ssid);
                state.nm_connect_rx = None;
                state.layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
                state.layer.wl_surface().commit();
                state.redraw(&qh);
            }
        }
        if last_tick.elapsed() >= Duration::from_secs(30) {
            state.refresh();
            state.redraw(&qh);
            last_tick = Instant::now();
        }
        conn.flush().ok();
        if let Some(guard) = queue.prepare_read() {
            use std::os::fd::AsFd;
            let mut pfd = [nix::poll::PollFd::new(conn.as_fd(), nix::poll::PollFlags::POLLIN)];
            let _ = nix::poll::poll(&mut pfd, nix::poll::PollTimeout::try_from(50i32).unwrap());
            let _ = guard.read();
        }
        if let Err(_) = queue.dispatch_pending(&mut state) { break; }
        if last_ipc_tick.elapsed() >= Duration::from_millis(8) {
            last_ipc_tick = Instant::now();
            if let Some(mut s) = state.ipc_stream.take() {
                let res = drain_ipc(&mut s, &mut state.ipc_rx_buf, &state.active_workspace);
                if res.alive { state.ipc_stream = Some(s); }
                if res.close_dropdowns {
                    state.dropdown = DropdownActive::None;
                    state.appmenu_open_idx = None;
                    state.update_size_and_redraw(&qh);
                }
                if let Some((app_id, title, pid)) = res.active_app {
                    // W34.13: respeita TODO ActiveApp broadcast (empty inclusive).
                    // Click fora da janela -> focus_changed=None -> empty -> bar limpa pills.
                    // Mac-style: pills only when janela focada. Trade-off: flash transient
                    // durante Iced window create (empty antes set_app_id). Aceito.
                    state.appmenu = crate::bar::appmenu::AppMenuState::fetch(pid, &app_id, &title);
                    state.appmenu_open_idx = None;
                    state.appmenu_submenu.clear();
                    state.redraw(&qh);
                }
                if res.clear_appmenu {
                    // W34.11: explicit clear (appsd fechou todas janelas).
                    state.appmenu = crate::bar::appmenu::AppMenuState::default();
                    state.appmenu_open_idx = None;
                    state.appmenu_submenu.clear();
                    state.redraw(&qh);
                }
                if res.theme_reloaded {
                    state.refresh_anim = LAAnimator::new(0.7f32, 1.0f32, AnimCurve::Bezier { curve: LACurve::ease_out_cubic(), duration: 0.25 });
                    state.refresh_animating = true;
                }
            }
        }
    }
}
