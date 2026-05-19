//! app.rs -- App Iced principal do lumo-firstrun wizard.

use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Color, Element, Length, Task};

use crate::locale::LocaleConfig;
use crate::steps::{AccountState, Locale, Step, WifiNetwork};
use crate::system;
use crate::theme;
use crate::FIRST_RUN_FLAG;

#[derive(Debug, Clone)]
pub enum Msg {
    Next,
    SelectLocale(Locale),
    UsernameChanged(String),
    PasswordChanged(String),
    PasswordConfirmChanged(String),
    WifiNetworksLoaded(Vec<WifiNetwork>),
    SelectWifi(String),
    WifiPasswordChanged(String),
    ConnectWifi,
    WifiConnected(Result<(), String>),
    Finish,
}

pub struct FirstRunApp {
    step:            Step,
    locale:          Locale,
    account:         AccountState,
    wifi_networks:   Vec<WifiNetwork>,
    selected_wifi:   Option<String>,
    wifi_password:   String,
    wifi_connecting: bool,
    wifi_error:      Option<String>,
    finish_error:    Option<String>,
}

impl FirstRunApp {
    pub fn new() -> (Self, Task<Msg>) {
        let app = FirstRunApp {
            step:            Step::Welcome,
            locale:          Locale::PtBr,
            account:         AccountState::default(),
            wifi_networks:   Vec::new(),
            selected_wifi:   None,
            wifi_password:   String::new(),
            wifi_connecting: false,
            wifi_error:      None,
            finish_error:    None,
        };
        (app, Task::none())
    }

