//! bar/dropdowns/datetime.rs - Dropdown calendario + relogio + struct
//! DateTimeInfo + helpers PT-BR pra weekday/month.
//!
//! Layout (DROPDOWN_DATETIME_W=280, _H=288):
//!   pad 14 top
//!   linha 1: "weekday_full, day de month_full" 14px bold
//!   linha 2: "HH:MM:SS" 22px (FONT_DROPDOWN_CLOCK) realtime
//!   separator linha 1px
//!   header navegacao: [<] mes ano [>]
//!   header weekdays "D S T Q Q S S" 12px subtle, 7 colunas uniform
//!   grid 6 linhas x 7 colunas, dia atual pill emerald solido
//!   footer botao Hoje
//!
//! Memory feedback_zero_neon_glow: pill emerald solido (sem glow).

use lumo_foundation::LumoColors;
use tiny_skia::{Paint, PixmapMut, Rect, Transform};

use crate::bar::fonts::{
    draw_text, draw_text_mono, measure_text, measure_text_mono, opaque, rgba_hex,
};
use crate::bar::icons::fill_rrect;
use crate::bar::tokens::*;

// ============================================================
// DateTimeInfo (A24) - calendario + hora detalhada.
// ============================================================

#[derive(Clone)]
pub struct DateTimeInfo {
    pub weekday_full: String,              // "domingo"
    pub day: u32,                          // 17 (today)
    pub month_full: String,                // "maio" (today)
    pub year: i32,                         // 2026 (today)
    pub hour: u8,                          // 17
    pub minute: u8,                        // 50
    pub second: u8,                        // 32
    pub month_grid: Vec<Vec<Option<u32>>>, // 6 weeks x 7 days, None = padding
    pub today_day: u32,
    pub today_month: u32, // A26: pra destacar today so quando viewed = today month/year
    pub today_year: i32,  // A26
    // A26: mes/ano visualizado no calendar (pode != today se user navegou).
    pub viewed_year: i32,
    pub viewed_month: u32,
    pub viewed_month_full: String,
    pub selected_day: Option<u32>,
}

impl Default for DateTimeInfo {
    fn default() -> Self {
        DateTimeInfo {
            weekday_full: String::new(),
            day: 1,
            month_full: String::new(),
            year: 2026,
            hour: 0,
            minute: 0,
            second: 0,
            month_grid: vec![vec![None; 7]; 6],
            today_day: 1,
            today_month: 1,
            today_year: 2026,
            viewed_year: 2026,
            viewed_month: 1,
            viewed_month_full: String::new(),
            selected_day: None,
        }
    }
}

pub fn weekday_full_pt(w: chrono::Weekday) -> &'static str {
    use chrono::Weekday::*;
    match w {
        Mon => "segunda-feira",
        Tue => "terca-feira",
        Wed => "quarta-feira",
        Thu => "quinta-feira",
        Fri => "sexta-feira",
        Sat => "sabado",
        Sun => "domingo",
    }
}

pub fn month_full_pt(m: u32) -> &'static str {
    match m {
        1 => "janeiro",
        2 => "fevereiro",
        3 => "marco",
        4 => "abril",
        5 => "maio",
        6 => "junho",
        7 => "julho",
        8 => "agosto",
        9 => "setembro",
        10 => "outubro",
        11 => "novembro",
        12 => "dezembro",
        _ => "?",
    }
}

// ============================================================
// draw_datetime_dropdown (A24+A26).
// ============================================================

/// A26: hit-tests retornados pelo draw_datetime_dropdown pra pointer_frame.
#[derive(Default, Clone)]
pub struct DateTimeHits {
    pub prev_rect: Option<(f32, f32, f32, f32)>,
    pub next_rect: Option<(f32, f32, f32, f32)>,
    pub today_rect: Option<(f32, f32, f32, f32)>,
    pub day_rects: Vec<(u32, (f32, f32, f32, f32))>,
}

