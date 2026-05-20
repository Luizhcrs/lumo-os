//! Modal dialogs: new_folder, rename, properties, context_menu overlay.
//!
//! Handler functions take `&mut App` and return `Task<Message>`.
//! View functions return `Element<Message>`.

use std::path::PathBuf;

use iced::widget::{button, column, container, horizontal_rule, row, text, text_input};
use iced::{Alignment, Color, Element, Length, Task};

use crate::app::{App, Message};
use crate::ops;
use crate::theme::LumoTheme;

// ---------------------------------------------------------------------------
// PropertiesState
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PropertiesState {
    pub path: PathBuf,
    pub name_edit: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub fn handle_open_properties(app: &mut App) -> Task<Message> {
    let paths = app.current_tab().file_list.selected_paths();
    if let Some(path) = paths.into_iter().next() {
        let name = path.file_name()
            .unwrap_or_default().to_string_lossy().to_string();
        app.properties = Some(PropertiesState { path, name_edit: name });
    }
    Task::none()
}

pub fn handle_close_properties(app: &mut App) -> Task<Message> {
    app.properties = None;
    Task::none()
}

pub fn handle_properties_name_changed(app: &mut App, s: String) -> Task<Message> {
    if let Some(ref mut p) = app.properties {
        p.name_edit = s;
    }
    Task::none()
}

pub fn handle_properties_apply(app: &mut App) -> Task<Message> {
    if let Some(props) = app.properties.take() {
        let _ = ops::rename(&props.path, &props.name_edit);
        return app.update(Message::Refresh);
    }
    Task::none()
}

pub fn handle_new_folder(app: &mut App) -> Task<Message> {
    app.new_folder_input = Some("Nova pasta".to_string());
    app.context_menu = None;
    Task::none()
}

pub fn handle_new_folder_input_changed(app: &mut App, s: String) -> Task<Message> {
    app.new_folder_input = Some(s);
    Task::none()
}

pub fn handle_new_folder_confirm(app: &mut App) -> Task<Message> {
    if let Some(name) = app.new_folder_input.take() {
        match ops::mkdir(&app.current_tab().current_dir.clone(), &name) {
            Ok(_) => return app.update(Message::Refresh),
            Err(e) => app.status = format!("Criar pasta falhou: {e}"),
        }
    }
    Task::none()
}

pub fn handle_new_folder_cancel(app: &mut App) -> Task<Message> {
    app.new_folder_input = None;
    Task::none()
}

// ---------------------------------------------------------------------------
// View: new folder inline bar
// ---------------------------------------------------------------------------

pub fn view_new_folder_bar<'a>(
    name: &'a str,
    fg: Color,
    muted: Color,
    panel: Color,
    panel_hi: Color,
) -> Element<'a, Message> {
    let input = text_input("Nome da pasta", name)
        .on_input(Message::NewFolderInputChanged)
        .on_submit(Message::NewFolderConfirm)
        .size(13)
        .padding([6, 10]);
    let btn_ok = button(text("OK").size(12).color(fg))
        .on_press(Message::NewFolderConfirm)
        .style(move |_, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(panel_hi)),
            border: iced::Border { radius: 4.0.into(), ..Default::default() },
            text_color: LumoTheme::fg(),
            ..Default::default()
        })
        .padding([4, 10]);
    let btn_cancel = button(text("Cancelar").size(12).color(muted))
        .on_press(Message::NewFolderCancel)
        .style(move |_, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(Color::TRANSPARENT)),
            border: iced::Border { radius: 4.0.into(), ..Default::default() },
            text_color: LumoTheme::fg(),
            ..Default::default()
        })
        .padding([4, 10]);
    container(
        row![
            text("Nova pasta:").size(13).color(muted),
            input,
            btn_ok,
            btn_cancel,
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([6, 12])
    .width(Length::Fill)
    .style(move |_| iced::widget::container::Style {
        background: Some(iced::Background::Color(panel)),
        ..Default::default()
    })
    .into()
}

// ---------------------------------------------------------------------------
// View: properties dialog overlay
// ---------------------------------------------------------------------------

pub fn view_properties_dialog<'a>(
    props: &'a PropertiesState,
    fg: Color,
    muted: Color,
) -> Element<'a, Message> {
    let name_input = text_input("Nome", &props.name_edit)
        .on_input(Message::PropertiesNameChanged)
        .on_submit(Message::PropertiesApply)
        .size(13)
        .padding([6, 10]);

    let path = &props.path;
    let size_str = crate::filelist::FileList::human_size(path);
    let mod_str = crate::filelist::FileList::human_modified(path);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("--").to_string();
    let perms = {
        use std::os::unix::fs::PermissionsExt;
        path.metadata().map(|m| format!("{:o}", m.permissions().mode() & 0o777)).unwrap_or_else(|_| "--".to_string())
    };

    let btn_apply = button(text("Aplicar").size(13).color(fg))
        .on_press(Message::PropertiesApply)
        .style(move |_, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(LumoTheme::accent())),
            border: iced::Border { radius: 4.0.into(), ..Default::default() },
            text_color: LumoTheme::bg(),
            ..Default::default()
        })
        .padding([6, 12]);

    let btn_cancel = button(text("Cancelar").size(13).color(muted))
        .on_press(Message::CloseProperties)
        .style(move |_, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(Color::TRANSPARENT)),
            border: iced::Border { radius: 4.0.into(), ..Default::default() },
            text_color: LumoTheme::muted(),
            ..Default::default()
        })
        .padding([6, 12]);

    container(
        column![
            text("Propriedades").size(16).color(fg),
            container(horizontal_rule(1)).padding([4, 0]).width(Length::Fill),
            text("Nome:").size(12).color(muted),
            name_input,
            text("Tamanho:").size(12).color(muted),
            text(size_str).size(13).color(fg),
            text("Modificado:").size(12).color(muted),
            text(mod_str).size(13).color(fg),
            text("Tipo:").size(12).color(muted),
            text(ext).size(13).color(fg),
            text("Permissoes:").size(12).color(muted),
            text(perms).size(13).color(fg),
            container(horizontal_rule(1)).padding([4, 0]).width(Length::Fill),
            row![btn_apply, btn_cancel].spacing(8),
        ]
        .spacing(6)
        .padding([20, 24]),
    )
    .width(Length::Fixed(400.0))
    .style(move |_| iced::widget::container::Style {
        background: Some(iced::Background::Color(LumoTheme::panel_hi())),
        border: iced::Border {
            color: LumoTheme::sep(),
            width: 1.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    })
    .into()
}
