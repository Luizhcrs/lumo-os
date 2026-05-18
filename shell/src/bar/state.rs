//! bar/state.rs - LumoBar struct + BarSnapshot + PaintResult + paint_frame.
//!
//! Centraliza o estado runtime da bar (layer surface, pool, pointer, hit
//! rects, theme/palette) e a logica de paint que consome um snapshot
//! imutavel. Render delegado pra modulos pills/icons/dropdowns.

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
};
use tiny_skia::{Color, Pixmap};

use lumo_foundation::{LumoColors, LumoTheme};
use lumo_animation::{AnimCurve, LAAnimator, LACurve};

use crate::bar::dropdowns::battery::{draw_battery_dropdown, BatteryInfo};
use crate::bar::dropdowns::datetime::{draw_datetime_dropdown, DateTimeInfo};
use crate::bar::dropdowns::lumo_menu::draw_lumo_menu;
use crate::bar::dropdowns::wifi::{draw_wifi_dropdown, WifiInfo};
use crate::bar::dropdowns::DropdownActive;
use crate::bar::fonts::{
    draw_text, draw_text_mono, measure_text, measure_text_mono, opaque, rgba_hex,
};
use crate::bar::icons::{
    battery_total_width, draw_battery, draw_brand_dot, draw_wifi, fill_circle,
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
    pub wifi_info: WifiInfo, // A23
    pub datetime_info: DateTimeInfo, // A24
    /// A27: indice do item em hover no menu Lumo (usize::MAX = nenhum).
    pub lumo_menu_hover_idx: usize,
    // C5: appmenu pills (app em foco).
    pub appmenu_items: Vec<crate::bar::appmenu::AppMenuItem>,
    pub appmenu_open_idx: Option<usize>,
    pub appmenu_submenu: Vec<crate::bar::appmenu::AppMenuItem>,
    // B4: fator de escala e opacidade do dropdown ativo (0.0=fechado, 1.0=aberto).
    pub dropdown_scale: f32,
    pub dropdown_alpha: f32,
}

/// Resultado de paint_frame: posicoes calculadas pra hit-test no proximo frame.
#[derive(Default, Clone)]
pub(crate) struct PaintResult {
    pub bat_hit_rect: Option<(f32, f32, f32, f32)>,
    pub wifi_hit_rect: Option<(f32, f32, f32, f32)>,     // A23
    pub datetime_hit_rect: Option<(f32, f32, f32, f32)>, // A24
    pub lumo_hit_rect: Option<(f32, f32, f32, f32)>,     // A27: brand "Lumo" pill esquerda
    // A26: hit-tests do calendar interativo (so populados quando dropdown=DateTime).
    pub cal_prev_rect: Option<(f32, f32, f32, f32)>,
    pub cal_next_rect: Option<(f32, f32, f32, f32)>,
    pub cal_today_rect: Option<(f32, f32, f32, f32)>,
    /// Cada (day, rect). Day = dia do mes visualizado (1..=31), rect em coords surface.
    pub cal_day_rects: Vec<(u32, (f32, f32, f32, f32))>,
    // A31.2: hit-rects do dropdown wifi (so populados quando dropdown=Wifi).
    pub wifi_toggle_rect: Option<(f32, f32, f32, f32)>,
    pub wifi_disconnect_rect: Option<(f32, f32, f32, f32)>,
    pub wifi_connect_rects: Vec<(String, (f32, f32, f32, f32))>,
    pub last_click_at: Option<Instant>,
    // C5: hit-rects pills appmenu top-level (idx, rect).
    pub appmenu_pill_rects: Vec<(usize, (f32, f32, f32, f32))>,
    // C5: hit-rects subitens submenu aberto (sidx, rect).
    pub appmenu_submenu_rects: Vec<(usize, (f32, f32, f32, f32))>,
}

// ============================================================
// paint_frame: pinta as 2 pills sobre fundo transparente.
// ============================================================

