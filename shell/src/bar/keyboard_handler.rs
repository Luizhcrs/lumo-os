//! bar/keyboard_handler.rs - KeyboardHandler impl para LumoBar.
//!
//! Ativo so quando password_modal.active == true.
//! Processa press_key: caracteres printaveis -> push_char,
//! Backspace -> pop_char, Escape -> fecha modal, Return -> confirma.

use smithay_client_toolkit::{
    reexports::client::{
        protocol::{wl_keyboard::WlKeyboard, wl_surface::WlSurface},
        Connection, QueueHandle,
    },
    seat::keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RepeatInfo},
};

use smithay_client_toolkit::shell::WaylandSurface;
use crate::bar::state::LumoBar;

impl KeyboardHandler for LumoBar {
    fn enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        _surface: &WlSurface,
        _serial: u32,
        _raw: &[u32],
        _keysyms: &[Keysym],
    ) {
    }

    fn leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        _surface: &WlSurface,
        _serial: u32,
    ) {
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        if !self.password_modal.active {
            return;
        }

        // xkeysym constants (u32 values).
        const XKB_KEY_Return: u32 = 0xFF0D;
        const XKB_KEY_KP_Enter: u32 = 0xFF8D;
        const XKB_KEY_Escape: u32 = 0xFF1B;
        const XKB_KEY_BackSpace: u32 = 0xFF08;
        const XKB_KEY_Delete: u32 = 0xFFFF;

        let sym_raw = event.keysym.raw();
        match sym_raw {
            s if s == XKB_KEY_Return || s == XKB_KEY_KP_Enter => {
                let ssid = self.password_modal.ssid.clone();
                let pwd = self.password_modal.buffer.clone();
                eprintln!("[lumo-bar] A31.3 modal Enter -> conectar ssid={:?}", ssid);
                crate::bar::system_info::nm_connect_with_password(ssid, pwd);
                self.password_modal.close();
                self.wifi_refresh_due = Some(std::time::Instant::now() + std::time::Duration::from_millis(3000));
                self.layer.set_keyboard_interactivity(
                    smithay_client_toolkit::shell::wlr_layer::KeyboardInteractivity::None);
                self.layer.wl_surface().commit();
                self.update_size_and_redraw(qh);
            }
            s if s == XKB_KEY_Escape => {
                eprintln!("[lumo-bar] A31.3 modal Escape -> cancelar");
                self.password_modal.close();
                self.layer.set_keyboard_interactivity(
                    smithay_client_toolkit::shell::wlr_layer::KeyboardInteractivity::None);
                self.layer.wl_surface().commit();
                self.update_size_and_redraw(qh);
            }
            s if s == XKB_KEY_BackSpace || s == XKB_KEY_Delete => {
                self.password_modal.pop_char();
                self.update_size_and_redraw(qh);
            }
            _ => {
                // Tenta obter char printavel pelo keysym.
                if let Some(c) = keysym_to_char(event.keysym.raw()) {
                    self.password_modal.push_char(c);
                    self.update_size_and_redraw(qh);
                }
            }
        }
    }

    fn release_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        _serial: u32,
        _event: KeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        _serial: u32,
        _modifiers: Modifiers,
        _layout: u32,
    ) {
    }

    fn update_repeat_info(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        _info: RepeatInfo,
    ) {
    }
}

/// Converte keysym raw em char ASCII/Latin-1 printavel.
/// Cobre: espaco, letras, digitos, pontuacao comum.
/// Ignora teclas de controle/funcao.
fn keysym_to_char(sym: u32) -> Option<char> {
    // xkeysym latin-1: 0x20..=0x7E e 0xA0..=0xFF mapeiam direto pra Unicode.
    if (0x20..=0x7E).contains(&sym) {
        return char::from_u32(sym);
    }
    if (0xA0..=0xFF).contains(&sym) {
        return char::from_u32(sym);
    }
    None
}
