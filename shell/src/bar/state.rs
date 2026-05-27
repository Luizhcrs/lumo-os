//! bar/state.rs - LumoBar struct + BarSnapshot + PaintResult + paint_frame.
//!
//! Centraliza o estado runtime da bar (layer surface, pool, pointer, hit
//! rects, theme/palette) e a logica de paint que consome um snapshot
//! imutavel. Render delegado pra modulos pills/icons/dropdowns.

use std::os::unix::net::UnixStream;
use std::sync::{atomic::AtomicU8, Arc};
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

use lumo_animation::{AnimCurve, LAAnimator, LACurve};
use lumo_foundation::{current_bar_layout, LumoColors, LumoTheme};

use crate::bar::dropdowns::battery::{draw_battery_dropdown, BatteryInfo};
use crate::bar::dropdowns::brightness::{draw_brightness_dropdown, BrightnessInfo};
use crate::bar::dropdowns::datetime::{draw_datetime_dropdown, DateTimeInfo};
use crate::bar::dropdowns::lumo_menu::draw_lumo_menu;
use crate::bar::dropdowns::wifi::{draw_wifi_dropdown, WifiInfo};
use crate::bar::dropdowns::DropdownActive;
use crate::bar::fonts::{
    draw_text, draw_text_mono, measure_text, measure_text_mono, opaque, rgba_hex,
};
use crate::bar::icons::{
    battery_total_width, draw_battery, draw_brand_dot, draw_brightness_sun, draw_wifi, fill_circle,
};
use crate::bar::password_modal::{draw_password_modal, PasswordModalHits, PasswordModalState};
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
    pub wifi_info: WifiInfo,             // A23
    pub brightness_info: BrightnessInfo, // L5
    pub datetime_info: DateTimeInfo,     // A24
    /// A27: indice do item em hover no menu Lumo (usize::MAX = nenhum).
    pub lumo_menu_hover_idx: usize,
    // C5: appmenu pills (app em foco).
    pub appmenu_items: Vec<crate::bar::appmenu::AppMenuItem>,
    pub appmenu_open_idx: Option<usize>,
    pub appmenu_submenu: Vec<crate::bar::appmenu::AppMenuItem>,
    /// W37.6: indice do item em hover no submenu appmenu (usize::MAX = nenhum).
    pub appmenu_submenu_hover_idx: usize,
    // S2: appmenu fallback metadata.
    pub appmenu_app_id: String,
    pub appmenu_title: String,
    pub appmenu_fallback_hover_idx: Option<usize>,
    // B4: fator de escala e opacidade do dropdown ativo (0.0=fechado, 1.0=aberto).
    pub dropdown_scale: f32,
    pub dropdown_alpha: f32,
    // M2: alpha global da bar (1.0 = normal, pisca em 0.7->1.0 no F5).
    pub bar_alpha: f32,
    // A31.3: estado do modal de senha wifi.
    pub password_modal: PasswordModalState,
    /// UX2: pills warning por feature degradada (code -> label).
    pub degraded: std::collections::BTreeMap<String, String>,
    /// UX3: apps em freeze (pid -> app_id). Title bar mostra "(Nao responde)".
    pub frozen: std::collections::BTreeMap<u32, String>,
}

