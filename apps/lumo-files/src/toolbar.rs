//! Toolbar com botoes de navegacao, busca, nova pasta e toggle de view mode.

use iced::widget::svg::Handle;
use iced::widget::{button, container, horizontal_space, row, text_input, Svg};
use iced::{Alignment, Color, Element, Length};

use crate::app::Message;
use crate::theme::LumoTheme;

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
    can_back: bool,
    can_forward: bool,
    search_visible: bool,
    search_query: &'a str,
    view_mode: ViewMode,
    breadcrumb_el: Element<'a, Message>,
) -> Element<'a, Message> {
    let btn_back = icon_btn(include_bytes!("../icons/chevron_left.svg"), Message::NavigateBack, can_back);
    let btn_fwd = icon_btn(include_bytes!("../icons/chevron_right.svg"), Message::NavigateForward, can_forward);
    let btn_up = icon_btn(include_bytes!("../icons/arrow_up.svg"), Message::NavigateUp, true);
    let btn_search = icon_btn_active(include_bytes!("../icons/search.svg"), Message::ToggleSearch, true, search_visible);
    let btn_new = icon_btn(include_bytes!("../icons/plus.svg"), Message::NewFolder, true);
    let btn_grid = view_mode_btn(include_bytes!("../icons/grid.svg"), Message::SetViewMode(ViewMode::Grid), view_mode == ViewMode::Grid);
    let btn_list = view_mode_btn(include_bytes!("../icons/list.svg"), Message::SetViewMode(ViewMode::List), view_mode == ViewMode::List);
    let btn_cols = view_mode_btn(include_bytes!("../icons/columns.svg"), Message::SetViewMode(ViewMode::Columns), view_mode == ViewMode::Columns);

    let search_el: Element<'a, Message> = if search_visible {
        text_input("Buscar...", search_query)
            .on_input(Message::SearchChanged)
            .size(13)
            .padding([4, 8])
            .into()
    } else {
        horizontal_space().into()
    };

    let left: Element<'a, Message> = row![btn_back, btn_fwd, btn_up]
        .spacing(2)
        .align_y(Alignment::Center)
        .into();

    let center: Element<'a, Message> = container(
        row![breadcrumb_el, search_el]
            .spacing(8)
            .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .into();

    let right: Element<'a, Message> = row![btn_search, btn_new, btn_grid, btn_list, btn_cols]
        .spacing(2)
        .align_y(Alignment::Center)
        .into();

    container(
        row![left, center, right]
            .spacing(6)
            .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([6, 12])
    .style(|_| iced::widget::container::Style {
        background: Some(iced::Background::Color(LumoTheme::panel())),
        ..Default::default()
    })
    .into()
}

fn icon_btn<'a>(bytes: &'static [u8], msg: Message, enabled: bool) -> Element<'a, Message> {
    icon_btn_active(bytes, msg, enabled, false)
}

fn icon_btn_active<'a>(
    bytes: &'static [u8],
    msg: Message,
    enabled: bool,
    active: bool,
) -> Element<'a, Message> {
    let bg = if active { LumoTheme::accent_alpha40() } else { Color::TRANSPARENT };
    let handle = Handle::from_memory(bytes);
    let icon = Svg::new(handle).width(Length::Fixed(16.0)).height(Length::Fixed(16.0));
    let btn = button(icon)
        .padding([4, 6])
        .style(move |_, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(bg)),
            border: iced::Border { radius: 4.0.into(), ..Default::default() },
            text_color: LumoTheme::fg(),
            ..Default::default()
        });
    if enabled {
        btn.on_press(msg).into()
    } else {
        let handle2 = Handle::from_memory(bytes);
        let icon2 = Svg::new(handle2).width(Length::Fixed(16.0)).height(Length::Fixed(16.0));
        button(icon2)
            .padding([4, 6])
            .style(move |_, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(Color::TRANSPARENT)),
                border: iced::Border { radius: 4.0.into(), ..Default::default() },
                text_color: LumoTheme::muted(),
                ..Default::default()
            })
            .into()
    }
}

fn view_mode_btn<'a>(bytes: &'static [u8], msg: Message, active: bool) -> Element<'a, Message> {
    let bg = if active { LumoTheme::accent_alpha40() } else { Color::TRANSPARENT };
    let handle = Handle::from_memory(bytes);
    let icon = Svg::new(handle).width(Length::Fixed(16.0)).height(Length::Fixed(16.0));
    button(icon)
        .on_press(msg)
        .padding([4, 6])
        .style(move |_, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(bg)),
            border: iced::Border { radius: 4.0.into(), ..Default::default() },
            text_color: LumoTheme::fg(),
            ..Default::default()
        })
        .into()
}
