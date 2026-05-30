//! Tab state + tab bar view + tab message handlers.
//!
//! Tab struct is the canonical source; re-exported via `crate::app::Tab`.
//! View: tab bar below the toolbar (Polish v2 pill style).
//! Handlers: NewTab, CloseTab, SwitchTab, TabNavigate, TabDirLoaded.

use std::path::PathBuf;

use iced::widget::svg::Handle;
use iced::widget::{button, container, row, text, Svg};
use iced::{Alignment, Border, Color, Element, Length, Task};

use crate::app::{App, Message};
use crate::icons;
use crate::theme::ThemeSnapshot;

// ---------------------------------------------------------------------------
// Tab struct
// Tab struct is canonical in app.rs
use crate::app::Tab;

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub fn handle_new_tab(app: &mut App) -> Task<Message> {
    let dir = app.current_tab().current_dir.clone();
    let tab = Tab::new(dir.clone());
    app.tabs.push(tab);
    app.active_tab = app.tabs.len() - 1;
    let idx = app.active_tab;
    let show_hidden = app.show_hidden;
    Task::perform(
        crate::app::load_dir(dir.clone(), show_hidden),
        move |r| match r {
            Ok(entries) => Message::TabDirLoaded(idx, dir.clone(), entries),
            Err(e) => Message::OpError(e),
        },
    )
}

pub fn handle_close_tab(app: &mut App, idx: usize) -> Task<Message> {
    if app.tabs.len() > 1 {
        app.tabs.remove(idx);
        app.active_tab = app.active_tab.min(app.tabs.len() - 1);
    }
    Task::none()
}

pub fn handle_switch_tab(app: &mut App, idx: usize) -> Task<Message> {
    if idx < app.tabs.len() {
        app.active_tab = idx;
        if let Some(tab) = app.tabs.get(idx) {
            let dir = tab.current_dir.clone();
            let dir2 = dir.clone();
            let show_hidden = app.show_hidden;
            return Task::perform(crate::app::load_dir(dir, show_hidden), move |r| match r {
                Ok(entries) => Message::DirLoaded(dir2.clone(), entries),
                Err(e) => Message::OpError(e),
            });
        }
    }
    Task::none()
}

pub fn handle_tab_navigate(app: &mut App, idx: usize, path: PathBuf) -> Task<Message> {
    if idx < app.tabs.len() {
        app.tabs[idx].current_dir = path.clone();
        let p2 = path.clone();
        let show_hidden2 = app.show_hidden;
        return Task::perform(crate::app::load_dir(path, show_hidden2), move |r| match r {
            Ok(entries) => Message::TabDirLoaded(idx, p2.clone(), entries),
            Err(e) => Message::OpError(e),
        });
    }
    Task::none()
}

pub fn handle_tab_dir_loaded(
    app: &mut App,
    idx: usize,
    path: PathBuf,
    entries: Vec<PathBuf>,
) -> Task<Message> {
    if let Some(tab) = app.tabs.get_mut(idx) {
        tab.label = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "/".to_string());
        tab.current_dir = path;
        tab.file_list.set_entries(entries);
        tab.file_list.sort(app.sort_by, app.sort_ascending);
    }
    Task::none()
}

// ---------------------------------------------------------------------------
// View: tab bar
// ---------------------------------------------------------------------------

pub fn view<'a>(th: &ThemeSnapshot, tabs: &'a [Tab], active: usize) -> Element<'a, Message> {
    let mut row_el = row![].spacing(2).align_y(Alignment::Center);

    for (i, tab) in tabs.iter().enumerate() {
        let is_active = i == active;
        row_el = row_el.push(tab_pill(th, i, &tab.label, is_active));
    }

    row_el = row_el.push(new_tab_btn(th));

    container(row_el)
        .width(Length::Fill)
        .padding([4, 8])
        .style({
            // W38: tab bar usa th.bg (cor do CONTEUDO), nao bg_subtle. Antes
            // toolbar(bg_subtle) + tab_bar(bg_subtle) empilhavam duas faixas
            // cinza identicas = "faixa dupla" no topo. Agora a tab bar funde com
            // o conteudo abaixo + hairline no topo separa da toolbar.
            let bg = th.bg;
            let bd = th.border;
            move |_| iced::widget::container::Style {
                background: Some(iced::Background::Color(bg)),
                border: Border {
                    color: bd,
                    width: 0.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            }
        })
        .into()
}

fn tab_pill<'a>(
    th: &ThemeSnapshot,
    idx: usize,
    label: &'a str,
    active: bool,
) -> Element<'a, Message> {
    let label_color = if active { th.fg } else { th.fg_subtle };
    let bg = if active { th.bg } else { Color::TRANSPARENT };

    let mut close_color = th.fg_subtle;
    close_color.a = 0.6;
    let close_btn = button(text("x").size(11).color(close_color))
        .on_press(Message::CloseTab(idx))
        .padding([2, 6])
        .style({
            let fg = th.fg;
            move |_, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(Color::TRANSPARENT)),
                border: Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                text_color: fg,
                ..Default::default()
            }
        });

    let content = row![
        text(label.to_string()).size(12).color(label_color),
        close_btn,
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let body = button(container(content).padding([6, 10]))
        .on_press(Message::SwitchTab(idx))
        .padding(0)
        .style({
            let fg = th.fg;
            move |_, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(bg)),
                border: Border {
                    radius: iced::border::Radius {
                        top_left: 8.0,
                        top_right: 8.0,
                        bottom_left: 0.0,
                        bottom_right: 0.0,
                    },
                    ..Default::default()
                },
                text_color: fg,
                ..Default::default()
            }
        });

    let underline_color = if active {
        th.accent
    } else {
        Color::TRANSPARENT
    };
    let underline = container(iced::widget::horizontal_space())
        .height(Length::Fixed(2.0))
        .width(Length::Fill)
        .style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(underline_color)),
            ..Default::default()
        });

    iced::widget::column![body, underline].spacing(0).into()
}

fn new_tab_btn<'a>(th: &ThemeSnapshot) -> Element<'a, Message> {
    let icon_color = th.fg_subtle;
    let icon = Svg::new(Handle::from_memory(icons::PLUS))
        .width(Length::Fixed(12.0))
        .height(Length::Fixed(12.0))
        .style(move |_, _| iced::widget::svg::Style {
            color: Some(icon_color),
        });
    button(
        container(icon)
            .width(Length::Fixed(12.0))
            .height(Length::Fixed(12.0)),
    )
    .on_press(Message::NewTab)
    .padding([6, 10])
    .style({
        let fg = th.fg;
        move |_, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(Color::TRANSPARENT)),
            border: Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            text_color: fg,
            ..Default::default()
        }
    })
    .into()
}
