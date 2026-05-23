//! bar/state.rs - LumoBar struct + BarSnapshot + PaintResult + paint_frame.

use std::os::unix::net::UnixStream;
use std::sync::{
    atomic::AtomicU8,
    Arc,
};
use std::time::Instant;

use smithay_client_toolkit::{
    compositor::CompositorState,
    output::OutputState,
    registry::RegistryState,
    seat::{pointer::ThemedPointer, SeatState},
    shell::wlr_layer::LayerSurface,
    shm::{slot::SlotPool, Shm},
    reexports::client::protocol::wl_keyboard::WlKeyboard,
};
use tiny_skia::{Color, Pixmap};

use lumo_foundation::{LumoColors, LumoTheme};
use lumo_animation::LAAnimator;

use crate::bar::dropdowns::battery::{draw_battery_dropdown, BatteryInfo};
use crate::bar::password_modal::PasswordModalState;
use crate::bar::dropdowns::brightness::{draw_brightness_dropdown, BrightnessInfo};
use crate::bar::dropdowns::datetime::{draw_datetime_dropdown, DateTimeInfo};
use crate::bar::dropdowns::lumo_menu::draw_lumo_menu;
use crate::bar::dropdowns::wifi::{draw_wifi_dropdown, WifiInfo};
pub(crate) use crate::bar::dropdowns::DropdownActive;
use crate::bar::fonts::{
    draw_text, draw_text_mono, measure_text, measure_text_mono, opaque, rgba_hex,
};
use crate::bar::icons::{draw_brightness_sun,
    battery_total_width, draw_battery, draw_brand_dot, draw_wifi,
};
use crate::bar::pills::draw_pill_bg;
use crate::bar::tokens::*;

// ============================================================
// BarSnapshot - estado imutavel passado pra paint_frame.
// ============================================================

pub(crate) struct BarSnapshot {
    pub width: u32,
    pub height: u32,
    pub battery_pct: u8,
    pub wifi_on: bool,
    pub palette: LumoColors,
    pub theme: LumoTheme,
    pub clock_hh: u8,
    pub clock_mm: u8,
    pub active_ws: u8,
    pub date_str: String,
    pub dropdown: DropdownActive,
    pub battery_info: BatteryInfo,
    pub wifi_info: WifiInfo,
    pub brightness_info: BrightnessInfo,
    pub datetime_info: DateTimeInfo,
    pub lumo_menu_hover_idx: usize,
    pub appmenu_items: Vec<crate::bar::appmenu::AppMenuItem>,
    pub appmenu_open_idx: Option<usize>,
    pub appmenu_submenu: Vec<crate::bar::appmenu::AppMenuItem>,
    pub appmenu_app_id: String,
    pub appmenu_title: String,
    pub appmenu_fallback_hover_idx: Option<usize>,
    pub dropdown_scale: f32,
    pub dropdown_alpha: f32,
    pub bar_alpha: f32,
    pub password_modal: PasswordModalState,
}

