//! Handlers for Message::AppMenu* variants.
//!
//! Each function takes `&mut App` and returns `Task<Message>`.

use iced::Task;

use crate::app::{App, Message};

pub fn handle_new_window(_app: &mut App) -> Task<Message> {
    let exe = std::env::current_exe().unwrap_or_default();
    Task::perform(
        async move {
            let _ = tokio::process::Command::new(&exe).spawn();
        },
        |_| Message::Refresh,
    )
}

pub fn handle_quit(_app: &mut App) -> Task<Message> {
    iced::exit()
}

pub fn handle_select_all(app: &mut App) -> Task<Message> {
    let n = app.current_tab().file_list.entries.len();
    for i in 0..n {
        app.current_tab_mut().file_list.selected.insert(i);
    }
    Task::none()
}

pub fn handle_toggle_hidden(app: &mut App) -> Task<Message> {
    app.show_hidden = !app.show_hidden;
    app.update(Message::Refresh)
}

pub fn handle_show_about(_app: &mut App) -> Task<Message> {
    Task::none()
}

pub fn handle_show_shortcuts(_app: &mut App) -> Task<Message> {
    Task::none()
}
