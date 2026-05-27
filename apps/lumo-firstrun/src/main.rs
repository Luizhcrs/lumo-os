//! lumo-firstrun -- wizard de primeiro uso do Lumo OS.
//!
//! Trigger: `/var/lib/lumo/first-run-done` ausente.
//! Apos completar: cria o arquivo flag e encerra.
//! O servico `lumo-firstrun.service` chama este binario antes do lumo-wm.

mod app;
mod locale;
mod steps;
mod system;
mod theme;

#[cfg(test)]
mod tests;

use app::FirstRunApp;
use iced::{Settings, Size};

/// Caminho do arquivo flag. Ausencia = first-run nao concluido.
pub const FIRST_RUN_FLAG: &str = "/var/lib/lumo/first-run-done";

fn main() -> iced::Result {
    lumo_error::hook::install_panic_hook("lumo-firstrun", lumo_error::Domain::App);
    // Se o flag ja existe, nao exibir wizard.
    if std::path::Path::new(FIRST_RUN_FLAG).exists() {
        return Ok(());
    }

    iced::application("Lumo OS", FirstRunApp::update, FirstRunApp::view)
        .settings(Settings {
            default_text_size: 15.0.into(),
            ..Default::default()
        })
        .window(iced::window::Settings {
            size: Size::new(720.0, 520.0),
            min_size: Some(Size::new(640.0, 480.0)),
            resizable: false,
            decorations: false,
            ..Default::default()
        })
        .run_with(FirstRunApp::new)
}
