//! lumo-text -- editor de texto minimal Iced para Lumo OS.

mod app;
mod appmenu;
mod theme;

use app::App;
use iced::{Settings, Size};

fn main() -> iced::Result {
    let args: Vec<String> = std::env::args().collect();
    let initial_path = args.get(1).cloned();

    let tx = appmenu::init_channel();
    std::thread::Builder::new()
        .name("lumo-text-appmenu".into())
        .spawn(move || appmenu::serve(tx))
        .expect("spawn appmenu thread");

    iced::application("Lumo Text", App::update, App::view)
        .subscription(App::subscription)
        .settings(Settings {
            default_text_size: 14.0.into(),
            ..Default::default()
        })
        .window(iced::window::Settings {
            size: Size::new(880.0, 600.0),
            min_size: Some(Size::new(400.0, 300.0)),
            ..Default::default()
        })
        .run_with(move || App::new(initial_path.clone()))
}