pub(crate) fn paint_frame(pixmap: &mut Pixmap, snap: &BarSnapshot) -> PaintResult {
    let palette = &snap.palette;
    // BAR BACKGROUND TRANSPARENTE (A18 - alpha 0). Compositor pinta atras.
    pixmap.fill(Color::TRANSPARENT);

    let mut result = PaintResult::default();
    let h = snap.height as f32;
    let pill_y = PILL_MARGIN_TOP;
    let pill_cy = pill_y + PILL_H / 2.0;

    // Cor pill bg: hex + alpha do tema. Mesma cor pra ambas as pills.
    let pill_bg = rgba_hex(palette.pill_bg, palette.pill_bg_alpha);
    let pill_fg = opaque(palette.pill_fg);
    let pill_fg_subtle = rgba_hex(palette.pill_fg, 0xB0); // 70% pra dim sobre pill
    let pill_sep = rgba_hex(palette.pill_sep, palette.pill_sep_alpha);
    let accent = opaque(palette.accent);

    // Topo y do texto dentro da pill (centralizado vertical).
    let text_h = FONT_PILL * 1.2;
    let text_top = pill_y + (PILL_H - text_h) / 2.0;
    let text_top = text_top.round();

    // ============================================================
    // PILL ESQUERDA: [dot] Lumo (A30.1 sem workspace numero)
    // ============================================================
    let lumo_w = measure_text("Lumo", FONT_PILL, true);
    let pill_l_content_w = BRAND_DOT_RADIUS * 2.0 + PILL_GAP + lumo_w;
    let pill_l_w = pill_l_content_w + PILL_PAD_X * 2.0;
    let pill_l_x = PILL_MARGIN_X;
    let _ = snap.active_ws; // suppress dead-code warn ate A35 workspaces reais

    {
        let mut canvas = pixmap.as_mut();
        draw_pill_bg(&mut canvas, pill_l_x, pill_y, pill_l_w, PILL_H, pill_bg, 0);

        let mut cx = pill_l_x + PILL_PAD_X;
        let lumo_hit_x_start = cx - 4.0;
        // Brand dot accent
        draw_brand_dot(&mut canvas, cx + BRAND_DOT_RADIUS, pill_cy, accent);
        cx += BRAND_DOT_RADIUS * 2.0 + PILL_GAP;
        // "Lumo" bold
        draw_text(&mut canvas, cx, text_top, "Lumo", FONT_PILL, pill_fg, true);
        let lumo_hit_x_end = cx + lumo_w + 4.0;
        result.lumo_hit_rect = Some((
            lumo_hit_x_start,
            pill_y,
            lumo_hit_x_end - lumo_hit_x_start,
            PILL_H,
        ));
    }

    // ============================================================
    // PILL DIREITA: [wifi] [bat icone] HH:MM (A19.8: removido texto %)
    // ============================================================
    let bat_icon_w = battery_total_width();
    let clock_s = format!("{:02}:{:02}", snap.clock_hh, snap.clock_mm);
    // A29: clock = Geist Mono (digito tabular).
    let clock_w = measure_text_mono(&clock_s, FONT_PILL, false);

    let date_w = measure_text(&snap.date_str, FONT_DATE, false);
    let pill_r_content_w =
        bat_icon_w + PILL_GAP + WIFI_SIZE + PILL_GAP + date_w + 8.0 + clock_w;
    let pill_r_w = pill_r_content_w + PILL_PAD_X * 2.0;
    let pill_r_x = snap.width as f32 - PILL_MARGIN_X - pill_r_w;

    {
        let mut canvas = pixmap.as_mut();
        draw_pill_bg(&mut canvas, pill_r_x, pill_y, pill_r_w, PILL_H, pill_bg, 0);
        let mut cx = pill_r_x + PILL_PAD_X;
        // A19.10: ordem bat -> wifi -> data -> hora (Mac-style)
        let bat_x_start = cx;
        // A31 fix: A30 alterou signature de draw_battery pra incluir flag charging.
        let bat_charging = snap.battery_info.status == "Charging";
        draw_battery(&mut canvas, cx, pill_cy - BAT_BODY_H / 2.0, snap.battery_pct, bat_charging, pill_fg, accent);
        // A20.13: hit area = SO o icone bateria (era pill inteira A20.4)
        result.bat_hit_rect = Some((bat_x_start - 4.0, pill_y, bat_icon_w + 8.0, PILL_H));
        cx += bat_icon_w + PILL_GAP;
        // A23: salvar wifi_hit_rect igual bat.
        let wifi_x_start = cx;
        draw_wifi(&mut canvas, cx, pill_cy - WIFI_SIZE / 2.0, snap.wifi_on, pill_fg, pill_fg_subtle);
        result.wifi_hit_rect = Some((wifi_x_start - 4.0, pill_y, WIFI_SIZE + 8.0, PILL_H));
        cx += WIFI_SIZE + PILL_GAP;
        // A24: hit area cobre data + hora juntas (mesmo dropdown calendario).
        let datetime_x_start = cx;
        draw_text(&mut canvas, cx, text_top, &snap.date_str, FONT_DATE, pill_fg, false);
        cx += date_w + 8.0;
        // A29: clock HH:MM = Geist Mono.
        draw_text_mono(&mut canvas, cx, text_top, &clock_s, FONT_PILL, pill_fg, false);
        let datetime_end = cx + clock_w;
        result.datetime_hit_rect = Some((
            datetime_x_start - 4.0,
            pill_y,
            (datetime_end - datetime_x_start) + 8.0,
            PILL_H,
        ));
    }

    // ============================================================
    // DROPDOWN (A20/A23, B4 springy) - render abaixo das pills.
    //
    // B4: render em Pixmap auxiliar do tamanho do dropdown, depois
    // composita sobre o principal com alpha global = dropdown_alpha.
    // Scale (0.85->1.0) via clip na altura: exibe apenas scale*H pixels
    // desde o topo (ancora topo-centro da pill, crescimento pra baixo).
    // ============================================================
    match snap.dropdown {
        DropdownActive::Battery => {
            if let Some((rx, ry, rw, rh)) = result.bat_hit_rect {
                let want_x = rx + rw / 2.0 - DROPDOWN_W / 2.0;
                let max_x = snap.width as f32 - PILL_MARGIN_X - DROPDOWN_W;
                let dropdown_x = want_x.max(PILL_MARGIN_X).min(max_x.max(PILL_MARGIN_X));
                let dropdown_y = ry + rh + DROPDOWN_GAP;
                if let Some(mut sub) = Pixmap::new(DROPDOWN_W as u32, DROPDOWN_H as u32) {
                    {
                        let mut canvas = sub.as_mut();
                        draw_battery_dropdown(
                            &mut canvas,
                            0.0,
                            0.0,
                            DROPDOWN_W,
                            DROPDOWN_H,
                            palette,
                            &snap.battery_info,
                        );
                    }
                    composite_dropdown(pixmap, &sub, dropdown_x, dropdown_y, snap.dropdown_scale, snap.dropdown_alpha);
                }
            }
        }
        DropdownActive::Wifi => {
            if let Some((rx, ry, rw, rh)) = result.wifi_hit_rect {
                let want_x = rx + rw / 2.0 - DROPDOWN_WIFI_W / 2.0;
                let max_x = snap.width as f32 - PILL_MARGIN_X - DROPDOWN_WIFI_W;
                let dropdown_x = want_x.max(PILL_MARGIN_X).min(max_x.max(PILL_MARGIN_X));
                let dropdown_y = ry + rh + DROPDOWN_GAP;
                if let Some(mut sub) = Pixmap::new(DROPDOWN_WIFI_W as u32, DROPDOWN_WIFI_H as u32) {
                    let hits = {
                        let mut canvas = sub.as_mut();
                        draw_wifi_dropdown(
                            &mut canvas,
                            0.0,
                            0.0,
                            DROPDOWN_WIFI_W,
                            DROPDOWN_WIFI_H,
                            palette,
                            &snap.wifi_info,
                        )
                    };
                    result.wifi_toggle_rect = hits.toggle_rect.map(|(x,y,w,h)| (x+dropdown_x, y+dropdown_y, w, h));
                    result.wifi_disconnect_rect = hits.disconnect_rect.map(|(x,y,w,h)| (x+dropdown_x, y+dropdown_y, w, h));
                    result.wifi_connect_rects = hits.connect_rects.iter().map(|(s,(x,y,w,h))| (s.clone(), (x+dropdown_x, y+dropdown_y, *w, *h))).collect();
                    composite_dropdown(pixmap, &sub, dropdown_x, dropdown_y, snap.dropdown_scale, snap.dropdown_alpha);
                }
            }
        }
        DropdownActive::DateTime => {
            if let Some((rx, ry, rw, rh)) = result.datetime_hit_rect {
                let want_x = rx + rw / 2.0 - DROPDOWN_DATETIME_W / 2.0;
                let max_x = snap.width as f32 - PILL_MARGIN_X - DROPDOWN_DATETIME_W;
                let dropdown_x = want_x.max(PILL_MARGIN_X).min(max_x.max(PILL_MARGIN_X));
                let dropdown_y = ry + rh + DROPDOWN_GAP;
                if let Some(mut sub) = Pixmap::new(DROPDOWN_DATETIME_W as u32, DROPDOWN_DATETIME_H as u32) {
                    let hits = {
                        let mut canvas = sub.as_mut();
                        draw_datetime_dropdown(
                            &mut canvas,
                            0.0,
                            0.0,
                            DROPDOWN_DATETIME_W,
                            DROPDOWN_DATETIME_H,
                            palette,
                            &snap.datetime_info,
                        )
                    };
                    result.cal_prev_rect = hits.prev_rect.map(|(x,y,w,h)| (x+dropdown_x, y+dropdown_y, w, h));
                    result.cal_next_rect = hits.next_rect.map(|(x,y,w,h)| (x+dropdown_x, y+dropdown_y, w, h));
                    result.cal_today_rect = hits.today_rect.map(|(x,y,w,h)| (x+dropdown_x, y+dropdown_y, w, h));
                    result.cal_day_rects = hits.day_rects.iter().map(|(d,(x,y,w,h))| (*d, (x+dropdown_x, y+dropdown_y, *w, *h))).collect();
                    composite_dropdown(pixmap, &sub, dropdown_x, dropdown_y, snap.dropdown_scale, snap.dropdown_alpha);
                }
            }
        }
        DropdownActive::LumoMenu => {
            if let Some((rx, ry, _rw, rh)) = result.lumo_hit_rect {
                use crate::menu;
                let menu_w = MENU_LUMO_W as u32;
                let menu_h_px = menu::menu_height(MENU_LUMO_ITEMS) as u32;
                let dropdown_x = rx.max(PILL_MARGIN_X);
                let dropdown_y = ry + rh + DROPDOWN_GAP;
                if let Some(mut sub) = Pixmap::new(menu_w, menu_h_px) {
                    {
                        let mut canvas = sub.as_mut();
                        draw_lumo_menu(&mut canvas, 0.0, 0.0, palette, snap.lumo_menu_hover_idx);
                    }
                    composite_dropdown(pixmap, &sub, dropdown_x, dropdown_y, snap.dropdown_scale, snap.dropdown_alpha);
                }
            }
        }
        DropdownActive::None => {}
    }

    // Suppress unused warns nos campos do snapshot (theme so usado pra debug log).
    let _ = (snap.theme, h);
    result
}

