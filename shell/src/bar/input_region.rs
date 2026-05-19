//! bar/input_region.rs - Define o input region da surface (A29).
//!
//! A bar inteira eh transparente fora das pills + dropdown ativo. Sem
//! input_region explicito, a surface inteira capturaria pointer events
//! mesmo nas areas vazias, bloqueando o desktop layer-shell abaixo.
//!
//! Memory feedback_input_feedback_imediato: o desktop precisa ver
//! pointer events em qualquer Y < BAR_HEIGHT quando nao ha pill ou
//! dropdown atras.

use smithay_client_toolkit::compositor::Region;
use smithay_client_toolkit::shell::WaylandSurface;

use crate::bar::dropdowns::DropdownActive;
use crate::bar::state::LumoBar;
use crate::bar::tokens::*;
use crate::menu;

impl LumoBar {
    /// A29: define input_region da surface pra cobrir SO as pills + dropdown
    /// quando aberto. Areas transparentes da surface passam input pro layer
    /// abaixo (desktop), liberando o right-click do menu desktop em qualquer
    /// posicao Y da tela.
    pub fn update_input_region(&mut self) {
        let Ok(region) = Region::new(&self.compositor_state) else {
            // Se nao conseguir criar region, ficamos com default (surface toda
            // captura input). Pior caso = comportamento atual.
            return;
        };

        // Pill esquerda (Lumo + workspace). Tudo dentro da pill = clicavel.
        if let Some((rx, ry, rw, rh)) = self.lumo_hit_rect {
            region.add(
                rx.floor() as i32,
                ry.floor() as i32,
                rw.ceil() as i32,
                rh.ceil() as i32,
            );
        }

        // Pill direita (bat + wifi + brightness + data + clock). Union dos 4 hit-rects.
        // Bug fix: brightness_hit_rect estava de fora -> clicks no sol caiam pro desktop.
        let right_rects = [self.bat_hit_rect, self.wifi_hit_rect, self.brightness_hit_rect, self.datetime_hit_rect];
        let mut union: Option<(f32, f32, f32, f32)> = None;
        for r in right_rects.iter().flatten() {
            union = Some(match union {
                None => *r,
                Some((ux, uy, uw, uh)) => {
                    let x0 = ux.min(r.0);
                    let y0 = uy.min(r.1);
                    let x1 = (ux + uw).max(r.0 + r.2);
                    let y1 = (uy + uh).max(r.1 + r.3);
                    (x0, y0, x1 - x0, y1 - y0)
                }
            });
        }
        if let Some((rx, ry, rw, rh)) = union {
            region.add(
                rx.floor() as i32,
                ry.floor() as i32,
                rw.ceil() as i32,
                rh.ceil() as i32,
            );
        }

        // Dropdown ativo: cobre area do painel pra capturar click dentro.
        match self.dropdown {
            DropdownActive::None => {}
            DropdownActive::AppFallback => {}
            DropdownActive::Battery => {
                if let Some((rx, ry, rw, rh)) = self.bat_hit_rect {
                    let dx = (rx + rw / 2.0 - DROPDOWN_W / 2.0)
                        .max(PILL_MARGIN_X)
                        .min(self.width as f32 - PILL_MARGIN_X - DROPDOWN_W);
                    let dy = ry + rh + DROPDOWN_GAP;
                    region.add(
                        dx.floor() as i32 - 8,
                        dy.floor() as i32 - 4,
                        DROPDOWN_W.ceil() as i32 + 16,
                        DROPDOWN_H.ceil() as i32 + 16,
                    );
                }
            }
            DropdownActive::Wifi => {
                // A31.2 fix: usar DROPDOWN_WIFI_W/H (era DROPDOWN_W/H = battery).
                // Causa raiz: clicks em redes meio/fim do dropdown wifi
                // (Y > 150) passavam pro desktop pq input_region cortava.
                if let Some((rx, ry, rw, rh)) = self.wifi_hit_rect {
                    let dx = (rx + rw / 2.0 - DROPDOWN_WIFI_W / 2.0)
                        .max(PILL_MARGIN_X)
                        .min(self.width as f32 - PILL_MARGIN_X - DROPDOWN_WIFI_W);
                    let dy = ry + rh + DROPDOWN_GAP;
                    region.add(
                        dx.floor() as i32 - 8,
                        dy.floor() as i32 - 4,
                        DROPDOWN_WIFI_W.ceil() as i32 + 16,
                        DROPDOWN_WIFI_H.ceil() as i32 + 16,
                    );
                }
            }
            DropdownActive::DateTime => {
                if let Some((rx, ry, rw, rh)) = self.datetime_hit_rect {
                    let dx = (rx + rw / 2.0 - DROPDOWN_DATETIME_W / 2.0)
                        .max(PILL_MARGIN_X)
                        .min(self.width as f32 - PILL_MARGIN_X - DROPDOWN_DATETIME_W);
                    let dy = ry + rh + DROPDOWN_GAP;
                    region.add(
                        dx.floor() as i32 - 8,
                        dy.floor() as i32 - 4,
                        DROPDOWN_DATETIME_W.ceil() as i32 + 16,
                        DROPDOWN_DATETIME_H.ceil() as i32 + 16,
                    );
                }
            }
            DropdownActive::LumoMenu => {
                if let Some((rx, ry, _rw, rh)) = self.lumo_hit_rect {
                    let dx = rx.max(PILL_MARGIN_X);
                    let dy = ry + rh + DROPDOWN_GAP;
                    let mh = menu::menu_height(MENU_LUMO_ITEMS);
                    region.add(
                        dx.floor() as i32 - 8,
                        dy.floor() as i32 - 4,
                        MENU_LUMO_W.ceil() as i32 + 16,
                        mh.ceil() as i32 + 16,
                    );
                }
            }
            DropdownActive::Brightness => {
                if let Some((rx, ry, rw, rh)) = self.brightness_hit_rect {
                    let dx = (rx + rw / 2.0 - DROPDOWN_BRIGHTNESS_W / 2.0)
                        .max(PILL_MARGIN_X)
                        .min(self.width as f32 - PILL_MARGIN_X - DROPDOWN_BRIGHTNESS_W);
                    let dy = ry + rh + DROPDOWN_GAP;
                    region.add(
                        dx.floor() as i32 - 8,
                        dy.floor() as i32 - 4,
                        DROPDOWN_BRIGHTNESS_W.ceil() as i32 + 16,
                        DROPDOWN_BRIGHTNESS_H.ceil() as i32 + 16,
                    );
                }
            }
        }

        self.layer
            .wl_surface()
            .set_input_region(Some(region.wl_region()));
        // W18.fix: GUARDA region em self -- drop apos proxima update.
        // Drop antes do commit destruia wl_region; server lia input_region=None
        // e bar layer pegava clicks Y=32..342 (bloqueava apps xdg-shell embaixo).
        self.current_input_region = Some(region);
    }
}