pub fn draw_datetime_dropdown(
    canvas: &mut PixmapMut,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    palette: &LumoColors,
    info: &DateTimeInfo,
) -> DateTimeHits {
    let bg = rgba_hex(palette.pill_bg, palette.pill_bg_alpha);
    let fg = opaque(palette.pill_fg);
    let fg_subtle = rgba_hex(palette.pill_fg, 0xA0);
    let sep_color = rgba_hex(palette.pill_sep, palette.pill_sep_alpha);
    let accent = opaque(palette.accent);
    let accent_subtle = rgba_hex(palette.accent_subtle, 0x60);
    let on_accent = opaque(0xFFFFFF);

    let mut hits = DateTimeHits::default();

    // Background rounded rect.
    fill_rrect(canvas, x, y, w, h, PILL_RADIUS, bg);

    let cx = x + DROPDOWN_PAD;
    let mut cy = y + DROPDOWN_PAD;

    // Linha 1: weekday + dia + mes (bold) - sempre today (info pessoal).
    let title = format!("{}, {} de {}", info.weekday_full, info.day, info.month_full);
    draw_text(canvas, cx, cy, &title, FONT_DROPDOWN_TITLE, fg, true);
    cy += FONT_DROPDOWN_TITLE * 1.5;

    // Linha 2: HH:MM:SS grande. A29: Geist Mono (digito tabular).
    let clock = format!("{:02}:{:02}:{:02}", info.hour, info.minute, info.second);
    draw_text_mono(canvas, cx, cy, &clock, FONT_DROPDOWN_CLOCK, fg, false);
    cy += FONT_DROPDOWN_CLOCK * 1.3;

    // Separator 1px.
    if let Some(rect) = Rect::from_xywh(x + DROPDOWN_PAD, cy.round(), w - DROPDOWN_PAD * 2.0, 1.0) {
        let mut p = Paint::default();
        p.set_color(sep_color);
        p.anti_alias = false;
        canvas.fill_rect(rect, &p, Transform::identity(), None);
    }
    cy += 8.0;

    // A26: Header navegacao "[<] mes ano [>]".
    let header_y = cy;
    let header_label = format!("{} {}", info.viewed_month_full, info.viewed_year);
    let header_text_w = measure_text(&header_label, FONT_CAL_NAV, true);

    // Botao prev: alinhado a esquerda do grid.
    let prev_x = x + DROPDOWN_PAD;
    let prev_y = header_y + (CAL_HEADER_H - CAL_NAV_BTN_H) / 2.0;
    fill_rrect(
        canvas,
        prev_x,
        prev_y,
        CAL_NAV_BTN_W,
        CAL_NAV_BTN_H,
        CAL_NAV_BTN_RADIUS,
        accent_subtle,
    );
    let arrow_l = "<";
    let arrow_l_w = measure_text(arrow_l, FONT_CAL_NAV, true);
    let arrow_l_x = prev_x + (CAL_NAV_BTN_W - arrow_l_w) / 2.0;
    let arrow_l_y = prev_y + (CAL_NAV_BTN_H - FONT_CAL_NAV) / 2.0 - 1.0;
    draw_text(
        canvas,
        arrow_l_x,
        arrow_l_y,
        arrow_l,
        FONT_CAL_NAV,
        fg,
        true,
    );
    hits.prev_rect = Some((prev_x, prev_y, CAL_NAV_BTN_W, CAL_NAV_BTN_H));

    // Botao next: alinhado a direita.
    let next_x = x + w - DROPDOWN_PAD - CAL_NAV_BTN_W;
    let next_y = prev_y;
    fill_rrect(
        canvas,
        next_x,
        next_y,
        CAL_NAV_BTN_W,
        CAL_NAV_BTN_H,
        CAL_NAV_BTN_RADIUS,
        accent_subtle,
    );
    let arrow_r = ">";
    let arrow_r_w = measure_text(arrow_r, FONT_CAL_NAV, true);
    let arrow_r_x = next_x + (CAL_NAV_BTN_W - arrow_r_w) / 2.0;
    let arrow_r_y = next_y + (CAL_NAV_BTN_H - FONT_CAL_NAV) / 2.0 - 1.0;
    draw_text(
        canvas,
        arrow_r_x,
        arrow_r_y,
        arrow_r,
        FONT_CAL_NAV,
        fg,
        true,
    );
    hits.next_rect = Some((next_x, next_y, CAL_NAV_BTN_W, CAL_NAV_BTN_H));

    // Label centralizado entre os botoes.
    let header_label_x = x + (w - header_text_w) / 2.0;
    let header_label_y = header_y + (CAL_HEADER_H - FONT_CAL_NAV) / 2.0 - 1.0;
    draw_text(
        canvas,
        header_label_x,
        header_label_y,
        &header_label,
        FONT_CAL_NAV,
        fg,
        true,
    );
    cy += CAL_HEADER_H + 2.0;

    // Grid horizontal centralizado em w. 7 colunas * DATETIME_CELL_W.
    let grid_total_w = DATETIME_CELL_W * 7.0;
    let grid_x = x + (w - grid_total_w) / 2.0;

    // Header weekdays: D S T Q Q S S (col 0 = Dom).
    let weekday_labels = ["D", "S", "T", "Q", "Q", "S", "S"];
    for (i, label) in weekday_labels.iter().enumerate() {
        let cell_x = grid_x + DATETIME_CELL_W * i as f32;
        let label_w = measure_text(label, FONT_DROPDOWN_CALENDAR, true);
        let lx = cell_x + (DATETIME_CELL_W - label_w) / 2.0;
        draw_text(
            canvas,
            lx,
            cy,
            label,
            FONT_DROPDOWN_CALENDAR,
            fg_subtle,
            true,
        );
    }
    cy += DATETIME_CELL_H;

    // Today destacado SO quando viewed_month/year == today_month/year.
    let viewing_today_month =
        info.viewed_year == info.today_year && info.viewed_month == info.today_month;

    // Grid 6x7 dias.
    for week in 0..6 {
        for col in 0..7 {
            if let Some(day) = info.month_grid[week][col] {
                let cell_x = grid_x + DATETIME_CELL_W * col as f32;
                let cell_y = cy + DATETIME_CELL_H * week as f32;
                let is_today = viewing_today_month && day == info.today_day;
                let is_selected = info.selected_day == Some(day);

                let day_str = day.to_string();
                // A29: digito calendario = Geist Mono (tabular figures).
                let day_w = measure_text_mono(&day_str, FONT_DROPDOWN_CALENDAR, is_today);
                let dx = cell_x + (DATETIME_CELL_W - day_w) / 2.0;
                let dy = cell_y + (DATETIME_CELL_H - FONT_DROPDOWN_CALENDAR) / 2.0 - 1.0;

                // A26: hit-rect celula inteira (alvo de click).
                hits.day_rects
                    .push((day, (cell_x, cell_y, DATETIME_CELL_W, DATETIME_CELL_H)));

                if is_today {
                    let pill_w = 22.0;
                    let pill_h = 18.0;
                    let pxp = cell_x + (DATETIME_CELL_W - pill_w) / 2.0;
                    let pyp = cell_y + (DATETIME_CELL_H - pill_h) / 2.0;
                    fill_rrect(canvas, pxp, pyp, pill_w, pill_h, 9.0, accent);
                    draw_text_mono(
                        canvas,
                        dx,
                        dy,
                        &day_str,
                        FONT_DROPDOWN_CALENDAR,
                        on_accent,
                        true,
                    );
                } else if is_selected {
                    let pill_w = 22.0;
                    let pill_h = 18.0;
                    let pxp = cell_x + (DATETIME_CELL_W - pill_w) / 2.0;
                    let pyp = cell_y + (DATETIME_CELL_H - pill_h) / 2.0;
                    fill_rrect(canvas, pxp, pyp, pill_w, pill_h, 9.0, accent_subtle);
                    draw_text_mono(canvas, dx, dy, &day_str, FONT_DROPDOWN_CALENDAR, fg, true);
                } else {
                    draw_text_mono(canvas, dx, dy, &day_str, FONT_DROPDOWN_CALENDAR, fg, false);
                }
            }
        }
    }
    cy += DATETIME_CELL_H * 6.0;

    // A26: footer com botao Hoje centralizado.
    let footer_y = y + h - CAL_FOOTER_H;
    let today_label = "Hoje";
    let today_x = x + (w - CAL_TODAY_BTN_W) / 2.0;
    let today_y = footer_y + (CAL_FOOTER_H - CAL_TODAY_BTN_H) / 2.0;
    fill_rrect(
        canvas,
        today_x,
        today_y,
        CAL_TODAY_BTN_W,
        CAL_TODAY_BTN_H,
        CAL_NAV_BTN_RADIUS,
        accent_subtle,
    );
    let today_w_text = measure_text(today_label, FONT_CAL_NAV, true);
    let today_label_x = today_x + (CAL_TODAY_BTN_W - today_w_text) / 2.0;
    let today_label_y = today_y + (CAL_TODAY_BTN_H - FONT_CAL_NAV) / 2.0 - 1.0;
    draw_text(
        canvas,
        today_label_x,
        today_label_y,
        today_label,
        FONT_CAL_NAV,
        fg,
        true,
    );
    hits.today_rect = Some((today_x, today_y, CAL_TODAY_BTN_W, CAL_TODAY_BTN_H));

    let _ = cy;
    hits
}
