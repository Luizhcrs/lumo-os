//! Input dispatch - converte eventos de backend em acoes do compositor.
//!
//! B2: KeyboardConfig com 16+ bindings Lumo-style carregados de
//! ~/.config/lumo/keyboard.toml (fallback para default_bindings()).
//! Handler handle_input procura match na lista de bindings e executa
//! a acao correspondente.

use smithay::backend::input::{
    AbsolutePositionEvent, ButtonState, Event as _, GestureBeginEvent,
    GestureEndEvent, GesturePinchUpdateEvent, GestureSwipeUpdateEvent,
    InputBackend, InputEvent, KeyState, KeyboardKeyEvent, PointerButtonEvent,
    PointerMotionEvent,
};
use smithay::input::keyboard::FilterResult;
use smithay::input::pointer::{ButtonEvent, MotionEvent};
use smithay::utils::SERIAL_COUNTER;
use smithay::wayland::seat::WaylandFocus;

use crate::input::keyboard::{KeyAction, TileDir};
use crate::input::touchpad::SwipeDirection;
use crate::state::LumoState;

impl LumoState {
    pub fn handle_input<I: InputBackend>(&mut self, event: InputEvent<I>) {
        match event {
            InputEvent::Keyboard { event } => {
                // W10.B: reset idle timer on any key event.
                {
                    let seat_ref = self.seat.clone();
                    self.idle_manager.reset();
                    self.idle_notifier_state.notify_activity(&seat_ref);
                }
                let serial = SERIAL_COUNTER.next_serial();
                let time = event.time_msec();
                let keycode = event.key_code();
                let state = event.state();
                let keyboard = self.keyboard.clone();
                let press = state == KeyState::Pressed;

                // A40: Cell pra capturar sym calculado dentro do closure.
                let last_sym_for_a40 = std::cell::Cell::new(
                    smithay::input::keyboard::xkb::Keysym::NoSymbol
                );
                // W12.C: capture sym on key release too (for picker SUPER detection).
                let last_sym_release = std::cell::Cell::new(
                    smithay::input::keyboard::xkb::Keysym::NoSymbol
                );
                let action_opt = keyboard.input::<KeyAction, _>(
                    self,
                    keycode,
                    state,
                    serial,
                    time,
                    |state, mods, kh| {
                        let sym = kh.modified_sym();
                        if !press {
                            last_sym_release.set(sym);
                            return FilterResult::Forward;
                        }
                        last_sym_for_a40.set(sym);
                        // Bug Luiz 2026-05-18 v3: caps/num lock LED sync direto
                        // via sysfs — SeatHandler::led_state_changed nao disparou.
                        use smithay::input::keyboard::xkb::Keysym;
                        if sym == Keysym::Caps_Lock {
                            state.caps_lock_on = !state.caps_lock_on;
                            write_sys_led("capslock", state.caps_lock_on);
                            // C2: broadcast OSD popup visual.
                            let osd_text = if state.caps_lock_on {
                                "Caps Lock Ligado".to_string()
                            } else {
                                "Caps Lock Desligado".to_string()
                            };
                            state.ipc.broadcast(&lumo_ipc::LumoEvent::ShowOsd {
                                text: osd_text,
                                icon: lumo_ipc::OsdIcon::Keyboard,
                                duration_ms: 2000,
                            });
                        } else if sym == Keysym::Num_Lock {
                            state.num_lock_on = !state.num_lock_on;
                            write_sys_led("numlock", state.num_lock_on);
                        }
                        if let Some(action) =
                            state.keyboard_config.match_binding(mods, sym)
                        {
                            FilterResult::Intercept(action.clone())
                        } else {
                            FilterResult::Forward
                        }
                    },
                );

                if let Some(action) = action_opt {
                    self.execute_key_action(action);
                }
                // W12.C: stack picker key handling.
                if self.stack_picker.is_some() {
                    use smithay::input::keyboard::xkb::Keysym;
                    let sym = last_sym_for_a40.get();
                    // Shift+Tab while picker open -> cycle prev.
                    if press {
                        let kb2 = self.keyboard.clone();
                        let mods_state = kb2.modifier_state();
                        if sym == Keysym::Tab && mods_state.shift && mods_state.logo {
                            if let Some(p) = self.stack_picker.as_mut() { p.cycle_prev(); }
                        }
                        // Esc -> dismiss without switching.
                        if sym == Keysym::Escape {
                            self.stack_picker = None;
                            tracing::info!("W12.C: picker dismissed via Esc");
                        }
                    }
                    // SUPER key release -> activate selected and close.
                    let release_sym = last_sym_release.get();
                    if !press && (release_sym == Keysym::Super_L || release_sym == Keysym::Super_R) {
                        if let Some(picker) = self.stack_picker.take() {
                            if let Some(win) = picker.selected_window() {
                                if let Some(surf) = win.wl_surface() {
                                    let owned = surf.into_owned();
                                    let serial = smithay::utils::SERIAL_COUNTER.next_serial();
                                    self.focus_manager.click_toplevel(owned.clone());
                                    let kb3 = self.keyboard.clone();
                                    self.space.raise_element(win, true);
                                    kb3.set_focus(self, Some(owned), serial);
                                    tracing::info!("W12.C: picker activated window on SUPER release");
                                }
                            }
                        }
                        #[cfg(feature = "drm-backend")]
                        { self.drm_force_repaint = true; }
                    }
                }
                // W12.B: overview key handling.
                if self.overview.is_some() && press {
                    use smithay::input::keyboard::xkb::Keysym;
                    let sym = last_sym_for_a40.get();
                    if sym == Keysym::Escape {
                        let a11y = lumo_foundation::A11yTokens::load_from_disk();
                        if let Some(ov) = self.overview.as_mut() { ov.close(a11y.reduced_motion); }
                        tracing::info!("W12.B: overview dismissed via Esc");
                    }
                }
                // A40: Return sem binding + sem toplevel focado
                // -> roteia pra desktop abrir icone selecionado.
                if press && last_sym_for_a40.get() == smithay::input::keyboard::xkb::Keysym::Return {
                    let has_focus = self.keyboard.current_focus().is_some();
                    if !has_focus {
                        tracing::info!("A40: Return sem toplevel -> DesktopOpenSelected");
                        self.broadcast_desktop_open_selected();
                    }
                }
            }

            InputEvent::PointerMotion { event } => {
                // W10.B: reset idle timer on any pointer movement.
                self.idle_manager.reset();
                {
                    let seat_ref = self.seat.clone();
                    self.idle_notifier_state.notify_activity(&seat_ref);
                }
                let dx = event.delta_x();
                let dy = event.delta_y();
                let new_x = (self.pointer_location.x + dx).clamp(0.0, 1919.0);
                let new_y = (self.pointer_location.y + dy).clamp(0.0, 1079.0);
                self.pointer_location = (new_x, new_y).into();

                let serial = SERIAL_COUNTER.next_serial();
                let under = self.surface_under(self.pointer_location);
                let pointer = self.pointer.clone();
                pointer.motion(
                    self,
                    under.clone().map(|(s, loc)| (s, loc.to_f64())),
                    &MotionEvent {
                        location: self.pointer_location,
                        serial,
                        time: event.time_msec(),
                    },
                );
                pointer.frame(self);

                // W12.B: update overview hover.
                if self.overview.is_some() {
                    let pos_l = self.pointer_location.to_i32_round();
                    let (ow, oh) = self.output_dimensions();
                    let hit = self.overview.as_ref()
                        .and_then(|ov| ov.hit_test(pos_l, ow, oh));
                    if let Some(ov) = self.overview.as_mut() {
                        ov.hovered = hit;
                    }
                }
                #[cfg(feature = "drm-backend")]
                {
                    self.drm_force_repaint = true;
                }
            }

            InputEvent::PointerMotionAbsolute { event } => {
                let x = event.x_transformed(1280);
                let y = event.y_transformed(720);
                self.pointer_location = (x, y).into();

                let serial = SERIAL_COUNTER.next_serial();
                let under = self.surface_under(self.pointer_location);

                let pointer = self.pointer.clone();
                pointer.motion(
                    self,
                    under.clone().map(|(s, loc)| (s, loc.to_f64())),
                    &MotionEvent {
                        location: self.pointer_location,
                        serial,
                        time: event.time_msec(),
                    },
                );
                pointer.frame(self);

                if let Some((surface, _)) = under {
                    let kb = self.keyboard.clone();
                    if kb.current_focus().as_ref() != Some(&surface) {
                        kb.set_focus(self, Some(surface), serial);
                    }
                }
            }

            InputEvent::PointerButton { event } => {
                let serial = SERIAL_COUNTER.next_serial();
                let button = event.button_code();
                let state: ButtonState = event.state();
                let pointer = self.pointer.clone();

                // C3 debug: log raw button code pra diagnostico BTN_RIGHT.
                tracing::debug!(button, state = ?state, pos = ?(self.pointer_location.x as i32, self.pointer_location.y as i32), "C3 PointerButton");

                if state == ButtonState::Pressed {
                    // M1: SSD hit-test antes de repassar o click ao cliente.
                    // Verifica close button e titlebar para janelas com SSD ativo.
                    // T1.1: hit-test SSD titlebar -- BTN_LEFT e BTN_RIGHT.
                    {
                        use crate::backend::render_common::{
                            ssd_close_btn_rect_logical, ssd_titlebar_rect_logical,
                        };
                        use smithay::input::pointer::Focus;
                        let ptr_pos = self.pointer_location.to_i32_round();
                        let mut ssd_handled = false;

                        // T1.1: se menu popup SSD esta aberto, testa clique dentro/fora.
                        if let Some((menu_win, menu_pos, _hover)) = self.titlebar_menu.clone() {
                            let menu_w = 180i32;
                            let item_h = 22i32;
                            let mx = menu_pos.x;
                            let my = menu_pos.y;
                            let in_menu = ptr_pos.x >= mx && ptr_pos.x <= mx + menu_w
                                && ptr_pos.y >= my && ptr_pos.y <= my + item_h * 5;
                            if in_menu && button == 0x110 {
                                let idx = ((ptr_pos.y - my) / item_h) as usize;
                                self.titlebar_menu = None;
                                match idx {
                                    0 => {
                                        if let Some(tl) = menu_win.toplevel() { tl.send_close(); }
                                    }
                                    1 => {
                                        if let Some(tl) = menu_win.toplevel() {
                                            use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State as XdgState;
                                            let is_fs = tl.current_state().states.contains(XdgState::Fullscreen);
                                            tl.with_pending_state(|st| {
                                                if is_fs { st.states.unset(XdgState::Fullscreen); }
                                                else { st.states.set(XdgState::Fullscreen); }
                                            });
                                            tl.send_configure();
                                        }
                                    }
                                    2 => { tracing::info!("T1.1 menu: Minimizar (stub)"); }
                                    3 => { /* separator */ }
                                    4 => {
                                        let app_id = menu_win.wl_surface()
                                            .map(|surf| {
                                                use smithay::wayland::shell::xdg::XdgToplevelSurfaceData;
                                                smithay::wayland::compositor::with_states(&surf, |states| {
                                                    states.data_map.get::<XdgToplevelSurfaceData>()
                                                        .map(|d| d.lock().unwrap().app_id.clone().unwrap_or_default())
                                                        .unwrap_or_default()
                                                })
                                            })
                                            .unwrap_or_default();
                                        tracing::info!("T1.1 menu: Sobre {app_id}");
                                    }
                                    _ => {}
                                }
                                ssd_handled = true;
                            } else if !in_menu {
                                // D2: dismiss-on-outside-click titlebar_menu.
                                self.titlebar_menu = None;
                                #[cfg(feature = "drm-backend")]
                                { self.drm_force_repaint = true; }
                            }
                            if ssd_handled {
                                pointer.frame(self);
                                return;
                            }
                        }

                        let windows: Vec<_> = self.space.elements().cloned().collect();
                        for window in &windows {
                            let surf_opt = window.toplevel().map(|t| t.wl_surface().clone());
                            let surf = match surf_opt { Some(s) => s, None => continue };
                            if !self.ssd_windows.contains(&surf) { continue; }
                            let loc = self.space.element_location(window).unwrap_or_default();
                            let geo = window.geometry();
                            let close_rect = ssd_close_btn_rect_logical(loc, geo.size.w);
                            if button == 0x110 && close_rect.contains(ptr_pos) {
                                if let Some(toplevel) = window.toplevel() { toplevel.send_close(); }
                                ssd_handled = true;
                                break;
                            }
                            let title_rect = ssd_titlebar_rect_logical(loc, geo.size.w);
                            // T1.1: BTN_RIGHT em titlebar = abre menu popup (nao fecha direto).
                            if button == 0x111 && title_rect.contains(ptr_pos) {
                                self.titlebar_menu = Some((window.clone(), ptr_pos, usize::MAX));
                                ssd_handled = true;
                                break;
                            }
                            if button == 0x110 && title_rect.contains(ptr_pos) {
                                self.space.raise_element(window, true);
                                if let Some(tl) = window.toplevel() {
                                    let surf_raise = tl.wl_surface().clone();
                                    let kb_raise = self.keyboard.clone();
                                    kb_raise.set_focus(self, Some(surf_raise), serial);
                                }
                                let pointer = self.pointer.clone();
                                let start_data = smithay::input::pointer::GrabStartData {
                                    focus: pointer.current_focus().map(|s| {
                                        let fl = self
                                            .surface_under(self.pointer_location)
                                            .map(|(_, l)| l.to_f64())
                                            .unwrap_or_default();
                                        (s, fl)
                                    }),
                                    button: 0x110,
                                    location: self.pointer_location,
                                };
                                let initial_window_location = loc;
                                let grab = crate::input::move_grab::MoveSurfaceGrab {
                                    start_data,
                                    window: window.clone(),
                                    initial_window_location,
                                };
                                pointer.set_grab(self, grab, serial, Focus::Clear);
                                ssd_handled = true;
                                break;
                            }
                        }
                        if ssd_handled {
                            pointer.frame(self);
                            return;
                        }
                    }

                    // W12.B: overview click: activate cell or dismiss.
                    if self.overview.is_some() {
                        let pos_l = self.pointer_location.to_i32_round();
                        let (ow, oh) = self.output_dimensions();
                        let hit = self.overview.as_ref()
                            .and_then(|ov| ov.hit_test(pos_l, ow, oh));
                        if let Some(idx) = hit {
                            let win_opt = self.overview.as_ref()
                                .and_then(|ov| ov.windows.get(idx).cloned());
                            if let Some(win) = win_opt {
                                if let Some(surf) = win.wl_surface() {
                                    let owned = surf.into_owned();
                                    let serial_ov = smithay::utils::SERIAL_COUNTER.next_serial();
                                    self.space.raise_element(&win, true);
                                    self.focus_manager.click_toplevel(owned.clone());
                                    let kb_ov = self.keyboard.clone();
                                    kb_ov.set_focus(self, Some(owned), serial_ov);
                                    tracing::info!(idx, "W12.B: overview cell activated");
                                }
                            }
                        }
                        let a11y_ov = lumo_foundation::A11yTokens::load_from_disk();
                        if let Some(ov) = self.overview.as_mut() { ov.close(a11y_ov.reduced_motion); }
                        #[cfg(feature = "drm-backend")]
                        { self.drm_force_repaint = true; }
                        pointer.frame(self);
                        return;
                    }
                    // D2: broadcast CloseDropdowns quando click fora da bar.
                    // Bar fecha dropdown se ativo; desktop fecha menu/ctx_menu.
                    // Nao broadcast se click esta dentro da bar (evita fechar o proprio dropdown).
                    // TODO D3: CloseDropdowns deve carregar coordenada do click; cada client decide se fecha.
                    if !self.pos_is_on_bar(self.pointer_location) {
                        self.ipc.broadcast(&lumo_ipc::LumoEvent::CloseDropdowns);
                    }

                    let kb = self.keyboard.clone();
                    let new_focus = if let Some((surface, _)) = self.surface_under(self.pointer_location) {
                        // L1: FocusManager centraliza policy de foco.
                        use smithay::wayland::shell::xdg::XdgToplevelSurfaceData;
                        let is_toplevel = smithay::wayland::compositor::with_states(
                            &surface,
                            |states| states.data_map.get::<XdgToplevelSurfaceData>().is_some(),
                        );
                        if is_toplevel {
                            // Q1: raise toplevel ao topo no click.
                            // Coletar antes de mutar (borrow check).
                            let win_to_raise = self.space.elements()
                                .find(|w| w.wl_surface().map(|s| *s == surface).unwrap_or(false))
                                .cloned();
                            if let Some(win) = win_to_raise {
                                self.space.raise_element(&win, true);
                            }
                            self.focus_manager.click_toplevel(surface)
                        } else {
                            // Layer-shell (bar, desktop) -> sem foco de teclado.
                            self.focus_manager.click_layer_shell()
                        }
                    } else {
                        // Area sem surface -> sem foco.
                        self.focus_manager.click_layer_shell()
                    };
                    kb.set_focus(self, new_focus, serial);
                }

                // D2: dismiss xdg popups sem grab quando click fora.
                if state == ButtonState::Pressed {
                    use smithay::desktop::PopupManager;
                    let ptr = self.pointer_location.to_i32_round();
                    let windows: Vec<_> = self.space.elements().cloned().collect();
                    for win in &windows {
                        if let Some(root_surf) = win.wl_surface() {
                            let win_loc = self.space.element_location(win).unwrap_or_default();
                            let popups: Vec<_> = PopupManager::popups_for_surface(&root_surf).collect();
                            for (popup, popup_offset) in popups {
                                let geo = popup.geometry();
                                let popup_loc = win_loc + popup_offset;
                                let rect = smithay::utils::Rectangle::from_loc_and_size(
                                    popup_loc + geo.loc,
                                    geo.size,
                                );
                                if !rect.contains(ptr) {
                                    // TODO P1.4: Check popup grab before dismiss.
                                    // Wayland spec: grabbed popup should only be dismissed by client.
                                    // Impact low while only Lumo apps run. Add grab tracking in D3.
                                    let _ = PopupManager::dismiss_popup(&root_surf, &popup);
                                    tracing::debug!("D2: popup dismissed outside click");
                                }
                            }
                        }
                    }
                }

                pointer.button(
                    self,
                    &ButtonEvent {
                        button,
                        state,
                        serial,
                        time: event.time_msec(),
                    },
                );
                pointer.frame(self);
                // R1.fix5: force repaint apos PointerButton (Motion ja seta).
                // Sem isso bar commit de dropdown novo fica preso ate proximo
                // vblank ou Motion event = dropdown invisivel ate mouse mover.
                // R1.fix7: dedup -- N PointerButton em slider drag = N flips
                // redundantes. So flipa se ainda nao agendado. VRR Wave 13
                // economiza quadros quando idle entre clicks.
                // TODO: skip set se VRR active + repaint <8ms atras (precisa
                // VrrState em LumoState; surface.vrr_active hoje vive em
                // backend::drm::DrmBackendData e nao eh acessivel daqui).
                #[cfg(feature = "drm-backend")]
                {
                    if !self.drm_force_repaint {
                        self.drm_force_repaint = true;
                    }
                }
            }

            InputEvent::GestureSwipeBegin { event } => {
                let fingers = event.fingers();
                self.gesture.on_swipe_begin(fingers);
                tracing::debug!(fingers, "gesture swipe begin");
            }

            InputEvent::GestureSwipeUpdate { event } => {
                self.gesture.on_swipe_update(event.delta_x(), event.delta_y());
            }

            InputEvent::GestureSwipeEnd { event } => {
                if let Some((fingers, dir)) = self.gesture.on_swipe_end(event.cancelled()) {
                    self.handle_swipe_gesture(fingers, dir);
                }
            }

            InputEvent::GesturePinchBegin { event } => {
                self.gesture.on_pinch_begin(event.fingers());
                tracing::debug!("gesture pinch begin");
            }

            InputEvent::GesturePinchUpdate { event } => {
                self.gesture.on_pinch_update(event.scale());
            }

            InputEvent::GesturePinchEnd { event } => {
                if let Some(scale) = self.gesture.on_pinch_end(event.cancelled()) {
                    tracing::info!(scale, "gesture pinch end -> forward cliente (futuro)");
                }
            }

            _ => {}
        }
    }

