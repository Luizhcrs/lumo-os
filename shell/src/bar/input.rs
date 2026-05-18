//! bar/input.rs - PointerHandler impl pro LumoBar.
//!
//! Roteia eventos Enter/Motion/Leave/Press pra:
//!   - hover tracking no menu Lumo (idx)
//!   - debounce de click 200ms (A20.10)
//!   - hit-tests cascade: cal nav -> bat -> wifi -> datetime -> lumo
//!   - right-click = fecha tudo + broadcast IPC CloseDropdowns
//!   - click fora = fecha dropdown ativo

use std::time::{Duration, Instant};

use chrono::{Datelike, Local};
use smithay_client_toolkit::{
    reexports::client::{protocol::wl_pointer, Connection, QueueHandle},
    seat::pointer::{PointerEvent, PointerEventKind, PointerHandler, BTN_LEFT, BTN_RIGHT},
};

use crate::bar::dropdowns::DropdownActive;
use crate::bar::state::LumoBar;
use crate::bar::tokens::MENU_LUMO_ITEMS;

impl PointerHandler for LumoBar {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        eprintln!("[lumo-bar] pointer_frame {} events", events.len());
        for ev in events {
            match ev.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    self.pointer_x = ev.position.0 as f32;
                    self.pointer_pos = Some(ev.position);
                    // Q4: drag brilho ativo — ajusta pct proporcional ao delta Y.
                    if self.brightness_dragging {
                        let py_now = ev.position.1 as f32;
                        let dy = self.brightness_drag_last_y - py_now; // arrasto pra cima = mais brilho
                        self.brightness_drag_last_y = py_now;
                        if dy.abs() >= 1.0 {
                            let delta = (dy * 0.5).round() as i16;
                            let new_pct = (self.brightness_info.pct as i16 + delta).clamp(5, 100) as u8;
                            if new_pct != self.brightness_info.pct {
                                crate::bar::system_info::set_brightness_pct(new_pct);
                                self.brightness_info.pct = new_pct;
                                self.update_size_and_redraw(qh);
                            }
                        }
                    }
                    // A27: hover tracking dentro do menu Lumo aberto.
                    if self.dropdown == DropdownActive::LumoMenu {
                        let new_idx = self.lumo_menu_hit_test(
                            ev.position.0 as f32,
                            ev.position.1 as f32,
                        ).unwrap_or(usize::MAX);
                        if new_idx != self.lumo_menu_hover_idx {
                            self.lumo_menu_hover_idx = new_idx;
                            self.update_size_and_redraw(qh);
                        }
                    }
                }
                PointerEventKind::Leave { .. } => {
                    self.pointer_pos = None;
                    if self.dropdown == DropdownActive::LumoMenu
                        && self.lumo_menu_hover_idx != usize::MAX
                    {
                        self.lumo_menu_hover_idx = usize::MAX;
                        self.update_size_and_redraw(qh);
                    }
                }
                PointerEventKind::Press { button, serial, time } => {
                    eprintln!("[lumo-bar] Press button={} serial={} time={} pos={:?} bat_rect={:?} wifi_rect={:?}", button, serial, time, ev.position, self.bat_hit_rect, self.wifi_hit_rect);

                    // A26: right-click em qualquer lugar da bar = fecha tudo
                    // (proprio dropdown + broadcast CloseDropdowns pra outros clients).
                    if button == BTN_RIGHT {
                        let need_redraw = self.dropdown != DropdownActive::None;
                        self.dropdown = DropdownActive::None;
                        self.send_ipc_close_dropdowns();
                        if need_redraw {
                            self.update_size_and_redraw(qh);
                        }
                        continue;
                    }

                    if button != BTN_LEFT { continue; }
                    // A20.10: debounce 200ms (re-size surface multipla = bug visual).
                    // C3 fix: skip debounce quando dropdown ja aberto -- clicks internos
                    // (rede wifi, dia calendario) nao mudam tamanho da surface.
                    let now = Instant::now();
                    let dropdown_open = self.dropdown != DropdownActive::None;
                    if !dropdown_open {
                        if let Some(last) = self.last_click_at {
                            if now.duration_since(last) < Duration::from_millis(200) {
                                eprintln!("[lumo-bar] click debounced");
                                continue;
                            }
                        }
                        self.last_click_at = Some(now);
                    }
                    let (px, py) = (ev.position.0 as f32, ev.position.1 as f32);
                    let mut handled = false;

                    // A26: PRIMEIRO testa controles internos do calendar quando aberto.
                    if !handled && self.dropdown == DropdownActive::DateTime {
                        // prev
                        if let Some((rx, ry, rw, rh)) = self.cal_prev_rect {
                            if px >= rx && px <= rx + rw && py >= ry && py <= ry + rh {
                                if self.viewed_month == 1 {
                                    self.viewed_month = 12;
                                    self.viewed_year -= 1;
                                } else {
                                    self.viewed_month -= 1;
                                }
                                // Reset selected pra evitar highlight em dia inexistente.
                                self.selected_day = None;
                                self.update_size_and_redraw(qh);
                                handled = true;
                            }
                        }
                        // next
                        if !handled {
                            if let Some((rx, ry, rw, rh)) = self.cal_next_rect {
                                if px >= rx && px <= rx + rw && py >= ry && py <= ry + rh {
                                    if self.viewed_month == 12 {
                                        self.viewed_month = 1;
                                        self.viewed_year += 1;
                                    } else {
                                        self.viewed_month += 1;
                                    }
                                    self.selected_day = None;
                                    self.update_size_and_redraw(qh);
                                    handled = true;
                                }
                            }
                        }
                        // today (reset)
                        if !handled {
                            if let Some((rx, ry, rw, rh)) = self.cal_today_rect {
                                if px >= rx && px <= rx + rw && py >= ry && py <= ry + rh {
                                    let now_local = Local::now();
                                    self.viewed_year = now_local.year();
                                    self.viewed_month = now_local.month();
                                    self.selected_day = Some(now_local.day());
                                    self.update_size_and_redraw(qh);
                                    handled = true;
                                }
                            }
                        }
                        // day cells
                        if !handled {
                            for (day, (rx, ry, rw, rh)) in &self.cal_day_rects {
                                if px >= *rx && px <= *rx + *rw && py >= *ry && py <= *ry + *rh {
                                    self.selected_day = Some(*day);
                                    self.update_size_and_redraw(qh);
                                    handled = true;
                                    break;
                                }
                            }
                        }
                    }

                    if !handled {
                        if let Some((rx, ry, rw, rh)) = self.bat_hit_rect {
                            if px >= rx && px <= rx + rw && py >= ry && py <= ry + rh {
                                if self.dropdown == DropdownActive::Battery {
                                    // B4: fechar com animacao.
                                    self.start_close_anim(DropdownActive::Battery);
                                } else {
                                    self.refresh();
                                    self.send_ipc_close_desktop_menu();
                                    self.dropdown = DropdownActive::Battery;
                                    self.start_open_anim();
                                }
                                self.update_size_and_redraw(qh);
                                handled = true;
                            }
                        }
                    }
                    if !handled {
                        if let Some((rx, ry, rw, rh)) = self.wifi_hit_rect {
                            if px >= rx && px <= rx + rw && py >= ry && py <= ry + rh {
                                if self.dropdown == DropdownActive::Wifi {
                                    self.start_close_anim(DropdownActive::Wifi);
                                } else {
                                    self.refresh();
                                    self.send_ipc_close_desktop_menu();
                                    self.dropdown = DropdownActive::Wifi;
                                    self.start_open_anim();
                                }
                                self.update_size_and_redraw(qh);
                                handled = true;
                            }
                        }
                    }
                    if !handled {
                        // L5: brilho pill -> abre dropdown Brightness.
                        if let Some((rx, ry, rw, rh)) = self.brightness_hit_rect {
                            if px >= rx && px <= rx + rw && py >= ry && py <= ry + rh {
                                // Q4: iniciar drag brilho.
                                self.brightness_dragging = true;
                                self.brightness_drag_last_y = py;
                                if self.dropdown == DropdownActive::Brightness {
                                    self.start_close_anim(DropdownActive::Brightness);
                                } else {
                                    self.send_ipc_close_desktop_menu();
                                    self.dropdown = DropdownActive::Brightness;
                                    self.start_open_anim();
                                }
                                self.update_size_and_redraw(qh);
                                handled = true;
                            }
                        }
                    }
                    if !handled {
                        if let Some((rx, ry, rw, rh)) = self.datetime_hit_rect {
                            if px >= rx && px <= rx + rw && py >= ry && py <= ry + rh {
                                if self.dropdown == DropdownActive::DateTime {
                                    self.start_close_anim(DropdownActive::DateTime);
                                } else {
                                    let now_local = Local::now();
                                    self.viewed_year = now_local.year();
                                    self.viewed_month = now_local.month();
                                    self.selected_day = None;
                                    self.send_ipc_close_desktop_menu();
                                    self.dropdown = DropdownActive::DateTime;
                                    self.start_open_anim();
                                }
                                self.update_size_and_redraw(qh);
                                handled = true;
                            }
                        }
                    }
                    // A27: click no brand "Lumo" pill esquerda -> toggle menu Lumo.
                    if !handled {
                        if let Some((rx, ry, rw, rh)) = self.lumo_hit_rect {
                            if px >= rx && px <= rx + rw && py >= ry && py <= ry + rh {
                                if self.dropdown == DropdownActive::LumoMenu {
                                    self.start_close_anim(DropdownActive::LumoMenu);
                                } else {
                                    self.lumo_menu_hover_idx = usize::MAX;
                                    self.send_ipc_close_desktop_menu();
                                    self.dropdown = DropdownActive::LumoMenu;
                                    self.start_open_anim();
                                }
                                self.update_size_and_redraw(qh);
                                handled = true;
                            }
                        }
                    }
                    // L5: click em dropdown brilho aberto -> slider ou preset.
                    if !handled && self.dropdown == DropdownActive::Brightness {
                        // Preset Dia 80%.
                        if let Some((rx, ry, rw, rh)) = self.brightness_preset_day_rect {
                            if px >= rx && px <= rx + rw && py >= ry && py <= ry + rh {
                                eprintln!("[lumo-bar] L5 brightness preset Dia 80%");
                                crate::bar::system_info::set_brightness_pct(80);
                                self.brightness_info.pct = 80;
                                self.update_size_and_redraw(qh);
                                handled = true;
                            }
                        }
                        if !handled {
                            if let Some((rx, ry, rw, rh)) = self.brightness_preset_night_rect {
                                if px >= rx && px <= rx + rw && py >= ry && py <= ry + rh {
                                    eprintln!("[lumo-bar] L5 brightness preset Noite 35%");
                                    crate::bar::system_info::set_brightness_pct(35);
                                    self.brightness_info.pct = 35;
                                    self.update_size_and_redraw(qh);
                                    handled = true;
                                }
                            }
                        }
                        // Slider: click sets pct from x position.
                        if !handled {
                            if let Some((rx, ry, rw, rh)) = self.brightness_slider_rect {
                                if px >= rx && px <= rx + rw && py >= ry && py <= ry + rh {
                                    let rel = ((px - rx) / rw).clamp(0.0, 1.0);
                                    let new_pct = (rel * 100.0).round() as u8;
                                    eprintln!("[lumo-bar] L5 brightness slider -> {}%", new_pct);
                                    crate::bar::system_info::set_brightness_pct(new_pct);
                                    self.brightness_info.pct = new_pct;
                                    self.update_size_and_redraw(qh);
                                    handled = true;
                                }
                            }
                        }
                    }
                    // L5: click em dropdown bateria aberto -> charge_limit toggle / profile cycle.
                    if !handled && self.dropdown == DropdownActive::Battery {
                        if let Some((rx, ry, rw, rh)) = self.bat_charge_limit_toggle_rect {
                            if px >= rx && px <= rx + rw && py >= ry && py <= ry + rh {
                                let current_limit = self.battery_info.charge_limit.unwrap_or(100);
                                let new_limit: u8 = if current_limit <= 80 { 100 } else { 80 };
                                eprintln!("[lumo-bar] L5 charge limit -> {}", new_limit);
                                let path = std::path::PathBuf::from(
                                    "/sys/class/power_supply/BAT1/charge_control_end_threshold");
                                let _ = std::fs::write(&path, new_limit.to_string());
                                self.battery_info.charge_limit = Some(new_limit);
                                self.update_size_and_redraw(qh);
                                handled = true;
                            }
                        }
                        if !handled {
                            if let Some((rx, ry, rw, rh)) = self.bat_profile_cycle_rect {
                                if px >= rx && px <= rx + rw && py >= ry && py <= ry + rh {
                                    let next = crate::bar::system_info::platform_profile_cycle_next();
                                    eprintln!("[lumo-bar] L5 profile cycle -> {:?}", next);
                                    if let Some(p) = next {
                                        self.battery_info.platform_profile = Some(p);
                                    }
                                    self.update_size_and_redraw(qh);
                                    handled = true;
                                }
                            }
                        }
                    }
                    // A31.2: click em dropdown wifi aberto -> toggle/connect/disconnect.
                    if !handled && self.dropdown == DropdownActive::Wifi {
                        // Toggle pill: liga/desliga radio wifi.
                        if let Some((rx, ry, rw, rh)) = self.wifi_toggle_rect {
                            if px >= rx && px <= rx + rw && py >= ry && py <= ry + rh {
                                let want_on = !self.wifi_info.up;
                                eprintln!("[lumo-bar] A31.2 toggle wifi -> {}", want_on);
                                crate::bar::system_info::nm_set_radio(want_on);
                                // Bug Luiz v4: optimistic update removido. UI atualiza pos nmcli confirmar.
                                self.wifi_refresh_due = Some(Instant::now() + Duration::from_millis(1500));
                                self.update_size_and_redraw(qh);
                                handled = true;
                            }
                        }
                        // Click linha rede atual -> disconnect.
                        if !handled {
                            if let Some((rx, ry, rw, rh)) = self.wifi_disconnect_rect {
                                if px >= rx && px <= rx + rw && py >= ry && py <= ry + rh {
                                    if let Some(iface) = self.wifi_info.iface.clone() {
                                        eprintln!("[lumo-bar] A31.2 disconnect iface={}", iface);
                                        crate::bar::system_info::nm_disconnect_iface(iface);
                                        self.wifi_refresh_due = Some(Instant::now() + Duration::from_millis(1500));
                                    }
                                    handled = true;
                                }
                            }
                        }
                        // Click linha rede outras -> connect.
                        if !handled {
                            let hit_ssid = self.wifi_connect_rects.iter().find_map(|(ssid, (rx, ry, rw, rh))| {
                                if px >= *rx && px <= rx + rw && py >= *ry && py <= ry + rh {
                                    Some(ssid.clone())
                                } else {
                                    None
                                }
                            });
                            if let Some(ssid) = hit_ssid {
                                eprintln!("[lumo-bar] A31.2 connect ssid={}", ssid);
                                crate::bar::system_info::nm_connect(ssid);
                                self.wifi_refresh_due = Some(Instant::now() + Duration::from_millis(2500));
                                handled = true;
                            }
                        }
                    }
                    // A27: click em item do menu Lumo aberto -> log stub + fecha.
                    if !handled && self.dropdown == DropdownActive::LumoMenu {
                        if let Some(idx) = self.lumo_menu_hit_test(px, py) {
                            eprintln!(
                                "[lumo-bar] menu Lumo item: '{}' (stub)",
                                MENU_LUMO_ITEMS[idx].label
                            );
                            self.dropdown = DropdownActive::None;
                            self.lumo_menu_hover_idx = usize::MAX;
                            self.update_size_and_redraw(qh);
                            handled = true;
                        }
                    }
                    // C5: click em pill appmenu top-level -> abre submenu.
                    if !handled {
                        let hit = self.appmenu_pill_rects.iter().find_map(|(idx, (rx, ry, rw, rh))| {
                            if px >= *rx && px <= rx + rw && py >= *ry && py <= ry + rh {
                                Some(*idx)
                            } else {
                                None
                            }
                        });
                        if let Some(idx) = hit {
                            if self.appmenu_open_idx == Some(idx) {
                                // Clicou no mesmo -> fecha.
                                self.appmenu_open_idx = None;
                                self.appmenu_submenu.clear();
                            } else {
                                // Busca submenu do item.
                                let submenu = self.appmenu.fetch_submenu(
                                    self.appmenu.items.get(idx).map(|it| it.id).unwrap_or(0)
                                );
                                self.appmenu_open_idx = Some(idx);
                                self.appmenu_submenu = submenu;
                            }
                            self.update_size_and_redraw(qh);
                            handled = true;
                        }
                    }
                    // C5: click em subitem do submenu appmenu aberto -> activate + fecha.
                    if !handled && self.appmenu_open_idx.is_some() {
                        let hit = self.appmenu_submenu_rects.iter().find_map(|(sidx, (rx, ry, rw, rh))| {
                            if px >= *rx && px <= rx + rw && py >= *ry && py <= ry + rh {
                                Some(*sidx)
                            } else {
                                None
                            }
                        });
                        if let Some(sidx) = hit {
                            if let Some(item) = self.appmenu_submenu.get(sidx) {
                                self.appmenu.activate(item.id);
                            }
                            self.appmenu_open_idx = None;
                            self.appmenu_submenu.clear();
                            self.update_size_and_redraw(qh);
                            handled = true;
                        }
                    }

                    if !handled && self.dropdown != DropdownActive::None {
                        self.dropdown = DropdownActive::None;
                        self.lumo_menu_hover_idx = usize::MAX; // A27
                        self.update_size_and_redraw(qh);
                    }
                }
                // N2: 2-finger scroll vertical sobre brightness pill -> ajusta brilho.
                PointerEventKind::Axis { vertical, .. } => {
                    if vertical.absolute.abs() > 0.0 || vertical.discrete != 0 {
                        if let Some(pos) = self.pointer_pos {
                            let px = pos.0 as f32;
                            let py = pos.1 as f32;
                            if let Some((rx, ry, rw, rh)) = self.brightness_hit_rect {
                                if px >= rx && px <= rx + rw && py >= ry && py <= ry + rh {
                                    // vertical.absolute > 0 = scroll down = menos brilho.
                                    let delta_pct: i16 = if vertical.absolute != 0.0 {
                                        let steps = (vertical.absolute / 15.0).round() as i16;
                                        -steps * 5
                                    } else {
                                        (-vertical.discrete as i16) * 5
                                    };
                                    let new_pct = (self.brightness_info.pct as i16 + delta_pct)
                                        .clamp(5, 100) as u8;
                                    if new_pct != self.brightness_info.pct {
                                        eprintln!("[lumo-bar] N2 scroll brilho {} -> {}", self.brightness_info.pct, new_pct);
                                        crate::bar::system_info::set_brightness_pct(new_pct);
                                        self.brightness_info.pct = new_pct;
                                        self.update_size_and_redraw(qh);
                                    }
                                }
                            }
                        }
                    }
                }
                PointerEventKind::Release { .. } => {
                    // Q4: encerra drag brilho no release.
                    if self.brightness_dragging {
                        self.brightness_dragging = false;
                    }
                }
                _ => {}
            }
        }
    }
}
