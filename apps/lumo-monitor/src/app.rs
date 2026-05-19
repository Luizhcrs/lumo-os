//! app.rs -- App principal do lumo-monitor.
//!
//! Abas: CPU, Memory, Disk, Network, Processes. Refresh 2s.

#[cfg(target_os = "linux")]
use libc;

use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Alignment, Element, Length, Subscription, Task};
use iced::time;
use std::time::Duration;

use crate::appmenu::appmenu_subscription;
use crate::proc::{
    compute_net_rates, cpu_percent, read_cpu_stat, read_meminfo, read_mounts, read_net_dev,
    read_processes, CpuStat, DiskMount, MemInfo, NetIface, ProcEntry,
};
use crate::theme::{bar_str, container_bg, container_panel, pct_color, LumoTheme, TabStyle};

const CPU_HISTORY: usize = 60;
const REFRESH_SECS: u64  = 2;

// ---------------------------------------------------------------------------
// Tab
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab { Cpu, Memory, Disk, Network, Processes }

impl Tab {
    const ALL: &'static [Tab] = &[Tab::Cpu, Tab::Memory, Tab::Disk, Tab::Network, Tab::Processes];

    fn label(self) -> &'static str {
        match self {
            Tab::Cpu       => "CPU",
            Tab::Memory    => "Memoria",
            Tab::Disk      => "Disco",
            Tab::Network   => "Rede",
            Tab::Processes => "Processos",
        }
    }
}

