//! bar/dropdowns/ - Painel descendente abaixo das pills.
//!
//! Cada dropdown eh self-contained: state (struct *Info) + render (draw_*_dropdown).
//! Memory feedback_lumo_arquitetura_clean: codigo direto, sem trait
//! abstrato "DropdownProvider".

pub mod battery;
pub mod datetime;
pub mod lumo_menu;
pub mod wifi;

/// Estado do dropdown ativo (mutex: so um aberto por vez).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum DropdownActive {
    None,
    Battery,
    Wifi,     // A23
    DateTime, // A24 - calendario + hora detalhada
    LumoMenu, // A27 - menu Apple-style abaixo brand "Lumo" pill esquerda
}
