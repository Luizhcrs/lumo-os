//! lumo-about - tela Sobre este Galaxy Book.
//!
//! Layout vertical center:
//!   - Lumo logo (texto, sem emoji)
//!   - "Galaxy Book 4 (NP750XGJ-KG7BR)"
//!   - "Lumo OS 0.1.0 - kernel 6.x"
//!   - Cards Hardware:
//!     * Processador: Intel U300 (5 cores: 1P+4E)
//!     * Memoria: 8 GB LPDDR5
//!     * Grafica: Intel UHD Xe-LP (48 EU)
//!     * Display: 15.6\" IPS 60Hz
//!     * Armazenamento: 256 GB NVMe
//!     * Bateria: 54 Wh, charge_end 80%
//!   - Footer: Uptime + Build hash

use iced::widget::{column, container, row, text, Space};
use iced::{Alignment, Element, Length, Task, Theme, Color, Background};

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
}

pub struct App {
    uptime: String,
    kernel: String,
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        (
            Self {
                uptime: read_uptime(),
                kernel: read_kernel(),
            },
            Task::none(),
        )
    }

    pub fn update(&mut self, _msg: Message) -> Task<Message> {
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let logo = text("Lumo OS")
            .size(36)
            .style(|_t: &Theme| iced::widget::text::Style {
                color: Some(Color::from_rgb8(0x4f, 0xd1, 0xa1)),
            });

        let model = text("Galaxy Book 4")
            .size(24);

        let model_sub = text("NP750XGJ-KG7BR")
            .size(13)
            .style(|_t: &Theme| iced::widget::text::Style {
                color: Some(Color::from_rgb8(0x9a, 0x9a, 0xa0)),
            });

        let version = text(format!("Lumo OS 0.1.0  -  Linux {}", self.kernel))
            .size(13)
            .style(|_t: &Theme| iced::widget::text::Style {
                color: Some(Color::from_rgb8(0xa0, 0xa0, 0xa6)),
            });

        let header = column![logo, model, model_sub, Space::with_height(4), version]
            .align_x(Alignment::Center)
            .spacing(2);

        let hw = column![
            spec_row("Processador", "Intel Processor U300 (1P + 4E cores)"),
            spec_row("Memoria",      "8 GB LPDDR5"),
            spec_row("Grafica",      "Intel UHD Graphics Xe-LP (48 EU)"),
            spec_row("Display",      "15.6\" IPS, 60 Hz"),
            spec_row("Armazenamento","256 GB NVMe"),
            spec_row("Bateria",      "54 Wh - charge end 80%"),
        ]
        .spacing(10);

        let footer = text(format!("Uptime: {}", self.uptime))
            .size(11)
            .style(|_t: &Theme| iced::widget::text::Style {
                color: Some(Color::from_rgb8(0x70, 0x70, 0x76)),
            });

        let inner = column![
            Space::with_height(28),
            header,
            Space::with_height(24),
            container(hw)
                .padding(18)
                .style(|_t: &Theme| iced::widget::container::Style {
                    background: Some(Background::Color(Color::from_rgb8(0x1f, 0x20, 0x24))),
                    border: iced::Border {
                        radius: 8.0.into(),
                        width: 1.0,
                        color: Color::from_rgb8(0x30, 0x30, 0x36),
                    },
                    ..Default::default()
                })
                .width(Length::Fill),
            Space::with_height(16),
            container(footer).center_x(Length::Fill),
            Space::with_height(16),
        ]
        .padding([0, 28])
        .align_x(Alignment::Center);

        container(inner)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_t: &Theme| iced::widget::container::Style {
                background: Some(Background::Color(Color::from_rgb8(0x14, 0x14, 0x18))),
                ..Default::default()
            })
            .into()
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        iced::Subscription::none()
    }
}

fn spec_row<'a>(label: &'a str, value: &'a str) -> Element<'a, Message> {
    row![
        text(label)
            .width(Length::Fixed(140.0))
            .style(|_t: &Theme| iced::widget::text::Style {
                color: Some(Color::from_rgb8(0x8a, 0x8a, 0x90)),
            }),
        text(value)
            .style(|_t: &Theme| iced::widget::text::Style {
                color: Some(Color::from_rgb8(0xe6, 0xe6, 0xea)),
            }),
    ]
    .spacing(10)
    .into()
}

fn read_uptime() -> String {
    std::fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| s.split_whitespace().next().map(|x| x.to_string()))
        .and_then(|sec| sec.parse::<f64>().ok())
        .map(|sec| {
            let s = sec as u64;
            let h = s / 3600;
            let m = (s % 3600) / 60;
            format!("{}h {}min", h, m)
        })
        .unwrap_or_else(|| "?".into())
}

fn read_kernel() -> String {
    std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .ok()
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "?".into())
}
