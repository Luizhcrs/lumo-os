//! lumo-store -- loja de aplicativos Lumo OS.
//!
//! MVP: grid de apps disponiveis, tabs Disponiveis/Instalados, busca, filtro categoria.
//! Instalacao: pkexec pacman -S <pkg>

mod app;
mod catalog;
mod install;
mod theme;

use app::StoreApp;
use iced::{Settings, Size};

fn main() -> iced::Result {
    iced::application("Lumo Store", StoreApp::update, StoreApp::view)
        .settings(Settings {
            default_text_size: 14.0.into(),
            ..Default::default()
        })
        .window(iced::window::Settings {
            size: Size::new(900.0, 640.0),
            min_size: Some(Size::new(720.0, 480.0)),
            ..Default::default()
        })
        .run_with(StoreApp::new)
}