#[derive(Default, Clone)]
pub(crate) struct PaintResult {
    pub bat_hit_rect: Option<(f32, f32, f32, f32)>,
    pub wifi_hit_rect: Option<(f32, f32, f32, f32)>,
    pub datetime_hit_rect: Option<(f32, f32, f32, f32)>,
    pub lumo_hit_rect: Option<(f32, f32, f32, f32)>,
    pub cal_prev_rect: Option<(f32, f32, f32, f32)>,
    pub cal_next_rect: Option<(f32, f32, f32, f32)>,
    pub cal_today_rect: Option<(f32, f32, f32, f32)>,
    pub cal_day_rects: Vec<(u32, (f32, f32, f32, f32))>,
    pub appmenu_fallback_rect: Option<(f32, f32, f32, f32)>,
    pub appmenu_fallback_dropdown_rects: Vec<(usize, (f32, f32, f32, f32))>,
    pub brightness_hit_rect: Option<(f32, f32, f32, f32)>,
    pub brightness_slider_rect: Option<(f32, f32, f32, f32)>,
    pub brightness_preset_day_rect: Option<(f32, f32, f32, f32)>,
    pub brightness_preset_night_rect: Option<(f32, f32, f32, f32)>,
    pub bat_charge_limit_toggle_rect: Option<(f32, f32, f32, f32)>,
    pub bat_profile_cycle_rect: Option<(f32, f32, f32, f32)>,
    pub wifi_toggle_rect: Option<(f32, f32, f32, f32)>,
    pub wifi_disconnect_rect: Option<(f32, f32, f32, f32)>,
    pub wifi_connect_rects: Vec<(String, (f32, f32, f32, f32))>,
    pub last_click_at: Option<Instant>,
    pub pwd_confirm_rect: Option<(f32, f32, f32, f32)>,
    pub pwd_cancel_rect: Option<(f32, f32, f32, f32)>,
    pub appmenu_pill_rects: Vec<(usize, (f32, f32, f32, f32))>,
    pub appmenu_submenu_rects: Vec<(usize, (f32, f32, f32, f32))>,
    pub dropdown_rect_real: Option<(f32, f32, f32, f32)>,
}

pub(crate) struct LumoBar {
    pub registry: RegistryState,
    pub output_state: OutputState,
    pub shm: Shm,
    pub seat_state: SeatState,
    pub compositor_state: CompositorState,
    pub current_input_region: Option<smithay_client_toolkit::compositor::Region>,
    pub layer: LayerSurface,
    pub pool: SlotPool,
    pub width: u32,
    pub height: u32,
    pub active_workspace: Arc<AtomicU8>,
    pub battery_pct: u8,
    pub battery_info: BatteryInfo,
    pub wifi_on: bool,
    pub wifi_info: WifiInfo,
    pub wifi_refresh_due: Option<Instant>,
    pub running: bool,
    pub first_configured: bool,
    pub pointer: Option<ThemedPointer>,
    pub keyboard: Option<WlKeyboard>,
    pub pointer_x: f32,
    pub pointer_pos: Option<(f64, f64)>,
    pub bat_hit_rect: Option<(f32, f32, f32, f32)>,
    pub wifi_hit_rect: Option<(f32, f32, f32, f32)>,
    pub datetime_hit_rect: Option<(f32, f32, f32, f32)>,
    pub lumo_hit_rect: Option<(f32, f32, f32, f32)>,
    pub lumo_menu_hover_idx: usize,
    pub cal_prev_rect: Option<(f32, f32, f32, f32)>,
    pub cal_next_rect: Option<(f32, f32, f32, f32)>,
    pub cal_today_rect: Option<(f32, f32, f32, f32)>,
    pub cal_day_rects: Vec<(u32, (f32, f32, f32, f32))>,
    pub bat_charge_limit_toggle_rect: Option<(f32, f32, f32, f32)>,
    pub bat_profile_cycle_rect: Option<(f32, f32, f32, f32)>,
    pub brightness_info: BrightnessInfo,
    pub brightness_hit_rect: Option<(f32, f32, f32, f32)>,
    pub brightness_slider_rect: Option<(f32, f32, f32, f32)>,
    pub brightness_preset_day_rect: Option<(f32, f32, f32, f32)>,
    pub brightness_preset_night_rect: Option<(f32, f32, f32, f32)>,
    pub brightness_dragging: bool,
    pub brightness_dragging_slider: bool,
    pub brightness_drag_last_y: f32,
    pub wifi_toggle_rect: Option<(f32, f32, f32, f32)>,
    pub wifi_disconnect_rect: Option<(f32, f32, f32, f32)>,
    pub wifi_connect_rects: Vec<(String, (f32, f32, f32, f32))>,
    pub last_click_at: Option<Instant>,
    pub dropdown: DropdownActive,
    pub dropdown_rect: Option<(f32, f32, f32, f32)>,
    pub dropdown_h_final: f32,
    pub viewed_year: i32,
    pub viewed_month: u32,
    pub selected_day: Option<u32>,
    pub appmenu: crate::bar::appmenu::AppMenuState,
    pub appmenu_open_idx: Option<usize>,
    pub appmenu_submenu: Vec<crate::bar::appmenu::AppMenuItem>,
    pub appmenu_pill_rects: Vec<(usize, (f32, f32, f32, f32))>,
    pub appmenu_submenu_rects: Vec<(usize, (f32, f32, f32, f32))>,
    pub appmenu_fallback_rect: Option<(f32, f32, f32, f32)>,
    pub appmenu_fallback_dropdown_rects: Vec<(usize, (f32, f32, f32, f32))>,
    pub appmenu_fallback_hover_idx: Option<usize>,
    pub registrar_handle: crate::bar::registrar::RegistrarHandle,
    pub ipc_stream: Option<UnixStream>,
    pub ipc_rx_buf: Vec<u8>,
    pub ipc_reconnect_at: Option<std::time::Instant>,
    pub ipc_reconnect_delay: std::time::Duration,
    pub theme: LumoTheme,
    pub palette: LumoColors,
    pub dropdown_scale_anim: LAAnimator<f32>,
    pub dropdown_alpha_anim: LAAnimator<f32>,
    pub dropdown_closing: bool,
    pub dropdown_closing_which: DropdownActive,
    pub refresh_anim: LAAnimator<f32>,
    pub refresh_animating: bool,
    pub password_modal: PasswordModalState,
    pub pwd_confirm_rect: Option<(f32, f32, f32, f32)>,
    pub pwd_cancel_rect: Option<(f32, f32, f32, f32)>,
    pub nm_connect_rx: Option<std::sync::mpsc::Receiver<crate::bar::system_info::NmConnectResult>>,
}