/// Resultado de paint_frame: posicoes calculadas pra hit-test no proximo frame.
#[derive(Default, Clone)]
pub(crate) struct PaintResult {
    pub bat_hit_rect: Option<(f32, f32, f32, f32)>,
    pub wifi_hit_rect: Option<(f32, f32, f32, f32)>, // A23
    pub datetime_hit_rect: Option<(f32, f32, f32, f32)>, // A24
    pub lumo_hit_rect: Option<(f32, f32, f32, f32)>, // A27: brand "Lumo" pill esquerda
    // A26: hit-tests do calendar interativo (so populados quando dropdown=DateTime).
    pub cal_prev_rect: Option<(f32, f32, f32, f32)>,
    pub cal_next_rect: Option<(f32, f32, f32, f32)>,
    pub cal_today_rect: Option<(f32, f32, f32, f32)>,
    /// Cada (day, rect). Day = dia do mes visualizado (1..=31), rect em coords surface.
    pub cal_day_rects: Vec<(u32, (f32, f32, f32, f32))>,
    // S2: appmenu fallback hit-rects pill + dropdown.
    pub appmenu_fallback_rect: Option<(f32, f32, f32, f32)>,
    pub appmenu_fallback_dropdown_rects: Vec<(usize, (f32, f32, f32, f32))>,
    // L5: brightness pill hit-rect.
    pub brightness_hit_rect: Option<(f32, f32, f32, f32)>,
    pub brightness_slider_rect: Option<(f32, f32, f32, f32)>,
    pub brightness_preset_day_rect: Option<(f32, f32, f32, f32)>,
    pub brightness_preset_night_rect: Option<(f32, f32, f32, f32)>,
    // L5: battery dropdown interactive hit-rects.
    pub bat_charge_limit_toggle_rect: Option<(f32, f32, f32, f32)>,
    pub bat_profile_cycle_rect: Option<(f32, f32, f32, f32)>,
    // A31.2: hit-rects do dropdown wifi (so populados quando dropdown=Wifi).
    pub wifi_toggle_rect: Option<(f32, f32, f32, f32)>,
    pub wifi_disconnect_rect: Option<(f32, f32, f32, f32)>,
    pub wifi_connect_rects: Vec<(String, (f32, f32, f32, f32))>,
    pub _last_click_at: Option<Instant>,
    // A31.3: hit-rects do modal de senha wifi.
    pub pwd_confirm_rect: Option<(f32, f32, f32, f32)>,
    pub pwd_cancel_rect: Option<(f32, f32, f32, f32)>,
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
    let layout = current_bar_layout();
    let pill_y = layout.margin_top;
    let pill_h = layout.height as f32 - layout.margin_top * 2.0;
    let pill_pad_x = layout.padding_x;
    let pill_gap = layout.pill_gap;
    let pill_margin = layout.margin_x;
    let pill_radius_ = layout.pill_radius;
    let bat_w_override = layout.find_pill("battery").and_then(|s| s.width);
    let wifi_w_override = layout.find_pill("wifi").and_then(|s| s.width);
    let bright_w_override = layout.find_pill("brightness").and_then(|s| s.width);
    let _dt_w_override = layout.find_pill("datetime").and_then(|s| s.width);
    let _ = PILL_MARGIN_TOP;
    let pill_cy = pill_y + pill_h / 2.0;

    // Cor pill bg: hex + alpha do tema. Mesma cor pra ambas as pills.
    let pill_bg = rgba_hex(palette.pill_bg, palette.pill_bg_alpha);
    let pill_fg = opaque(palette.pill_fg);
    let pill_fg_subtle = rgba_hex(palette.pill_fg, 0xB0); // 70% pra dim sobre pill
    let pill_sep = rgba_hex(palette.pill_sep, palette.pill_sep_alpha);
    let accent = opaque(palette.accent);

    // Topo y do texto dentro da pill (centralizado vertical).
    let text_h = FONT_PILL * 1.2;
    let text_top = pill_y + (pill_h - text_h) / 2.0;
    let text_top = text_top.round();

    // ============================================================
    // PILL ESQUERDA: [dot] Lumo (A30.1 sem workspace numero)
    // ============================================================
    let lumo_w = measure_text("Lumo", FONT_PILL, true);
    let pill_l_content_w = BRAND_DOT_RADIUS * 2.0 + pill_gap + lumo_w;
    let pill_l_w = pill_l_content_w + pill_pad_x * 2.0;
    let pill_l_x = pill_margin;
    let _ = snap.active_ws; // suppress dead-code warn ate A35 workspaces reais

    {
        let mut canvas = pixmap.as_mut();
        draw_pill_bg(&mut canvas, pill_l_x, pill_y, pill_l_w, pill_h, pill_bg, 0);

        let mut cx = pill_l_x + pill_pad_x;
        let lumo_hit_x_start = cx - 4.0;
        // Brand dot accent
        draw_brand_dot(&mut canvas, cx + BRAND_DOT_RADIUS, pill_cy, accent);
        cx += BRAND_DOT_RADIUS * 2.0 + pill_gap;
        // "Lumo" bold
        draw_text(&mut canvas, cx, text_top, "Lumo", FONT_PILL, pill_fg, true);
        let lumo_hit_x_end = cx + lumo_w + 4.0;
        result.lumo_hit_rect = Some((
            lumo_hit_x_start,
            pill_y,
            lumo_hit_x_end - lumo_hit_x_start,
            pill_h,
        ));

        // S2: pill fallback AppName ▾ apos pill Lumo se nao ha dbusmenu items.
        if snap.appmenu_items.is_empty() && !snap.appmenu_app_id.is_empty() {
            // UX3: sufixo " (Nao responde)" se pid em foco esta frozen.
            // Resolve via cache foco -> pid via state.degraded nao serve;
            // usamos snap.frozen direto procurando se algum pid existe.
            // Sem pid em snap fallback, marcamos se freeze set nao vazio
            // (compositor ja garante so emite freeze pro app em foco).
            let freeze_sfx = if !snap.frozen.is_empty() {
                " (Nao responde)"
            } else {
                ""
            };
            let base: String = if !snap.appmenu_title.is_empty() {
                snap.appmenu_title.chars().take(24).collect()
            } else {
                snap.appmenu_app_id.chars().take(24).collect()
            };
            let label = format!("{}{}", base, freeze_sfx);
            let label_w = measure_text(&label, FONT_PILL, false);
            let fb_w = label_w + pill_pad_x * 2.0 + 18.0;
            let fb_x = pill_l_x + pill_l_w + pill_gap;
            draw_pill_bg(&mut canvas, fb_x, pill_y, fb_w, pill_h, pill_bg, 0);
            draw_text(
                &mut canvas,
                fb_x + pill_pad_x,
                text_top,
                &label,
                FONT_PILL,
                pill_fg,
                false,
            );
            draw_text(
                &mut canvas,
                fb_x + pill_pad_x + label_w + 4.0,
                text_top,
                "v",
                FONT_PILL,
                pill_fg,
                false,
            );
            result.appmenu_fallback_rect = Some((fb_x, pill_y, fb_w, pill_h));
        }
    }

