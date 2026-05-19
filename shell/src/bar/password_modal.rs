//! bar/password_modal.rs - Modal de senha Wi-Fi (A31.3).
//!
//! Overlay desenhado sobre a bar surface quando uma rede nao-saved
//! requere senha (nmcli retorna "Secrets were required").
//!
//! Layout: backdrop 60% alpha + pill 320x140 centrado:
//!   "Senha para {SSID}"
//!   [input mascarado *****             ]
//!   [Conectar]  [Cancelar]
//!
//! Input: a bar seta keyboard_interactivity=Exclusive e consome
//! wl_keyboard.key events no handler de teclado.
//! Buffer de texto acumulado aqui.

use tiny_skia::PixmapMut;
use lumo_foundation::LumoColors;

use crate::bar::fonts::{draw_text, measure_text, opaque, rgba_hex};
use crate::bar::icons::fill_rrect;
use crate::bar::tokens::{FONT_DROPDOWN_BODY, FONT_DROPDOWN_TITLE};

/// Estado do modal de senha ativo.
#[derive(Clone, Default, Debug)]
pub struct PasswordModalState {
    /// SSID alvo (preenchido ao abrir).
    pub ssid: String,
    /// Buffer de texto digitado (plaintext em memoria).
    pub buffer: String,
    /// true = modal visivel e capturando teclado.
    pub active: bool,
}

impl PasswordModalState {
    pub fn open(&mut self, ssid: String) {
        self.ssid = ssid;
        self.buffer.clear();
        self.active = true;
    }

    pub fn close(&mut self) {
        self.active = false;
        self.buffer.clear();
        self.ssid.clear();
    }

    /// Processa um caractere printavel.
    pub fn push_char(&mut self, c: char) {
        // WPA2 max passphrase = 63 chars ASCII.
        if self.buffer.len() < 63 {
            self.buffer.push(c);
        }
    }

    /// Remove ultimo caractere (Backspace).
    pub fn pop_char(&mut self) {
        self.buffer.pop();
    }
}

/// Hit-rects retornados pelo draw.
#[derive(Default, Clone, Debug)]
pub struct PasswordModalHits {
    pub confirm_rect: Option<(f32, f32, f32, f32)>,
    pub cancel_rect: Option<(f32, f32, f32, f32)>,
}

const MODAL_W: f32 = 320.0;
const MODAL_H: f32 = 140.0;

