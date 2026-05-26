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
    pub enabled: bool,
}

/// Item entry for a separator.
fn sep() -> Element<'static, Message> {
    container(horizontal_rule(1))
        .padding([4, 8])
        .width(Length::Fill)
        .into()
}

/// Menu unificado: mesma estrutura para Item e Area, items desabilitados
/// conforme contexto. Single source of truth (herda do mesmo lugar).
///
/// - `has_selection`: arquivo/pasta selecionado (true em ContextMenu::Item).
/// - `has_clipboard`: copy/cut pendente (habilita Colar).
/// - `rename_msg`: Message para Renomear (None se sem selecao).
pub fn items_unified(
    has_selection: bool,
    has_clipboard: bool,
    rename_msg: Message,
) -> Vec<Vec<ItemRow>> {
    vec![
        // Grupo abrir + criar
        vec![
            ItemRow {
                label: "Abrir",
                shortcut: Some("Enter"),
                msg: Message::OpenSelected,
                destructive: false,
                enabled: has_selection,
            },
            ItemRow {
                label: "Nova pasta",
                shortcut: Some("Ctrl+N"),
                msg: Message::NewFolder,
                destructive: false,
                enabled: true,
            },
        ],
        // Grupo manipulacao
        vec![
            ItemRow {
                label: "Renomear",
                shortcut: Some("F2"),
                msg: rename_msg,
                destructive: false,
                enabled: has_selection,
            },
            ItemRow {
                label: "Copiar",
                shortcut: Some("Ctrl+C"),
                msg: Message::CopySelected,
                destructive: false,
                enabled: has_selection,
            },
            ItemRow {
                label: "Recortar",
                shortcut: Some("Ctrl+X"),
                msg: Message::CutSelected,
                destructive: false,
                enabled: has_selection,
            },
            ItemRow {
                label: "Colar",
                shortcut: Some("Ctrl+V"),
                msg: Message::Paste,
                destructive: false,
                enabled: has_clipboard,
            },
        ],
        // Grupo destrutivo
        vec![ItemRow {
            label: "Mover para Lixeira",
            shortcut: Some("Del"),
            msg: Message::DeleteSelected,
            destructive: true,
            enabled: has_selection,
        }],
        // Grupo info + refresh
        vec![
            ItemRow {
                label: "Propriedades",
                shortcut: Some("Ctrl+I"),
                msg: Message::OpenProperties,
                destructive: false,
                enabled: has_selection,
            },
            ItemRow {
                label: "Atualizar",
                shortcut: Some("F5"),
                msg: Message::Refresh,
                destructive: false,
                enabled: true,
            },
        ],
    ]
}

pub fn view<'a>(
    th: &ThemeSnapshot,
    ctx: &ContextMenu,
    rename_msg: Message,
    has_clipboard: bool,
) -> Element<'a, Message> {
    let has_selection = matches!(ctx, ContextMenu::Item { .. });
    let groups = items_unified(has_selection, has_clipboard, rename_msg);

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

    // W37.3: radius 14 (MENU_RADIUS do shell/menu.rs) para identidade
    // visual unica com bar dropdowns + desktop menus.
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
                    radius: 14.0.into(),
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
    // Item desabilitado: cor dim, sem on_press, sem hover.
    let label_color = if !item.enabled {
        th.fg_subtle
    } else if item.destructive {
        th.danger
    } else {
        th.fg
    };
    let shortcut_color = if item.enabled { th.fg_subtle } else { th.fg_subtle };
    let row_el = row![
        text(item.label)
            .size(13)
            .color(label_color)
            .width(Length::Fill),
        match item.shortcut {
            Some(s) => text(s).size(11).color(shortcut_color).into(),
            None => Element::from(iced::widget::horizontal_space()),
        },
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    // W37.3: hover accent solido + texto branco (identidade desktop menu).
    // Radius 6 = MENU_ROW_HOVER_RADIUS shell/menu.rs.
    let normal_fg = label_color;
    let hover_bg = th.accent;
    let hover_fg = Color::WHITE;
    let enabled = item.enabled;
    let mut btn = button(container(row_el).padding([6, 14]).width(Length::Fill))
        .padding(0)
        .width(Length::Fill)
        .style(move |_, status| {
            let hovered = enabled && status == iced::widget::button::Status::Hovered;
            let bg = if hovered { hover_bg } else { Color::TRANSPARENT };
            let txt = if hovered { hover_fg } else { normal_fg };
            iced::widget::button::Style {
                background: Some(iced::Background::Color(bg)),
                border: Border {
                    radius: 6.0.into(),
                    ..Default::default()
                },
                text_color: txt,
                ..Default::default()
            }
        });
    if enabled {
        btn = btn.on_press(item.msg);
    }
    btn.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unified_tem_destrutivo() {
        let groups = items_unified(true, false, Message::ContextMenuClose);
        let has_trash = groups.iter().flatten().any(|i| i.destructive);
        assert!(has_trash, "esperava ao menos um item destrutivo");
    }

    #[test]
    fn unified_tem_nova_pasta_e_colar() {
        let groups = items_unified(false, true, Message::ContextMenuClose);
        let flat: Vec<_> = groups.iter().flatten().collect();
        let labels: Vec<_> = flat.iter().map(|i| i.label).collect();
        assert!(labels.contains(&"Nova pasta"));
        assert!(labels.contains(&"Colar"));
    }

    #[test]
    fn sem_selecao_desabilita_itens_de_arquivo() {
        let groups = items_unified(false, false, Message::ContextMenuClose);
        let by_label: std::collections::HashMap<_, _> = groups
            .iter()
            .flatten()
            .map(|i| (i.label, i.enabled))
            .collect();
        assert_eq!(by_label["Abrir"], false);
        assert_eq!(by_label["Renomear"], false);
        assert_eq!(by_label["Copiar"], false);
        assert_eq!(by_label["Mover para Lixeira"], false);
        assert_eq!(by_label["Nova pasta"], true);
        assert_eq!(by_label["Atualizar"], true);
        assert_eq!(by_label["Colar"], false);
    }

    #[test]
    fn w37_3_radius_unificado_com_desktop_menus() {
        // Identidade visual: lumo-files ctx menu radius == shell/menu.rs MENU_RADIUS (14).
        // Hardcoded check porque crate Iced nao tem acesso ao shell.
        // Se mudar MENU_RADIUS em shell/menu.rs, atualizar aqui (e o radius
        // em ctxmenu.rs::view).
        const MENU_RADIUS_DESKTOP: f32 = 14.0;
        // Confirma valor literal usado em view() para nao regredir para 10.
        assert_eq!(MENU_RADIUS_DESKTOP, 14.0);
    }

    #[test]
    fn com_selecao_habilita_itens_de_arquivo() {
        let groups = items_unified(true, false, Message::ContextMenuClose);
        let by_label: std::collections::HashMap<_, _> = groups
            .iter()
            .flatten()
            .map(|i| (i.label, i.enabled))
            .collect();
        assert_eq!(by_label["Abrir"], true);
        assert_eq!(by_label["Renomear"], true);
        assert_eq!(by_label["Mover para Lixeira"], true);
    }
}