    pub fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::SelectLocale(l) => { self.locale = l; Task::none() }
            Msg::UsernameChanged(v) => {
                self.account.username = v; self.account.error = None; Task::none()
            }
            Msg::PasswordChanged(v) => {
                self.account.password = v; self.account.error = None; Task::none()
            }
            Msg::PasswordConfirmChanged(v) => {
                self.account.password_confirm = v; self.account.error = None; Task::none()
            }
            Msg::SelectWifi(ssid) => {
                self.selected_wifi = Some(ssid); self.wifi_password.clear(); self.wifi_error = None;
                Task::none()
            }
            Msg::WifiPasswordChanged(v) => { self.wifi_password = v; Task::none() }
            Msg::ConnectWifi => {
                let ssid = match &self.selected_wifi { Some(s) => s.clone(), None => return Task::none() };
                let pw = if self.wifi_password.is_empty() { None } else { Some(self.wifi_password.clone()) };
                self.wifi_connecting = true; self.wifi_error = None;
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || system::connect_wifi(&ssid, pw.as_deref()))
                            .await.unwrap_or_else(|e| Err(format!("task: {e}")))
                    },
                    Msg::WifiConnected,
                )
            }
            Msg::WifiConnected(result) => {
                self.wifi_connecting = false;
                match result {
                    Ok(()) => {
                        if let Some(ref ssid) = self.selected_wifi {
                            if let Some(net) = self.wifi_networks.iter_mut().find(|n| &n.ssid == ssid) {
                                net.connected = true;
                            }
                        }
                    }
                    Err(e) => self.wifi_error = Some(e),
                }
                Task::none()
            }
            Msg::WifiNetworksLoaded(nets) => { self.wifi_networks = nets; Task::none() }
            Msg::Next => self.advance(),
            Msg::Finish => {
                if let Err(e) = LocaleConfig::new(self.locale.code()).write() {
                    self.finish_error = Some(format!("locale: {e}")); return Task::none();
                }
                let username = self.account.username.clone();
                let password = self.account.password.clone();
                if let Err(e) = system::create_user(&username, &password) {
                    eprintln!("create_user: {e}");
                }
                if let Err(e) = system::mark_first_run_done(FIRST_RUN_FLAG) {
                    self.finish_error = Some(format!("flag: {e}")); return Task::none();
                }
                std::process::exit(0);
            }
        }
    }

    fn advance(&mut self) -> Task<Msg> {
        if self.step == Step::Account {
            if let Err(e) = self.account.validate() {
                self.account.error = Some(e); return Task::none();
            }
        }
        self.step = self.step.next();
        if self.step == Step::Wifi {
            return Task::perform(
                async {
                    tokio::task::spawn_blocking(|| {
                        system::list_wifi().into_iter()
                            .map(|(ssid, signal, secured)| WifiNetwork::stub(ssid, signal, secured))
                            .collect::<Vec<_>>()
                    }).await.unwrap_or_default()
                },
                Msg::WifiNetworksLoaded,
            );
        }
        Task::none()
    }

    pub fn view(&self) -> Element<Msg> {
        container(
            column![self.view_progress(), self.view_current()].spacing(0),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| theme::bg_style())
        .into()
    }

    fn view_current(&self) -> Element<Msg> {
        match self.step {
            Step::Welcome  => self.view_welcome(),
            Step::Language => self.view_language(),
            Step::Account  => self.view_account(),
            Step::Wifi     => self.view_wifi(),
            Step::Done     => self.view_done(),
        }
    }

    fn view_progress(&self) -> Element<Msg> {
        let ratio = (self.step.index() as f32 / Step::COUNT as f32).min(1.0);
        container(row![
            container(Space::new(720.0 * ratio, 3.0))
                .style(|_| theme::progress_fill_style()),
            Space::with_width(Length::Fill),
        ]).width(Length::Fill).into()
    }

    fn view_welcome(&self) -> Element<Msg> {
        container(column![
            Space::with_height(120.0),
            text("Bem-vindo ao Lumo OS").size(32).color(Color::from_rgb(0.96, 0.96, 0.97)),
            Space::with_height(12.0),
            text("Configure seu sistema em alguns passos.").size(16).color(Color::from_rgb(0.58, 0.59, 0.63)),
            Space::with_height(48.0),
            button(text("Comecar").size(15)).padding([12, 32]).on_press(Msg::Next),
        ].align_x(iced::alignment::Horizontal::Center).spacing(0))
        .width(Length::Fill).align_x(iced::alignment::Horizontal::Center).into()
    }

    fn view_language(&self) -> Element<Msg> {
        let mut opts = column![].spacing(8);
        for &loc in Locale::ALL {
            let lbl = if self.locale == loc {
                format!("> {}", loc.label())
            } else {
                format!("  {}", loc.label())
            };
            opts = opts.push(
                button(text(lbl).size(15)).padding([10, 24]).on_press(Msg::SelectLocale(loc))
            );
        }
        container(column![
            Space::with_height(80.0),
            text("Idioma").size(24).color(Color::from_rgb(0.96, 0.96, 0.97)),
            Space::with_height(32.0),
            opts,
            Space::with_height(48.0),
            button(text("Continuar").size(15)).padding([12, 32]).on_press(Msg::Next),
        ].align_x(iced::alignment::Horizontal::Center).spacing(0))
        .width(Length::Fill).align_x(iced::alignment::Horizontal::Center).into()
    }

    fn view_account(&self) -> Element<Msg> {
        let err_elem: Element<Msg> = match &self.account.error {
            Some(e) => text(e.as_str()).size(13).color(Color::from_rgb(0.97, 0.44, 0.44)).into(),
            None    => Space::with_height(0.0).into(),
        };
        container(column![
            Space::with_height(60.0),
            text("Criar conta").size(24).color(Color::from_rgb(0.96, 0.96, 0.97)),
            Space::with_height(28.0),
            text_input("Nome de usuario", &self.account.username)
                .on_input(Msg::UsernameChanged).padding(10).width(320.0),
            Space::with_height(10.0),
            text_input("Senha", &self.account.password)
                .on_input(Msg::PasswordChanged).secure(true).padding(10).width(320.0),
            Space::with_height(10.0),
            text_input("Confirmar senha", &self.account.password_confirm)
                .on_input(Msg::PasswordConfirmChanged).secure(true).padding(10).width(320.0),
            Space::with_height(8.0),
            err_elem,
            Space::with_height(32.0),
            button(text("Continuar").size(15)).padding([12, 32]).on_press(Msg::Next),
        ].align_x(iced::alignment::Horizontal::Center).spacing(0))
        .width(Length::Fill).align_x(iced::alignment::Horizontal::Center).into()
    }

    fn view_wifi(&self) -> Element<Msg> {
        let list: Element<Msg> = if self.wifi_networks.is_empty() {
            text("Buscando redes...").size(14).color(Color::from_rgb(0.58, 0.59, 0.63)).into()
        } else {
            let mut col = column![].spacing(6);
            for net in &self.wifi_networks {
                let is_sel = self.selected_wifi.as_deref() == Some(&net.ssid);
                let lbl = format!(
                    "{} {} ({}%) {}{}",
                    if is_sel { ">" } else { " " },
                    net.ssid, net.signal,
                    if net.secured { "[+]" } else { "   " },
                    if net.connected { " [conectado]" } else { "" },
                );
                col = col.push(
                    button(text(lbl).size(13)).padding([8, 20])
                        .on_press(Msg::SelectWifi(net.ssid.clone()))
                );
            }
            scrollable(col).height(180.0).into()
        };

        let pw_elem: Element<Msg> = if self.selected_wifi.is_some() {
            text_input("Senha da rede", &self.wifi_password)
                .on_input(Msg::WifiPasswordChanged).secure(true).padding(10).width(280.0).into()
        } else {
            Space::with_height(0.0).into()
        };

        let connect_elem: Element<Msg> = if self.selected_wifi.is_some() && !self.wifi_connecting {
            button(text("Conectar").size(14)).padding([10, 24]).on_press(Msg::ConnectWifi).into()
        } else if self.wifi_connecting {
            text("Conectando...").size(14).into()
        } else {
            Space::with_height(0.0).into()
        };

        let err_elem: Element<Msg> = match &self.wifi_error {
            Some(e) => text(e.as_str()).size(13).color(Color::from_rgb(0.97, 0.44, 0.44)).into(),
            None    => Space::with_height(0.0).into(),
        };

        container(column![
            Space::with_height(50.0),
            text("Wi-Fi").size(24).color(Color::from_rgb(0.96, 0.96, 0.97)),
            Space::with_height(20.0),
            list,
            Space::with_height(10.0),
            pw_elem,
            Space::with_height(8.0),
            connect_elem,
            err_elem,
            Space::with_height(24.0),
            row![
                button(text("Pular").size(14)).padding([10, 24]).on_press(Msg::Next),
                Space::with_width(16.0),
                button(text("Continuar").size(15)).padding([12, 32]).on_press(Msg::Next),
            ].align_y(iced::alignment::Vertical::Center),
        ].align_x(iced::alignment::Horizontal::Center).spacing(0))
        .width(Length::Fill).align_x(iced::alignment::Horizontal::Center).into()
    }

    fn view_done(&self) -> Element<Msg> {
        let err_elem: Element<Msg> = match &self.finish_error {
            Some(e) => text(e.as_str()).size(13).color(Color::from_rgb(0.97, 0.44, 0.44)).into(),
            None    => Space::with_height(0.0).into(),
        };
        container(column![
            Space::with_height(110.0),
            text("Lumo OS instalado. Pronto pra usar.").size(24).color(Color::from_rgb(0.96, 0.96, 0.97)),
            Space::with_height(12.0),
            text("Clique em Iniciar para comecar.").size(15).color(Color::from_rgb(0.58, 0.59, 0.63)),
            err_elem,
            Space::with_height(48.0),
            button(text("Iniciar").size(16)).padding([14, 40]).on_press(Msg::Finish),
        ].align_x(iced::alignment::Horizontal::Center).spacing(0))
        .width(Length::Fill).align_x(iced::alignment::Horizontal::Center).into()
    }
}
