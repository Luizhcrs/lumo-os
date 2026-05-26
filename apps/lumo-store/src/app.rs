//! app.rs -- App Iced do lumo-store.

use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Color, Element, Length, Task};

use crate::catalog::{AppEntry, Catalog};
use crate::install;
use crate::theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Available,
    Installed,
}

#[derive(Debug, Clone)]
pub enum Msg {
    TabChanged(Tab),
    SearchChanged(String),
    CategorySelected(Option<String>),
    InstallApp(String),
    RemoveApp(String),
    InstallResult {
        pkg: String,
        result: Result<(), String>,
    },
    RemoveResult {
        pkg: String,
        result: Result<(), String>,
    },
    InstalledLoaded(Vec<String>),
}

pub struct StoreApp {
    catalog: Catalog,
    tab: Tab,
    search: String,
    selected_category: Option<String>,
    installed_pkgs: Vec<String>,
    pending: Vec<String>,
    status_msg: Option<String>,
}

impl StoreApp {
    pub fn new() -> (Self, Task<Msg>) {
        let catalog = Catalog::embedded();
        let app = StoreApp {
            catalog,
            tab: Tab::Available,
            search: String::new(),
            selected_category: None,
            installed_pkgs: Vec::new(),
            pending: Vec::new(),
            status_msg: None,
        };
        let task = Task::perform(
            async {
                tokio::task::spawn_blocking(install::list_installed)
                    .await
                    .unwrap_or_default()
            },
            Msg::InstalledLoaded,
        );
        (app, task)
    }