    fn handle_swipe_gesture(&mut self, fingers: u32, dir: SwipeDirection) {
        use lumo_ipc::MAX_WORKSPACES;
        match fingers {
            3 => match dir {
                SwipeDirection::Left => {
                    let next = (self.active_workspace % MAX_WORKSPACES) + 1;
                    tracing::info!(from = self.active_workspace, to = next, "3-finger left -> workspace next");
                    self.set_workspace(next);
                }
                SwipeDirection::Right => {
                    let prev = if self.active_workspace <= 1 {
                        MAX_WORKSPACES
                    } else {
                        self.active_workspace - 1
                    };
                    tracing::info!(from = self.active_workspace, to = prev, "3-finger right -> workspace prev");
                    self.set_workspace(prev);
                }
                SwipeDirection::Up => {
                    tracing::info!("3-finger up -> mission control W12.B");
                    self.execute_key_action(crate::input::keyboard::KeyAction::MissionControl);
                }
                SwipeDirection::Down => {
                    tracing::info!("3-finger down -> app expose (stub)");
                }
            },
            4 => {
                tracing::info!(dir = ?dir, "4-finger swipe -> desktop reveal (stub)");
            }
            _ => {
                tracing::debug!(fingers, dir = ?dir, "swipe gesture nao mapeado");
            }
        }
    }

