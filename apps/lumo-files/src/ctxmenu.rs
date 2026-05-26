//! Floating context menu para arquivo / pasta / area vazia.
//!
//! Polish v2:
//!   - Panel rounded 10 px, border 1 px neutro, shadow neutro.
//!   - Items 13 px com hover bg_subtle.
//!   - Separadores entre grupos.

use iced::widget::{button, column, container, horizontal_rule, row, text};
use iced::{Alignment, Border, Color, Element, Length};

use crate::app::{ContextMenu, Message};
use crate::theme::ThemeSnapshot;

#[derive(Debug, Clone)]
pub struct ItemRow {
    pub label: &'static str,
    pub shortcut: Option<&'static str>,
    pub msg: Message,
    pub destructive: bool,
}

/// Item entry for a separator.
fn sep() -> Element<'static, Message> {
    container(horizontal_rule(1))
        .padding([4, 8])
        .width(Length::Fill)
        .into()
}

pub fn items_for_item(rename_msg: Message) -> Vec<Vec<ItemRow>> {
    vec![
        vec![ItemRow {
            label: "Abrir",
            shortcut: Some("Enter"),
            msg: Message::OpenSelected,
            destructive: false,
        }],
        vec![
            ItemRow {
                label: "Renomear",
                shortcut: Some("F2"),
                msg: rename_msg,
                destructive: false,
            },
            ItemRow {
                label: "Copiar",
                shortcut: Some("Ctrl+C"),
                msg: Message::CopySelected,
                destructive: false,
            },
            ItemRow {
                label: "Recortar",
                shortcut: Some("Ctrl+X"),
                msg: Message::CutSelected,
                destructive: false,
            },
        ],
        vec![ItemRow {
            label: "Mover para Lixeira",
            shortcut: Some("Del"),
            msg: Message::DeleteSelected,
            destructive: true,
        }],
        vec![ItemRow {
            label: "Propriedades",
            shortcut: Some("Ctrl+I"),
            msg: Message::OpenProperties,
            destructive: false,
        }],
    ]
}

pub fn items_for_area() -> Vec<Vec<ItemRow>> {
    vec![
        vec![
            ItemRow {
                label: "Nova pasta",
                shortcut: Some("Ctrl+N"),
                msg: Message::NewFolder,
                destructive: false,
            },
            ItemRow {
                label: "Colar",
                shortcut: Some("Ctrl+V"),
                msg: Message::Paste,
                destructive: false,
            },
        ],
        vec![ItemRow {
            label: "Atualizar",
            shortcut: Some("F5"),
            msg: Message::Refresh,
            destructive: false,
        }],
    ]
}

pub fn view<'a>(
    th: &ThemeSnapshot,
    ctx: &ContextMenu,
    rename_msg: Message,
) -> Element<'a, Message> {
    let groups = match ctx {
        ContextMenu::Item { .. } => items_for_item(rename_msg),
        ContextMenu::Area { .. } => items_for_area(),
    };

    let mut col = column![].spacing(0).padding([6, 0]);
    let last = groups.len().saturating_sub(1);
    for (i, group) in groups.into_iter().enumerate() {
        for item in group {
            col = col.push(menu_btn(th, item));
        }
        if i != last {
            col = col.push(sep());
        }
    }

    container(col)
        .width(Length::Fixed(240.0))
        .style({
            let bg = th.bg;
            let bd = th.border;
            move |_| iced::widget::container::Style {
                background: Some(iced::Background::Color(bg)),
                border: Border {
                    color: bd,
                    width: 1.0,
                    radius: 10.0.into(),
                },
                shadow: iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.35),
                    offset: iced::Vector { x: 0.0, y: 4.0 },
                    blur_radius: 16.0,
                },
                ..Default::default()
            }
        })
        .into()
}

fn menu_btn<'a>(th: &ThemeSnapshot, item: ItemRow) -> Element<'a, Message> {
    let label_color = if item.destructive { th.danger } else { th.fg };
    let row_el = row![
        text(item.label)
            .size(13)
            .color(label_color)
            .width(Length::Fill),
        match item.shortcut {
            Some(s) => text(s).size(11).color(th.fg_subtle).into(),
            None => Element::from(iced::widget::horizontal_space()),
        },
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    let fg = th.fg;
    let hover = th.bg_subtle;
    button(container(row_el).padding([6, 14]).width(Length::Fill))
        .on_press(item.msg)
        .padding(0)
        .width(Length::Fill)
        .style(move |_, status| {
            let bg = if status == iced::widget::button::Status::Hovered {
                hover
            } else {
                Color::TRANSPARENT
            };
            iced::widget::button::Style {
                background: Some(iced::Background::Color(bg)),
                border: Border {
                    radius: 6.0.into(),
                    ..Default::default()
                },
                text_color: fg,
                ..Default::default()
            }
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_for_item_tem_grupo_destrutivo() {
        let groups = items_for_item(Message::ContextMenuClose);
        let has_trash = groups.iter().flatten().any(|i| i.destructive);
        assert!(has_trash, "esperava ao menos um item destrutivo");
    }

    #[test]
    fn items_for_area_tem_nova_pasta_e_colar() {
        let groups = items_for_area();
        let flat: Vec<_> = groups.iter().flatten().collect();
        let labels: Vec<_> = flat.iter().map(|i| i.label).collect();
        assert!(labels.contains(&"Nova pasta"));
        assert!(labels.contains(&"Colar"));
    }

    #[test]
    fn items_for_item_tem_atalhos() {
        let groups = items_for_item(Message::ContextMenuClose);
        let with_shortcut: Vec<_> = groups
            .iter()
            .flatten()
            .filter(|i| i.shortcut.is_some())
            .collect();
        assert!(with_shortcut.len() >= 4);
    }
}