    // ============================================================
    // APPMENU PILLS (C5): renderiza items top-level do app em foco
    // como pills individuais ao lado do pill Lumo.
    // 24px altura, Geist 13pt, hover highlight via appmenu_open_idx.
    // ============================================================
    {
        let mut ax = pill_l_x + pill_l_w + pill_gap;
        let mut appmenu_rects: Vec<(usize, (f32, f32, f32, f32))> = Vec::new();
        for (idx, item) in snap.appmenu_items.iter().enumerate() {
            if item.label == "---" {
                ax += 4.0; // separador visual
                continue;
            }
            let label_w = measure_text(&item.label, FONT_PILL, false);
            let pill_w = label_w + pill_pad_x * 2.0;
            let is_open = snap.appmenu_open_idx == Some(idx);
            let bg_color = if is_open {
                rgba_hex(palette.accent, 0xCC)
            } else {
                pill_bg
            };
            let fg_color = if is_open { opaque(0xFFFFFF) } else { pill_fg };
            {
                let mut canvas = pixmap.as_mut();
                draw_pill_bg(&mut canvas, ax, pill_y, pill_w, pill_h, bg_color, 0);
                draw_text(
                    &mut canvas,
                    ax + pill_pad_x,
                    text_top,
                    &item.label,
                    FONT_PILL,
                    fg_color,
                    false,
                );
            }
            appmenu_rects.push((idx, (ax, pill_y, pill_w, pill_h)));
            ax += pill_w + pill_gap / 2.0;
        }
        result.appmenu_pill_rects = appmenu_rects;
    }

    // ============================================================
    // PILL CENTRO: [Focused Window Title] (W34.11)
    // ============================================================
    if !snap.appmenu_title.is_empty() {
        let title_w = measure_text(&snap.appmenu_title, FONT_PILL, true);
        let pill_c_w = pill_pad_x * 2.0 + title_w;
        let pill_c_x = (snap.width as f32 - pill_c_w) / 2.0;

        let mut canvas = pixmap.as_mut();
        draw_pill_bg(&mut canvas, pill_c_x, pill_y, pill_c_w, pill_h, pill_bg, 0);

        draw_text(
            &mut canvas,
            pill_c_x + pill_pad_x,
            text_top,
            &snap.appmenu_title,
            FONT_PILL,
            pill_fg,
            true,
        );
    }

    // ============================================================
    // PILL DEGRADED (UX2): pill amber a esquerda da pill direita
    // quando ha codigos degraded ativos. Texto: label se 1, "N issues" se 2+.
    // ============================================================
    let degraded_text =
        crate::bar::status_pills::compute_degraded_text(&snap.degraded);
    let degraded_pill_w_calc: Option<f32> = degraded_text.as_deref().map(|t| {
        let w = measure_text(t, FONT_PILL, false);
        w + pill_pad_x * 2.0 + 8.0
    });

    // ============================================================
    // PILL DIREITA: [wifi] [bat icone] HH:MM (A19.8: removido texto %)
    // ============================================================
    let bat_icon_w = bat_w_override.unwrap_or_else(battery_total_width);
    let clock_s = format!("{:02}:{:02}", snap.clock_hh, snap.clock_mm);
    // A29: clock = Geist Mono (digito tabular).
    let clock_w = measure_text_mono(&clock_s, FONT_PILL, false);

