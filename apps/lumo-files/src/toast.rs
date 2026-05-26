//! Toast queue para erros nao-criticos.
//!
//! Polish v2:
//!   - Bottom-right anchor, max 3 visiveis.
//!   - Auto-fade apos 4 s via Tick.
//!   - Sem dialog modal — discreto.

use std::time::{Duration, Instant};

use iced::widget::{column, container, row, text};
use iced::{Alignment, Border, Color, Element, Length};

use crate::app::Message;
use crate::theme::ThemeSnapshot;

/// Severidade do toast.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub kind: ToastKind,
    pub message: String,
    pub created: Instant,
}

impl Toast {
    pub fn new(kind: ToastKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            created: Instant::now(),
        }
    }

    pub fn is_expired(&self, now: Instant, ttl: Duration) -> bool {
        now.duration_since(self.created) > ttl
    }
}

#[derive(Debug, Default)]
pub struct ToastQueue {
    items: Vec<Toast>,
    max_visible: usize,
}

impl ToastQueue {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_visible: 3,
        }
    }

    pub fn push(&mut self, toast: Toast) {
        if self.items.len() >= self.max_visible {
            self.items.remove(0);
        }
        self.items.push(toast);
    }

    pub fn evict_expired(&mut self, ttl: Duration) {
        let now = Instant::now();
        self.items.retain(|t| !t.is_expired(now, ttl));
    }

    pub fn items(&self) -> &[Toast] {
        &self.items
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

pub fn view<'a>(th: &ThemeSnapshot, queue: &'a ToastQueue) -> Element<'a, Message> {
    let mut col = column![].spacing(8);
    for t in queue.items().iter().rev() {
        col = col.push(toast_card(th, t));
    }
    container(col)
        .padding(16)
        .align_x(iced::alignment::Horizontal::Right)
        .align_y(iced::alignment::Vertical::Bottom)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn toast_card<'a>(th: &ThemeSnapshot, toast: &'a Toast) -> Element<'a, Message> {
    let accent = match toast.kind {
        ToastKind::Info => th.accent,
        ToastKind::Warning => Color::from_rgb(0.92, 0.65, 0.20),
        ToastKind::Error => th.danger,
    };
    let kind_label = match toast.kind {
        ToastKind::Info => "i",
        ToastKind::Warning => "!",
        ToastKind::Error => "x",
    };
    let stripe = container(iced::widget::horizontal_space())
        .width(Length::Fixed(3.0))
        .height(Length::Fixed(28.0))
        .style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(accent)),
            border: Border {
                radius: 2.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });

    let body = row![
        stripe,
        text(kind_label).size(13).color(accent),
        text(&toast.message).size(12).color(th.fg),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    container(body)
        .padding([10, 14])
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
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.30),
                    offset: iced::Vector { x: 0.0, y: 6.0 },
                    blur_radius: 20.0,
                },
                ..Default::default()
            }
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_push_respeita_max_3() {
        let mut q = ToastQueue::new();
        q.push(Toast::new(ToastKind::Info, "a"));
        q.push(Toast::new(ToastKind::Info, "b"));
        q.push(Toast::new(ToastKind::Info, "c"));
        q.push(Toast::new(ToastKind::Info, "d"));
        assert_eq!(q.items().len(), 3);
        assert_eq!(q.items()[0].message, "b");
        assert_eq!(q.items()[2].message, "d");
    }

    #[test]
    fn queue_evict_expired_remove_velhos() {
        let mut q = ToastQueue::new();
        let mut old = Toast::new(ToastKind::Info, "velho");
        old.created = Instant::now() - Duration::from_secs(10);
        q.push(old);
        q.push(Toast::new(ToastKind::Info, "novo"));
        q.evict_expired(Duration::from_secs(5));
        assert_eq!(q.items().len(), 1);
        assert_eq!(q.items()[0].message, "novo");
    }

    #[test]
    fn toast_is_expired_boundary() {
        let now = Instant::now();
        let mut t = Toast::new(ToastKind::Info, "x");
        t.created = now - Duration::from_secs(3);
        assert!(!t.is_expired(now, Duration::from_secs(4)));
        assert!(t.is_expired(now, Duration::from_secs(2)));
    }
}
