//! lumo-monitor -- monitor do sistema Iced para Lumo OS.

mod app;
mod appmenu;
mod theme;
mod proc;

use app::App;
use iced::{Settings, Size};

fn main() -> iced::Result {
    let tx = appmenu::init_channel();
    std::thread::Builder::new()
        .name("lumo-monitor-appmenu".into())
        .spawn(move || appmenu::serve(tx))
        .expect("spawn appmenu thread");

    iced::application("Lumo Monitor", App::update, App::view)
        .subscription(App::subscription)
        .settings(Settings {
            default_text_size: 13.0.into(),
            ..Default::default()
        })
        .window(iced::window::Settings {
            size: Size::new(900.0, 640.0),
            min_size: Some(Size::new(700.0, 480.0)),
            ..Default::default()
        })
        .run_with(App::new)
}
