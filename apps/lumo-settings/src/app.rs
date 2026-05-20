//! app.rs -- App principal do lumo-settings.
//!
//! Estado global, update e view. Cada aba renderizada via tab_view().

use iced::widget::svg::Handle as SvgHandle;
use iced::widget::{button, column, container, row, scrollable, slider, text, text_input, Space, Svg};
use iced::{Alignment, Color, Element, Length, Subscription, Task};

use crate::appmenu::appmenu_subscription;
use crate::icons;
use crate::tabs::Tab;
use crate::theme::{ButtonStyle, ContainerStyle, LumoTheme};

// ---------------------------------------------------------------------------
// Accent color options for Appearance tab
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccentOption {
    Emerald,
    Sky,
    Violet,
    Rose,
    Amber,
    Slate,
}

impl AccentOption {
    pub const ALL: &'static [AccentOption] = &[
        AccentOption::Emerald,
        AccentOption::Sky,
        AccentOption::Violet,
        AccentOption::Rose,
        AccentOption::Amber,
        AccentOption::Slate,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AccentOption::Emerald => "Emerald",
            AccentOption::Sky     => "Sky",
            AccentOption::Violet  => "Violet",
            AccentOption::Rose    => "Rose",
            AccentOption::Amber   => "Amber",
            AccentOption::Slate   => "Slate",
        }
    }

    pub fn hex(self) -> &'static str {
        match self {
            AccentOption::Emerald => "#059669",
            AccentOption::Sky     => "#0284c7",
            AccentOption::Violet  => "#7c3aed",
            AccentOption::Rose    => "#e11d48",
            AccentOption::Amber   => "#d97706",
            AccentOption::Slate   => "#64748b",
        }
    }
}

// ---------------------------------------------------------------------------
// WifiNetwork stub
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct WifiNetwork {
    pub ssid: String,
    pub signal: u8,
    pub connected: bool,
}

// ---------------------------------------------------------------------------
// BluetoothDevice stub
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BluetoothDevice {
    pub name: String,
    pub paired: bool,
}

