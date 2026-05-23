//! bar/tokens.rs - Constantes de layout, fontes, cores e dimensoes da bar.
//!
//! Cada valor justificado (memory feedback_design_lapidado). Modulo
//! intencionalmente plano com `pub const` pra import via glob
//! (use crate::bar::tokens::*).
//!
//! Memory feedback_zero_neon_glow: nenhum token aqui carrega box-shadow
//! colorido com accent; sombras vem 4-layer preto neutro em pills.rs.

use crate::menu;

// ============================================================
// Layout constants (lapidado: cada valor justificado).
// ============================================================

/// Altura total da bar (layer-shell exclusive zone).
/// 40px = 28px pill + 6px margin topo + 6px margem inferior (sombra cabe).
pub const BAR_HEIGHT: u32 = 40;

/// Altura de cada pill. 28px = compact responsivo touch.
pub const PILL_H: f32 = 28.0;

/// Margem topo: distancia entre topo da bar e topo da pill.
/// 6px = respiro suficiente sem desperdicar real-estate.
pub const PILL_MARGIN_TOP: f32 = 6.0;

/// Margem lateral: distancia entre borda da bar e a pill.
/// 14px = mesmo PAD_X do design anterior (continuidade visual).
pub const PILL_MARGIN_X: f32 = 14.0;

/// Border-radius das pills. 16px = bem arredondado, pill-shape (28h / 2 = 14
/// daria capsule pura; 16 amacia mas mantem identidade pill).
pub const PILL_RADIUS: f32 = 14.0;

/// Padding horizontal interno da pill (entre borda da pill e conteudo).
/// 14px = respiracao premium.
pub const PILL_PAD_X: f32 = 14.0;

/// Gap entre items dentro da pill (icone/texto adjacentes).
/// 8px = denso mas legivel.
pub const PILL_GAP: f32 = 8.0;

/// Brand dot diametro 8px (radius 4). Atomo visual estavel.
pub const BRAND_DOT_RADIUS: f32 = 4.0;

/// Separator dot middle (entre items dentro da pill esquerda).
/// 4px diametro = sutil mas perceptivel.
pub const SEP_DOT_RADIUS: f32 = 2.0;

/// Font sizes (px). Conteudo de pill todo em 13px (compact uniform).
pub const FONT_PILL: f32 = 13.0;
pub const FONT_DATE: f32 = 13.0; // A19.14 igual clock

/// Wifi icone 16x16 (compact pra caber dentro de pill 28h).
pub const WIFI_SIZE: f32 = 16.0;

/// Bateria icone 14x8 (proporcional a 28h pill).
pub const BAT_BODY_W: f32 = 22.0; // A19.14 mais larga Mac-style
pub const BAT_BODY_H: f32 = 11.0;

// ============================================================
// Dropdown (A20).
// ============================================================
//
// Painel descendente abaixo da pill direita quando icone bat eh clicado.
// Largura 280 (>= pill direita), altura 200 (cabe 5 linhas key:value + header).
// Gap 6px abaixo da pill (respiro visual sem desconectar).
// Padding interno 14 igual PILL_PAD_X (continuidade).
pub const DROPDOWN_W: f32 = 280.0;
pub const DROPDOWN_H: f32 = 320.0; // A31.2.fix: era 150, sobra 26px embaixo "Tempo" // A20.1 menor (3 rows) - bateria
pub const DROPDOWN_GAP: f32 = 6.0;
pub const DROPDOWN_PAD: f32 = 14.0;
pub const DROPDOWN_ROW_H: f32 = 18.0;
pub const FONT_DROPDOWN_TITLE: f32 = 14.0;
pub const FONT_DROPDOWN_BODY: f32 = 13.0;

// A31: Wifi tem layout proprio (gerenciador redes), altura variavel.
// Calc: pad(14) + header(20) + connected_row(22) + spacer(6) +
//   label(20) + 6 * row(22) + sep_pad(12) + footer_row(22) + pad(14)
//   = 14 + 20 + 22 + 6 + 20 + 132 + 12 + 22 + 14 = ~262. Pad +4 = 266.
pub const DROPDOWN_WIFI_W: f32 = 300.0;
pub const DROPDOWN_WIFI_H: f32 = 266.0;
/// Altura linha de rede individual (icon + ssid + pct). 22 = font 13 + 9
/// padding vertical = area de click confortavel sem inflar dropdown.
pub const DROPDOWN_WIFI_ROW_H: f32 = 22.0;

// ============================================================
// Dropdown DateTime (A24).
// ============================================================
pub const DROPDOWN_DATETIME_W: f32 = 280.0;
pub const DROPDOWN_DATETIME_H: f32 = 288.0; // A26 +36 = espaco pra footer com botao Hoje
pub const DATETIME_CELL_W: f32 = 32.0;
pub const DATETIME_CELL_H: f32 = 22.0;
pub const FONT_DROPDOWN_CLOCK: f32 = 22.0;
pub const FONT_DROPDOWN_CALENDAR: f32 = 12.0;

// A26: navegacao interativa do calendario.
pub const CAL_NAV_BTN_W: f32 = 22.0;
pub const CAL_NAV_BTN_H: f32 = 20.0;
pub const CAL_NAV_BTN_RADIUS: f32 = 8.0;
pub const CAL_TODAY_BTN_W: f32 = 56.0;
pub const CAL_TODAY_BTN_H: f32 = 22.0;
pub const CAL_FOOTER_H: f32 = 30.0;
pub const CAL_HEADER_H: f32 = 22.0;
pub const FONT_CAL_NAV: f32 = 13.0;

// ============================================================
// A27: Menu Lumo (click brand "Lumo" da pill esquerda).
// ============================================================
pub const MENU_LUMO_W: f32 = menu::MENU_W_LUMO;
pub const MENU_LUMO_ITEMS: &[menu::MenuItem] = &[
    menu::MenuItem::action("Sobre este Galaxy Book..."),
    menu::MenuItem::action("Software Update..."),
    menu::MenuItem::action("Lumo Store"),
    menu::MenuItem::separator(),
    menu::MenuItem::action("Preferencias do Sistema..."),
    menu::MenuItem::separator(),
    menu::MenuItem::action("Bloquear tela"),
    menu::MenuItem::action("Suspender"),
    menu::MenuItem::action("Reiniciar..."),
    menu::MenuItem::action("Desligar..."),
];

// ============================================================
// L5: Brightness dropdown.
// ============================================================
/// Width of brightness dropdown. Same as battery dropdown for visual consistency.
pub const DROPDOWN_BRIGHTNESS_W: f32 = 280.0;
/// Height: pad + title + spacer + slider_row + spacer + preset_row + pad.
/// 14 + 20 + 8 + 24 + 8 + 22 + 14 = 110.
pub const DROPDOWN_BRIGHTNESS_H: f32 = 110.0;
/// Slider track height (horizontal fill bar).
pub const BRIGHTNESS_SLIDER_H: f32 = 8.0;
