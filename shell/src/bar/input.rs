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
                    // A20.10: debounce 200ms (re-size surface multipla = bug visual)
                    let now = Instant::now();
                    if let Some(last) = self.last_click_at {
                        if now.duration_since(last) < Duration::from_millis(200) {
                            eprintln!("[lumo-bar] click debounced");
                            continue;
                        }
                    }
                    self.last_click_at = Some(now);
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
                                self.dropdown = if self.dropdown == DropdownActive::Battery {
                                    DropdownActive::None
                                } else {
                                    self.refresh();
                                    // A26: mutex - abriu dropdown bar -> fecha menu desktop.
                                    self.send_ipc_close_desktop_menu();
                                    DropdownActive::Battery
                                };
                                self.update_size_and_redraw(qh);
                                handled = true;
                            }
                        }
                    }
                    if !handled {
                        if let Some((rx, ry, rw, rh)) = self.wifi_hit_rect {
                            if px >= rx && px <= rx + rw && py >= ry && py <= ry + rh {
                                self.dropdown = if self.dropdown == DropdownActive::Wifi {
                                    DropdownActive::None
                                } else {
                                    self.refresh();
                                    self.send_ipc_close_desktop_menu();
                                    DropdownActive::Wifi
                                };
                                self.update_size_and_redraw(qh);
                                handled = true;
                            }
                        }
                    }
                    if !handled {
                        if let Some((rx, ry, rw, rh)) = self.datetime_hit_rect {
                            if px >= rx && px <= rx + rw && py >= ry && py <= ry + rh {
                                self.dropdown = if self.dropdown == DropdownActive::DateTime {
                                    DropdownActive::None
                                } else {
                                    // A26: ao abrir, sincroniza viewed_* com today (pode estar stale).
                                    let now_local = Local::now();
                                    self.viewed_year = now_local.year();
                                    self.viewed_month = now_local.month();
                                    self.selected_day = None;
                                    self.send_ipc_close_desktop_menu();
                                    DropdownActive::DateTime
                                };
                                self.update_size_and_redraw(qh);
                                handled = true;
                            }
                        }
                    }
                    // A27: click no brand "Lumo" pill esquerda -> toggle menu Lumo.
                    if !handled {
                        if let Some((rx, ry, rw, rh)) = self.lumo_hit_rect {
                            if px >= rx && px <= rx + rw && py >= ry && py <= ry + rh {
                                self.dropdown = if self.dropdown == DropdownActive::LumoMenu {
                                    DropdownActive::None
                                } else {
                                    self.lumo_menu_hover_idx = usize::MAX;
                                    self.send_ipc_close_desktop_menu(); // A26 mutex
                                    DropdownActive::LumoMenu
                                };
                                self.update_size_and_redraw(qh);
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
                    if !handled && self.dropdown != DropdownActive::None {
                        self.dropdown = DropdownActive::None;
                        self.lumo_menu_hover_idx = usize::MAX; // A27
                        self.update_size_and_redraw(qh);
                    }
                }
                _ => {}
            }
        }
    }
}
