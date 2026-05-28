//! Input dispatch - converte eventos de backend em acoes do compositor.
//!
//! B2: KeyboardConfig com 16+ bindings Lumo-style carregados de
//! ~/.config/lumo/keyboard.toml (fallback para default_bindings()).
//! Handler handle_input procura match na lista de bindings e executa
//! a acao correspondente.

use smithay::backend::input::{
    AbsolutePositionEvent, Axis, AxisSource, ButtonState, Event as _, GestureBeginEvent,
    GestureEndEvent, GesturePinchUpdateEvent, GestureSwipeUpdateEvent, InputBackend, InputEvent,
    KeyState, KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent, PointerMotionEvent,
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
                self.should_render = true;
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
                // Windows-style focus steal protection: key press marca gesto.
                if press {
                    self.record_user_gesture();
                }

                // A40: Cell pra capturar sym calculado dentro do closure.
                let last_sym_for_a40 =
                    std::cell::Cell::new(smithay::input::keyboard::xkb::Keysym::NoSymbol);
                // W12.C: capture sym on key release too (for picker SUPER detection).
                let last_sym_release =
                    std::cell::Cell::new(smithay::input::keyboard::xkb::Keysym::NoSymbol);
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
                        if let Some(action) = state.keyboard_config.match_binding(mods, sym) {
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
                            if let Some(p) = self.stack_picker.as_mut() {
                                p.cycle_prev();
                            }
                        }
                        // Esc -> dismiss without switching.
                        if sym == Keysym::Escape {
                            self.stack_picker = None;
                            tracing::trace!("W12.C: picker dismissed via Esc");
                        }
                    }
                    // SUPER key release -> activate selected and close.
                    let release_sym = last_sym_release.get();
                    if !press && (release_sym == Keysym::Super_L || release_sym == Keysym::Super_R)
                    {
                        if let Some(picker) = self.stack_picker.take() {
                            if let Some(win) = picker.selected_window() {
                                if let Some(surf) = win.wl_surface() {
                                    let owned = surf.into_owned();
                                    let serial = smithay::utils::SERIAL_COUNTER.next_serial();
                                    self.focus_manager.click_toplevel(owned.clone());
                                    let kb3 = self.keyboard.clone();
                                    self.space.raise_element(win, true);
                                    kb3.set_focus(self, Some(owned), serial);
                                    tracing::trace!(
                                        "W12.C: picker activated window on SUPER release"
                                    );
                                }
                            }
                        }
                        #[cfg(feature = "drm-backend")]
                        {
                            self.drm_force_repaint = true;
                        }
                    }
                }
                // W12.B: overview key handling.
                if self.overview.is_some() && press {
                    use smithay::input::keyboard::xkb::Keysym;
                    let sym = last_sym_for_a40.get();
                    if sym == Keysym::Escape {
                        let a11y = lumo_foundation::A11yTokens::load_from_disk();
                        if let Some(ov) = self.overview.as_mut() {
                            ov.close(a11y.reduced_motion);
                        }
                        tracing::trace!("W12.B: overview dismissed via Esc");
                    }
                }
                // A40: Return sem binding + sem toplevel focado
                // -> roteia pra desktop abrir icone selecionado.
                if press && last_sym_for_a40.get() == smithay::input::keyboard::xkb::Keysym::Return
                {
                    let has_toplevel_focus = matches!(self.focus_manager.state, crate::focus::FocusState::Toplevel(_));
                    if !has_toplevel_focus {
                        tracing::trace!("A40: Return sem toplevel -> DesktopOpenSelected");
                        self.broadcast_desktop_open_selected();
                    }
                }
            }

            InputEvent::PointerMotion { event } => {
                self.should_render = true;
                self.cursor_last_motion_ts = Some(std::time::Instant::now());
                // W10.B: reset idle timer on any pointer movement.
                self.idle_manager.reset();
                {
                    let seat_ref = self.seat.clone();
                    self.idle_notifier_state.notify_activity(&seat_ref);
                }
                let dx = event.delta_x();
                let dy = event.delta_y();
                // Clamp pelas dimensoes REAIS do output (antes: 1919x1079 fixo
                // -> em painel != 1080p o cursor ficava preso numa caixa e as
                // bordas direita/inferior eram inalcancaveis).
                let (ow, oh) = self.output_dimensions();
                let max_x = (ow as f64 - 1.0).max(0.0);
                let max_y = (oh as f64 - 1.0).max(0.0);
                let new_x = (self.pointer_location.x + dx).clamp(0.0, max_x);
                let new_y = (self.pointer_location.y + dy).clamp(0.0, max_y);
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
                // INSTR.F4: libinput-real motion post-dispatch.
                {
                    let cf = pointer.current_focus();
                    tracing::trace!(
                        ploc = ?(self.pointer_location.x as i32, self.pointer_location.y as i32),
                        under_some = under.is_some(),
                        current_focus_some = cf.is_some(),
                        "INSTR.F4 libinput_motion post-motion"
                    );
                }

                // W12.B: update overview hover.
                if self.overview.is_some() {
                    let pos_l = self.pointer_location.to_i32_round();
                    let (ow, oh) = self.output_dimensions();
                    let hit = self
                        .overview
                        .as_ref()
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
                self.should_render = true;
                self.cursor_last_motion_ts = Some(std::time::Instant::now());
                // Transform pelas dimensoes REAIS do output (antes: 1280x720
                // fixo -> winit dev + touch/tablet caiam o cursor a ~66% da
                // tela, hit-test de botoes errava).
                let (ow, oh) = self.output_dimensions();
                let x = event.x_transformed(ow as u32);
                let y = event.y_transformed(oh as u32);
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
                self.should_render = true;
                self.cursor_last_motion_ts = Some(std::time::Instant::now());
                let serial = SERIAL_COUNTER.next_serial();
                let button = event.button_code();
                let state: ButtonState = event.state();
                let pointer = self.pointer.clone();

                // C3 debug: log raw button code pra diagnostico BTN_RIGHT.
                tracing::trace!(button, state = ?state, pos = ?(self.pointer_location.x as i32, self.pointer_location.y as i32), "C3 PointerButton");

                // Telemetry: record click event + store input timestamp for input-to-paint.
                {
                    use lumo_telemetry::EventKind;
                    let mut meta = std::collections::HashMap::new();
                    meta.insert("button".to_string(), format!("{}", button));
                    meta.insert(
                        "pos".to_string(),
                        format!(
                            "{},{}",
                            self.pointer_location.x as i32, self.pointer_location.y as i32
                        ),
                    );
                    lumo_telemetry::record_event(EventKind::Click, meta);
                    self.last_input_ts = Some(std::time::Instant::now());
                    // Windows-style focus steal protection: marca gesto user.
                    self.record_user_gesture();
                }

                if state == ButtonState::Pressed {
                    // M1: SSD hit-test antes de repassar o click ao cliente.
                    // Verifica close button e titlebar para janelas com SSD ativo.
                    // T1.1: hit-test SSD titlebar -- BTN_LEFT e BTN_RIGHT.
                    {
                        use crate::backend::render_common::{
                            ssd_close_btn_rect_logical, ssd_max_btn_rect_logical,
                            ssd_min_btn_rect_logical, ssd_titlebar_rect_logical,
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
                            let in_menu = ptr_pos.x >= mx
                                && ptr_pos.x <= mx + menu_w
                                && ptr_pos.y >= my
                                && ptr_pos.y <= my + item_h * 5;
                            if in_menu && button == 0x110 {
                                let idx = ((ptr_pos.y - my) / item_h) as usize;
                                self.titlebar_menu = None;
                                match idx {
                                    0 => {
                                        // W32.6: snap close (igual btn X) - unmap imediato.
                                        if let Some(tl) = menu_win.toplevel() {
                                            tl.send_close();
                                        }
                                        if let Some(s) = menu_win.wl_surface() {
                                            self.ssd_windows.remove(&*s);
                                        }
                                        self.space.unmap_elem(&menu_win);
                                        self.should_render = true;
                                    }
                                    1 => {
                                        if let Some(tl) = menu_win.toplevel() {
                                            use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State as XdgState;
                                            let is_fs = tl
                                                .current_state()
                                                .states
                                                .contains(XdgState::Fullscreen);
                                            tl.with_pending_state(|st| {
                                                if is_fs {
                                                    st.states.unset(XdgState::Fullscreen);
                                                } else {
                                                    st.states.set(XdgState::Fullscreen);
                                                }
                                            });
                                            tl.send_configure();
                                        }
                                    }
                                    2 => {
                                        tracing::trace!("T1.1 menu: Minimizar (stub)");
                                    }
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
                                        tracing::trace!("T1.1 menu: Sobre {app_id}");
                                    }
                                    _ => {}
                                }
                                ssd_handled = true;
                            } else if !in_menu {
                                // D2: dismiss-on-outside-click titlebar_menu.
                                self.titlebar_menu = None;
                                #[cfg(feature = "drm-backend")]
                                {
                                    self.drm_force_repaint = true;
                                }
                            }
                            if ssd_handled {
                                pointer.frame(self);
                                return;
                            }
                        }

                        // Front-first iter pra topmost ganhar hit (bug user
                        // janela atras "pulando pra frente").
                        let windows: Vec<_> = self.space.elements().cloned().collect();
                        for window in windows.iter().rev() {
                            let surf_opt = window.toplevel().map(|t| t.wl_surface().clone());
                            let surf = match surf_opt {
                                Some(s) => s,
                                None => continue,
                            };
                            if !self.ssd_windows.contains(&surf) {
                                continue;
                            }
                            let loc = self.space.element_location(window).unwrap_or_default();
                            let geo = window.geometry();
                            // Bug user (2026-05): clicar no CONTEUDO da janela da
                            // frente onde o titlebar de uma janela de tras esta
                            // (atras, ocluso) ativava a de tras (pulava pra frente).
                            // Guard de oclusao: se qualquer janela mais ao topo
                            // (depois desta em windows, que e back-to-front) cobre
                            // o ponto com seu conteudo, o titlebar desta esta ocluso
                            // -> ignora. So a janela topmost no ponto recebe acao SSD.
                            let cur_idx =
                                windows.iter().position(|w| w == window).unwrap_or(0);
                            let occluded = windows[cur_idx + 1..].iter().any(|higher| {
                                let hloc =
                                    self.space.element_location(higher).unwrap_or_default();
                                let hgeo = higher.geometry();
                                smithay::utils::Rectangle::new(
                                    smithay::utils::Point::from((
                                        hloc.x + hgeo.loc.x,
                                        hloc.y + hgeo.loc.y,
                                    )),
                                    hgeo.size,
                                )
                                .contains(ptr_pos)
                            });
                            if occluded {
                                continue;
                            }
                            let close_rect = ssd_close_btn_rect_logical(loc, geo.size.w);
                            let max_rect = ssd_max_btn_rect_logical(loc, geo.size.w);
                            let min_rect = ssd_min_btn_rect_logical(loc, geo.size.w);

                            // W17.1: minimize button (amarelo) -- stub log ate iconify protocol.
                            if button == 0x110 && min_rect.contains(ptr_pos) {
                                tracing::trace!(
                                    "W17.1: minimize click (stub, no Wayland iconify protocol)"
                                );
                                ssd_handled = true;
                                break;
                            }
                            // W17.1 + W37.4: maximize button (verde) toggle Maximized.
                            // Antes usava Fullscreen sem size -> client mantinha tamanho
                            // anterior (bug: maximizada parecia snap-half).
                            // Agora: Maximized + size = (out_w, out_h - BAR_HEIGHT).
                            if button == 0x110 && max_rect.contains(ptr_pos) {
                                if let Some(tl) = window.toplevel() {
                                    use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State as XdgState;
                                    use smithay::utils::{Point, Size};
                                    let is_max =
                                        tl.current_state().states.contains(XdgState::Maximized);
                                    let (ow, oh) = self.output_dimensions();
                                    let geom = crate::tiling::maximized_geometry(ow, oh);
                                    let win_clone = window.clone();
                                    tl.with_pending_state(|st| {
                                        if is_max {
                                            st.states.unset(XdgState::Maximized);
                                            st.size = None;
                                        } else {
                                            st.states.set(XdgState::Maximized);
                                            st.size = Some(Size::from((geom.w, geom.h)));
                                        }
                                    });
                                    tl.send_configure();
                                    if !is_max {
                                        self.space.map_element(
                                            win_clone,
                                            Point::from((geom.x, geom.y)),
                                            true,
                                        );
                                    }
                                    tracing::trace!(
                                        was_max = is_max,
                                        out_w = ow,
                                        out_h = oh,
                                        "W37.4: maximize toggle"
                                    );
                                }
                                ssd_handled = true;
                                break;
                            }
                            if button == 0x110 && close_rect.contains(ptr_pos) {
                                // W32.5: snap close - unmap imediato + send_close async.
                                // Visual: janela some no proximo frame (sem espera cliente).
                                if let Some(toplevel) = window.toplevel() {
                                    toplevel.send_close();
                                }
                                if let Some(s) = window.wl_surface() {
                                    self.ssd_windows.remove(&*s);
                                }
                                self.space.unmap_elem(&window);
                                self.should_render = true;
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
                        let hit = self
                            .overview
                            .as_ref()
                            .and_then(|ov| ov.hit_test(pos_l, ow, oh));
                        if let Some(idx) = hit {
                            let win_opt = self
                                .overview
                                .as_ref()
                                .and_then(|ov| ov.windows.get(idx).cloned());
                            if let Some(win) = win_opt {
                                if let Some(surf) = win.wl_surface() {
                                    let owned = surf.into_owned();
                                    let serial_ov = smithay::utils::SERIAL_COUNTER.next_serial();
                                    self.space.raise_element(&win, true);
                                    self.focus_manager.click_toplevel(owned.clone());
                                    let kb_ov = self.keyboard.clone();
                                    kb_ov.set_focus(self, Some(owned), serial_ov);
                                    tracing::trace!(idx, "W12.B: overview cell activated");
                                }
                            }
                        }
                        let a11y_ov = lumo_foundation::A11yTokens::load_from_disk();
                        if let Some(ov) = self.overview.as_mut() {
                            ov.close(a11y_ov.reduced_motion);
                        }
                        #[cfg(feature = "drm-backend")]
                        {
                            self.drm_force_repaint = true;
                        }
                        pointer.frame(self);
                        return;
                    }
                    // D2: broadcast CloseDropdowns quando LEFT click fora da bar.
                    // Bar fecha dropdown se ativo; desktop fecha menu/ctx_menu.
                    // W37: NAO broadcast em right-click - desktop ABRE menu no right
                    //      click, broadcast logo apos abertura fechava o menu (race).
                    //      Right-click no compositor nao deve fechar dropdowns alheios.
                    if button == 0x110 && !self.pos_is_on_bar(self.pointer_location) {
                        self.ipc.broadcast(&lumo_ipc::LumoEvent::CloseDropdowns);
                    }

                    let kb = self.keyboard.clone();
                    let new_focus = if let Some((surface, _)) =
                        self.surface_under(self.pointer_location)
                    {
                        // Q2: keyboard focus SEMPRE no root xdg_toplevel.
                        // Chromium/Firefox usam subsurfaces pra popups multi-process.
                        // Click em subsurface -> precisa achar root toplevel via
                        // wl_compositor::get_parent chain.
                        // Sem isso: foco vai pra None, clicks subsequentes em
                        // Chrome nao registram (bug user 2026-05).
                        // Ref: gitlab.freedesktop.org/wayland/wayland/-/issues/294
                        use smithay::wayland::compositor as wl_compositor;
                        use smithay::wayland::shell::xdg::XdgToplevelSurfaceData;
                        let mut root = surface.clone();
                        while let Some(parent) = wl_compositor::get_parent(&root) {
                            root = parent;
                        }
                        let root_is_toplevel =
                            wl_compositor::with_states(&root, |states| {
                                states.data_map.get::<XdgToplevelSurfaceData>().is_some()
                            });
                        if root_is_toplevel {
                            // Q1: raise root toplevel ao topo no click.
                            let win_to_raise = self
                                .space
                                .elements()
                                .find(|w| w.wl_surface().map(|s| *s == root).unwrap_or(false))
                                .cloned();
                            if let Some(win) = win_to_raise {
                                self.space.raise_element(&win, true);
                            }
                            self.focus_manager.click_toplevel(root)
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
                            let popups: Vec<_> =
                                PopupManager::popups_for_surface(&root_surf).collect();
                            for (popup, popup_offset) in popups {
                                let geo = popup.geometry();
                                let popup_loc = win_loc + popup_offset;
                                let rect =
                                    smithay::utils::Rectangle::new(popup_loc + geo.loc, geo.size);
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

            InputEvent::PointerAxis { event } => {
                // Q3: scroll universal (wheel mouse + touchpad 2-finger).
                // Antes: branch ausente, axis events caiam em _ => {} =
                // apps nao recebiam scroll. Bug user 2026-05.
                use smithay::input::pointer::AxisFrame;
                let time = event.time_msec();
                let source = event.source();
                let mut frame = AxisFrame::new(time).source(source);
                let h = event.amount(Axis::Horizontal).unwrap_or(0.0);
                let v = event.amount(Axis::Vertical).unwrap_or(0.0);
                if v != 0.0 {
                    frame = frame.value(Axis::Vertical, v);
                }
                if h != 0.0 {
                    frame = frame.value(Axis::Horizontal, h);
                }
                // V120 (alta resolucao): wheel mouse gera passos discretos
                // 120-unit. Touchpad continuous nao tem v120.
                if matches!(source, AxisSource::Wheel | AxisSource::WheelTilt) {
                    if let Some(v120_h) = event.amount_v120(Axis::Horizontal) {
                        frame = frame.v120(Axis::Horizontal, v120_h as i32);
                    }
                    if let Some(v120_v) = event.amount_v120(Axis::Vertical) {
                        frame = frame.v120(Axis::Vertical, v120_v as i32);
                    }
                }
                // libinput stop event = amount 0 em todos eixos.
                if v == 0.0 && h == 0.0 && matches!(source, AxisSource::Finger) {
                    frame = frame.stop(Axis::Vertical).stop(Axis::Horizontal);
                }
                let pointer = self.pointer.clone();
                pointer.axis(self, frame);
                pointer.frame(self);
                self.should_render = true;
                self.record_user_gesture();
                #[cfg(feature = "drm-backend")]
                {
                    self.drm_force_repaint = true;
                }
            }

            InputEvent::GestureSwipeBegin { event } => {
                let fingers = event.fingers();
                self.gesture.on_swipe_begin(fingers);
                tracing::debug!(fingers, "gesture swipe begin");
            }

            InputEvent::GestureSwipeUpdate { event } => {
                self.gesture
                    .on_swipe_update(event.delta_x(), event.delta_y());
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
                    tracing::trace!(scale, "gesture pinch end -> forward cliente (futuro)");
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
                    tracing::trace!(
                        from = self.active_workspace,
                        to = next,
                        "3-finger left -> workspace next"
                    );
                    self.set_workspace(next);
                }
                SwipeDirection::Right => {
                    let prev = if self.active_workspace <= 1 {
                        MAX_WORKSPACES
                    } else {
                        self.active_workspace - 1
                    };
                    tracing::trace!(
                        from = self.active_workspace,
                        to = prev,
                        "3-finger right -> workspace prev"
                    );
                    self.set_workspace(prev);
                }
                SwipeDirection::Up => {
                    tracing::trace!("3-finger up -> mission control W12.B");
                    self.execute_key_action(crate::input::keyboard::KeyAction::MissionControl);
                }
                SwipeDirection::Down => {
                    tracing::trace!("3-finger down -> app expose (stub)");
                }
            },
            4 => {
                tracing::trace!(dir = ?dir, "4-finger swipe -> desktop reveal (stub)");
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
                tracing::trace!("F5 refresh compositor (force redraw)");
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
                    self.ipc
                        .broadcast(&lumo_ipc::LumoEvent::ThemeReloaded { mode });
                }
            }
            KeyAction::Lock => {
                tracing::trace!("lock pendente A40");
            }
            KeyAction::Launcher => {
                tracing::trace!("launcher pendente A38");
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
                    TileDir::Up => "Up",
                    TileDir::Down => "Down",
                    TileDir::Left => "Left",
                    TileDir::Right => "Right",
                };
                tracing::trace!(dir = dir_str, "TileMove arrow");
            }
            KeyAction::TilingCycle => {
                self.tiling_mode = self.tiling_mode.next();
                let (out_w, out_h) = self.output_dimensions();
                crate::tiling::apply_tiling(&mut self.space, self.tiling_mode, out_w, out_h);
                tracing::trace!(mode = self.tiling_mode.name(), "W12.A: tiling cycled");
                #[cfg(feature = "drm-backend")]
                {
                    self.drm_force_repaint = true;
                }
            }
            KeyAction::TilingRebalance => {
                let (out_w, out_h) = self.output_dimensions();
                crate::tiling::apply_tiling(&mut self.space, self.tiling_mode, out_w, out_h);
                tracing::trace!(mode = self.tiling_mode.name(), "W12.A: tiling rebalanced");
                #[cfg(feature = "drm-backend")]
                {
                    self.drm_force_repaint = true;
                }
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
                    tracing::trace!("W12.B: mission control opened");
                }
                #[cfg(feature = "drm-backend")]
                {
                    self.drm_force_repaint = true;
                }
            }
            KeyAction::StackPicker => {
                if let Some(picker) = self.stack_picker.as_mut() {
                    picker.cycle_next();
                } else {
                    let kb = self.keyboard.clone();
                    let focused = kb.current_focus();
                    let picker =
                        crate::stack_picker::StackPickerState::new(&self.space, focused.as_ref());
                    if !picker.is_empty() {
                        self.stack_picker = Some(picker);
                        tracing::trace!("W12.C: stack picker opened");
                    }
                }
                #[cfg(feature = "drm-backend")]
                {
                    self.drm_force_repaint = true;
                }
            }
            KeyAction::FullscreenToggle => {
                self.toggle_fullscreen_focused();
            }
            KeyAction::Minimize => {
                tracing::trace!("minimize pendente (sem iconify protocol)");
            }
            KeyAction::Quit => {
                tracing::trace!("Ctrl+Alt+Backspace -> sair");
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
                            tracing::trace!(vt = n, "change_vt ok");
                        }
                    } else {
                        tracing::trace!(vt = n, "switch_vt request sem session");
                    }
                }
                #[cfg(not(feature = "drm-backend"))]
                {
                    tracing::trace!(vt = n, "switch_vt request (no-op fora de DRM)");
                }
            }
            KeyAction::HideWindow => {
                // F1.5-D1: hide window focused (sem fechar).
                // Iconify protocol nao tem em xdg-shell core; usar workspace
                // virtual "hidden" como workaround: unmap from space.
                if let Some(focused) = self.keyboard.current_focus() {
                    let win = self
                        .space
                        .elements()
                        .find(|w| w.wl_surface().map(|s| *s == focused).unwrap_or(false))
                        .cloned();
                    if let Some(w) = win {
                        self.space.unmap_elem(&w);
                        tracing::info!("F1.5-D1: HideWindow unmap focused");
                    }
                }
            }
            KeyAction::ShortcutHelp => {
                // F1.5-D1: emit IPC pra bar/desktop renderizar overlay help.
                // Por enquanto so log; full overlay design backlog.
                tracing::info!("F1.5-D1: ShortcutHelp (overlay TBD)");
            }
            KeyAction::JumpToWindow(n) => {
                // F1.5-D1: focus N-th window (1-indexed) do space.
                let windows: Vec<_> = self.space.elements().cloned().collect();
                let idx = (n as usize).saturating_sub(1);
                if let Some(win) = windows.get(idx) {
                    if let Some(surf) = win.wl_surface() {
                        let owned = surf.into_owned();
                        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
                        self.space.raise_element(win, true);
                        let new_focus = self.focus_manager.click_toplevel(owned.clone());
                        let kb = self.keyboard.clone();
                        kb.set_focus(self, new_focus, serial);
                        tracing::info!(n, "F1.5-D1: JumpToWindow");
                    }
                }
            }
            KeyAction::ClipboardHistory => {
                // F1.5-C2: Super+Shift+V abre lumo-clip picker.
                // Mantido pra compat; novos bindings usam InvokeApp(Clipboard).
                tracing::info!("F1.5-C2: ClipboardHistory spawn lumo-clip");
                self.invoke_app(lumo_ipc::ShellApp::Clipboard);
            }
            KeyAction::InvokeApp(app) => {
                // A2 review: resolve via ShellAppRegistry (com defaults).
                self.invoke_app(app);
            }
        }
    }

    /// A2 review: resolve ShellApp -> ActivationKind via registry e executa.
    /// Spawn -> spawn_cmd; Signal/DBus pendentes (TBD).
    fn invoke_app(&self, app: lumo_ipc::ShellApp) {
        let registry = self.shell_app_registry();
        let Some(activation) = registry.lookup(app) else {
            tracing::warn!(?app, "invoke_app: nao registrado");
            return;
        };
        match activation {
            lumo_ipc::ActivationKind::Spawn { command } => {
                let cmd = command.clone();
                self.spawn_cmd(&cmd);
            }
            lumo_ipc::ActivationKind::Signal { pidfile, signal } => {
                tracing::info!(?app, ?pidfile, ?signal, "Signal activation TBD");
            }
            lumo_ipc::ActivationKind::DBus {
                bus_name,
                object_path,
                interface,
                method,
            } => {
                tracing::info!(
                    ?app, ?bus_name, ?object_path, ?interface, ?method,
                    "DBus activation TBD"
                );
            }
        }
    }

    /// A2 review: retorna registry de ShellApps. Default por enquanto;
    /// carregar de ~/.config/lumo/shell-apps.toml pendente.
    fn shell_app_registry(&self) -> lumo_ipc::ShellAppRegistry {
        lumo_ipc::ShellAppRegistry::default()
    }

    /// Spawna um processo com o ambiente Wayland correto.
    ///
    /// Security (review M3 + H4):
    /// - M3 C2: resolve `cmd` pra path absoluto antes de spawn pra evitar
    ///   PATH hijack. Tenta `/usr/bin/<cmd>`, `/usr/local/bin/<cmd>`, `~/.local/bin/<cmd>`,
    ///   senao deixa fallback Command::new (PATH lookup, com warn).
    /// - H4: pre_exec setsid pra desacoplar processo do compositor; SIGHUP
    ///   nao mata os filhos quando compositor sair. process_group(0) tambem
    ///   evita zombies via SIGCHLD ignore (kernel reaps).
    fn spawn_cmd(&self, cmd: &str) {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        let xdg = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{home}/.config"));
        let resolved = resolve_command_path(cmd, &home);
        let mut proc = std::process::Command::new(&resolved);
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
        // H4: setsid via pre_exec — child vira leader de session+pgroup,
        // sobrevive SIGHUP do compositor + nao herda controlling tty.
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            unsafe {
                proc.pre_exec(|| {
                    if libc::setsid() == -1 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
        }
        match proc.spawn() {
            Ok(child) => tracing::trace!(pid = child.id(), cmd, ?resolved, "spawn ok"),
            Err(err) => tracing::warn!(?err, cmd, ?resolved, "spawn falhou"),
        }
    }

    /// Fecha a janela com foco via xdg_toplevel send_close.
    /// W32.6: snap unmap pra close instantaneo visual.
    fn close_focused_window(&mut self) {
        let kb = self.keyboard.clone();
        if let Some(focused) = kb.current_focus() {
            let window = self
                .space
                .elements()
                .find(|w| w.wl_surface().map(|s| *s == focused).unwrap_or(false))
                .cloned();
            if let Some(win) = window {
                if let Some(toplevel) = win.toplevel() {
                    toplevel.send_close();
                }
                if let Some(s) = win.wl_surface() {
                    self.ssd_windows.remove(&*s);
                }
                self.space.unmap_elem(&win);
                self.should_render = true;
            }
        }
    }

    /// Cicla o foco entre janelas no espaco.
    fn _cycle_window_focus(&mut self, delta: i8) {
        let windows: Vec<_> = self.space.elements().cloned().collect();
        if windows.is_empty() {
            return;
        }
        let kb = self.keyboard.clone();
        let current = kb.current_focus();
        let current_idx = current.as_ref().and_then(|focused| {
            windows
                .iter()
                .position(|w| w.wl_surface().map(|s| *s == *focused).unwrap_or(false))
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
                .find(|w| w.wl_surface().map(|s| *s == focused).unwrap_or(false))
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

// SI.1: synthetic input primitives.
//
// Injetam eventos no PointerHandle/KeyboardHandle do compositor sem passar por
// libinput/uinput. Usado pelo lumo-bridge (HTTP) e agentes LLM para remotar
// input quando ydotool nao funciona (uinput hot-plug nao detectado pelo wm).
//
// Notas:
// - Tempo monotonico vem de `self.clock.now().as_millis()`, mesmo padrao do
//   resto do compositor.
// - PointerMove faz hit-test (`surface_under`) pra entregar enter/leave
//   corretamente ao surface alvo.
// - SyntheticKey recebe **evdev keycode** (KEY_*), nao keysym xkb. O compositor
//   converte pra xkb internamente (evdev + 8). Layout/mods sao do estado atual
//   do KbdInternal -- ou seja, segue o teclado real.
// - Combo usa std::thread::sleep curto entre press all / release reversed. OK
//   porque o calloop tick que dispara handle_ipc_command nao bloqueia clientes
//   wayland por menos de 20ms (release acontece no mesmo tick).

impl LumoState {
    /// SI.1: Injeta motion absoluto. Coords em pixels logicos.
    pub fn handle_synthetic_pointer_move(&mut self, x: f64, y: f64) {
        let serial = SERIAL_COUNTER.next_serial();
        let time = self.clock.now().as_millis();
        self.pointer_location = (x, y).into();
        let under = self.surface_under(self.pointer_location);
        let pointer = self.pointer.clone();
        pointer.motion(
            self,
            under.clone().map(|(s, loc)| (s, loc.to_f64())),
            &MotionEvent {
                location: self.pointer_location,
                serial,
                time,
            },
        );
        pointer.frame(self);
        // INSTR.F: log pointer.current_focus() apos motion sintetico.
        {
            let cf = pointer.current_focus();
            tracing::trace!(
                x,
                y,
                under_some = under.is_some(),
                current_focus_some = cf.is_some(),
                "INSTR.F1 synth_move post-motion"
            );
        }
        #[cfg(feature = "drm-backend")]
        {
            self.drm_force_repaint = true;
        }
        tracing::debug!(x, y, "SI.1: SyntheticPointerMove");
    }

    /// SI.1: Injeta press/release de botao do ponteiro.
    /// `button` = codigo linux/input-event-codes (BTN_LEFT=0x110 etc).
    pub fn handle_synthetic_pointer_button(&mut self, button: u32, pressed: bool) {
        use smithay::backend::input::ButtonState;
        let serial = SERIAL_COUNTER.next_serial();
        let time = self.clock.now().as_millis();
        let state = if pressed {
            ButtonState::Pressed
        } else {
            ButtonState::Released
        };

        // SI.2: replica do PointerButton real -- SSD btn hit-test +
        // titlebar drag grab. Sem isso clicks via IPC sintetico nao
        // acionam close/max/min nem MoveSurfaceGrab.
        // Escopo minimo (sem titlebar_menu popup, sem overview, sem
        // dismiss popups -- esses ficam para evolucao incremental).
        if pressed {
            use crate::backend::render_common::{
                ssd_close_btn_rect_logical, ssd_max_btn_rect_logical, ssd_min_btn_rect_logical,
                ssd_titlebar_rect_logical,
            };
            use smithay::input::pointer::Focus;
            let ptr_pos = self.pointer_location.to_i32_round();
            let mut ssd_handled = false;

            // Bug user (2026-05): space.elements() retorna back-to-front.
            // Iter front-first (rev) garante topmost window pega hit
            // primeiro. Sem rev, click em SSD area onde 2 windows overlap
            // raisava window de tras (que aparecia "pulando pra frente").
            let windows: Vec<_> = self.space.elements().cloned().collect();
            for window in windows.iter().rev() {
                let surf_opt = window.toplevel().map(|t| t.wl_surface().clone());
                let surf = match surf_opt {
                    Some(s) => s,
                    None => continue,
                };
                if !self.ssd_windows.contains(&surf) {
                    continue;
                }
                let loc = self.space.element_location(window).unwrap_or_default();
                let geo = window.geometry();
                // Guard de oclusao (mesmo bug do loop de pointer real): so a
                // janela topmost no ponto recebe acao SSD. Janela mais ao topo
                // (depois desta em windows, back-to-front) cobrindo o ponto com
                // conteudo oclui o titlebar desta.
                let cur_idx = windows.iter().position(|w| w == window).unwrap_or(0);
                let occluded = windows[cur_idx + 1..].iter().any(|higher| {
                    let hloc = self.space.element_location(higher).unwrap_or_default();
                    let hgeo = higher.geometry();
                    smithay::utils::Rectangle::new(
                        smithay::utils::Point::from((hloc.x + hgeo.loc.x, hloc.y + hgeo.loc.y)),
                        hgeo.size,
                    )
                    .contains(ptr_pos)
                });
                if occluded {
                    continue;
                }
                let close_rect = ssd_close_btn_rect_logical(loc, geo.size.w);
                let max_rect = ssd_max_btn_rect_logical(loc, geo.size.w);
                let min_rect = ssd_min_btn_rect_logical(loc, geo.size.w);

                if button == 0x110 && min_rect.contains(ptr_pos) {
                    tracing::trace!("SI.2: synthetic minimize click (stub)");
                    ssd_handled = true;
                    break;
                }
                if button == 0x110 && max_rect.contains(ptr_pos) {
                    if let Some(tl) = window.toplevel() {
                        use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State as XdgState;
                        let is_fs = tl.current_state().states.contains(XdgState::Fullscreen);
                        tl.with_pending_state(|st| {
                            if is_fs {
                                st.states.unset(XdgState::Fullscreen);
                            } else {
                                st.states.set(XdgState::Fullscreen);
                            }
                        });
                        tl.send_configure();
                        tracing::trace!(was_fs = is_fs, "SI.2: synthetic maximize toggle");
                    }
                    ssd_handled = true;
                    break;
                }
                if button == 0x110 && close_rect.contains(ptr_pos) {
                    if let Some(toplevel) = window.toplevel() {
                        toplevel.send_close();
                    }
                    ssd_handled = true;
                    break;
                }
                let title_rect = ssd_titlebar_rect_logical(loc, geo.size.w);
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
                let pointer = self.pointer.clone();
                pointer.frame(self);
                #[cfg(feature = "drm-backend")]
                {
                    self.drm_force_repaint = true;
                }
                tracing::debug!(
                    button = format!("0x{:x}", button),
                    "SI.2: synthetic consumed by SSD/grab"
                );
                return;
            }
        }

        // SI.1.fix: ao PRESSIONAR sobre toplevel xdg-shell, raise + focus
        // (mesmo flow do PointerButton real). Sem isso clients nao recebem
        // o evento + focus de teclado nao acompanha o clique.
        if pressed {
            if let Some((surface, _)) = self.surface_under(self.pointer_location) {
                use smithay::wayland::shell::xdg::XdgToplevelSurfaceData;
                let is_toplevel = smithay::wayland::compositor::with_states(&surface, |states| {
                    states.data_map.get::<XdgToplevelSurfaceData>().is_some()
                });
                if is_toplevel {
                    let win_to_raise = self
                        .space
                        .elements()
                        .find(|w| w.wl_surface().map(|s| *s == surface).unwrap_or(false))
                        .cloned();
                    if let Some(win) = win_to_raise {
                        self.space.raise_element(&win, true);
                    }
                    let new_focus = self.focus_manager.click_toplevel(surface);
                    let kb = self.keyboard.clone();
                    kb.set_focus(self, new_focus, serial);
                }
            }
        }

        let pointer = self.pointer.clone();
        // INSTR.F2: log focus state ANTES de pointer.button (synthetic).
        {
            let cf = pointer.current_focus();
            let su = self.surface_under(self.pointer_location);
            tracing::trace!(
                button = format!("0x{:x}", button), pressed,
                ploc = ?(self.pointer_location.x as i32, self.pointer_location.y as i32),
                current_focus_some = cf.is_some(),
                surface_under_some = su.is_some(),
                "INSTR.F2 synth_button pre-button"
            );
        }
        pointer.button(
            self,
            &smithay::input::pointer::ButtonEvent {
                button,
                state,
                serial,
                time,
            },
        );
        pointer.frame(self);
        // INSTR.F3: log focus apos button+frame (synthetic).
        {
            let cf = pointer.current_focus();
            tracing::trace!(
                button = format!("0x{:x}", button),
                pressed,
                current_focus_some_after = cf.is_some(),
                "INSTR.F3 synth_button post-frame"
            );
        }
        #[cfg(feature = "drm-backend")]
        {
            self.drm_force_repaint = true;
        }
        tracing::debug!(
            button = format!("0x{:x}", button),
            pressed,
            "SI.1: SyntheticPointerButton"
        );
    }

    /// SI.1: Injeta scroll. dx horizontal, dy vertical.
    /// Conhecido: emitido como Continuous source -- nao gera passos discretos.
    pub fn handle_synthetic_pointer_scroll(&mut self, dx: f64, dy: f64) {
        use smithay::backend::input::{Axis, AxisSource};
        use smithay::input::pointer::AxisFrame;
        let time = self.clock.now().as_millis();
        let mut frame = AxisFrame::new(time).source(AxisSource::Continuous);
        if dy != 0.0 {
            frame = frame.value(Axis::Vertical, dy);
        }
        if dx != 0.0 {
            frame = frame.value(Axis::Horizontal, dx);
        }
        let pointer = self.pointer.clone();
        pointer.axis(self, frame);
        pointer.frame(self);
        #[cfg(feature = "drm-backend")]
        {
            self.drm_force_repaint = true;
        }
        tracing::debug!(dx, dy, "SI.1: SyntheticPointerScroll");
    }

    /// SI.1: Injeta press/release de tecla individual.
    /// `keycode` = evdev KEY_* (1..=255 tipicamente).
    pub fn handle_synthetic_key(&mut self, keycode: u32, pressed: bool) {
        use smithay::backend::input::KeyState;
        use smithay::input::keyboard::{FilterResult, Keycode};
        let serial = SERIAL_COUNTER.next_serial();
        let time = self.clock.now().as_millis();
        let state = if pressed {
            KeyState::Pressed
        } else {
            KeyState::Released
        };
        // Smithay/xkbcommon usa keycode = evdev + 8.
        let xkb_code = Keycode::new(keycode.saturating_add(8));
        let keyboard = self.keyboard.clone();
        keyboard.input::<(), _>(self, xkb_code, state, serial, time, |_, _, _| {
            FilterResult::Forward
        });
        tracing::debug!(keycode, pressed, "SI.1: SyntheticKey");
    }

    /// SI.1: Atalho. Press todas em ordem -> pausa curta -> release reverse.
    pub fn handle_synthetic_key_combo(&mut self, keys: &[u32]) {
        if keys.is_empty() {
            return;
        }
        for k in keys {
            self.handle_synthetic_key(*k, true);
        }
        // 10ms entre press-all e release-all. Mesmo tick do calloop;
        // clientes wayland recebem todos os press antes de qualquer release.
        std::thread::sleep(std::time::Duration::from_millis(10));
        for k in keys.iter().rev() {
            self.handle_synthetic_key(*k, false);
        }
        tracing::trace!(count = keys.len(), "SI.1: SyntheticKeyCombo dispatched");
    }
}

/// C2 (review): resolve `cmd` pra path absoluto pra evitar PATH hijack.
/// Procura em ordem: /usr/bin, /usr/local/bin, $HOME/.local/bin.
/// Retorna PathBuf. Se nada existe, retorna PathBuf::from(cmd) (PATH lookup
/// fallback do Command::new).
pub(crate) fn resolve_command_path(cmd: &str, home: &str) -> std::path::PathBuf {
    use std::path::PathBuf;
    // Se ja absoluto (contem '/' como prefix), respeita.
    if cmd.starts_with('/') {
        return PathBuf::from(cmd);
    }
    let candidates = [
        PathBuf::from("/usr/bin").join(cmd),
        PathBuf::from("/usr/local/bin").join(cmd),
        PathBuf::from(format!("{home}/.local/bin")).join(cmd),
    ];
    for c in &candidates {
        if c.is_file() {
            return c.clone();
        }
    }
    PathBuf::from(cmd)
}

#[cfg(test)]
mod spawn_tests {
    use super::resolve_command_path;
    use std::path::PathBuf;

    #[test]
    fn absolute_path_passthrough() {
        let p = resolve_command_path("/opt/bin/x", "/home/u");
        assert_eq!(p, PathBuf::from("/opt/bin/x"));
    }

    #[test]
    fn nonexistent_command_falls_back_to_name() {
        // Inputs improvavel: nada vai existir em /usr/bin/zzzz-nope etc.
        let p = resolve_command_path("zzzz-definitivamente-nao-existe", "/home/none");
        assert_eq!(p, PathBuf::from("zzzz-definitivamente-nao-existe"));
    }

    #[test]
    fn resolve_prefers_first_existing_path() {
        // Cria binario stub em dir tmp pra emular ~/.local/bin.
        let dir = std::env::temp_dir().join(format!(
            "lumo-spawn-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let local_bin = dir.join(".local/bin");
        std::fs::create_dir_all(&local_bin).unwrap();
        let target = local_bin.join("fake-bin");
        std::fs::write(&target, b"#!/bin/sh\n").unwrap();
        let resolved = resolve_command_path("fake-bin", dir.to_str().unwrap());
        // Como nao existe em /usr/bin nem /usr/local/bin, vai cair em ~/.local/bin.
        assert_eq!(resolved, target);
        std::fs::remove_dir_all(&dir).ok();
    }
}