    pub fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::TabChanged(t) => {
                self.tab = t;
                if t == Tab::Installed {
                    return Task::perform(
                        async {
                            tokio::task::spawn_blocking(install::list_installed)
                                .await
                                .unwrap_or_default()
                        },
                        Msg::InstalledLoaded,
                    );
                }
                Task::none()
            }
            Msg::SearchChanged(q) => {
                self.search = q;
                Task::none()
            }
            Msg::CategorySelected(c) => {
                self.selected_category = c;
                Task::none()
            }
            Msg::InstallApp(pkg) => {
                self.pending.push(pkg.clone());
                self.status_msg = Some(format!("Instalando {pkg}..."));
                Task::perform(
                    async move {
                        let p = pkg.clone();
                        let result = tokio::task::spawn_blocking(move || install::install_pkg(&p))
                            .await
                            .unwrap_or_else(|e| Err(format!("task: {e}")));
                        Msg::InstallResult { pkg, result }
                    },
                    |m| m,
                )
            }
            Msg::RemoveApp(pkg) => {
                self.pending.push(pkg.clone());
                self.status_msg = Some(format!("Removendo {pkg}..."));
                Task::perform(
                    async move {
                        let p = pkg.clone();
                        let result = tokio::task::spawn_blocking(move || install::remove_pkg(&p))
                            .await
                            .unwrap_or_else(|e| Err(format!("task: {e}")));
                        Msg::RemoveResult { pkg, result }
                    },
                    |m| m,
                )
            }
            Msg::InstallResult { pkg, result } => {
                self.pending.retain(|p| p != &pkg);
                match result {
                    Ok(()) => {
                        self.installed_pkgs.push(pkg.clone());
                        self.status_msg = Some(format!("{pkg} instalado."));
                    }
                    Err(e) => {
                        self.status_msg = Some(format!("Erro ao instalar {pkg}: {e}"));
                    }
                }
                Task::none()
            }
            Msg::RemoveResult { pkg, result } => {
                self.pending.retain(|p| p != &pkg);
                match result {
                    Ok(()) => {
                        self.installed_pkgs.retain(|p| p != &pkg);
                        self.status_msg = Some(format!("{pkg} removido."));
                    }
                    Err(e) => {
                        self.status_msg = Some(format!("Erro ao remover {pkg}: {e}"));
                    }
                }
                Task::none()
            }
            Msg::InstalledLoaded(pkgs) => {
                self.installed_pkgs = pkgs;
                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<Msg> {
        let status_bar: Element<Msg> = match &self.status_msg {
            Some(msg) => container(
                text(msg.as_str())
                    .size(13)
                    .color(Color::from_rgb(0.58, 0.59, 0.63)),
            )
            .padding([4, 16])
            .width(Length::Fill)
            .into(),
            None => Space::with_height(0.0).into(),
        };

        container(
            column![
                self.view_header(),
                row![self.view_sidebar(), self.view_content()].height(Length::Fill),
                status_bar,
            ]
            .spacing(0),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| theme::bg_style())
        .into()
    }

    fn view_header(&self) -> Element<Msg> {
        let title = text("Lumo Store")
            .size(20)
            .color(Color::from_rgb(0.96, 0.96, 0.97));
        let search = text_input("Buscar apps...", &self.search)
            .on_input(Msg::SearchChanged)
            .padding(8)
            .width(260.0);
        let tab_avail = button(text("Disponiveis").size(13))
            .padding([6, 16])
            .on_press(Msg::TabChanged(Tab::Available));
        let tab_inst = button(text("Instalados").size(13))
            .padding([6, 16])
            .on_press(Msg::TabChanged(Tab::Installed));

        container(
            row![
                title,
                Space::with_width(Length::Fill),
                search,
                Space::with_width(12.0),
                tab_avail,
                Space::with_width(4.0),
                tab_inst
            ]
            .align_y(iced::alignment::Vertical::Center)
            .padding([12, 16])
            .spacing(0),
        )
        .width(Length::Fill)
        .into()
    }

    fn view_sidebar(&self) -> Element<Msg> {
        let mut col = column![button(text("Todas").size(13))
            .padding([6, 12])
            .on_press(Msg::CategorySelected(None)),]
        .spacing(4);

        for cat in self.catalog.categories() {
            let is_sel = self.selected_category.as_deref() == Some(cat);
            let lbl = if is_sel {
                format!("> {cat}")
            } else {
                format!("  {cat}")
            };
            col = col.push(
                button(text(lbl).size(13))
                    .padding([6, 12])
                    .on_press(Msg::CategorySelected(Some(cat.to_string()))),
            );
        }

        container(scrollable(col).height(Length::Fill))
            .width(160.0)
            .padding([12, 8])
            .into()
    }

    fn view_content(&self) -> Element<Msg> {
        let apps: Vec<&AppEntry> = match self.tab {
            Tab::Available => {
                let query = self.search.trim();
                if !query.is_empty() {
                    self.catalog.search(query).collect()
                } else if let Some(ref cat) = self.selected_category {
                    self.catalog.by_category(cat).collect()
                } else {
                    self.catalog.apps.iter().collect()
                }
            }
            Tab::Installed => self
                .catalog
                .apps
                .iter()
                .filter(|a| self.installed_pkgs.iter().any(|p| p == &a.pkg))
                .collect(),
        };

        if apps.is_empty() {
            return container(
                text("Nenhum aplicativo encontrado.")
                    .size(14)
                    .color(Color::from_rgb(0.58, 0.59, 0.63)),
            )
            .width(Length::Fill)
            .padding([40, 0])
            .align_x(iced::alignment::Horizontal::Center)
            .into();
        }

        let cols = 3_usize;
        let mut grid = column![].spacing(12);
        for chunk in apps.chunks(cols) {
            let mut r = row![].spacing(12);
            for app in chunk {
                r = r.push(self.view_app_card(app));
            }
            for _ in chunk.len()..cols {
                r = r.push(Space::with_width(Length::Fill));
            }
            grid = grid.push(r);
        }

        container(scrollable(grid).height(Length::Fill))
            .width(Length::Fill)
            .padding([12, 16])
            .into()
    }

    fn view_app_card<'a>(&'a self, app: &'a AppEntry) -> Element<'a, Msg> {
        let is_installed = self.installed_pkgs.iter().any(|p| p == &app.pkg);
        let is_pending = self.pending.contains(&app.pkg);

        let action_btn: Element<Msg> = if is_pending {
            text("...").size(12).into()
        } else if is_installed {
            button(text("Remover").size(12))
                .padding([5, 12])
                .on_press(Msg::RemoveApp(app.pkg.clone()))
                .into()
        } else {
            button(text("Instalar").size(12))
                .padding([5, 12])
                .on_press(Msg::InstallApp(app.pkg.clone()))
                .into()
        };

        container(
            column![
                text(app.category.as_str())
                    .size(11)
                    .color(theme::accent_color()),
                Space::with_height(4.0),
                text(app.name.as_str()).size(14).color(theme::text_color()),
                Space::with_height(4.0),
                text(app.description.as_str())
                    .size(12)
                    .color(theme::muted_color()),
                Space::with_height(8.0),
                action_btn,
            ]
            .spacing(0),
        )
        .style(|_| theme::card_style())
        .padding([14, 16])
        .width(Length::Fill)
        .into()
    }
}