    let date_w = measure_text(&snap.date_str, FONT_DATE, false);
    let brightness_icon_w: f32 = bright_w_override.unwrap_or(14.0);
    let wifi_icon_w = wifi_w_override.unwrap_or(WIFI_SIZE);
    let pill_r_content_w = bat_icon_w
        + pill_gap
        + wifi_icon_w
        + pill_gap
        + brightness_icon_w
        + pill_gap
        + date_w
        + 8.0
        + clock_w;
    let pill_r_w = pill_r_content_w + pill_pad_x * 2.0;
    let pill_r_x = snap.width as f32 - pill_margin - pill_r_w;

    // UX2: render pill degraded amber a esquerda da pill direita.
    if let (Some(text), Some(deg_w)) = (degraded_text.as_deref(), degraded_pill_w_calc) {
        let deg_x = pill_r_x - pill_gap - deg_w;
        // Amber warning bg: #FFA500 com alpha similar pill_bg.
        let amber_bg = rgba_hex(0xFFA500, palette.pill_bg_alpha);
        let amber_fg = opaque(0x1A1A1A); // texto escuro pra contraste em amber
        let mut canvas = pixmap.as_mut();
        draw_pill_bg(&mut canvas, deg_x, pill_y, deg_w, pill_h, amber_bg, 0);
        // Dot warning antes texto.
        draw_brand_dot(&mut canvas, deg_x + pill_pad_x + BRAND_DOT_RADIUS, pill_cy, opaque(0xCC4400));
        draw_text(
            &mut canvas,
            deg_x + pill_pad_x + BRAND_DOT_RADIUS * 2.0 + 4.0,
            text_top,
            text,
            FONT_PILL,
            amber_fg,
            true,
        );
    }

