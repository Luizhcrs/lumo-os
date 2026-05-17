//! Input event handling (Layer 4.1.9).
//!
//! Esta camada existe pra desacoplar a `lumo-gfx-core` do `winit` na hora de
//! propagar eventos pro `widget::ButtonHandle` (e widgets futuros). O bridge
//! `winit_event_to_lumo` faz o adapter; o resto da stack so conhece
//! `LumoEvent` / `PointerState`.
//!
//! Convencoes:
//! - posicao em pixels fisicos (mesmo espaco do viewport)
//! - origem top-left (compativel com `widget::Rect` e `Button::queue`)
//! - botoes do mouse como bitmask (compat com `PointerState.buttons`)

use winit::event::{ElementState, MouseButton as WinitMouseButton, WindowEvent};
use winit::keyboard::ModifiersState;

/// Bitmask de botoes do mouse pra `PointerState`.
pub const MOUSE_BUTTON_LEFT: u32 = 1;
pub const MOUSE_BUTTON_RIGHT: u32 = 2;
pub const MOUSE_BUTTON_MIDDLE: u32 = 4;

/// Snapshot da posicao + botoes do ponteiro em um frame.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct PointerState {
    /// Posicao em pixels (origem top-left).
    pub position: [f32; 2],
    /// Bitmask: bit 0 = left, bit 1 = right, bit 2 = middle.
    pub buttons: u32,
}

impl PointerState {
    pub fn left_down(&self) -> bool {
        self.buttons & MOUSE_BUTTON_LEFT != 0
    }
    pub fn right_down(&self) -> bool {
        self.buttons & MOUSE_BUTTON_RIGHT != 0
    }
    pub fn middle_down(&self) -> bool {
        self.buttons & MOUSE_BUTTON_MIDDLE != 0
    }
}

/// Botao de mouse (variant claro pra match).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl MouseButton {
    pub fn bit(self) -> u32 {
        match self {
            MouseButton::Left => MOUSE_BUTTON_LEFT,
            MouseButton::Right => MOUSE_BUTTON_RIGHT,
            MouseButton::Middle => MOUSE_BUTTON_MIDDLE,
        }
    }
}

/// Modifier keys (shift/ctrl/alt/logo).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub logo: bool,
}

impl Modifiers {
    pub fn from_winit(m: ModifiersState) -> Self {
        Self {
            shift: m.shift_key(),
            ctrl: m.control_key(),
            alt: m.alt_key(),
            logo: m.super_key(),
        }
    }
}

/// Evento normalizado consumido pelos widgets.
#[derive(Clone, Debug, PartialEq)]
pub enum LumoEvent {
    PointerMove {
        position: [f32; 2],
    },
    PointerPress {
        position: [f32; 2],
        button: MouseButton,
    },
    PointerRelease {
        position: [f32; 2],
        button: MouseButton,
    },
    KeyPress {
        code: u32,
        modifiers: Modifiers,
    },
    KeyRelease {
        code: u32,
        modifiers: Modifiers,
    },
    Resize {
        size: [u32; 2],
    },
}

/// Estado mutavel mantido pelo bridge entre frames. winit reporta o cursor
/// position separado dos cliques, entao a gente cacheia a ultima posicao pra
/// emitir `PointerPress { position }` ja com coords corretas.
#[derive(Clone, Copy, Debug, Default)]
pub struct InputBridge {
    pub pointer: PointerState,
    pub modifiers: Modifiers,
}

impl InputBridge {
    pub fn new() -> Self {
        Self::default()
    }

    /// Faz adapter de `winit::WindowEvent` pra `LumoEvent`, atualizando o
    /// `pointer` cached. Retorna `None` pra eventos winit que nao mapeiam
    /// (foco, ime, etc.).
    pub fn map(&mut self, event: &WindowEvent) -> Option<LumoEvent> {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let p = [position.x as f32, position.y as f32];
                self.pointer.position = p;
                Some(LumoEvent::PointerMove { position: p })
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let mb = match button {
                    WinitMouseButton::Left => MouseButton::Left,
                    WinitMouseButton::Right => MouseButton::Right,
                    WinitMouseButton::Middle => MouseButton::Middle,
                    _ => return None,
                };
                match state {
                    ElementState::Pressed => {
                        self.pointer.buttons |= mb.bit();
                        Some(LumoEvent::PointerPress {
                            position: self.pointer.position,
                            button: mb,
                        })
                    }
                    ElementState::Released => {
                        self.pointer.buttons &= !mb.bit();
                        Some(LumoEvent::PointerRelease {
                            position: self.pointer.position,
                            button: mb,
                        })
                    }
                }
            }
            WindowEvent::ModifiersChanged(m) => {
                self.modifiers = Modifiers::from_winit(m.state());
                None
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let code = match event.physical_key {
                    winit::keyboard::PhysicalKey::Code(k) => k as u32,
                    winit::keyboard::PhysicalKey::Unidentified(_) => return None,
                };
                match event.state {
                    ElementState::Pressed => Some(LumoEvent::KeyPress {
                        code,
                        modifiers: self.modifiers,
                    }),
                    ElementState::Released => Some(LumoEvent::KeyRelease {
                        code,
                        modifiers: self.modifiers,
                    }),
                }
            }
            WindowEvent::Resized(s) => Some(LumoEvent::Resize {
                size: [s.width, s.height],
            }),
            _ => None,
        }
    }
}

/// Helper standalone para casos simples (sem manter state) — apenas eventos
/// que nao precisam de pointer cached.
pub fn winit_event_to_lumo(event: &WindowEvent) -> Option<LumoEvent> {
    match event {
        WindowEvent::CursorMoved { position, .. } => Some(LumoEvent::PointerMove {
            position: [position.x as f32, position.y as f32],
        }),
        WindowEvent::Resized(s) => Some(LumoEvent::Resize {
            size: [s.width, s.height],
        }),
        _ => None,
    }
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_state_default_clear() {
        let p = PointerState::default();
        assert!(!p.left_down());
        assert!(!p.right_down());
        assert!(!p.middle_down());
    }

    #[test]
    fn mouse_button_bit_matches_constants() {
        assert_eq!(MouseButton::Left.bit(), MOUSE_BUTTON_LEFT);
        assert_eq!(MouseButton::Right.bit(), MOUSE_BUTTON_RIGHT);
        assert_eq!(MouseButton::Middle.bit(), MOUSE_BUTTON_MIDDLE);
    }
}
