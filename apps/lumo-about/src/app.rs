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
use iced::{Alignment, Background, Color, Element, Length, Task, Theme};

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
}

pub struct App {
    uptime: String,
    kernel: String,
    cpu_model: String,
    mem_total: String,
    disk_used: String,
    battery: String,
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        (
            Self {
                uptime: read_uptime(),
                kernel: read_kernel(),
                cpu_model: read_cpu_model(),
                mem_total: read_mem_total(),
                disk_used: read_disk_usage(),
                battery: read_battery(),
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

        let model = text("Galaxy Book 4").size(24);

        let model_sub =
            text("NP750XGJ-KG7BR")
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
            spec_row_owned("Processador", self.cpu_model.clone()),
            spec_row_owned("Memoria", self.mem_total.clone()),
            spec_row("Grafica", "Intel UHD Graphics Xe-LP (48 EU)"),
            spec_row("Display", "15.6\" IPS, 60 Hz"),
            spec_row_owned("Armazenamento", self.disk_used.clone()),
            spec_row_owned("Bateria", self.battery.clone()),
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
        text(value).style(|_t: &Theme| iced::widget::text::Style {
            color: Some(Color::from_rgb8(0xe6, 0xe6, 0xea)),
        }),
    ]
    .spacing(10)
    .into()
}

fn spec_row_owned<'a>(label: &'a str, value: String) -> Element<'a, Message> {
    row![
        text(label)
            .width(Length::Fixed(140.0))
            .style(|_t: &Theme| iced::widget::text::Style {
                color: Some(Color::from_rgb8(0x8a, 0x8a, 0x90)),
            }),
        text(value).style(|_t: &Theme| iced::widget::text::Style {
            color: Some(Color::from_rgb8(0xe6, 0xe6, 0xea)),
        }),
    ]
    .spacing(10)
    .into()
}

fn read_cpu_model() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split(':').nth(1))
                .map(|v| v.trim().to_string())
        })
        .unwrap_or_else(|| "Intel Processor U300".into())
}

fn read_mem_total() -> String {
    std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("MemTotal"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|v| v.parse::<u64>().ok())
        })
        .map(|kb| {
            let gb = (kb as f64) / 1024.0 / 1024.0;
            format!("{:.1} GB LPDDR5", gb)
        })
        .unwrap_or_else(|| "8 GB LPDDR5".into())
}

fn read_disk_usage() -> String {
    let out = std::process::Command::new("df")
        .args(["-B1", "--output=size,used", "/"])
        .output()
        .ok();
    if let Some(out) = out {
        if let Ok(s) = String::from_utf8(out.stdout) {
            if let Some(line) = s.lines().nth(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() == 2 {
                    if let (Ok(sz), Ok(used)) = (parts[0].parse::<u64>(), parts[1].parse::<u64>()) {
                        let gb_total = (sz as f64) / 1e9;
                        let gb_used = (used as f64) / 1e9;
                        return format!("{:.0} GB NVMe ({:.0} GB usado)", gb_total, gb_used);
                    }
                }
            }
        }
    }
    "256 GB NVMe".into()
}

fn read_battery() -> String {
    let cap = std::fs::read_to_string("/sys/class/power_supply/BAT0/capacity")
        .ok()
        .map(|s| s.trim().to_string());
    let limit =
        std::fs::read_to_string("/sys/class/power_supply/BAT0/charge_control_end_threshold")
            .ok()
            .map(|s| s.trim().to_string());
    match (cap, limit) {
        (Some(c), Some(l)) => format!("{}% atual - charge end {}%", c, l),
        (Some(c), None) => format!("{}% atual", c),
        _ => "54 Wh - charge end 80%".into(),
    }
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
