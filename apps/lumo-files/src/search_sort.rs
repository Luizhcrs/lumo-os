//! Search, sort, and view_mode handlers.

use iced::Task;

use crate::app::{App, Message};

pub fn handle_toggle_search(app: &mut App) -> Task<Message> {
    app.search_visible = !app.search_visible;
    if !app.search_visible {
        app.search_query.clear();
    }
    Task::none()
}

pub fn handle_search_changed(app: &mut App, s: String) -> Task<Message> {
    app.search_query = s;
    Task::none()
}

pub fn handle_set_view_mode(app: &mut App, m: crate::toolbar::ViewMode) -> Task<Message> {
    app.view_mode = m;
    Task::none()
}

pub fn handle_set_sort_by(app: &mut App, s: crate::filelist::SortBy) -> Task<Message> {
    if app.sort_by == s {
        app.sort_ascending = !app.sort_ascending;
    } else {
        app.sort_by = s;
        app.sort_ascending = true;
    }
    let sb = app.sort_by;
    let sa = app.sort_ascending;
    app.current_tab_mut().file_list.sort(sb, sa);
    Task::none()
}

pub fn handle_toggle_sort_order(app: &mut App) -> Task<Message> {
    app.sort_ascending = !app.sort_ascending;
    let sb = app.sort_by;
    let sa = app.sort_ascending;
    app.current_tab_mut().file_list.sort(sb, sa);
    Task::none()
}

pub fn handle_toggle_preview(app: &mut App) -> Task<Message> {
    app.preview_visible = !app.preview_visible;
    Task::none()
}
