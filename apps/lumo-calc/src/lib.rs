//! lumo-calc - library entry pra lumo-apps dispatcher (W33).

pub mod app;
pub mod appmenu;
pub mod theme;

use app::App;
use iced::{Settings, Size};

pub fn run() -> iced::Result {

    let tx = appmenu::init_channel();
    std::thread::Builder::new()
        .name("lumo-calc-appmenu".into())
        .spawn(move || appmenu::serve(tx))
        .expect("spawn appmenu thread");

    iced::application("Lumo Calc", App::update, App::view)
        .subscription(App::subscription)
        .theme(|_| iced::Theme::Dark)
        .settings(Settings {
            default_text_size: 14.0.into(),
            ..Default::default()
        })
        .window(iced::window::Settings {
            size: Size::new(460.0, 560.0),
            min_size: Some(Size::new(380.0, 480.0)),
            decorations: true,
            position: iced::window::Position::Centered,
            ..Default::default()
        })
        .run_with(App::new)
}
