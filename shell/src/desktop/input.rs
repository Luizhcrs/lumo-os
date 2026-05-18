//! desktop/input.rs - PointerHandler impl pro LumoDesktop.
//! A33+A34 integrated.

use std::time::{Duration, Instant};

use smithay_client_toolkit::{
    reexports::client::{protocol::wl_pointer, Connection, QueueHandle},
    seat::pointer::{PointerEvent, PointerEventKind, PointerHandler, BTN_LEFT, BTN_RIGHT},
};

use crate::desktop::icons::{ctx_menu_hit, DBLCLICK_MS};
use crate::desktop::menu_overlay::handle_item;
use crate::desktop::state::{send_close_dropdowns, LumoDesktop, MenuActive};

impl PointerHandler for LumoDesktop {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        let mut need_redraw = false;
        for ev in events {
            match ev.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    self.pointer_pos = Some(ev.position);
                    let (mx, my) = (ev.position.0 as f32, ev.position.1 as f32);
                    if self.menu.visible {
                        let new_idx = self.hit_test_menu(mx, my).unwrap_or(usize::MAX);
                        if new_idx != self.menu.hover_idx {
                            self.menu.hover_idx = new_idx;
                            need_redraw = true;
                        }
                    }
                    if let Some((_, cx, cy)) = self.icons.ctx_menu {
                        let new_idx = ctx_menu_hit(mx, my, cx, cy).unwrap_or(usize::MAX);
                        if new_idx != self.icons.ctx_hover {
                            self.icons.ctx_hover = new_idx;
                            need_redraw = true;
                        }
                    }
                    if self.icons.drag.is_some() {
                        if self.icons.motion_drag(mx, my) {
                            need_redraw = true;
                        }
                    } else if self.rubber_band.active {
                        self.rubber_band.update(mx, my);
                        if let Some((rx, ry, rw, rh)) = self.rubber_band.normalized_rect() {
                            self.icons.select_by_rect(rx, ry, rw, rh);
                        }
                        need_redraw = true;
                    }
                }
                PointerEventKind::Leave { .. } => {
                    self.pointer_pos = None;
                    if self.menu.visible && self.menu.hover_idx != usize::MAX {
                        self.menu.hover_idx = usize::MAX;
                        need_redraw = true;
                    }
                    if self.icons.ctx_hover != usize::MAX {
                        self.icons.ctx_hover = usize::MAX;
                        need_redraw = true;
                    }
                }
                PointerEventKind::Press { button, .. } => {
                    let (px, py) = (ev.position.0 as f32, ev.position.1 as f32);
                    let now = Instant::now();
                    if let Some(last) = self.last_click_at {
                        if now.duration_since(last) < Duration::from_millis(30) {
                            continue;
                        }
                    }
                    self.last_click_at = Some(now);
                    if button == BTN_RIGHT {
                        if self.menu.visible {
                            self.menu.visible = false;
                            need_redraw = true;
                        }
                        if self.icons.ctx_menu.is_some() {
                            self.icons.ctx_menu = None;
                            need_redraw = true;
                        }
                        if let Some(idx) = self.icons.hit(px, py) {
                            self.icons.ctx_menu = Some((idx, px, py));
                            self.icons.ctx_hover = usize::MAX;
                            need_redraw = true;
                        } else {
                            send_close_dropdowns(&mut self.ipc_stream);
                            self.menu = MenuActive { visible: true, x: px, y: py, hover_idx: usize::MAX };
                            need_redraw = true;
                        }
                    } else if button == BTN_LEFT {
                        let mut closed_ctx = false;
                        if let Some((_, cx, cy)) = self.icons.ctx_menu {
                            if let Some(item_idx) = ctx_menu_hit(px, py, cx, cy) {
                                let icon_idx = self.icons.ctx_menu.unwrap().0;
                                handle_ctx_item(&mut self.icons, icon_idx, item_idx);
                                self.icons.ctx_menu = None;
                                self.icons.scan();
                                need_redraw = true;
                                continue;
                            } else {
                                self.icons.ctx_menu = None;
                                closed_ctx = true;
                                need_redraw = true;
                            }
                        }
                        if self.menu.visible {
                            if let Some(idx) = self.hit_test_menu(px, py) {
                                if handle_item(idx) {
                                    self.icons.create_folder();
                                }
                            }
                            self.menu.visible = false;
                            self.menu.hover_idx = usize::MAX;
                            need_redraw = true;
                            continue;
                        }
                        if let Some(idx) = self.icons.hit(px, py) {
                            self.icons.clear_selection();
                            self.icons.icons[idx].selected = true;
                            need_redraw = true;
                            let is_dbl = if let Some((last_idx, last_t)) = self.icons.last_click {
                                last_idx == idx && now.duration_since(last_t).as_millis() < DBLCLICK_MS
                            } else { false };
                            if is_dbl {
                                self.icons.open_icon(idx);
                                self.icons.last_click = None;
                            } else {
                                self.icons.last_click = Some((idx, now));
                                self.icons.press_icon(idx, px, py);
                            }
                        } else if !closed_ctx {
                            self.icons.clear_selection();
                            self.rubber_band.cancel();
                            send_close_dropdowns(&mut self.ipc_stream);
                            need_redraw = true;
                            self.rubber_band.start(px, py);
                        }
                    }
                }
                PointerEventKind::Release { button, .. } => {
                    if button == BTN_LEFT {
                        let dragged = self.icons.release_drag();
                        if dragged { need_redraw = true; }
                        if self.rubber_band.active {
                            if let Some((rx, ry, rw, rh)) = self.rubber_band.finish() {
                                self.icons.select_by_rect(rx, ry, rw, rh);
                            } else {
                                self.rubber_band.cancel();
                            }
                            need_redraw = true;
                        }
                    }
                }
                _ => {}
            }
        }
        if need_redraw { self.redraw(qh); }
    }
}

fn handle_ctx_item(icons: &mut crate::desktop::icons::IconsState, icon_idx: usize, item_idx: usize) {
    match item_idx {
        0 => { icons.open_icon(icon_idx); }
        1 => {
            if let Some(ic) = icons.icons.get(icon_idx) {
                eprintln!("[lumo-desktop] renomear stub: {}", ic.name);
            }
        }
        2 => {
            if let Some(ic) = icons.icons.get(icon_idx) {
                eprintln!("[lumo-desktop] lixeira stub: {}", ic.name);
            }
        }
        _ => {}
    }
}