// ---------------------------------------------------------------------------
// Message
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Message {
    // nav
    TabSelected(Tab),

    // display
    BrightnessChanged(u8),
    SetDayPreset,
    SetNightPreset,

    // wifi
    WifiToggle,
    WifiConnect(usize),

    // audio
    VolumeChanged(u8),

    // battery
    ChargeLimitToggle,

    // appearance
    ThemeToggle,
    AccentSelected(AccentOption),
    ApplyAppearance,

    // keyboard
    KeyboardLayoutSelected(usize),

    // touchpad
    TapToClickToggle,
    NaturalScrollToggle,
    AccelToggle,
    ApplyTouchpad,

    // appmenu
    Quit,
    ShowAbout,

    // accessibility
    ReducedMotionToggle,
    HighContrastToggle,
    FontScaleChanged(f32),
    ApplyAccessibility,
    A11ySaved,

    // async ops
    BrightnessLoaded(u8),
    BatteryLoaded { percent: u8, health: u8 },
    WifiListLoaded(Vec<WifiNetwork>),
    ApplyDone,
    Nop,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

pub struct App {
    pub tab: Tab,

    // display
    pub brightness: u8,

    // wifi
    pub wifi_enabled: bool,
    pub wifi_networks: Vec<WifiNetwork>,

    // bluetooth (stub)
    pub bt_devices: Vec<BluetoothDevice>,

    // audio
    pub volume: u8,

    // battery
    pub battery_percent: u8,
    pub battery_health: u8,
    pub charge_limit_80: bool,

    // appearance
    pub dark_mode: bool,
    pub accent: AccentOption,

    // keyboard
    pub kbd_layout_idx: usize,

    // touchpad
    pub tap_to_click: bool,
    pub natural_scroll: bool,
    pub accel_enabled: bool,

    // accessibility
    pub a11y_reduced_motion: bool,
    pub a11y_high_contrast: bool,
    pub a11y_font_scale: f32,

    // ui
    pub status_msg: String,
}

pub const KBD_LAYOUTS: &[&str] = &["pt-BR (ABNT2)", "en-US (QWERTY)", "pt-PT", "es-419"];

impl App {
    pub fn new() -> (Self, Task<Message>) {
        let app = Self {
            tab: Tab::Display,
            brightness: 80,
            wifi_enabled: true,
            wifi_networks: vec![
                WifiNetwork { ssid: "Lumo_Net".into(), signal: 95, connected: true },
                WifiNetwork { ssid: "Vizinho_5G".into(), signal: 72, connected: false },
                WifiNetwork { ssid: "IOT_Home".into(), signal: 55, connected: false },
            ],
            bt_devices: vec![
                BluetoothDevice { name: "Galaxy Buds3 Pro".into(), paired: true },
                BluetoothDevice { name: "BT Mouse".into(), paired: false },
            ],
            volume: 60,
            battery_percent: 0,
            battery_health: 0,
            charge_limit_80: false,
            dark_mode: true,
            accent: AccentOption::Emerald,
            kbd_layout_idx: 0,
            tap_to_click: true,
            natural_scroll: true,
            accel_enabled: true,
            a11y_reduced_motion: false,
            a11y_high_contrast: false,
            a11y_font_scale: 1.0,
            status_msg: String::new(),
        };
        let task = Task::perform(load_battery(), |r| match r {
            Ok((p, h)) => Message::BatteryLoaded { percent: p, health: h },
            Err(_)     => Message::Nop,
        });
        (app, task)
    }

    pub fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::TabSelected(t) => { self.tab = t; Task::none() }

            Message::BrightnessChanged(v) => {
                self.brightness = v;
                Task::perform(set_brightness(v), |_| Message::Nop)
            }
            Message::SetDayPreset   => { self.brightness = 100; Task::perform(set_brightness(100), |_| Message::Nop) }
            Message::SetNightPreset => { self.brightness = 30;  Task::perform(set_brightness(30),  |_| Message::Nop) }

            Message::WifiToggle => { self.wifi_enabled = !self.wifi_enabled; Task::none() }
            Message::WifiConnect(idx) => {
                if let Some(net) = self.wifi_networks.get(idx) {
                    let ssid = net.ssid.clone();
                    Task::perform(wifi_connect(ssid), |_| Message::Nop)
                } else { Task::none() }
            }

            Message::VolumeChanged(v) => {
                self.volume = v;
                Task::perform(set_volume(v), |_| Message::Nop)
            }

            Message::ChargeLimitToggle => {
                self.charge_limit_80 = !self.charge_limit_80;
                let limit = self.charge_limit_80;
                Task::perform(set_charge_limit(limit), |_| Message::Nop)
            }

            Message::ThemeToggle => { self.dark_mode = !self.dark_mode; Task::none() }
            Message::AccentSelected(a) => { self.accent = a; Task::none() }
            Message::ApplyAppearance => {
                self.status_msg = "Aparencia aplicada.".into();
                Task::perform(persist_appearance(self.dark_mode, self.accent), |_| Message::ApplyDone)
            }

            Message::KeyboardLayoutSelected(i) => { self.kbd_layout_idx = i; Task::none() }

            Message::TapToClickToggle   => { self.tap_to_click    = !self.tap_to_click;    Task::none() }
            Message::NaturalScrollToggle => { self.natural_scroll = !self.natural_scroll; Task::none() }
            Message::AccelToggle        => { self.accel_enabled   = !self.accel_enabled;   Task::none() }
            Message::ApplyTouchpad => {
                self.status_msg = "Touchpad salvo.".into();
                Task::perform(persist_touchpad(self.tap_to_click, self.natural_scroll, self.accel_enabled), |_| Message::ApplyDone)
            }

            Message::ReducedMotionToggle => { self.a11y_reduced_motion = !self.a11y_reduced_motion; Task::none() }
            Message::HighContrastToggle   => { self.a11y_high_contrast   = !self.a11y_high_contrast;   Task::none() }
            Message::FontScaleChanged(v)  => { self.a11y_font_scale = v.clamp(0.8, 1.4);               Task::none() }
            Message::ApplyAccessibility   => {
                self.status_msg = "Acessibilidade salva.".into();
                let rm = self.a11y_reduced_motion;
                let hc = self.a11y_high_contrast;
                let fs = self.a11y_font_scale;
                Task::perform(persist_accessibility(rm, hc, fs), |_| Message::A11ySaved)
            }
            Message::A11ySaved => { Task::none() }

            Message::BatteryLoaded { percent, health } => {
                self.battery_percent = percent;
                self.battery_health  = health;
                Task::none()
            }

            Message::ApplyDone => { Task::none() }

            Message::Quit => std::process::exit(0),
            Message::ShowAbout => { self.status_msg = "lumo-settings 0.1.0".into(); Task::none() }

            _ => Task::none(),
        }
    }

    pub fn view(&self) -> Element<Message> {
        let sidebar = self.view_sidebar();
        let content = self.view_tab();
        let layout = row![
            container(sidebar)
                .style(|_| ContainerStyle::Sidebar.style())
                .width(Length::Fixed(180.0))
                .height(Length::Fill),
            container(content)
                .style(|_| ContainerStyle::Main.style())
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(24),
        ];
        container(layout)
            .style(|_| ContainerStyle::Main.style())
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_sidebar(&self) -> Element<Message> {
        let items: Vec<Element<Message>> = Tab::ALL.iter().map(|&t| {
            let active = self.tab == t;
            button(
                row![
                    {
                        let icon_color = if active { LumoTheme::accent() } else { LumoTheme::muted() };
                        let svg_bytes: &'static [u8] = match t {
                            Tab::Display       => icons::DISPLAY,
                            Tab::Wifi          => icons::WIFI,
                            Tab::Bluetooth     => icons::BLUETOOTH,
                            Tab::Audio         => icons::AUDIO,
                            Tab::Battery       => icons::BATTERY,
                            Tab::Appearance    => icons::APPEARANCE,
                            Tab::Keyboard      => icons::KEYBOARD,
                            Tab::Touchpad      => icons::TOUCHPAD,
                            Tab::Accessibility => icons::ACCESSIBILITY,
                        };
                        Svg::new(SvgHandle::from_memory(svg_bytes))
                            .width(16)
                            .height(16)
                            .style(move |_, _| iced::widget::svg::Style { color: Some(icon_color) })
                    },
                    Space::with_width(8),
                    text(t.label()).size(13).color(if active { LumoTheme::accent() } else { LumoTheme::fg() }),
                ]
                .align_y(Alignment::Center)
            )
            .on_press(Message::TabSelected(t))
            .style(move |_, _| ButtonStyle::SidebarItem { active }.style())
            .width(Length::Fill)
            .padding([8, 12])
            .into()
        }).collect();

        let col = column(items).spacing(2).padding(12);
        scrollable(col).into()
    }

    fn view_tab(&self) -> Element<Message> {
        match self.tab {
            Tab::Display    => self.view_display(),
            Tab::Wifi       => self.view_wifi(),
            Tab::Bluetooth  => self.view_bluetooth(),
            Tab::Audio      => self.view_audio(),
            Tab::Battery    => self.view_battery(),
            Tab::Appearance => self.view_appearance(),
            Tab::Keyboard   => self.view_keyboard(),
            Tab::Touchpad   => self.view_touchpad(),
            Tab::Accessibility => self.view_accessibility(),
        }
    }

    fn section_title(label: &str) -> Element<'static, Message> {
        text(label.to_string()).size(18).color(LumoTheme::fg()).into()
    }

    fn view_display(&self) -> Element<Message> {
        column![
            Self::section_title("Display"),
            Space::with_height(16),
            text("Brilho da tela").size(13).color(LumoTheme::muted()),
            Space::with_height(8),
            slider(0..=100, self.brightness, Message::BrightnessChanged)
                .width(Length::Fixed(320.0)),
            Space::with_height(4),
            text(format!("{}%", self.brightness)).size(12).color(LumoTheme::muted()),
            Space::with_height(16),
            text("Presets").size(13).color(LumoTheme::muted()),
            Space::with_height(8),
            row![
                button(text("Dia (100%)").size(12).color(LumoTheme::bg()))
                    .on_press(Message::SetDayPreset)
                    .style(|_, _| ButtonStyle::Primary.style())
                    .padding([6, 14]),
                Space::with_width(10),
                button(text("Noite (30%)").size(12).color(LumoTheme::fg()))
                    .on_press(Message::SetNightPreset)
                    .style(|_, _| ButtonStyle::Secondary.style())
                    .padding([6, 14]),
            ],
        ]
        .spacing(0)
        .into()
    }

    fn view_wifi(&self) -> Element<Message> {
        let toggle_label = if self.wifi_enabled { "Wi-Fi: Ligado" } else { "Wi-Fi: Desligado" };
        let networks: Vec<Element<Message>> = self.wifi_networks.iter().enumerate().map(|(i, net)| {
            let conn_label = if net.connected { " [conectado]" } else { "" };
            row![
                text(format!("{}{} ({}%)", net.ssid, conn_label, net.signal)).size(13).color(LumoTheme::fg()),
                Space::with_width(Length::Fill),
                if !net.connected {
                    button(text("Conectar").size(11).color(LumoTheme::bg()))
                        .on_press(Message::WifiConnect(i))
                        .style(|_, _| ButtonStyle::Primary.style())
                        .padding([4, 10])
                } else {
                    button(text("Conectado").size(11).color(LumoTheme::muted()))
                        .style(|_, _| ButtonStyle::Ghost.style())
                        .padding([4, 10])
                }
            ]
            .align_y(Alignment::Center)
            .into()
        }).collect();

        column![
            Self::section_title("Wi-Fi"),
            Space::with_height(16),
            button(text(toggle_label).size(13).color(LumoTheme::fg()))
                .on_press(Message::WifiToggle)
                .style(|_, _| ButtonStyle::Secondary.style())
                .padding([8, 16]),
            Space::with_height(16),
            text("Redes disponíveis").size(13).color(LumoTheme::muted()),
            Space::with_height(8),
            column(networks).spacing(8),
        ]
        .spacing(0)
        .into()
    }

    fn view_bluetooth(&self) -> Element<Message> {
        let devices: Vec<Element<Message>> = self.bt_devices.iter().map(|d| {
            let paired = if d.paired { " [pareado]" } else { "" };
            text(format!("{}{}", d.name, paired)).size(13).color(LumoTheme::fg()).into()
        }).collect();

        column![
            Self::section_title("Bluetooth"),
            Space::with_height(16),
            text("Dispositivos").size(13).color(LumoTheme::muted()),
            Space::with_height(8),
            column(devices).spacing(8),
            Space::with_height(16),
            text("Bluetooth: stub (nmcli bt nao disponivel)").size(11).color(LumoTheme::muted()),
        ]
        .spacing(0)
        .into()
    }

    fn view_audio(&self) -> Element<Message> {
        column![
            Self::section_title("Audio"),
            Space::with_height(16),
            text("Volume do sistema").size(13).color(LumoTheme::muted()),
            Space::with_height(8),
            slider(0..=100, self.volume, Message::VolumeChanged)
                .width(Length::Fixed(320.0)),
            Space::with_height(4),
            text(format!("{}%", self.volume)).size(12).color(LumoTheme::muted()),
            Space::with_height(12),
            text("Audio via pactl (stub)").size(11).color(LumoTheme::muted()),
        ]
        .spacing(0)
        .into()
    }

    fn view_battery(&self) -> Element<Message> {
        let limit_label = if self.charge_limit_80 { "Limite 80%: Ligado" } else { "Limite 80%: Desligado" };
        column![
            Self::section_title("Bateria"),
            Space::with_height(16),
            text(format!("Nivel: {}%", self.battery_percent)).size(14).color(LumoTheme::fg()),
            Space::with_height(4),
            text(format!("Saude: {}%", self.battery_health)).size(13).color(LumoTheme::muted()),
            Space::with_height(16),
            button(text(limit_label).size(13).color(LumoTheme::fg()))
                .on_press(Message::ChargeLimitToggle)
                .style(|_, _| ButtonStyle::Secondary.style())
                .padding([8, 16]),
            Space::with_height(8),
            text("charge_control_end_threshold via samsung-galaxybook driver").size(11).color(LumoTheme::muted()),
        ]
        .spacing(0)
        .into()
    }

    fn view_appearance(&self) -> Element<Message> {
        let theme_label = if self.dark_mode { "Tema: Escuro" } else { "Tema: Claro" };
        let accents: Vec<Element<Message>> = AccentOption::ALL.iter().map(|&a| {
            let active = self.accent == a;
            button(
                text(format!("{} {}", a.label(), if active { "[*]" } else { "" }))
                    .size(12)
                    .color(if active { LumoTheme::accent() } else { LumoTheme::fg() })
            )
            .on_press(Message::AccentSelected(a))
            .style(move |_, _| if active { ButtonStyle::Primary.style() } else { ButtonStyle::Secondary.style() })
            .padding([5, 10])
            .into()
        }).collect();

        column![
            Self::section_title("Aparencia"),
            Space::with_height(16),
            button(text(theme_label).size(13).color(LumoTheme::fg()))
                .on_press(Message::ThemeToggle)
                .style(|_, _| ButtonStyle::Secondary.style())
                .padding([8, 16]),
            Space::with_height(16),
            text("Cor de destaque").size(13).color(LumoTheme::muted()),
            Space::with_height(8),
            row(accents).spacing(8).wrap(),
            Space::with_height(16),
            text(format!("Accent atual: {} ({})", self.accent.label(), self.accent.hex())).size(11).color(LumoTheme::muted()),
            Space::with_height(16),
            button(text("Aplicar").size(13).color(LumoTheme::bg()))
                .on_press(Message::ApplyAppearance)
                .style(|_, _| ButtonStyle::Primary.style())
                .padding([8, 20]),
            Space::with_height(8),
            text(self.status_msg.clone()).size(11).color(LumoTheme::accent()),
        ]
        .spacing(0)
        .into()
    }

    fn view_keyboard(&self) -> Element<Message> {
        let layouts: Vec<Element<Message>> = KBD_LAYOUTS.iter().enumerate().map(|(i, &l)| {
            let active = self.kbd_layout_idx == i;
            button(
                text(format!("{}{}", l, if active { " [*]" } else { "" }))
                    .size(13)
                    .color(if active { LumoTheme::accent() } else { LumoTheme::fg() })
            )
            .on_press(Message::KeyboardLayoutSelected(i))
            .style(move |_, _| if active { ButtonStyle::Primary.style() } else { ButtonStyle::Secondary.style() })
            .padding([6, 14])
            .into()
        }).collect();

        column![
            Self::section_title("Teclado"),
            Space::with_height(16),
            text("Layout").size(13).color(LumoTheme::muted()),
            Space::with_height(8),
            column(layouts).spacing(6),
            Space::with_height(12),
            text("Layout stub (localectl set-keymap)").size(11).color(LumoTheme::muted()),
        ]
        .spacing(0)
        .into()
    }

    fn view_touchpad(&self) -> Element<Message> {
        let bool_btn = |label: &'static str, val: bool, msg: Message| -> Element<Message> {
            button(
                text(format!("{}: {}", label, if val { "Sim" } else { "Nao" }))
                    .size(13)
                    .color(LumoTheme::fg())
            )
            .on_press(msg)
            .style(move |_, _| if val { ButtonStyle::Primary.style() } else { ButtonStyle::Secondary.style() })
            .padding([7, 14])
            .into()
        };

        column![
            Self::section_title("Touchpad"),
            Space::with_height(16),
            bool_btn("Tap para clicar",  self.tap_to_click,    Message::TapToClickToggle),
            Space::with_height(8),
            bool_btn("Rolagem natural",  self.natural_scroll,   Message::NaturalScrollToggle),
            Space::with_height(8),
            bool_btn("Aceleracao",       self.accel_enabled,    Message::AccelToggle),
            Space::with_height(16),
            button(text("Aplicar").size(13).color(LumoTheme::bg()))
                .on_press(Message::ApplyTouchpad)
                .style(|_, _| ButtonStyle::Primary.style())
                .padding([8, 20]),
            Space::with_height(8),
            text(self.status_msg.clone()).size(11).color(LumoTheme::accent()),
        ]
        .spacing(0)
        .into()
    }

    fn view_accessibility(&self) -> Element<Message> {
        let rm_label = if self.a11y_reduced_motion { "Reducir animacoes: Sim" } else { "Reducir animacoes: Nao" };
        let hc_label = if self.a11y_high_contrast  { "Alto contraste: Sim"    } else { "Alto contraste: Nao"   };
        column![
            Self::section_title("Acessibilidade"),
            Space::with_height(16),
            button(text(rm_label).size(13).color(LumoTheme::fg()))
                .on_press(Message::ReducedMotionToggle)
                .style(move |_, _| if self.a11y_reduced_motion { ButtonStyle::Primary.style() } else { ButtonStyle::Secondary.style() })
                .padding([8, 16]),
            Space::with_height(8),
            button(text(hc_label).size(13).color(LumoTheme::fg()))
                .on_press(Message::HighContrastToggle)
                .style(move |_, _| if self.a11y_high_contrast { ButtonStyle::Primary.style() } else { ButtonStyle::Secondary.style() })
                .padding([8, 16]),
            Space::with_height(16),
            text(format!("Escala de fonte: {:.1}x (0.8 - 1.4)", self.a11y_font_scale)).size(13).color(LumoTheme::muted()),
            Space::with_height(8),
            slider(0..=100, ((self.a11y_font_scale - 0.8) / 0.006) as u8, move |v| {
                Message::FontScaleChanged(0.8 + v as f32 * 0.006)
            }).width(Length::Fixed(320.0)),
            Space::with_height(16),
            button(text("Aplicar").size(13).color(LumoTheme::bg()))
                .on_press(Message::ApplyAccessibility)
                .style(|_, _| ButtonStyle::Primary.style())
                .padding([8, 20]),
            Space::with_height(8),
            text(self.status_msg.clone()).size(11).color(LumoTheme::accent()),
        ]
        .spacing(0)
        .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        appmenu_subscription()
    }
}