// ============================================================
// composite_dropdown (B4) - composita sub-pixmap sobre main com alpha.
//
// Implementa o efeito springy de abertura:
//   scale: 0.85->1.0  (clip de altura: exibe scale*h pixels desde o topo)
//   alpha: 0.0->1.0   (multiplicado pixel a pixel, canal A premultiplied)
//
// tiny-skia nao tem transform de scale em filhos. MVP: clip-by-height
// (o dropdown "cresce" de cima pra baixo) + alpha global via loop manual.
// ============================================================
fn composite_dropdown(
    dst: &mut Pixmap,
    src: &Pixmap,
    x: f32,
    y: f32,
    scale: f32,
    alpha: f32,
) {
    use tiny_skia::{BlendMode, FilterQuality, PixmapPaint, Transform};

    // Altura visivel = scale * src.height (cresce de cima).
    let visible_h = (scale * src.height() as f32).round() as i32;
    let visible_h = visible_h.max(1).min(src.height() as i32);

    // Nao ha API de clip em draw_pixmap; simulamos copiando so as linhas visiveis
    // via um sub-pixmap temporario.
    let sub_w = src.width();
    let sub_h = visible_h as u32;

    if let Some(mut clipped) = Pixmap::new(sub_w, sub_h) {
        // Copia as primeiras visible_h linhas do src pro clipped.
        let src_data = src.data();
        let dst_data = clipped.data_mut();
        let row_bytes = (sub_w * 4) as usize;
        for row in 0..(sub_h as usize) {
            let src_off = row * row_bytes;
            let dst_off = row * row_bytes;
            if src_off + row_bytes <= src_data.len() && dst_off + row_bytes <= dst_data.len() {
                dst_data[dst_off..dst_off + row_bytes].copy_from_slice(&src_data[src_off..src_off + row_bytes]);
            }
        }

        // Aplica alpha global: multiplica canal A de todos os pixels.
        let alpha_u8 = (alpha.clamp(0.0, 1.0) * 255.0) as u8;
        if alpha_u8 < 255 {
            for chunk in dst_data.chunks_mut(4) {
                // Premultiplied: multiplica todos os canais pelo alpha global.
                chunk[0] = ((chunk[0] as u32 * alpha_u8 as u32) / 255) as u8;
                chunk[1] = ((chunk[1] as u32 * alpha_u8 as u32) / 255) as u8;
                chunk[2] = ((chunk[2] as u32 * alpha_u8 as u32) / 255) as u8;
                chunk[3] = ((chunk[3] as u32 * alpha_u8 as u32) / 255) as u8;
            }
        }

        dst.draw_pixmap(
            x as i32,
            y as i32,
            clipped.as_ref(),
            &PixmapPaint {
                blend_mode: BlendMode::SourceOver,
                opacity: 1.0, // alpha ja aplicado manualmente acima
                quality: FilterQuality::Nearest,
            },
            Transform::identity(),
            None,
        );
    }
}

