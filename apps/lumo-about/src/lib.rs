pub mod app;

pub fn run() -> iced::Result {
    use app::App;
    use iced::{Settings, Size};

    iced::application("Sobre este Galaxy Book", App::update, App::view)
        .theme(|_| iced::Theme::Dark)
        .settings(Settings {
            default_text_size: 14.0.into(),
            ..Default::default()
        })
        .window(iced::window::Settings {
            size: Size::new(540.0, 600.0),
            min_size: Some(Size::new(480.0, 520.0)),
            resizable: false,
            decorations: true,
            position: iced::window::Position::Centered,
            ..Default::default()
        })
        .run_with(App::new)
}