// ---------------------------------------------------------------------------
// Async helpers
// ---------------------------------------------------------------------------

async fn load_battery() -> Result<(u8, u8), String> {
    use lumo_sensors::SensorRegistry;
    let reg = SensorRegistry::discover().map_err(|e| e.to_string())?;
    let bat = reg.battery();
    let percent = bat.percent().unwrap_or(0);
    let health  = bat.health_percent().unwrap_or(100);
    Ok((percent, health))
}

async fn set_brightness(pct: u8) -> Result<(), String> {
    use lumo_sensors::Backlight;
    if let Some(bl) = Backlight::discover() {
        bl.set_percent(pct).map_err(|e| e.to_string())?;
    }
    Ok(())
}

async fn wifi_connect(ssid: String) -> Result<(), String> {
    tokio::process::Command::new("nmcli")
        .args(["con", "up", &ssid])
        .output()
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn set_volume(pct: u8) -> Result<(), String> {
    tokio::process::Command::new("pactl")
        .args(["set-sink-volume", "@DEFAULT_SINK@", &format!("{}%", pct)])
        .output()
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn set_charge_limit(enabled: bool) -> Result<(), String> {
    use lumo_sensors::SensorRegistry;
    let reg = SensorRegistry::discover().map_err(|e| e.to_string())?;
    let bat = reg.battery();
    let threshold: u8 = if enabled { 80 } else { 100 };
    bat.set_charge_limit(threshold).map_err(|e| e.to_string())?;
    Ok(())
}

async fn persist_appearance(dark: bool, accent: AccentOption) -> Result<(), String> {
    let dir = dirs_from_xdg();
    tokio::fs::create_dir_all(&dir).await.map_err(|e| e.to_string())?;
    let content = format!("[appearance]\ndark_mode = {}\naccent = \"{}\"\n", dark, accent.hex());
    tokio::fs::write(format!("{}/appearance.toml", dir), content).await.map_err(|e| e.to_string())?;
    Ok(())
}

async fn persist_touchpad(tap: bool, natural: bool, accel: bool) -> Result<(), String> {
    let dir = dirs_from_xdg();
    tokio::fs::create_dir_all(&dir).await.map_err(|e| e.to_string())?;
    let content = format!(
        "[touchpad]\ntap_to_click = {}\nnatural_scroll = {}\naccel_enabled = {}\n",
        tap, natural, accel
    );
    tokio::fs::write(format!("{}/touchpad.toml", dir), content).await.map_err(|e| e.to_string())?;
    Ok(())
}

async fn persist_accessibility(rm: bool, hc: bool, fs: f32) -> Result<(), String> {
    let dir = dirs_from_xdg();
    tokio::fs::create_dir_all(&dir).await.map_err(|e| e.to_string())?;
    let content = format!(
        "[accessibility]
reduced_motion = {}
high_contrast = {}
font_scale = {:.2}
",
        rm, hc, fs
    );
    tokio::fs::write(format!("{}/accessibility.toml", dir), content)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn dirs_from_xdg() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    format!("{}/.config/lumo", home)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_accent_all_count() {
        assert_eq!(AccentOption::ALL.len(), 6);
    }

    #[test]
    fn test_accent_hex_nonempty() {
        for a in AccentOption::ALL {
            assert!(a.hex().starts_with('#'), "hex invalido para {:?}", a);
        }
    }

    #[test]
    fn test_kbd_layouts_count() {
        assert_eq!(KBD_LAYOUTS.len(), 4);
    }

    #[test]
    fn test_kbd_layout_ptbr_first() {
        assert!(KBD_LAYOUTS[0].contains("pt-BR"));
    }

    #[test]
    fn test_dirs_from_xdg_contains_lumo() {
        let d = dirs_from_xdg();
        assert!(d.contains("lumo"));
    }
}