// ============================================================
// LumoBar - estado runtime do binario lumo-bar.
// ============================================================

pub(crate) struct LumoBar {
    pub registry: RegistryState,
    pub output_state: OutputState,
    pub shm: Shm,
    pub seat_state: SeatState,
    /// A29: precisa pra criar wl_regions (set_input_region).
    pub compositor_state: CompositorState,
    pub layer: LayerSurface,
    pub pool: SlotPool,
    pub width: u32,
    pub height: u32,
    pub active_workspace: Arc<AtomicU8>,
    pub battery_pct: u8,
    pub battery_info: BatteryInfo,
    pub wifi_on: bool,
    pub wifi_info: WifiInfo, // A23
    /// A31.2.fix: agenda refresh wifi async pra evitar bloquear click handler
    /// com nmcli list (~500ms). Main loop checa e dispara refresh ao expirar.
    pub wifi_refresh_due: Option<Instant>,
    pub running: bool,
    pub first_configured: bool,
    pub pointer: Option<ThemedPointer>,
    pub pointer_x: f32,
    pub pointer_pos: Option<(f64, f64)>,
    pub bat_hit_rect: Option<(f32, f32, f32, f32)>,
    pub wifi_hit_rect: Option<(f32, f32, f32, f32)>,     // A23
    pub datetime_hit_rect: Option<(f32, f32, f32, f32)>, // A24
    pub lumo_hit_rect: Option<(f32, f32, f32, f32)>,     // A27
    pub lumo_menu_hover_idx: usize,                      // A27
    // A26: hit-tests calendar interativo.
    pub cal_prev_rect: Option<(f32, f32, f32, f32)>,
    pub cal_next_rect: Option<(f32, f32, f32, f32)>,
    pub cal_today_rect: Option<(f32, f32, f32, f32)>,
    pub cal_day_rects: Vec<(u32, (f32, f32, f32, f32))>,
    // A31.2: hit-rects wifi (toggle + linhas).
    pub wifi_toggle_rect: Option<(f32, f32, f32, f32)>,
    pub wifi_disconnect_rect: Option<(f32, f32, f32, f32)>,
    pub wifi_connect_rects: Vec<(String, (f32, f32, f32, f32))>,
    pub last_click_at: Option<Instant>,
    pub dropdown: DropdownActive,
    // A26: mes/ano visualizado no calendar (independente do today real).
    pub viewed_year: i32,
    pub viewed_month: u32,
    /// Dia selecionado (highlight extra alem do today). None = nenhum.
    pub selected_day: Option<u32>,
    // C5: cache appmenu do app em foco.
    pub appmenu: crate::bar::appmenu::AppMenuState,
    pub appmenu_open_idx: Option<usize>,
    pub appmenu_submenu: Vec<crate::bar::appmenu::AppMenuItem>,
    pub appmenu_pill_rects: Vec<(usize, (f32, f32, f32, f32))>,
    pub appmenu_submenu_rects: Vec<(usize, (f32, f32, f32, f32))>,
    // C5.1: handle compartilhado com thread Registrar DBus.
    pub registrar_handle: crate::bar::registrar::RegistrarHandle,
    pub ipc_stream: Option<UnixStream>,
    pub ipc_rx_buf: Vec<u8>,
    pub theme: LumoTheme,
    pub palette: LumoColors,
    // B4: animadores de abertura/fechamento de dropdown (scale 0.85->1.0, alpha 0->1).
    pub dropdown_scale_anim: LAAnimator<f32>,
    pub dropdown_alpha_anim: LAAnimator<f32>,
    // B4: true quando uma animacao de fechamento esta em andamento.
    pub dropdown_closing: bool,
    // B4: ultimo dropdown que estava aberto (para fechar com animacao correta).
    pub dropdown_closing_which: crate::bar::dropdowns::DropdownActive,
}
