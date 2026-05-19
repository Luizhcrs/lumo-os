//! Toolbar com botoes de navegacao, busca, nova pasta e toggle de view mode.
//!
//! Polish v2:
//!   - Height ~44 px, padding [8, 12].
//!   - Icon buttons 32 px alvo com radius 8, hover bg_subtle.
//!   - Search input com prefix icone search.
//!   - View toggle como segmented control 3-item (list / grid / columns).

use iced::widget::svg::Handle;
use iced::widget::{button, container, horizontal_space, row, text_input, Svg};
use iced::{Alignment, Border, Color, Element, Length};

use crate::app::Message;
use crate::icons;
use crate::theme::ThemeSnapshot;

/// View mode enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    Grid,
    List,
    Columns,
}

/// Renderiza a toolbar completa.
pub fn view<'a>(
    th: &ThemeSnapshot,
    can_back: bool,
    can_forward: bool,
    search_visible: bool,
    search_query: &'a str,
    view_mode: ViewMode,
    breadcrumb_el: Element<'a, Message>,
) -> Element<'a, Message> {
    let btn_back = icon_btn(th, icons::CHEVRON_LEFT, Message::NavigateBack, can_back, false);
    let btn_fwd = icon_btn(th, icons::CHEVRON_RIGHT, Message::NavigateForward, can_forward, false);
    let btn_up = icon_btn(th, icons::ARROW_UP, Message::NavigateUp, true, false);
    let btn_new = icon_btn(th, icons::PLUS, Message::NewFolder, true, false);
    let btn_search = icon_btn(th, icons::SEARCH, Message::ToggleSearch, true, search_visible);

    let seg_list = segment_btn(th, icons::LIST, Message::SetViewMode(ViewMode::List), view_mode == ViewMode::List);
    let seg_grid = segment_btn(th, icons::GRID, Message::SetViewMode(ViewMode::Grid), view_mode == ViewMode::Grid);
    let seg_cols = segment_btn(th, icons::COLUMNS, Message::SetViewMode(ViewMode::Columns), view_mode == ViewMode::Columns);

    let view_toggle = container(
        row![seg_list, seg_grid, seg_cols]
            .spacing(0)
            .align_y(Alignment::Center),
    )
    .padding(2)
    .style({
        let bg = th.bg;
        let bd = th.border;
        move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(bg)),
            border: Border { color: bd, width: 1.0, radius: 10.0.into() },
            ..Default::default()
        }
    });

    let search_el: Element<'a, Message> = if search_visible {
        let icon_color = th.fg_subtle;
        let icon_prefix = Svg::new(Handle::from_memory(icons::SEARCH))
            .width(Length::Fixed(12.0))
            .height(Length::Fixed(12.0))
            .style(move |_, _| iced::widget::svg::Style { color: Some(icon_color) });
        container(
            row![
                icon_prefix,
                text_input("Buscar nesta pasta...", search_query)
                    .on_input(Message::SearchChanged)
                    .size(13)
                    .padding([4, 0]),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .padding([4, 10])
        .width(Length::Fixed(240.0))
        .style({
            let bg = th.bg;
            let bd = th.border;
            move |_| iced::widget::container::Style {
                background: Some(iced::Background::Color(bg)),
                border: Border { color: bd, width: 1.0, radius: 8.0.into() },
                ..Default::default()
            }
        })
        .into()
    } else {
        horizontal_space().into()
    };

    let left: Element<'a, Message> = row![btn_back, btn_fwd, btn_up]
        .spacing(2)
        .align_y(Alignment::Center)
        .into();

    let center: Element<'a, Message> = container(breadcrumb_el)
        .width(Length::Fill)
        .into();

    let right: Element<'a, Message> = row![search_el, btn_search, btn_new, view_toggle]
        .spacing(6)
        .align_y(Alignment::Center)
        .into();

    container(
        row![left, center, right]
            .spacing(10)
            .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([8, 12])
    .style({
        let bg = th.bg_subtle;
        move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(bg)),
            border: Border::default(),
            ..Default::default()
        }
    })
    .into()
}

fn icon_btn<'a>(
    th: &ThemeSnapshot,
    bytes: &'static [u8],
    msg: Message,
    enabled: bool,
    active: bool,
) -> Element<'a, Message> {
    let bg = if active { th.accent_subtle } else { Color::TRANSPARENT };
    let icon_color = if !enabled {
        let mut c = th.fg_subtle;
        c.a = 0.4;
        c
    } else if active {
        th.accent
    } else {
        th.fg
    };
    let handle = Handle::from_memory(bytes);
    let icon = Svg::new(handle)
        .width(Length::Fixed(16.0))
        .height(Length::Fixed(16.0))
        .style(move |_, _| iced::widget::svg::Style { color: Some(icon_color) });
    let fg = th.fg;
    let mut btn = button(container(icon).width(Length::Fixed(16.0)).height(Length::Fixed(16.0)))
        .padding([8, 8])
        .style(move |_, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(bg)),
            border: Border { radius: 8.0.into(), ..Default::default() },
            text_color: fg,
            ..Default::default()
        });
    if enabled {
        btn = btn.on_press(msg);
    }
    btn.into()
}

fn segment_btn<'a>(
    th: &ThemeSnapshot,
    bytes: &'static [u8],
    msg: Message,
    active: bool,
) -> Element<'a, Message> {
    let bg = if active { th.accent_subtle } else { Color::TRANSPARENT };
    let icon_color = if active { th.accent } else { th.fg_subtle };
    let handle = Handle::from_memory(bytes);
    let icon = Svg::new(handle)
        .width(Length::Fixed(14.0))
        .height(Length::Fixed(14.0))
        .style(move |_, _| iced::widget::svg::Style { color: Some(icon_color) });
    let fg = th.fg;
    button(container(icon).width(Length::Fixed(14.0)).height(Length::Fixed(14.0)))
        .on_press(msg)
        .padding([6, 10])
        .style(move |_, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(bg)),
            border: Border { radius: 8.0.into(), ..Default::default() },
            text_color: fg,
            ..Default::default()
        })
        .into()
}