    /// Executa uma KeyAction. Centraliza o dispatch pos-match.
    pub fn execute_key_action(&mut self, action: KeyAction) {
        match action {
            KeyAction::Spawn(cmd) => {
                self.spawn_cmd(&cmd);
            }
            KeyAction::CloseWindow => {
                self.close_focused_window();
            }
            KeyAction::Refresh => {
                tracing::info!("F5 refresh compositor (force redraw)");
                #[cfg(feature = "drm-backend")]
                {
                    self.drm_force_repaint = true;
                }
                // T1.6: broadcast ThemeReloaded com tema atual (nao hardcoded Light).
                {
                    let tokens = lumo_foundation::LumoTokens::load_from_disk();
                    let mode = match tokens.mode {
                        lumo_foundation::LumoTheme::Light => lumo_ipc::ThemeMode::Light,
                        lumo_foundation::LumoTheme::Dark => lumo_ipc::ThemeMode::Dark,
                    };
                    self.ipc.broadcast(&lumo_ipc::LumoEvent::ThemeReloaded { mode });
                }
            }
            KeyAction::Lock => {
                tracing::info!("lock pendente A40");
            }
            KeyAction::Launcher => {
                tracing::info!("launcher pendente A38");
            }
            KeyAction::Workspace(n) => {
                self.set_workspace(n);
            }
            KeyAction::MoveToWorkspace(n) => {
                self.move_focused_to_workspace(n);
            }
            KeyAction::CycleWindow(delta) => {
                // L1: SUPER+Tab -> FocusManager.cycle.
                let serial = smithay::utils::SERIAL_COUNTER.next_serial();
                let kb = self.keyboard.clone();
                let new_focus = self.focus_manager.cycle(&kb, &self.space, delta);
                kb.set_focus(self, new_focus, serial);
            }
            KeyAction::TileMove(dir) => {
                let dir_str = match dir {
                    TileDir::Up    => "Up",
                    TileDir::Down  => "Down",
                    TileDir::Left  => "Left",
                    TileDir::Right => "Right",
                };
                tracing::info!(dir = dir_str, "TileMove arrow");
            }
            KeyAction::TilingCycle => {
                self.tiling_mode = self.tiling_mode.next();
                let (out_w, out_h) = self.output_dimensions();
                crate::tiling::apply_tiling(&mut self.space, self.tiling_mode, out_w, out_h);
                tracing::info!(mode = self.tiling_mode.name(), "W12.A: tiling cycled");
                #[cfg(feature = "drm-backend")]
                { self.drm_force_repaint = true; }
            }
            KeyAction::TilingRebalance => {
                let (out_w, out_h) = self.output_dimensions();
                crate::tiling::apply_tiling(&mut self.space, self.tiling_mode, out_w, out_h);
                tracing::info!(mode = self.tiling_mode.name(), "W12.A: tiling rebalanced");
                #[cfg(feature = "drm-backend")]
                { self.drm_force_repaint = true; }
            }
            KeyAction::TilingFocusPrev => {
                let windows: Vec<_> = self.space.elements().cloned().collect();
                let kb = self.keyboard.clone();
                let cur = kb.current_focus();
                if let Some(win) = crate::tiling::focus_prev(&windows, cur.as_ref()) {
                    if let Some(surf) = win.wl_surface() {
                        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
                        let owned = surf.into_owned();
                        self.focus_manager.click_toplevel(owned.clone());
                        kb.set_focus(self, Some(owned), serial);
                    }
                }
            }
            KeyAction::TilingFocusNext => {
                let windows: Vec<_> = self.space.elements().cloned().collect();
                let kb = self.keyboard.clone();
                let cur = kb.current_focus();
                if let Some(win) = crate::tiling::focus_next(&windows, cur.as_ref()) {
                    if let Some(surf) = win.wl_surface() {
                        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
                        let owned = surf.into_owned();
                        self.focus_manager.click_toplevel(owned.clone());
                        kb.set_focus(self, Some(owned), serial);
                    }
                }
            }
            KeyAction::MissionControl => {
                if self.overview.is_some() {
                    let a11y = lumo_foundation::A11yTokens::load_from_disk();
                    if let Some(ov) = self.overview.as_mut() {
                        ov.close(a11y.reduced_motion);
                    }
                } else {
                    let a11y = lumo_foundation::A11yTokens::load_from_disk();
                    let kb = self.keyboard.clone();
                    let focused = kb.current_focus();
                    self.overview = Some(crate::overview::OverviewState::new(
                        &self.space,
                        focused.as_ref(),
                        a11y.reduced_motion,
                    ));
                    tracing::info!("W12.B: mission control opened");
                }
                #[cfg(feature = "drm-backend")]
                { self.drm_force_repaint = true; }
            }
            KeyAction::StackPicker => {
                if let Some(picker) = self.stack_picker.as_mut() {
                    picker.cycle_next();
                } else {
                    let kb = self.keyboard.clone();
                    let focused = kb.current_focus();
                    let picker = crate::stack_picker::StackPickerState::new(
                        &self.space,
                        focused.as_ref(),
                    );
                    if !picker.is_empty() {
                        self.stack_picker = Some(picker);
                        tracing::info!("W12.C: stack picker opened");
                    }
                }
                #[cfg(feature = "drm-backend")]
                { self.drm_force_repaint = true; }
            }
            KeyAction::FullscreenToggle => {
                self.toggle_fullscreen_focused();
            }
            KeyAction::Minimize => {
                tracing::info!("minimize pendente (sem iconify protocol)");
            }
            KeyAction::Quit => {
                tracing::info!("Ctrl+Alt+Backspace -> sair");
                self.running = false;
            }
            KeyAction::SwitchVt(n) => {
                #[cfg(feature = "drm-backend")]
                {
                    use smithay::backend::session::Session as _;
                    if let Some(sess) = self.session.as_mut() {
                        if let Err(err) = sess.change_vt(n) {
                            tracing::warn!(vt = n, ?err, "change_vt falhou");
                        } else {
                            tracing::info!(vt = n, "change_vt ok");
                        }
                    } else {
                        tracing::info!(vt = n, "switch_vt request sem session");
                    }
                }
                #[cfg(not(feature = "drm-backend"))]
                {
                    tracing::info!(vt = n, "switch_vt request (no-op fora de DRM)");
                }
            }
        }
    }