    {
        let mut canvas = pixmap.as_mut();
        draw_pill_bg(&mut canvas, pill_r_x, pill_y, pill_r_w, pill_h, pill_bg, 0);
        let mut cx = pill_r_x + pill_pad_x;
        // A19.10: ordem bat -> wifi -> data -> hora (Mac-style)
        let bat_x_start = cx;
        // A31 fix: A30 alterou signature de draw_battery pra incluir flag charging.
        let bat_charging = snap.battery_info.status == "Charging";
        draw_battery(
            &mut canvas,
            cx,
            pill_cy - BAT_BODY_H / 2.0,
            snap.battery_pct,
            bat_charging,
            pill_fg,
            accent,
        );
        // A20.13: hit area = SO o icone bateria (era pill inteira A20.4)
        result.bat_hit_rect = Some((bat_x_start - 4.0, pill_y, bat_icon_w + 8.0, pill_h));
        cx += bat_icon_w + pill_gap;
        // A23: salvar wifi_hit_rect igual bat.
        let wifi_x_start = cx;
        draw_wifi(
            &mut canvas,
            cx,
            pill_cy - wifi_icon_w / 2.0,
            snap.wifi_on,
            pill_fg,
            pill_fg_subtle,
        );
        result.wifi_hit_rect = Some((wifi_x_start - 4.0, pill_y, wifi_icon_w + 8.0, pill_h));
        cx += wifi_icon_w + pill_gap;
        // L5: brightness pill (sun icon + pct).
        let brightness_x_start = cx;
        draw_brightness_sun(
            &mut canvas,
            cx + 7.0,
            pill_cy,
            snap.brightness_info.pct,
            pill_fg,
            opaque(palette.accent),
        );
        result.brightness_hit_rect = Some((
            brightness_x_start - 4.0,
            pill_y,
            brightness_icon_w + 8.0,
            pill_h,
        ));
        cx += brightness_icon_w + pill_gap;
        // A24: hit area cobre data + hora juntas (mesmo dropdown calendario).
        let datetime_x_start = cx;
        draw_text(
            &mut canvas,
            cx,
            text_top,
            &snap.date_str,
            FONT_DATE,
            pill_fg,
            false,
        );
        cx += date_w + 8.0;
        // A29: clock HH:MM = Geist Mono.
        draw_text_mono(
            &mut canvas,
            cx,
            text_top,
            &clock_s,
            FONT_PILL,
            pill_fg,
            false,
        );
        let datetime_end = cx + clock_w;
        result.datetime_hit_rect = Some((
            datetime_x_start - 4.0,
            pill_y,
            (datetime_end - datetime_x_start) + 8.0,
            pill_h,
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
        DropdownActive::AppFallback => {
            // UX unificada: usa crate::menu::draw_menu_dyn igual submenu
            // appmenu nativo + ctx menu desktop/files. Hover accent solido,
            // radius 10, fonte Inter — mesma identidade visual.
            if let Some((rx, ry, rw, rh)) = result.appmenu_fallback_rect {
                use crate::menu::{draw_menu_dyn, hit_test_dyn, menu_height_dyn, DynMenuItem};
                let items = [
                    DynMenuItem::action("Sobre"),
                    DynMenuItem::action("Versao"),
                    DynMenuItem::action("Ajuda"),
                    DynMenuItem::separator(),
                    DynMenuItem::action("Fechar"),
                ];
                let dropdown_w = 200.0f32;
                let dropdown_h = menu_height_dyn(&items);
                let want_x = rx;
                let max_x = snap.width as f32 - pill_margin - dropdown_w;
                let dropdown_x = want_x.max(pill_margin).min(max_x.max(pill_margin));
                let dropdown_y = ry + rh + DROPDOWN_GAP;
                if let Some(mut sub) = Pixmap::new(dropdown_w as u32, dropdown_h as u32) {
                    {
                        let mut canvas = sub.as_mut();
                        let hover = snap.appmenu_fallback_hover_idx.unwrap_or(usize::MAX);
                        draw_menu_dyn(
                            &mut canvas,
                            0.0,
                            0.0,
                            dropdown_w,
                            &items,
                            hover,
                            palette,
                            |c, x, y, w, h, r, color| {
                                crate::bar::icons::fill_rrect(c, x, y, w, h, r, color);
                            },
                            |c, x, y, label, size, color| {
                                draw_text(c, x, y, label, size, color, false);
                            },
                        );
                    }
                    // Hit-rects via hit_test_dyn pattern — calc rect por item.
                    let mut fb_rects: Vec<(usize, (f32, f32, f32, f32))> = Vec::new();
                    for i in 0..items.len() {
                        if !items[i].is_clickable() {
                            continue;
                        }
                        // Bbox approximate: hit_test_dyn da idx via py,
                        // mas pra rect preciso de offset. Itera y manual.
                        if let Some(_) = hit_test_dyn(
                            &items,
                            0.0,
                            0.0,
                            dropdown_w,
                            dropdown_w / 2.0,
                            // probe y center do item i
                            4.0 + 28.0 * i as f32 + 14.0,
                        ) {
                            fb_rects.push((
                                i,
                                (
                                    dropdown_x,
                                    dropdown_y + 4.0 + 28.0 * i as f32,
                                    dropdown_w,
                                    28.0,
                                ),
                            ));
                        }
                    }
                    result.appmenu_fallback_dropdown_rects = fb_rects;
                    composite_dropdown(
                        pixmap,
                        &sub,
                        dropdown_x,
                        dropdown_y,
                        snap.dropdown_scale,
                        snap.dropdown_alpha,
                    );
                }
            }
        }
        DropdownActive::Battery => {
            if let Some((rx, ry, rw, rh)) = result.bat_hit_rect {
                let want_x = rx + rw / 2.0 - DROPDOWN_W / 2.0;
                let max_x = snap.width as f32 - pill_margin - DROPDOWN_W;
                let dropdown_x = want_x.max(pill_margin).min(max_x.max(pill_margin));
                let dropdown_y = ry + rh + DROPDOWN_GAP;
                if let Some(mut sub) = Pixmap::new(DROPDOWN_W as u32, DROPDOWN_H as u32) {
                    let bat_hits = {
                        let mut canvas = sub.as_mut();
                        draw_battery_dropdown(
                            &mut canvas,
                            0.0,
                            0.0,
                            DROPDOWN_W,
                            DROPDOWN_H,
                            palette,
                            &snap.battery_info,
                        )
                    };
                    result.bat_charge_limit_toggle_rect = bat_hits
                        .charge_limit_toggle_rect
                        .map(|(bx, by, bw, bh)| (bx + dropdown_x, by + dropdown_y, bw, bh));
                    result.bat_profile_cycle_rect = bat_hits
                        .profile_cycle_rect
                        .map(|(bx, by, bw, bh)| (bx + dropdown_x, by + dropdown_y, bw, bh));
                    composite_dropdown(
                        pixmap,
                        &sub,
                        dropdown_x,
                        dropdown_y,
                        snap.dropdown_scale,
                        snap.dropdown_alpha,
                    );
                }
            }
        }
        DropdownActive::Brightness => {
            if let Some((rx, ry, rw, rh)) = result.brightness_hit_rect {
                let want_x = rx + rw / 2.0 - DROPDOWN_BRIGHTNESS_W / 2.0;
                let max_x = snap.width as f32 - pill_margin - DROPDOWN_BRIGHTNESS_W;
                let dropdown_x = want_x.max(pill_margin).min(max_x.max(pill_margin));
                let dropdown_y = ry + rh + DROPDOWN_GAP;
                if let Some(mut sub) =
                    Pixmap::new(DROPDOWN_BRIGHTNESS_W as u32, DROPDOWN_BRIGHTNESS_H as u32)
                {
                    let br_hits = {
                        let mut canvas = sub.as_mut();
                        draw_brightness_dropdown(
                            &mut canvas,
                            0.0,
                            0.0,
                            DROPDOWN_BRIGHTNESS_W,
                            DROPDOWN_BRIGHTNESS_H,
                            palette,
                            &snap.brightness_info,
                        )
                    };
                    result.brightness_slider_rect = br_hits
                        .slider_rect
                        .map(|(bx, by, bw, bh)| (bx + dropdown_x, by + dropdown_y, bw, bh));
                    result.brightness_preset_day_rect = br_hits
                        .preset_day_rect
                        .map(|(bx, by, bw, bh)| (bx + dropdown_x, by + dropdown_y, bw, bh));
                    result.brightness_preset_night_rect = br_hits
                        .preset_night_rect
                        .map(|(bx, by, bw, bh)| (bx + dropdown_x, by + dropdown_y, bw, bh));
                    composite_dropdown(
                        pixmap,
                        &sub,
                        dropdown_x,
                        dropdown_y,
                        snap.dropdown_scale,
                        snap.dropdown_alpha,
                    );
                }
            }
        }
        DropdownActive::Wifi => {
            if let Some((rx, ry, rw, rh)) = result.wifi_hit_rect {
                let want_x = rx + rw / 2.0 - DROPDOWN_WIFI_W / 2.0;
                let max_x = snap.width as f32 - pill_margin - DROPDOWN_WIFI_W;
                let dropdown_x = want_x.max(pill_margin).min(max_x.max(pill_margin));
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
                    result.wifi_toggle_rect = hits
                        .toggle_rect
                        .map(|(x, y, w, h)| (x + dropdown_x, y + dropdown_y, w, h));
                    result.wifi_disconnect_rect = hits
                        .disconnect_rect
                        .map(|(x, y, w, h)| (x + dropdown_x, y + dropdown_y, w, h));
                    result.wifi_connect_rects = hits
                        .connect_rects
                        .iter()
                        .map(|(s, (x, y, w, h))| {
                            (s.clone(), (x + dropdown_x, y + dropdown_y, *w, *h))
                        })
                        .collect();
                    composite_dropdown(
                        pixmap,
                        &sub,
                        dropdown_x,
                        dropdown_y,
                        snap.dropdown_scale,
                        snap.dropdown_alpha,
                    );
                }
            }
        }
        DropdownActive::DateTime => {
            if let Some((rx, ry, rw, rh)) = result.datetime_hit_rect {
                let want_x = rx + rw / 2.0 - DROPDOWN_DATETIME_W / 2.0;
                let max_x = snap.width as f32 - pill_margin - DROPDOWN_DATETIME_W;
                let dropdown_x = want_x.max(pill_margin).min(max_x.max(pill_margin));
                let dropdown_y = ry + rh + DROPDOWN_GAP;
                if let Some(mut sub) =
                    Pixmap::new(DROPDOWN_DATETIME_W as u32, DROPDOWN_DATETIME_H as u32)
                {
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
                    result.cal_prev_rect = hits
                        .prev_rect
                        .map(|(x, y, w, h)| (x + dropdown_x, y + dropdown_y, w, h));
                    result.cal_next_rect = hits
                        .next_rect
                        .map(|(x, y, w, h)| (x + dropdown_x, y + dropdown_y, w, h));
                    result.cal_today_rect = hits
                        .today_rect
                        .map(|(x, y, w, h)| (x + dropdown_x, y + dropdown_y, w, h));
                    result.cal_day_rects = hits
                        .day_rects
                        .iter()
                        .map(|(d, (x, y, w, h))| (*d, (x + dropdown_x, y + dropdown_y, *w, *h)))
                        .collect();
                    composite_dropdown(
                        pixmap,
                        &sub,
                        dropdown_x,
                        dropdown_y,
                        snap.dropdown_scale,
                        snap.dropdown_alpha,
                    );
                }
            }
        }
        DropdownActive::LumoMenu => {
            if let Some((rx, ry, _rw, rh)) = result.lumo_hit_rect {
                use crate::menu;
                let menu_w = MENU_LUMO_W as u32;
                let menu_h_px = menu::menu_height(MENU_LUMO_ITEMS) as u32;
                let dropdown_x = rx.max(pill_margin);
                let dropdown_y = ry + rh + DROPDOWN_GAP;
                if let Some(mut sub) = Pixmap::new(menu_w, menu_h_px) {
                    {
                        let mut canvas = sub.as_mut();
                        draw_lumo_menu(&mut canvas, 0.0, 0.0, palette, snap.lumo_menu_hover_idx);
                    }
                    composite_dropdown(
                        pixmap,
                        &sub,
                        dropdown_x,
                        dropdown_y,
                        snap.dropdown_scale,
                        snap.dropdown_alpha,
                    );
                }
            }
        }
        DropdownActive::None => {}
    }

    // ============================================================
    // APPMENU SUBMENU (C5): dropdown do item top-level aberto.
    // Renderizado diretamente (sem DropdownActive -- submenu appmenu
    // e independente dos dropdowns de sistema).
    // ============================================================
    if let Some(open_idx) = snap.appmenu_open_idx {
        if let Some(&(_, (rx, ry, rw, rh))) = result
            .appmenu_pill_rects
            .iter()
            .find(|(i, _)| *i == open_idx)
        {
            if !snap.appmenu_submenu.is_empty() {
                // W37.6: usa menu::draw_menu_dyn pra identidade visual unica
                // com bar dropdowns + desktop ctx menus (palette, radius 14,
                // font 13, hover accent solid).
                use crate::menu;
                let dyn_items: Vec<menu::DynMenuItem<'_>> = snap
                    .appmenu_submenu
                    .iter()
                    .map(|it| {
                        if it.label == "---" {
                            menu::DynMenuItem::separator()
                        } else {
                            menu::DynMenuItem::action(&it.label)
                        }
                    })
                    .collect();

                let max_label_w = snap
                    .appmenu_submenu
                    .iter()
                    .filter(|it| it.label != "---")
                    .map(|it| measure_text(&it.label, menu::FONT_MENU, false))
                    .fold(0.0f32, f32::max);
                let sub_w = (max_label_w + menu::MENU_PAD_X * 2.0).max(menu::MENU_W_DESKTOP * 0.8);
                let sub_h = menu::menu_height_dyn(&dyn_items);
                let sub_x = rx.max(pill_margin);
                let sub_y = ry + rh + DROPDOWN_GAP;

                let _ = rw; // suprime warning unused legado.

                if let Some(mut sub) = tiny_skia::Pixmap::new(sub_w as u32, sub_h as u32) {
                    use crate::bar::icons::fill_rrect;
                    {
                        let mut canvas = sub.as_mut();
                        menu::draw_menu_dyn(
                            &mut canvas,
                            0.0,
                            0.0,
                            sub_w,
                            &dyn_items,
                            snap.appmenu_submenu_hover_idx,
                            &palette,
                            fill_rrect,
                            |c, x, y, label, size, color| {
                                draw_text(c, x, y, label, size, color, false);
                            },
                        );
                    }
                    // Hit rects para handler de click (precisa coords absolutas).
                    let mut submenu_rects: Vec<(usize, (f32, f32, f32, f32))> = Vec::new();
                    let mut item_y = menu::MENU_PAD_Y;
                    for (sidx, item) in snap.appmenu_submenu.iter().enumerate() {
                        if item.label == "---" {
                            item_y += menu::MENU_SEPARATOR_BLOCK_H;
                            continue;
                        }
                        submenu_rects.push((
                            sidx,
                            (
                                sub_x + menu::MENU_ROW_HOVER_INSET,
                                sub_y + item_y,
                                sub_w - menu::MENU_ROW_HOVER_INSET * 2.0,
                                menu::MENU_ROW_H,
                            ),
                        ));
                        item_y += menu::MENU_ROW_H;
                    }
                    result.appmenu_submenu_rects = submenu_rects;
                    composite_dropdown(pixmap, &sub, sub_x, sub_y, 1.0, 1.0);
                }
            }
        }
    }

    // A31.3: overlay modal de senha (renderizado por cima de tudo).
    if snap.password_modal.active {
        let mut canvas = pixmap.as_mut();
        let modal_hits = draw_password_modal(
            &mut canvas,
            snap.width as f32,
            snap.height as f32,
            palette,
            &snap.password_modal,
        );
        result.pwd_confirm_rect = modal_hits.confirm_rect;
        result.pwd_cancel_rect = modal_hits.cancel_rect;
    }

    // Suppress unused warns nos campos do snapshot (theme so usado pra debug log).
    let _ = (snap.theme, h, pill_radius_);
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
fn composite_dropdown(dst: &mut Pixmap, src: &Pixmap, x: f32, y: f32, scale: f32, alpha: f32) {
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
                dst_data[dst_off..dst_off + row_bytes]
                    .copy_from_slice(&src_data[src_off..src_off + row_bytes]);
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
    /// W18.fix: mantem Region viva ate proximo update -- drop antes do commit
    /// destruia wl_region e server lia input_region=None (bar capturava tudo).
    pub current_input_region: Option<smithay_client_toolkit::compositor::Region>,
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
    // A31.3: handle de teclado (adquirido quando seat reporta Capability::Keyboard).
    pub keyboard:
        Option<smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard>,
    pub pointer_x: f32,
    pub pointer_pos: Option<(f64, f64)>,
    pub bat_hit_rect: Option<(f32, f32, f32, f32)>,
    pub wifi_hit_rect: Option<(f32, f32, f32, f32)>, // A23
    pub datetime_hit_rect: Option<(f32, f32, f32, f32)>, // A24
    pub lumo_hit_rect: Option<(f32, f32, f32, f32)>, // A27
    pub lumo_menu_hover_idx: usize,                  // A27
    // A26: hit-tests calendar interativo.
    pub cal_prev_rect: Option<(f32, f32, f32, f32)>,
    pub cal_next_rect: Option<(f32, f32, f32, f32)>,
    pub cal_today_rect: Option<(f32, f32, f32, f32)>,
    pub cal_day_rects: Vec<(u32, (f32, f32, f32, f32))>,
    // L5: battery dropdown interactive hit-rects.
    pub bat_charge_limit_toggle_rect: Option<(f32, f32, f32, f32)>,
    pub bat_profile_cycle_rect: Option<(f32, f32, f32, f32)>,
    // L5: brightness.
    pub brightness_info: BrightnessInfo,
    pub brightness_hit_rect: Option<(f32, f32, f32, f32)>,
    pub brightness_slider_rect: Option<(f32, f32, f32, f32)>,
    pub brightness_preset_day_rect: Option<(f32, f32, f32, f32)>,
    pub brightness_preset_night_rect: Option<(f32, f32, f32, f32)>,
    // Q4: drag brilho — segurar e arrastar sobre pill.
    pub brightness_dragging: bool,
    pub brightness_drag_last_y: f32,
    pub brightness_dragging_slider: bool,
    pub dropdown_rect: Option<(f32, f32, f32, f32)>,
    pub _dropdown_h_final: f32,
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
    /// W37.6: hover idx no submenu appmenu.
    pub appmenu_submenu_hover_idx: usize,
    pub appmenu_pill_rects: Vec<(usize, (f32, f32, f32, f32))>,
    pub appmenu_submenu_rects: Vec<(usize, (f32, f32, f32, f32))>,
    // S2: appmenu fallback mirror.
    pub appmenu_fallback_rect: Option<(f32, f32, f32, f32)>,
    pub appmenu_fallback_dropdown_rects: Vec<(usize, (f32, f32, f32, f32))>,
    pub appmenu_fallback_hover_idx: Option<usize>,
    // C5.1: handle compartilhado com thread Registrar DBus.
    pub _registrar_handle: crate::bar::registrar::RegistrarHandle,
    pub ipc_stream: Option<UnixStream>,
    pub ipc_rx_buf: Vec<u8>,
    // IPC reconnect backoff: None = not pending; Some(t) = retry at t.
    pub _ipc_reconnect_at: Option<std::time::Instant>,
    pub _ipc_reconnect_delay: std::time::Duration,
    pub theme: LumoTheme,
    pub palette: LumoColors,
    // B4: animadores de abertura/fechamento de dropdown (scale 0.85->1.0, alpha 0->1).
    pub dropdown_scale_anim: LAAnimator<f32>,
    pub dropdown_alpha_anim: LAAnimator<f32>,
    // B4: true quando uma animacao de fechamento esta em andamento.
    pub dropdown_closing: bool,
    // B4: ultimo dropdown que estava aberto (para fechar com animacao correta).
    pub dropdown_closing_which: crate::bar::dropdowns::DropdownActive,
    // M2: animacao de fade no F5 (bar_alpha 0.7->1.0 em 250ms).
    pub refresh_anim: LAAnimator<f32>,
    pub refresh_animating: bool,
    // A31.3: modal de senha wifi.
    pub password_modal: PasswordModalState,
    // A31.3: hit-rects do modal (atualizados pelo paint_frame).
    pub pwd_confirm_rect: Option<(f32, f32, f32, f32)>,
    pub pwd_cancel_rect: Option<(f32, f32, f32, f32)>,
    // A31.3: receiver do thread nm_connect (None = sem conexao pendente).
    pub nm_connect_rx: Option<std::sync::mpsc::Receiver<crate::bar::system_info::NmConnectResult>>,
    /// UX2: pills warning. code -> label.
    pub degraded: std::collections::BTreeMap<String, String>,
    /// UX3: apps em freeze. pid -> app_id.
    pub frozen: std::collections::BTreeMap<u32, String>,
}
