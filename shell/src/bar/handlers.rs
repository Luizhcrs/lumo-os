//! bar/handlers.rs - Implementacao dos handlers Wayland (Compositor, Output,
//! Shm, Seat, Registry, LayerSurface) + helpers LumoBar (refresh, redraw,
//! update_size_and_redraw, computed_height).
//!
//! Os handlers sao delegados em main_loop.rs via `delegate_*!` macros.

use std::sync::atomic::Ordering;

use chrono::{Local, Timelike};
use smithay_client_toolkit::reexports::client::{
    protocol::{wl_output, wl_seat, wl_shm, wl_surface},
    Connection, QueueHandle,
};
use smithay_client_toolkit::{
    compositor::CompositorHandler,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{keyboard::KeyboardData, pointer::ThemeSpec, Capability, SeatHandler, SeatState},
    shell::wlr_layer::{LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
    shell::WaylandSurface,
    shm::{Shm, ShmHandler},
};
use tiny_skia::Pixmap;

use crate::bar::dropdowns::DropdownActive;
use crate::bar::state::{paint_frame, BarSnapshot, LumoBar};
use crate::bar::system_info::{
    format_date_pt, read_battery_info, read_brightness_info, read_datetime_info, read_wifi,
    read_wifi_info,
};
use crate::bar::tokens::*;
use crate::menu;
use lumo_animation::{AnimCurve, LAAnimator, LACurve};

impl LumoBar {
    pub fn refresh(&mut self) {
        // A20: leitura completa /sys/class/power_supply.
        self.battery_info = read_battery_info();
        self.battery_pct = self.battery_info.pct;
        self.brightness_info = read_brightness_info();
        self.wifi_on = read_wifi();
        // A23: leitura wifi via iw + ip.
        self.wifi_info = read_wifi_info();
    }

    /// B4: inicia animacao de abertura do dropdown (scale 0.85->1.0, alpha 0->1).
    pub fn start_open_anim(&mut self) {
        self.dropdown_closing = false;
        // Scale: arranca de 0.85, curva rapida ~280ms.
        self.dropdown_scale_anim = LAAnimator::new(
            0.85f32,
            1.0f32,
            AnimCurve::Bezier {
                curve: LACurve::ease_in_out(),
                duration: 0.28,
            },
        );
        // Alpha: fade-in mais curto ~220ms. from=0.05 em vez de 0.0
        // garante dropdown visivel no PRIMEIRO frame apos open (review HIGH
        // R1.fix5): pre-tick 16ms hardcoded em 60Hz era off-by-2-frames em
        // painel 120Hz Galaxy Book 4. start em 5% opaco eh imperceptivel
        // visualmente mas elimina race com main_loop.anim_block.
        self.dropdown_alpha_anim = LAAnimator::new(
            0.05f32,
            1.0f32,
            AnimCurve::Bezier {
                curve: LACurve::ease_out_cubic(),
                duration: 0.22,
            },
        );
    }

    /// B4: inicia animacao de fechamento (scale 1.0->0.85, alpha 1->0).
    /// NAO muda self.dropdown ainda — o main_loop zera quando anim termina.
    pub fn start_close_anim(&mut self, which: crate::bar::dropdowns::DropdownActive) {
        self.dropdown_closing = true;
        self.dropdown_closing_which = which;
        self.dropdown_scale_anim = LAAnimator::new(
            1.0f32,
            0.85f32,
            AnimCurve::Bezier {
                curve: LACurve::ease_out_cubic(),
                duration: 0.18,
            },
        );
        self.dropdown_alpha_anim = LAAnimator::new(
            1.0f32,
            0.0f32,
            AnimCurve::Bezier {
                curve: LACurve::ease_out_cubic(),
                duration: 0.15,
            },
        );
    }

    /// A27: hit-test absoluto sobre o menu Lumo aberto.
    pub fn lumo_menu_hit_test(&self, px: f32, py: f32) -> Option<usize> {
        let (rx, ry, _rw, rh) = self.lumo_hit_rect?;
        let mx = rx.max(PILL_MARGIN_X);
        let my = ry + rh + DROPDOWN_GAP;
        menu::hit_test(MENU_LUMO_ITEMS, mx, my, MENU_LUMO_W, px, py)
    }

    /// Altura efetiva da surface (bar + dropdown opcional).
    pub fn _computed_height(&self) -> u32 {
        let lumo_menu_h = menu::menu_height(MENU_LUMO_ITEMS) as u32; // A27
        let max_drop = DROPDOWN_H
            .max(DROPDOWN_WIFI_H) // A31: wifi maior que battery
            .max(DROPDOWN_DATETIME_H)
            .max(lumo_menu_h as f32) as u32; // A24+A27
        match self.dropdown {
            DropdownActive::None => BAR_HEIGHT,
            DropdownActive::Battery => BAR_HEIGHT + DROPDOWN_GAP as u32 + DROPDOWN_H as u32 + 8,
            DropdownActive::Wifi => BAR_HEIGHT + DROPDOWN_GAP as u32 + DROPDOWN_WIFI_H as u32 + 8,
            DropdownActive::DateTime => {
                BAR_HEIGHT + DROPDOWN_GAP as u32 + DROPDOWN_DATETIME_H as u32 + 8
            }
            DropdownActive::LumoMenu => BAR_HEIGHT + DROPDOWN_GAP as u32 + lumo_menu_h + 8,
            DropdownActive::Brightness => {
                BAR_HEIGHT + DROPDOWN_GAP as u32 + DROPDOWN_BRIGHTNESS_H as u32 + 8
            }
            DropdownActive::AppFallback => {
                // S2: dropdown fallback ~5 items * 28 + pad.
                BAR_HEIGHT + DROPDOWN_GAP as u32 + 160 + 8
            }
        }
        .max(BAR_HEIGHT + DROPDOWN_GAP as u32 + max_drop + 8)
    }

    /// Reconfigura tamanho do layer e redesenha (toggle dropdown).
    /// IMPORTANTE: exclusive_zone fixo = BAR_HEIGHT (DEPS.md A19.18).
    pub fn update_size_and_redraw(&mut self, qh: &QueueHandle<Self>) {
        // A20.11: surface SEMPRE altura max. NAO faz set_size dinamico
        // (causava flicker open/close cycle).
        self.redraw(qh);
    }

    pub fn redraw(&mut self, _qh: &QueueHandle<Self>) {
        let now = Local::now();
        let snap = BarSnapshot {
            width: self.width,
            height: self.height,
            battery_pct: self.battery_pct,
            wifi_on: self.wifi_on,
            palette: self.palette,
            theme: self.theme,
            clock_hh: now.hour() as u8,
            clock_mm: now.minute() as u8,
            active_ws: self.active_workspace.load(Ordering::Relaxed),
            date_str: format_date_pt(&now),
            dropdown: self.dropdown,
            battery_info: self.battery_info.clone(),
            wifi_info: self.wifi_info.clone(),             // A23
            brightness_info: self.brightness_info.clone(), // L5
            datetime_info: read_datetime_info(
                self.viewed_year,
                self.viewed_month,
                self.selected_day,
            ), // A24+A26
            lumo_menu_hover_idx: self.lumo_menu_hover_idx, // A27
            // C5: appmenu do app em foco.
            appmenu_items: self.appmenu.items.clone(),
            appmenu_open_idx: self.appmenu_open_idx,
            appmenu_submenu: self.appmenu_submenu.clone(),
            appmenu_submenu_hover_idx: self.appmenu_submenu_hover_idx,
            // S2: fallback app state.
            appmenu_app_id: self.appmenu.app_id.clone(),
            appmenu_title: self.appmenu.title.clone(),
            appmenu_fallback_hover_idx: self.appmenu_fallback_hover_idx,
            // B4: valores correntes do animador (tick ja foi chamado antes do redraw).
            dropdown_scale: {
                let v = self.dropdown_scale_anim.tick(0.0);
                v.clamp(0.0, 1.0)
            },
            dropdown_alpha: {
                let v = self.dropdown_alpha_anim.tick(0.0);
                v.clamp(0.0, 1.0)
            },
            // M2: alpha global da bar (F5 feedback).
            bar_alpha: {
                if self.refresh_animating {
                    let v = self.refresh_anim.tick(0.0);
                    if self.refresh_anim.is_done() {
                        self.refresh_animating = false;
                    }
                    v.clamp(0.0, 1.0)
                } else {
                    1.0
                }
            },
            // A31.3: modal de senha wifi.
            password_modal: self.password_modal.clone(),
        };

        let stride = self.width as i32 * 4;
        // A18: VOLTA pra Argb8888 (alpha real pra pills semi-translucent).
        let (buffer, canvas) = match self.pool.create_buffer(
            self.width as i32,
            self.height as i32,
            stride,
            wl_shm::Format::Argb8888,
        ) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[lumo-bar] create_buffer falhou: {e:?}");
                return;
            }
        };

        if let Some(mut px) = Pixmap::new(self.width, self.height) {
            let paint_result = paint_frame(&mut px, &snap);
            self.bat_hit_rect = paint_result.bat_hit_rect;
            self.wifi_hit_rect = paint_result.wifi_hit_rect; // A23
            self.datetime_hit_rect = paint_result.datetime_hit_rect; // A24
            self.lumo_hit_rect = paint_result.lumo_hit_rect; // A27
                                                             // A26: calendar hit-tests salvos pra pointer_frame consumir.
            self.cal_prev_rect = paint_result.cal_prev_rect;
            self.cal_next_rect = paint_result.cal_next_rect;
            self.cal_today_rect = paint_result.cal_today_rect;
            self.cal_day_rects = paint_result.cal_day_rects;
            // L5: battery dropdown interactive hit-rects.
            self.bat_charge_limit_toggle_rect = paint_result.bat_charge_limit_toggle_rect;
            self.bat_profile_cycle_rect = paint_result.bat_profile_cycle_rect;
            self.brightness_hit_rect = paint_result.brightness_hit_rect;
            self.brightness_slider_rect = paint_result.brightness_slider_rect;
            self.brightness_preset_day_rect = paint_result.brightness_preset_day_rect;
            self.brightness_preset_night_rect = paint_result.brightness_preset_night_rect;
            // A31.2: wifi hit-rects.
            self.wifi_toggle_rect = paint_result.wifi_toggle_rect;
            self.wifi_disconnect_rect = paint_result.wifi_disconnect_rect;
            self.wifi_connect_rects = paint_result.wifi_connect_rects;
            // C5: appmenu pill hit-rects.
            self.appmenu_pill_rects = paint_result.appmenu_pill_rects;
            self.appmenu_submenu_rects = paint_result.appmenu_submenu_rects;
            // S2: fallback pill hit-rects.
            self.appmenu_fallback_rect = paint_result.appmenu_fallback_rect;
            self.appmenu_fallback_dropdown_rects = paint_result.appmenu_fallback_dropdown_rects;
            // A31.3: modal senha hit-rects.
            self.pwd_confirm_rect = paint_result.pwd_confirm_rect;
            self.pwd_cancel_rect = paint_result.pwd_cancel_rect;
            let src = px.data();
            let dst = canvas;
            let n = (self.width * self.height) as usize;
            // tiny-skia Pixmap = RGBA premul. wl_shm Argb8888 LE = BGRA na
            // memoria. Swap canais; alpha preservado (premul ja correto).
            // M2: aplica bar_alpha global (F5 feedback fade).
            let bar_alpha_u8 = (snap.bar_alpha.clamp(0.0, 1.0) * 255.0) as u32;
            for i in 0..n {
                let o = i * 4;
                if o + 3 < dst.len() && o + 3 < src.len() {
                    if bar_alpha_u8 < 255 {
                        dst[o] = ((src[o + 2] as u32 * bar_alpha_u8) / 255) as u8;
                        dst[o + 1] = ((src[o + 1] as u32 * bar_alpha_u8) / 255) as u8;
                        dst[o + 2] = ((src[o] as u32 * bar_alpha_u8) / 255) as u8;
                        dst[o + 3] = ((src[o + 3] as u32 * bar_alpha_u8) / 255) as u8;
                    } else {
                        dst[o] = src[o + 2]; // B
                        dst[o + 1] = src[o + 1]; // G
                        dst[o + 2] = src[o]; // R
                        dst[o + 3] = src[o + 3]; // A
                    }
                }
            }
        }

        // A29: input_region = SO pills + dropdown ativo (passes through resto).
        self.update_input_region();

        let surface = self.layer.wl_surface();
        surface.damage_buffer(0, 0, self.width as i32, self.height as i32);
        buffer.attach_to(surface).ok();
        surface.commit();
    }
}