    /// Spawna um processo com o ambiente Wayland correto.
    fn spawn_cmd(&self, cmd: &str) {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        let xdg = std::env::var("XDG_CONFIG_HOME")
            .unwrap_or_else(|_| format!("{home}/.config"));
        let mut proc = std::process::Command::new(cmd);
        proc.env("HOME", &home);
        proc.env("XDG_CONFIG_HOME", &xdg);
        proc.env("LC_CTYPE", "C.UTF-8");
        if let Some(sock) = self.socket_name.as_deref() {
            proc.env("WAYLAND_DISPLAY", sock);
        }
        // Q3: GTK/Qt env pra appmenu funcionar em GTK3.
        proc.env("GTK_MODULES", "appmenu-gtk-module");
        proc.env("QT_QPA_PLATFORMTHEME", "appmenu-qt5");
        proc.env("UBUNTU_MENUPROXY", "1");
        if cmd == "foot" {
            proc.arg("-c").arg(format!("{home}/.config/foot/foot.ini"));
        }
        match proc.spawn() {
            Ok(child) => tracing::info!(pid = child.id(), cmd, "spawn ok"),
            Err(err) => tracing::warn!(?err, cmd, "spawn falhou"),
        }
    }

    /// Fecha a janela com foco via xdg_toplevel send_close.
    fn close_focused_window(&self) {
        let kb = self.keyboard.clone();
        if let Some(focused) = kb.current_focus() {
            let window = self
                .space
                .elements()
                .find(|w| {
                    w.wl_surface()
                        .map(|s| *s == focused)
                        .unwrap_or(false)
                })
                .cloned();
            if let Some(win) = window {
                if let Some(toplevel) = win.toplevel() {
                    toplevel.send_close();
                }
            }
        }
    }