// ---------------------------------------------------------------------------
// Message
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Message {
    TabSelected(Tab),
    Tick,
    Quit,
    ShowAbout,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

pub struct App {
    pub tab: Tab,

    // cpu
    pub cpu_prev: CpuStat,
    pub cpu_history: Vec<f32>,
    pub cpu_pct: f32,

    // mem
    pub mem: MemInfo,

    // disk
    pub mounts: Vec<DiskMount>,

    // net
    pub net_prev: Vec<NetIface>,
    pub net_curr: Vec<NetIface>,

    // proc
    pub cpu_total_prev: u64,
    pub processes: Vec<ProcEntry>,

    pub status: String,
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        let cpu_prev = read_cpu_stat();
        let net_prev = read_net_dev();
        let mem      = read_meminfo();
        let mounts   = read_mounts();
        let net_curr = read_net_dev();
        let processes = read_processes(0, ticks_per_sec());

        let app = Self {
            tab: Tab::Cpu,
            cpu_prev: cpu_prev.clone(),
            cpu_history: Vec::new(),
            cpu_pct: 0.0,
            mem,
            mounts,
            net_prev,
            net_curr,
            cpu_total_prev: cpu_prev.total(),
            processes,
            status: String::new(),
        };
        (app, Task::none())
    }

    pub fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::TabSelected(t) => { self.tab = t; Task::none() }

            Message::Tick => {
                // CPU
                let curr_stat = read_cpu_stat();
                self.cpu_pct = cpu_percent(&self.cpu_prev, &curr_stat);
                if self.cpu_history.len() >= CPU_HISTORY { self.cpu_history.remove(0); }
                self.cpu_history.push(self.cpu_pct);

                // CPU total diff for process accounting
                let curr_total = curr_stat.total();
                let cpu_total_diff = curr_total.saturating_sub(self.cpu_total_prev);
                self.cpu_total_prev = curr_total;
                self.cpu_prev = curr_stat;

                // Memory
                self.mem = read_meminfo();

                // Disk (less frequent; still refresh)
                self.mounts = read_mounts();

                // Network
                let mut new_net = read_net_dev();
                compute_net_rates(&self.net_curr, &mut new_net, REFRESH_SECS as f32);
                self.net_prev = std::mem::replace(&mut self.net_curr, new_net);

                // Processes
                self.processes = read_processes(cpu_total_diff, ticks_per_sec());

                Task::none()
            }

            Message::Quit => std::process::exit(0),
            Message::ShowAbout => { self.status = "lumo-monitor 0.1.0".into(); Task::none() }
        }
    }

    pub fn view(&self) -> Element<Message> {
        let tab_bar: Vec<Element<Message>> = Tab::ALL.iter().map(|&t| {
            let active = self.tab == t;
            button(text(t.label()).size(12).color(if active { LumoTheme::accent() } else { LumoTheme::muted() }))
                .on_press(Message::TabSelected(t))
                .style(move |_, _| if active { TabStyle::Active.style() } else { TabStyle::Inactive.style() })
                .padding([6, 14])
                .into()
        }).collect();

        let tabs = container(row(tab_bar).spacing(4).align_y(Alignment::Center))
            .style(|_| container_bg())
            .padding([8, 12]);

        let content = self.view_tab();

        let status = container(
            text(self.status.clone()).size(11).color(LumoTheme::muted())
        )
        .padding([4, 12]);

        column![tabs, content, status]
            .into()
    }

    fn view_tab(&self) -> Element<Message> {
        match self.tab {
            Tab::Cpu       => self.view_cpu(),
            Tab::Memory    => self.view_memory(),
            Tab::Disk      => self.view_disk(),
            Tab::Network   => self.view_network(),
            Tab::Processes => self.view_processes(),
        }
    }

    fn view_cpu(&self) -> Element<Message> {
        let bar = bar_str(self.cpu_pct, 40);
        let history_text: String = self.cpu_history.iter().rev().take(30)
            .map(|&v| format!("{:4.1}%", v))
            .collect::<Vec<_>>()
            .join("  ");

        container(
            scrollable(
                column![
                    text("CPU").size(18).color(LumoTheme::fg()),
                    Space::with_height(12),
                    text(format!("Uso atual: {:.1}%", self.cpu_pct))
                        .size(14)
                        .color(pct_color(self.cpu_pct)),
                    Space::with_height(8),
                    text(bar).size(13).color(pct_color(self.cpu_pct)),
                    Space::with_height(16),
                    text("Historico (60s, sample 2s)").size(12).color(LumoTheme::muted()),
                    Space::with_height(6),
                    text(history_text).size(11).color(LumoTheme::muted()),
                ]
                .padding(16)
                .spacing(0)
            )
        )
        .style(|_| container_bg())
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn view_memory(&self) -> Element<Message> {
        let used_gb  = self.mem.used_kb() as f32 / 1_048_576.0;
        let total_gb = self.mem.total_kb as f32 / 1_048_576.0;
        let pct = self.mem.used_percent();
        let bar = bar_str(pct, 40);

        container(
            column![
                text("Memoria").size(18).color(LumoTheme::fg()),
                Space::with_height(12),
                text(format!("Usada: {:.2} GB / {:.2} GB ({:.1}%)", used_gb, total_gb, pct))
                    .size(14)
                    .color(pct_color(pct)),
                Space::with_height(8),
                text(bar).size(13).color(pct_color(pct)),
                Space::with_height(12),
                text(format!("Disponivel: {:.2} GB", self.mem.available_kb as f32 / 1_048_576.0)).size(12).color(LumoTheme::muted()),
                text(format!("Buffers: {} MB", self.mem.buffers_kb / 1024)).size(12).color(LumoTheme::muted()),
                text(format!("Cache:   {} MB", self.mem.cached_kb / 1024)).size(12).color(LumoTheme::muted()),
            ]
            .padding(16)
            .spacing(6)
        )
        .style(|_| container_bg())
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn view_disk(&self) -> Element<Message> {
        let rows: Vec<Element<Message>> = self.mounts.iter().map(|m| {
            let pct = m.used_percent();
            let bar = bar_str(pct, 20);
            container(
                column![
                    row![
                        text(m.mount.clone()).size(13).color(LumoTheme::fg()),
                        Space::with_width(Length::Fill),
                        text(format!("{} | {}", m.fstype, m.device)).size(11).color(LumoTheme::muted()),
                    ],
                    text(format!("{} GB / {} GB ({:.1}%)",
                        m.used_kb / 1_048_576,
                        m.total_kb / 1_048_576,
                        pct))
                        .size(12)
                        .color(pct_color(pct)),
                    text(bar).size(11).color(pct_color(pct)),
                ]
                .spacing(3)
            )
            .style(|_| container_panel())
            .padding([8, 12])
            .width(Length::Fill)
            .into()
        }).collect();

        container(
            scrollable(
                column(
                    std::iter::once(text("Disco").size(18).color(LumoTheme::fg()).into())
                        .chain(std::iter::once(Space::with_height(12).into()))
                        .chain(rows)
                        .collect::<Vec<_>>()
                )
                .spacing(8)
                .padding(16)
            )
        )
        .style(|_| container_bg())
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn view_network(&self) -> Element<Message> {
        let rows: Vec<Element<Message>> = self.net_curr.iter().map(|iface| {
            let rx_kb = iface.rx_bytes / 1024;
            let tx_kb = iface.tx_bytes / 1024;
            let rx_rate_kb = iface.rx_rate as f32 / 1024.0;
            let tx_rate_kb = iface.tx_rate as f32 / 1024.0;

            container(
                row![
                    text(iface.name.clone()).size(13).color(LumoTheme::fg()).width(Length::Fixed(80.0)),
                    Space::with_width(Length::Fill),
                    column![
                        text(format!("RX: {} KB  ({:.1} KB/s)", rx_kb, rx_rate_kb)).size(11).color(LumoTheme::muted()),
                        text(format!("TX: {} KB  ({:.1} KB/s)", tx_kb, tx_rate_kb)).size(11).color(LumoTheme::muted()),
                    ]
                    .spacing(2),
                ]
                .align_y(Alignment::Center)
            )
            .style(|_| container_panel())
            .padding([8, 12])
            .width(Length::Fill)
            .into()
        }).collect();

        container(
            scrollable(
                column(
                    std::iter::once(text("Rede").size(18).color(LumoTheme::fg()).into())
                        .chain(std::iter::once(Space::with_height(12).into()))
                        .chain(rows)
                        .collect::<Vec<_>>()
                )
                .spacing(8)
                .padding(16)
            )
        )
        .style(|_| container_bg())
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn view_processes(&self) -> Element<Message> {
        let header = container(
            row![
                text("PID").size(11).color(LumoTheme::muted()).width(Length::Fixed(60.0)),
                text("Nome").size(11).color(LumoTheme::muted()).width(Length::Fixed(150.0)),
                text("CPU%").size(11).color(LumoTheme::muted()).width(Length::Fixed(60.0)),
                text("RSS").size(11).color(LumoTheme::muted()).width(Length::Fixed(80.0)),
                text("Cmd").size(11).color(LumoTheme::muted()),
            ]
        )
        .style(|_| container_panel())
        .padding([6, 12]);

        let proc_rows: Vec<Element<Message>> = self.processes.iter().take(30).map(|p| {
            let cpu_color = pct_color(p.cpu_pct);
            row![
                text(p.pid.to_string()).size(11).color(LumoTheme::muted()).width(Length::Fixed(60.0)),
                text(truncate(&p.name, 18)).size(11).color(LumoTheme::fg()).width(Length::Fixed(150.0)),
                text(format!("{:.1}", p.cpu_pct)).size(11).color(cpu_color).width(Length::Fixed(60.0)),
                text(format!("{} KB", p.rss_kb)).size(11).color(LumoTheme::muted()).width(Length::Fixed(80.0)),
                text(truncate(&p.cmd, 40)).size(11).color(LumoTheme::muted()),
            ]
            .align_y(Alignment::Center)
            .into()
        }).collect();

        container(
            column![
                text("Processos").size(18).color(LumoTheme::fg()),
                Space::with_height(8),
                text(format!("{} processos", self.processes.len())).size(11).color(LumoTheme::muted()),
                Space::with_height(8),
                header,
                scrollable(column(proc_rows).spacing(4)).height(Length::Fill),
            ]
            .padding(16)
            .spacing(0)
        )
        .style(|_| container_bg())
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            appmenu_subscription(),
            time::every(Duration::from_secs(REFRESH_SECS)).map(|_| Message::Tick),
        ])
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() }
    else { format!("{}...", &s[..max.saturating_sub(3)]) }
}

fn ticks_per_sec() -> u64 {
    #[cfg(target_os = "linux")]
    { unsafe { libc::sysconf(libc::_SC_CLK_TCK) as u64 } }
    #[cfg(not(target_os = "linux"))]
    { 100 }
}
