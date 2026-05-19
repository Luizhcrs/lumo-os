//! lumo-files -- file manager Iced nativo para Lumo OS.
//!
//! Entry point: instancia App e chama iced::application.

mod app;
mod appmenu;
mod breadcrumb;
mod ctxmenu;
mod filelist;
mod icons;
mod ops;
mod sidebar;
mod statusbar;
mod tabs;
mod theme;
mod thumbs;
mod toast;
mod toolbar;

use app::App;
use iced::{Settings, Size};

fn main() -> iced::Result {
    let tx = appmenu::init_channel();
    std::thread::Builder::new()
        .name("lumo-appmenu".into())
        .spawn(move || appmenu::serve(tx))
        .expect("spawn appmenu thread");

    // W19 fix: setar application_id para Wayland (xdg_toplevel.set_app_id).
    // Sem isso INSTR.E loga app_id='' e bar/dock nao agrupam por aplicacao.
    // Default Iced eh String::new() -- precisa explicit pra apps Lumo.
    let mut window_settings = iced::window::Settings {
        size: Size::new(1024.0, 640.0),
        min_size: Some(Size::new(640.0, 400.0)),
        decorations: true,
        position: iced::window::Position::Centered,
        ..Default::default()
    };
    window_settings.platform_specific.application_id = "com.lumo.files".to_string();

    iced::application("Lumo Files", App::update, App::view)
        .subscription(App::subscription)
        .theme(|_| iced::Theme::Dark)
        .settings(Settings {
            default_text_size: 14.0.into(),
            ..Default::default()
        })
        .window(window_settings)
        .run_with(App::new)
}
