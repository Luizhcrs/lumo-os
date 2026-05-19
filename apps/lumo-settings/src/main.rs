//! lumo-settings — painel de configuracoes Iced nativo para Lumo OS.
//!
//! 8 abas: Display, Wi-Fi, Bluetooth, Audio, Bateria, Aparencia, Teclado, Touchpad.

mod app;
mod appmenu;
mod theme;
mod tabs;

use app::App;
use iced::{Settings, Size};

fn main() -> iced::Result {
    let tx = appmenu::init_channel();
    std::thread::Builder::new()
        .name("lumo-settings-appmenu".into())
        .spawn(move || appmenu::serve(tx))
        .expect("spawn appmenu thread");

    iced::application("Lumo Settings", App::update, App::view)
        .subscription(App::subscription)
        .settings(Settings {
            default_text_size: 14.0.into(),
            ..Default::default()
        })
        .window(iced::window::Settings {
            size: Size::new(900.0, 620.0),
            min_size: Some(Size::new(700.0, 480.0)),
            ..Default::default()
        })
        .run_with(App::new)
}
