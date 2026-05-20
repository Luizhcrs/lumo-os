//! Clipboard operations: copy, cut, paste, delete.
//!
//! ClipboardOp enum + handler functions.

use std::path::PathBuf;

use iced::Task;

use crate::app::{App, Message};
use crate::ops;

// ---------------------------------------------------------------------------
// ClipboardOp
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum ClipboardOp {
    Copy(Vec<PathBuf>),
    Cut(Vec<PathBuf>),
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub fn handle_copy(app: &mut App) -> Task<Message> {
    let paths = app.current_tab().file_list.selected_paths();
    if !paths.is_empty() {
        app.clipboard = Some(ClipboardOp::Copy(paths));
        app.status = "Copiado".to_string();
    }
    app.context_menu = None;
    Task::none()
}

pub fn handle_cut(app: &mut App) -> Task<Message> {
    let paths = app.current_tab().file_list.selected_paths();
    if !paths.is_empty() {
        app.clipboard = Some(ClipboardOp::Cut(paths));
        app.status = "Recortado".to_string();
    }
    app.context_menu = None;
    Task::none()
}

pub fn handle_paste(app: &mut App) -> Task<Message> {
    let dest = app.current_tab().current_dir.clone();
    match app.clipboard.clone() {
        Some(ClipboardOp::Copy(paths)) => {
            for p in &paths {
                if let Err(e) = ops::copy_to(p, &dest) {
                    app.status = format!("Colar falhou: {e}");
                    return Task::none();
                }
            }
        }
        Some(ClipboardOp::Cut(paths)) => {
            for p in &paths {
                if let Err(e) = ops::move_to(p, &dest) {
                    app.status = format!("Mover falhou: {e}");
                    return Task::none();
                }
            }
            app.clipboard = None;
        }
        None => {}
    }
    app.context_menu = None;
    app.update(Message::Refresh)
}

pub fn handle_delete(app: &mut App) -> Task<Message> {
    let paths = app.current_tab().file_list.selected_paths();
    for path in &paths {
        if let Err(e) = ops::move_to_trash(path) {
            app.status = format!("Erro lixeira: {e}");
            return Task::none();
        }
    }
    app.current_tab_mut().file_list.clear_selection();
    app.context_menu = None;
    app.update(Message::Refresh)
}
