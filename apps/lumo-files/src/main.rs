//! lumo-files — file manager Iced nativo para Lumo OS.
//!
//! Entry point: instancia App e chama iced::application.

mod app;
mod breadcrumb;
mod filelist;
mod icons;
mod ops;
mod sidebar;
mod theme;

use app::App;
use iced::{Settings, Size};

fn main() -> iced::Result {
    iced::application("Lumo Files", App::update, App::view)
        .subscription(App::subscription)
        .settings(Settings {
            default_text_size: 14.0.into(),
            ..Default::default()
        })
        .window(iced::window::Settings {
            size: Size::new(1024.0, 640.0),
            min_size: Some(Size::new(640.0, 400.0)),
            ..Default::default()
        })
        .run_with(App::new)
}