    /// Cicla o foco entre janelas no espaco.
    fn cycle_window_focus(&mut self, delta: i8) {
        let windows: Vec<_> = self.space.elements().cloned().collect();
        if windows.is_empty() {
            return;
        }
        let kb = self.keyboard.clone();
        let current = kb.current_focus();
        let current_idx = current.as_ref().and_then(|focused| {
            windows.iter().position(|w| {
                w.wl_surface()
                    .map(|s| *s == *focused)
                    .unwrap_or(false)
            })
        });
        let len = windows.len() as isize;
        let next_idx = match current_idx {
            Some(i) => ((i as isize + delta as isize).rem_euclid(len)) as usize,
            None => 0,
        };
        if let Some(next_win) = windows.get(next_idx) {
            if let Some(surface) = next_win.wl_surface() {
                let serial = SERIAL_COUNTER.next_serial();
                let owned = surface.into_owned();
                kb.set_focus(self, Some(owned), serial);
            }
        }
    }

    /// Alterna fullscreen na janela com foco.
    fn toggle_fullscreen_focused(&self) {
        use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State as XdgState;
        let kb = self.keyboard.clone();
        if let Some(focused) = kb.current_focus() {
            let window = self
                .space
                .elements()
                .find(|w| {
                    w.wl_surface()
                        .map(|s| *s == focused)
                        .unwrap_or(false)
                })
                .cloned();
            if let Some(win) = window {
                if let Some(toplevel) = win.toplevel() {
                    let is_fs = toplevel
                        .current_state()
                        .states
                        .contains(XdgState::Fullscreen);
                    toplevel.with_pending_state(|state| {
                        if is_fs {
                            state.states.unset(XdgState::Fullscreen);
                        } else {
                            state.states.set(XdgState::Fullscreen);
                        }
                    });
                    toplevel.send_configure();
                }
            }
        }
    }
}


fn write_sys_led(name: &str, on: bool) {
    let dir = std::path::Path::new("/sys/class/leds");
    let val = if on { b"1" as &[u8] } else { b"0" };
    let suffix = format!("::{}", name);
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.ends_with(&suffix) {
                let _ = std::fs::write(format!("/sys/class/leds/{}/brightness", n), val);
            }
        }
    }
}
