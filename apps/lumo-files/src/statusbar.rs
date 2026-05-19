//! Status bar inferior.
//!
//! Polish v2:
//!   - Height ~26 px, padding [4, 12], bg = bg_subtle, border-top 1 px.
//!   - "N itens"  ou  "N de M selecionados"  + bullet  +  "X livre de Y".

use iced::widget::{container, row, text};
use iced::{Alignment, Border, Element, Length};

use crate::app::Message;
use crate::theme::ThemeSnapshot;

/// Formata a parte de selecao.
pub fn format_selection(selected: usize, total: usize) -> String {
    if selected == 0 {
        match total {
            0 => "Pasta vazia".to_string(),
            1 => "1 item".to_string(),
            n => format!("{n} itens"),
        }
    } else {
        format!("{selected} de {total} selecionados")
    }
}

/// Formata bytes livres -> "X.X GB livres de Y.Y GB"
pub fn format_disk(free_bytes: u64, total_bytes: u64) -> String {
    let to_gb = |b: u64| (b as f64) / (1024.0 * 1024.0 * 1024.0);
    if total_bytes == 0 {
        return String::new();
    }
    format!("{:.1} GB livres de {:.1} GB", to_gb(free_bytes), to_gb(total_bytes))
}

/// Renderiza a status bar inteira.
pub fn view<'a>(
    th: &ThemeSnapshot,
    selected: usize,
    total: usize,
    free_bytes: u64,
    total_bytes: u64,
    status: &'a str,
) -> Element<'a, Message> {
    let left = text(format_selection(selected, total)).size(11).color(th.fg_subtle);

    let mid: Element<'a, Message> = if !status.is_empty() {
        row![
            text("\u{2022}").size(11).color(th.fg_subtle),
            text(status.to_string()).size(11).color(th.fg),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
    } else {
        iced::widget::horizontal_space().into()
    };

    let right_str = format_disk(free_bytes, total_bytes);
    let right_el: Element<'a, Message> = if right_str.is_empty() {
        iced::widget::horizontal_space().into()
    } else {
        text(right_str).size(11).color(th.fg_subtle).into()
    };

    container(
        row![
            left,
            iced::widget::horizontal_space(),
            mid,
            iced::widget::horizontal_space(),
            right_el,
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([4, 12])
    .style({
        let bg = th.bg_subtle;
        let bd = th.border;
        move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(bg)),
            border: Border {
                color: bd,
                width: 0.0,
                ..Default::default()
            },
            ..Default::default()
        }
    })
    .into()
}

/// Le statvfs(3) via syscall via `df --output=avail,size --block-size=1`.
/// Simples e portavel sem novas crates. Roda em spawn_blocking pelo caller.
/// Em erro retorna (0, 0).
pub fn disk_usage(path: &std::path::Path) -> (u64, u64) {
    use std::process::Command;
    let out = match Command::new("df")
        .args(["--output=avail,size", "--block-size=1"])
        .arg(path)
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return (0, 0),
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut lines = stdout.lines();
    let _ = lines.next(); // header
    if let Some(line) = lines.next() {
        let mut parts = line.split_whitespace();
        let avail = parts.next().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
        let total = parts.next().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
        return (avail, total);
    }
    (0, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_zero_total_zero_e_pasta_vazia() {
        assert_eq!(format_selection(0, 0), "Pasta vazia");
    }

    #[test]
    fn selection_zero_total_um_e_1_item() {
        assert_eq!(format_selection(0, 1), "1 item");
    }

    #[test]
    fn selection_zero_total_n_e_n_itens() {
        assert_eq!(format_selection(0, 5), "5 itens");
    }

    #[test]
    fn selection_com_selecionados_mostra_de_total() {
        assert_eq!(format_selection(2, 5), "2 de 5 selecionados");
    }

    #[test]
    fn format_disk_zero_total_e_vazio() {
        assert_eq!(format_disk(0, 0), "");
    }

    #[test]
    fn format_disk_formata_gb() {
        let s = format_disk(1024 * 1024 * 1024, 4 * 1024 * 1024 * 1024);
        assert!(s.contains("1.0 GB"));
        assert!(s.contains("4.0 GB"));
    }
}
