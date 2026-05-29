//! lumo-files - library entry (W33).

pub mod app;
pub mod appmenu;
pub mod breadcrumb;
pub mod ctxmenu;
pub mod filelist;
pub mod icons;
pub mod ops;
pub mod sidebar;
pub mod statusbar;
pub mod tabs;
pub mod theme;
pub mod mime_icon;
pub mod thumbs;
pub mod toast;
pub mod toolbar;

#[cfg(test)]
mod app_tests;

use app::App;
use iced::{Settings, Size};

pub fn run() -> iced::Result {
    let _launch_t0 = std::time::Instant::now();
    lumo_telemetry::init();
    {
        let mut meta = std::collections::HashMap::new();
        meta.insert("app".to_string(), "lumo-files".to_string());
        lumo_telemetry::record_event(lumo_telemetry::EventKind::AppLaunch, meta);
    }
    let tx = appmenu::init_channel();
    std::thread::Builder::new()
        .name("lumo-appmenu".into())
        .spawn(move || appmenu::serve(tx))
        .expect("spawn appmenu thread");

    // W19 fix: setar application_id para Wayland (xdg_toplevel.set_app_id).
    // Sem isso INSTR.E loga app_id='' e bar/dock nao agrupam por aplicacao.
    // Default Iced eh String::new() -- precisa explicit pra apps Lumo.
    let window_settings = iced::window::Settings {
        size: Size::new(1024.0, 768.0),
        min_size: Some(Size::new(640.0, 400.0)),
        decorations: true,
        position: iced::window::Position::Centered,
        ..Default::default()
    };
    #[cfg(target_os = "linux")]
    let mut window_settings = window_settings;
    #[cfg(target_os = "linux")]
    {
        window_settings.platform_specific.application_id = "com.lumo.files".to_string();
    }

    // Record startup time before blocking on iced event loop.
    lumo_telemetry::histogram("app_launch_us", _launch_t0.elapsed().as_micros() as u64);

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
