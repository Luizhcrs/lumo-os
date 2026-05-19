//! Tab bar abaixo da toolbar.
//!
//! Polish v2:
//!   - Tab pill com padding [8, 14], radius topo 8.
//!   - Inactive: fg_subtle text, transparent bg.
//!   - Active: fg text + bg = th.bg + 2 px accent underline.
//!   - Close button sempre visivel a 60% opacity.
//!   - + button no fim.

use iced::widget::svg::Handle;
use iced::widget::{button, container, row, text, Svg};
use iced::{Alignment, Border, Color, Element, Length};

use crate::app::{Message, Tab};
use crate::icons;
use crate::theme::ThemeSnapshot;

pub fn view<'a>(th: &ThemeSnapshot, tabs: &'a [Tab], active: usize) -> Element<'a, Message> {
    let mut row_el = row![].spacing(2).align_y(Alignment::Center);

    for (i, tab) in tabs.iter().enumerate() {
        let is_active = i == active;
        row_el = row_el.push(tab_pill(th, i, &tab.label, is_active));
    }

    // + new tab
    row_el = row_el.push(new_tab_btn(th));

    container(row_el)
        .width(Length::Fill)
        .padding([4, 8])
        .style({
            let bg = th.bg_subtle;
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

fn tab_pill<'a>(th: &ThemeSnapshot, idx: usize, label: &'a str, active: bool) -> Element<'a, Message> {
    let label_color = if active { th.fg } else { th.fg_subtle };
    let bg = if active { th.bg } else { Color::TRANSPARENT };

    // Close x — sempre visivel a 60% opacidade.
    let mut close_color = th.fg_subtle;
    close_color.a = 0.6;
    let close_btn = button(text("x").size(11).color(close_color))
        .on_press(Message::CloseTab(idx))
        .padding([2, 6])
        .style({
            let fg = th.fg;
            move |_, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(Color::TRANSPARENT)),
                border: Border { radius: 4.0.into(), ..Default::default() },
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

    // Outer column: button (tab body) + underline strip below.
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

    let underline_color = if active { th.accent } else { Color::TRANSPARENT };
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
        .style(move |_, _| iced::widget::svg::Style { color: Some(icon_color) });
    button(container(icon).width(Length::Fixed(12.0)).height(Length::Fixed(12.0)))
        .on_press(Message::NewTab)
        .padding([6, 10])
        .style({
            let fg = th.fg;
            move |_, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(Color::TRANSPARENT)),
                border: Border { radius: 8.0.into(), ..Default::default() },
                text_color: fg,
                ..Default::default()
            }
        })
        .into()
}