/// Desenha o modal sobre o canvas da bar.
/// surface_w/surface_h sao as dimensoes totais da surface Wayland.
pub fn draw_password_modal(
    canvas: &mut PixmapMut,
    surface_w: f32,
    surface_h: f32,
    palette: &LumoColors,
    state: &PasswordModalState,
) -> PasswordModalHits {
    let mut hits = PasswordModalHits::default();
    if !state.active {
        return hits;
    }

    // Backdrop semi-transparente (60% preto sobre toda surface).
    fill_rrect(canvas, 0.0, 0.0, surface_w, surface_h, 0.0,
        tiny_skia::Color::from_rgba8(0, 0, 0, 153));

    // Caixa modal centrada.
    let mx = ((surface_w - MODAL_W) / 2.0).round();
    let my = ((surface_h - MODAL_H) / 2.0).round();

    let bg = rgba_hex(palette.pill_bg, 0xFF);
    let fg = opaque(palette.pill_fg);
    let fg_dim = rgba_hex(palette.pill_fg, 0xA0);
    let accent = opaque(palette.accent);
    let sep = rgba_hex(palette.pill_sep, palette.pill_sep_alpha);

    fill_rrect(canvas, mx, my, MODAL_W, MODAL_H, 12.0, bg);

    let pad = 16.0;
    let mut cy = my + pad;

    // Titulo "Senha para {SSID}" (truncado a 28 chars).
    let ssid_short: String = state.ssid.chars().take(28).collect();
    let title = format!("Senha para {}", ssid_short);
    draw_text(canvas, mx + pad, cy, &title, FONT_DROPDOWN_TITLE, fg, true);
    cy += FONT_DROPDOWN_TITLE * 1.8;

    // Campo input (fundo secundario + borda accent).
    let input_h: f32 = 28.0;
    let input_w = MODAL_W - pad * 2.0;
    let input_bg = rgba_hex(palette.pill_bg, 0xD0);
    fill_rrect(canvas, mx + pad, cy, input_w, input_h, 6.0, input_bg);

    // Borda accent 1.5px (stroke via PathBuilder).
    {
        use tiny_skia::{Paint, PathBuilder, Stroke, Transform};
        let mut stroke_paint = Paint::default();
        stroke_paint.set_color(accent);
        let r = 6.0_f32;
        let x0 = mx + pad;
        let y0 = cy;
        let x1 = x0 + input_w;
        let y1 = y0 + input_h;
        let mut pb = PathBuilder::new();
        pb.move_to(x0 + r, y0);
        pb.line_to(x1 - r, y0);
        pb.quad_to(x1, y0, x1, y0 + r);
        pb.line_to(x1, y1 - r);
        pb.quad_to(x1, y1, x1 - r, y1);
        pb.line_to(x0 + r, y1);
        pb.quad_to(x0, y1, x0, y1 - r);
        pb.line_to(x0, y0 + r);
        pb.quad_to(x0, y0, x0 + r, y0);
        pb.close();
        if let Some(path) = pb.finish() {
            let mut stroke = Stroke::default();
            stroke.width = 1.5;
            canvas.stroke_path(&path, &stroke_paint, &stroke, Transform::identity(), None);
        }
    }

    // Texto mascarado ou placeholder.
    let text_y = cy + (input_h - FONT_DROPDOWN_BODY * 1.2) / 2.0;
    let masked: String = "*".repeat(state.buffer.len().min(40));
    if state.buffer.is_empty() {
        let placeholder_color = rgba_hex(palette.pill_fg, 0x50);
        draw_text(canvas, mx + pad + 8.0, text_y,
            "Senha Wi-Fi", FONT_DROPDOWN_BODY, placeholder_color, false);
    } else {
        draw_text(canvas, mx + pad + 8.0, text_y,
            &masked, FONT_DROPDOWN_BODY, fg, false);
    }

    // Cursor de texto (linha vertical, sempre visivel neste frame).
    let cursor_base_x = mx + pad + 8.0;
    let cursor_offset = measure_text(&masked, FONT_DROPDOWN_BODY, false);
    fill_rrect(canvas, cursor_base_x + cursor_offset + 2.0, cy + 4.0,
        1.5, input_h - 8.0, 0.0, accent);

    cy += input_h + 12.0;

    // Botoes [Conectar] [Cancelar].
    let btn_h: f32 = 28.0;
    let btn_gap: f32 = 8.0;
    let btn_w = (MODAL_W - pad * 2.0 - btn_gap) / 2.0;

    // [Conectar] (accent).
    let btn_connect_x = mx + pad;
    fill_rrect(canvas, btn_connect_x, cy, btn_w, btn_h, 8.0, accent);
    let lbl = "Conectar";
    let lw = measure_text(lbl, FONT_DROPDOWN_BODY, true);
    draw_text(canvas, btn_connect_x + (btn_w - lw) / 2.0,
        cy + (btn_h - FONT_DROPDOWN_BODY * 1.2) / 2.0,
        lbl, FONT_DROPDOWN_BODY, opaque(0xFFFFFF), true);
    hits.confirm_rect = Some((btn_connect_x, cy, btn_w, btn_h));

    // [Cancelar] (sep color).
    let btn_cancel_x = mx + pad + btn_w + btn_gap;
    fill_rrect(canvas, btn_cancel_x, cy, btn_w, btn_h, 8.0, sep);
    let lbl2 = "Cancelar";
    let lw2 = measure_text(lbl2, FONT_DROPDOWN_BODY, false);
    draw_text(canvas, btn_cancel_x + (btn_w - lw2) / 2.0,
        cy + (btn_h - FONT_DROPDOWN_BODY * 1.2) / 2.0,
        lbl2, FONT_DROPDOWN_BODY, fg_dim, false);
    hits.cancel_rect = Some((btn_cancel_x, cy, btn_w, btn_h));

    hits
}
