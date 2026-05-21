//! lumo-store - library entry (W33).

pub mod app;
pub mod catalog;
pub mod install;
pub mod theme;

use app::StoreApp;
use iced::{Settings, Size};

pub fn run() -> iced::Result {

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
