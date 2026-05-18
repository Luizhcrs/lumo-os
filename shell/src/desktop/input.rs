//! desktop/input.rs - PointerHandler impl pro LumoDesktop.
//!
//! Right-click abre menu contextual; left-click em area vazia despacha
//! CloseDropdowns IPC; left-click no menu = ativa item + fecha.

use std::time::{Duration, Instant};

use smithay_client_toolkit::{
    reexports::client::{protocol::wl_pointer, Connection, QueueHandle},
    seat::pointer::{PointerEvent, PointerEventKind, PointerHandler, BTN_LEFT, BTN_RIGHT},
};

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
                    if self.menu.visible {
                        let new_idx = self
                            .hit_test_menu(ev.position.0 as f32, ev.position.1 as f32)
                            .unwrap_or(usize::MAX);
                        if new_idx != self.menu.hover_idx {
                            self.menu.hover_idx = new_idx;
                            need_redraw = true;
                        }
                    }
                }
                PointerEventKind::Leave { .. } => {
                    self.pointer_pos = None;
                    if self.menu.visible && self.menu.hover_idx != usize::MAX {
                        self.menu.hover_idx = usize::MAX;
                        need_redraw = true;
                    }
                }
                PointerEventKind::Press { button, .. } => {
                    let (px, py) = (ev.position.0 as f32, ev.position.1 as f32);
                    let now = Instant::now();
                    if let Some(last) = self.last_click_at {
                        if now.duration_since(last) < Duration::from_millis(150) {
                            continue;
                        }
                    }
                    self.last_click_at = Some(now);

                    if button == BTN_RIGHT {
                        self.menu = MenuActive {
                            visible: true,
                            x: px,
                            y: py,
                            hover_idx: usize::MAX,
                        };
                        need_redraw = true;
                        eprintln!("[lumo-desktop] right-click ({}, {}) -> menu open", px, py);
                    } else if button == BTN_LEFT {
                        if self.menu.visible {
                            if let Some(idx) = self.hit_test_menu(px, py) {
                                handle_item(idx);
                            }
                            self.menu.visible = false;
                            self.menu.hover_idx = usize::MAX;
                            need_redraw = true;
                        } else {
                            send_close_dropdowns(&mut self.ipc_stream);
                            eprintln!("[lumo-desktop] left-click empty -> CloseDropdowns IPC");
                        }
                    }
                }
                _ => {}
            }
        }
        if need_redraw {
            self.redraw(qh);
        }
    }
}