pub(crate) fn paint_frame(pixmap: &mut Pixmap, snap: &BarSnapshot) -> PaintResult {
    let palette = &snap.palette;
    pixmap.fill(Color::TRANSPARENT);

    let mut result = PaintResult::default();
    
    // Tokens CSS-like
    let pill_y       = PILL_MARGIN_TOP;
    let pill_h       = PILL_H;
    let pill_pad_x   = PILL_PAD_X;
    let pill_gap     = PILL_GAP;
    let pill_margin  = PILL_MARGIN_X;
    let pill_cy      = pill_y + pill_h / 2.0;

    let pill_bg = rgba_hex(palette.pill_bg, palette.pill_bg_alpha);
    let pill_fg = opaque(palette.pill_fg);
    let pill_fg_subtle = rgba_hex(palette.pill_fg, 0x80);
    let accent = opaque(palette.accent);

    let text_top = pill_y + (pill_h - FONT_PILL * 1.2) / 2.0;

    // 1. Pill Esquerda: Brand + AppMenu
    let lumo_w = measure_text("Lumo", FONT_PILL, true);
    let brand_w = BRAND_DOT_RADIUS * 2.0 + pill_gap + lumo_w;
    
    let mut menu_items_w = 0.0;
    for it in &snap.appmenu_items {
        menu_items_w += measure_text(&it.label, FONT_PILL, false) + pill_gap * 2.0;
    }
    
    let pill_l_w = brand_w + menu_items_w + pill_pad_x * 2.0;
    let pill_l_x = pill_margin;
    
    {
        let mut canvas = pixmap.as_mut();
        draw_pill_bg(&mut canvas, pill_l_x, pill_y, pill_l_w, pill_h, pill_bg, 0);
        
        let mut cx = pill_l_x + pill_pad_x;
        draw_brand_dot(&mut canvas, cx + BRAND_DOT_RADIUS, pill_cy, accent);
        cx += BRAND_DOT_RADIUS * 2.0 + pill_gap;
        
        draw_text(&mut canvas, cx, text_top, "Lumo", FONT_PILL, pill_fg, true);
        result.lumo_hit_rect = Some((pill_l_x, pill_y, brand_w + pill_pad_x, pill_h));
        cx += lumo_w + pill_gap;
        
        for (idx, item) in snap.appmenu_items.iter().enumerate() {
            let lw = measure_text(&item.label, FONT_PILL, false);
            let item_w = lw + pill_gap * 2.0;
            draw_text(&mut canvas, cx + pill_gap, text_top, &item.label, FONT_PILL, pill_fg, false);
            result.appmenu_pill_rects.push((idx, (cx, pill_y, item_w, pill_h)));
            cx += item_w;
        }
    }

    // 2. Pill Direita: Workspace + Status
    let ws_str = snap.active_ws.to_string();
    let ws_w = measure_text_mono(&ws_str, FONT_PILL, true) + pill_gap;
    
    let clock_s = format!("{:02}:{:02}", snap.clock_hh, snap.clock_mm);
    let clock_w = measure_text_mono(&clock_s, FONT_PILL, true);
    
    let date_w = measure_text(&snap.date_str, FONT_DATE, false);
    let bat_w = battery_total_width();
    let wifi_w = WIFI_SIZE;
    
    // Grid: ws | wifi | bat | br | date | clock
    let pill_r_w = ws_w + (pill_gap * 2.0) + wifi_w + pill_gap + bat_w + pill_gap + 14.0 + pill_gap + date_w + pill_gap + clock_w + pill_pad_x * 2.0;
    let pill_r_x = snap.width as f32 - pill_margin - pill_r_w;
    
    {
        let mut canvas = pixmap.as_mut();
        draw_pill_bg(&mut canvas, pill_r_x, pill_y, pill_r_w, pill_h, pill_bg, 0);
        
        let mut rx = pill_r_x + pill_pad_x;
        
        // Workspace
        draw_text_mono(&mut canvas, rx, text_top, &ws_str, FONT_PILL, accent, true);
        rx += ws_w + pill_gap;
        
        // Wifi
        draw_wifi(&mut canvas, rx, pill_cy - WIFI_SIZE/2.0, snap.wifi_on, pill_fg, pill_fg_subtle);
        result.wifi_hit_rect = Some((rx - 4.0, pill_y, wifi_w + 8.0, pill_h));
        rx += wifi_w + pill_gap;
        
        // Bateria
        let charging = snap.battery_info.status == "Charging";
        draw_battery(&mut canvas, rx, pill_cy - BAT_BODY_H/2.0, snap.battery_pct, charging, pill_fg, accent);
        result.bat_hit_rect = Some((rx - 4.0, pill_y, bat_w + 8.0, pill_h));
        rx += bat_w + pill_gap;
        
        // Brilho Sun
        draw_brightness_sun(&mut canvas, rx + 7.0, pill_cy, snap.brightness_info.pct, pill_fg, accent);
        result.brightness_hit_rect = Some((rx - 4.0, pill_y, 22.0, pill_h));
        rx += 14.0 + pill_gap;
        
        // Date
        draw_text(&mut canvas, rx, text_top, &snap.date_str, FONT_DATE, pill_fg, false);
        rx += date_w + pill_gap;
        
        // Clock
        draw_text_mono(&mut canvas, rx, text_top, &clock_s, FONT_PILL, pill_fg, true);
        result.datetime_hit_rect = Some((rx - 4.0, pill_y, clock_w + 12.0, pill_h));
    }

    // 3. Dropdowns (Com as mesmas coordenadas das pills)
    match snap.dropdown {
        DropdownActive::Battery => {
            if let Some((rx, ry, rw, rh)) = result.bat_hit_rect {
                let dropdown_w = DROPDOWN_W;
                let dropdown_x = (rx + rw - dropdown_w - 4.0).max(pill_margin).min(snap.width as f32 - pill_margin - dropdown_w);
                let dropdown_y = ry + rh + DROPDOWN_GAP;
                let mut final_h = DROPDOWN_H;
                if let Some(temp) = snap.battery_info.cpu_temp_c { if temp > 70.0 { final_h += 34.0; } }
                result.dropdown_rect_real = Some((dropdown_x, dropdown_y, dropdown_w, final_h));
                if let Some(mut sub) = Pixmap::new(dropdown_w as u32, final_h as u32) {
                    let hits = draw_battery_dropdown(&mut sub.as_mut(), 0.0, 0.0, dropdown_w, final_h, palette, &snap.battery_info);
                    result.bat_charge_limit_toggle_rect = hits.charge_limit_toggle_rect.map(|(x,y,w,h)| (x+dropdown_x, y+dropdown_y, w, h));
                    result.bat_profile_cycle_rect = hits.profile_cycle_rect.map(|(x,y,w,h)| (x+dropdown_x, y+dropdown_y, w, h));
                    composite_dropdown(pixmap, &sub, dropdown_x, dropdown_y, snap.dropdown_scale, snap.dropdown_alpha);
                }
            }
        },
        DropdownActive::Brightness => {
            if let Some((rx, ry, rw, rh)) = result.brightness_hit_rect {
                let dropdown_w = DROPDOWN_BRIGHTNESS_W;
                let dropdown_x = (rx + rw/2.0 - dropdown_w/2.0).max(pill_margin).min(snap.width as f32 - pill_margin - dropdown_w);
                let dropdown_y = ry + rh + DROPDOWN_GAP;
                result.dropdown_rect_real = Some((dropdown_x, dropdown_y, dropdown_w, DROPDOWN_BRIGHTNESS_H));
                if let Some(mut sub) = Pixmap::new(dropdown_w as u32, DROPDOWN_BRIGHTNESS_H as u32) {
                    let hits = draw_brightness_dropdown(&mut sub.as_mut(), 0.0, 0.0, dropdown_w, DROPDOWN_BRIGHTNESS_H, palette, &snap.brightness_info);
                    result.brightness_slider_rect = hits.slider_rect.map(|(x,y,w,h)| (x+dropdown_x, y+dropdown_y, w, h));
                    result.brightness_preset_day_rect = hits.preset_day_rect.map(|(x,y,w,h)| (x+dropdown_x, y+dropdown_y, w, h));
                    result.brightness_preset_night_rect = hits.preset_night_rect.map(|(x,y,w,h)| (x+dropdown_x, y+dropdown_y, w, h));
                    composite_dropdown(pixmap, &sub, dropdown_x, dropdown_y, snap.dropdown_scale, snap.dropdown_alpha);
                }
            }
        },
        DropdownActive::Wifi => {
            if let Some((rx, ry, rw, rh)) = result.wifi_hit_rect {
                let dropdown_w = DROPDOWN_WIFI_W;
                let dropdown_x = (rx + rw/2.0 - dropdown_w/2.0).max(pill_margin).min(snap.width as f32 - pill_margin - dropdown_w);
                let dropdown_y = ry + rh + DROPDOWN_GAP;
                result.dropdown_rect_real = Some((dropdown_x, dropdown_y, dropdown_w, DROPDOWN_WIFI_H));
                if let Some(mut sub) = Pixmap::new(dropdown_w as u32, DROPDOWN_WIFI_H as u32) {
                    let hits = draw_wifi_dropdown(&mut sub.as_mut(), 0.0, 0.0, dropdown_w, DROPDOWN_WIFI_H, palette, &snap.wifi_info);
                    result.wifi_toggle_rect = hits.toggle_rect.map(|(x,y,w,h)| (x+dropdown_x, y+dropdown_y, w, h));
                    result.wifi_disconnect_rect = hits.disconnect_rect.map(|(x,y,w,h)| (x+dropdown_x, y+dropdown_y, w, h));
                    result.wifi_connect_rects = hits.connect_rects.iter().map(|(s,(x,y,w,h))| (s.clone(), (x+dropdown_x, y+dropdown_y, *w, *h))).collect();
                    composite_dropdown(pixmap, &sub, dropdown_x, dropdown_y, snap.dropdown_scale, snap.dropdown_alpha);
                }
            }
        },
        DropdownActive::DateTime => {
            if let Some((rx, ry, rw, rh)) = result.datetime_hit_rect {
                let dropdown_w = DROPDOWN_DATETIME_W;
                let dropdown_x = (rx + rw/2.0 - dropdown_w/2.0).max(pill_margin).min(snap.width as f32 - pill_margin - dropdown_w);
                let dropdown_y = ry + rh + DROPDOWN_GAP;
                result.dropdown_rect_real = Some((dropdown_x, dropdown_y, dropdown_w, DROPDOWN_DATETIME_H));
                if let Some(mut sub) = Pixmap::new(dropdown_w as u32, DROPDOWN_DATETIME_H as u32) {
                    let hits = draw_datetime_dropdown(&mut sub.as_mut(), 0.0, 0.0, dropdown_w, DROPDOWN_DATETIME_H, palette, &snap.datetime_info);
                    result.cal_prev_rect = hits.prev_rect.map(|(x,y,w,h)| (x+dropdown_x, y+dropdown_y, w, h));
                    result.cal_next_rect = hits.next_rect.map(|(x,y,w,h)| (x+dropdown_x, y+dropdown_y, w, h));
                    result.cal_today_rect = hits.today_rect.map(|(x,y,w,h)| (x+dropdown_x, y+dropdown_y, w, h));
                    result.cal_day_rects = hits.day_rects.iter().map(|(d,(x,y,w,h))| (*d, (x+dropdown_x, y+dropdown_y, *w, *h))).collect();
                    composite_dropdown(pixmap, &sub, dropdown_x, dropdown_y, snap.dropdown_scale, snap.dropdown_alpha);
                }
            }
        },
        DropdownActive::LumoMenu => {
            if let Some((rx, ry, _rw, rh)) = result.lumo_hit_rect {
                let menu_w = MENU_LUMO_W as u32;
                let menu_h_px = crate::menu::menu_height(MENU_LUMO_ITEMS) as u32;
                let dropdown_x = rx.max(pill_margin);
                let dropdown_y = ry + rh + DROPDOWN_GAP;
                result.dropdown_rect_real = Some((dropdown_x, dropdown_y, menu_w as f32, menu_h_px as f32));
                if let Some(mut sub) = Pixmap::new(menu_w, menu_h_px) {
                    draw_lumo_menu(&mut sub.as_mut(), 0.0, 0.0, palette, snap.lumo_menu_hover_idx);
                    composite_dropdown(pixmap, &sub, dropdown_x, dropdown_y, snap.dropdown_scale, snap.dropdown_alpha);
                }
            }
        },
        _ => {}
    }

    if snap.password_modal.active {
        let hits = crate::bar::password_modal::draw_password_modal(&mut pixmap.as_mut(), snap.width as f32, snap.height as f32, palette, &snap.password_modal);
        result.pwd_confirm_rect = hits.confirm_rect;
        result.pwd_cancel_rect = hits.cancel_rect;
    }

    result
}

fn composite_dropdown(dst: &mut Pixmap, src: &Pixmap, x: f32, y: f32, scale: f32, alpha: f32) {
    let visible_h = (scale * src.height() as f32).round() as u32;
    if visible_h == 0 { return; }
    if let Some(mut clipped) = Pixmap::new(src.width(), visible_h) {
        let rb = (src.width() * 4) as usize;
        let s_data = src.data();
        let d_data = clipped.data_mut();
        for r in 0..(visible_h as usize) { d_data[r*rb..r*rb+rb].copy_from_slice(&s_data[r*rb..r*rb+rb]); }
        if alpha < 1.0 {
            let a_u32 = (alpha * 255.0) as u32;
            for c in clipped.data_mut().chunks_mut(4) {
                c[0] = ((c[0] as u32 * a_u32)/255) as u8; c[1] = ((c[1] as u32 * a_u32)/255) as u8;
                c[2] = ((c[2] as u32 * a_u32)/255) as u8; c[3] = ((c[3] as u32 * a_u32)/255) as u8;
            }
        }
        use tiny_skia::{PixmapPaint, Transform};
        dst.draw_pixmap(x as i32, y as i32, clipped.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
    }
}