// ============================================================
// Wayland delegate handlers.
// ============================================================

impl CompositorHandler for LumoBar {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: i32,
    ) {
    }
    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }
    fn frame(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {
        self.redraw(qh);
    }
    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for LumoBar {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl LayerShellHandler for LumoBar {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.running = false;
    }

    fn configure(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &LayerSurface,
        cfg: LayerSurfaceConfigure,
        _: u32,
    ) {
        let (w, h) = cfg.new_size;
        // A19.13: forca 1920 sempre (compositor passa width parcial as vezes)
        self.width = 1920;
        // A20.11 + A24 + A27: altura max cobre maior dropdown.
        let lumo_menu_h = menu::menu_height(MENU_LUMO_ITEMS) as u32;
        let max_drop = DROPDOWN_H.max(DROPDOWN_DATETIME_H).max(lumo_menu_h as f32) as u32;
        self.height = BAR_HEIGHT + DROPDOWN_GAP as u32 + max_drop + 8;
        self.first_configured = true;
        eprintln!(
            "[lumo-bar] configured cfg_size=({},{}) FORCED width=1920 height={}",
            w, h, self.height
        );
        self.refresh();
        self.redraw(qh);
    }
}

impl ShmHandler for LumoBar {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl SeatHandler for LumoBar {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer && self.pointer.is_none() {
            if let Ok(p) = self.seat_state.get_pointer_with_theme(
                qh,
                &seat,
                self.shm.wl_shm(),
                self.layer.wl_surface().clone(),
                ThemeSpec::System,
            ) {
                self.pointer = Some(p);
                eprintln!("[lumo-bar] pointer adquirido ThemedPointer");
            }
        }
        // A31.3: adquire teclado pra capturar senha no modal.
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            match self.seat_state.get_keyboard::<_, LumoBar>(qh, &seat, None) {
                Ok(kb) => {
                    self.keyboard = Some(kb);
                    eprintln!("[lumo-bar] A31.3 keyboard adquirido");
                }
                Err(e) => eprintln!("[lumo-bar] A31.3 keyboard get falhou: {:?}", e),
            }
        }
    }
    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        cap: Capability,
    ) {
        if cap == Capability::Keyboard {
            self.keyboard = None;
        }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl ProvidesRegistryState for LumoBar {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers!(OutputState, SeatState);
}
