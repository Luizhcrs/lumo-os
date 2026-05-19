//! Struct App + Message enum + update + view.
//!
//! Entry point logico da aplicacao Iced.
//! Organizado por feature: sidebar, grid, toolbar/breadcrumb, context menu.

use std::collections::VecDeque;
use std::path::PathBuf;

use iced::keyboard::{self, Key, Modifiers};
use iced::widget::{
    button, column, container, horizontal_rule, row, scrollable, text, text_input,
};
use iced::{Alignment, Color, Element, Length, Subscription, Task};

use crate::breadcrumb;
use crate::filelist::{FileList, SortBy};
use crate::icons::{icon_for_path, icon_label, IconKind};
use crate::toolbar::ViewMode;
use crate::appmenu;
use crate::ops;
use crate::sidebar::{build_sidebar, SidebarItem, SidebarKind};
use crate::theme::LumoTheme;

// ---------------------------------------------------------------------------
// Clipboard state (path ops)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum ClipboardOp {
    Copy(Vec<PathBuf>),
    Cut(Vec<PathBuf>),
}

// ---------------------------------------------------------------------------
// Context menu
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum ContextMenu {
    /// Menu sobre item(s) selecionados.
    Item { x: f32, y: f32 },
    /// Menu sobre area vazia.
    Area { x: f32, y: f32 },
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Message {
    // Navegacao
    Navigate(PathBuf),
    NavigateBack,
    NavigateForward,
    NavigateUp,

    // Grid
    ItemClicked { idx: usize, ctrl: bool, shift: bool },
    ItemDoubleClicked(usize),
    ClearSelection,

    // Contexto
    ContextMenuOpen(ContextMenu),
    ContextMenuClose,

    // Operacoes
    OpenSelected,
    DeleteSelected,
    RenameStart(usize),
    RenameInputChanged(String),
    RenameConfirm,
    RenameCancel,
    CopySelected,
    CutSelected,
    Paste,
    NewFolder,
    NewFolderInputChanged(String),
    NewFolderConfirm,
    NewFolderCancel,
    Refresh,

    // AppMenu actions
    AppMenuNewWindow,
    AppMenuQuit,
    AppMenuSelectAll,
    AppMenuToggleHidden,
    AppMenuShowAbout,
    AppMenuShowShortcuts,

    // Busca + view
    ToggleSearch,
    SearchChanged(String),
    SetViewMode(crate::toolbar::ViewMode),
    SetSortBy(SortBy),
    ToggleSortOrder,

    // Teclado
    KeyPressed(Key, Modifiers),

    // Resultado async de listagem
    DirLoaded(PathBuf, Vec<PathBuf>),
    OpError(String),
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

pub struct App {
    pub current_dir: PathBuf,
    pub file_list: FileList,
    pub sidebar: Vec<SidebarItem>,
    pub back_stack: VecDeque<PathBuf>,
    pub forward_stack: VecDeque<PathBuf>,
    pub clipboard: Option<ClipboardOp>,
    pub context_menu: Option<ContextMenu>,
    pub status: String,
    /// Modal de nova pasta: Some(nome_digitado) quando ativo.
    pub new_folder_input: Option<String>,
    /// Mostrar arquivos ocultos (prefixo .).
    pub show_hidden: bool,
    /// Busca: se a caixa esta visivel.
    pub search_visible: bool,
    /// Texto de busca atual.
    pub search_query: String,
    /// Modo de exibicao (grid/list/columns).
    pub view_mode: crate::toolbar::ViewMode,
    /// Criterio de ordenacao.
    pub sort_by: SortBy,
    /// Ordem crescente (true) ou decrescente (false).
    pub sort_ascending: bool,
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        let home = dirs_home();
        let sidebar = build_sidebar(&username());
        let app = Self {
            current_dir: home.clone(),
            file_list: FileList::default(),
            sidebar,
            back_stack: VecDeque::new(),
            forward_stack: VecDeque::new(),
            clipboard: None,
            context_menu: None,
            status: String::new(),
            new_folder_input: None,
            show_hidden: false,
            search_visible: false,
            search_query: String::new(),
            view_mode: crate::toolbar::ViewMode::Grid,
            sort_by: SortBy::Name,
            sort_ascending: true,
        };
        let task = Task::perform(load_dir(home.clone(), false), move |r| match r {
            Ok(entries) => Message::DirLoaded(home.clone(), entries),
            Err(e) => Message::OpError(e),
        });
        (app, task)
    }

    // -----------------------------------------------------------------------
    // Update
    // -----------------------------------------------------------------------

    pub fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            // -- Navegacao --------------------------------------------------
            Message::Navigate(path) => {
                self.push_back();
                self.forward_stack.clear();
                let p2 = path.clone();
                Task::perform(load_dir(path, self.show_hidden), move |r| match r {
                    Ok(entries) => Message::DirLoaded(p2.clone(), entries),
                    Err(e) => Message::OpError(e),
                })
            }

            Message::NavigateBack => {
                if let Some(prev) = self.back_stack.pop_back() {
                    self.forward_stack.push_back(self.current_dir.clone());
                    let p2 = prev.clone();
                    Task::perform(load_dir(prev, self.show_hidden), move |r| match r {
                        Ok(entries) => Message::DirLoaded(p2.clone(), entries),
                        Err(e) => Message::OpError(e),
                    })
                } else {
                    Task::none()
                }
            }

            Message::NavigateForward => {
                if let Some(next) = self.forward_stack.pop_back() {
                    self.push_back();
                    let p2 = next.clone();
                    Task::perform(load_dir(next, self.show_hidden), move |r| match r {
                        Ok(entries) => Message::DirLoaded(p2.clone(), entries),
                        Err(e) => Message::OpError(e),
                    })
                } else {
                    Task::none()
                }
            }

            Message::NavigateUp => {
                if let Some(parent) = self.current_dir.parent().map(|p| p.to_path_buf()) {
                    self.update(Message::Navigate(parent))
                } else {
                    Task::none()
                }
            }

            // -- Grid ----------------------------------------------------------
            Message::ItemClicked { idx, ctrl, shift } => {
                if shift {
                    self.file_list.shift_click(idx);
                } else if ctrl {
                    self.file_list.ctrl_click(idx);
                } else {
                    self.file_list.click(idx);
                }
                Task::none()
            }

            Message::ItemDoubleClicked(idx) => {
                if let Some(path) = self.file_list.entries.get(idx).cloned() {
                    if path.is_dir() {
                        self.update(Message::Navigate(path))
                    } else {
                        let p = path.clone();
                        Task::perform(xdg_open(p), |r| {
                            if let Err(e) = r {
                                Message::OpError(e)
                            } else {
                                Message::Refresh
                            }
                        })
                    }
                } else {
                    Task::none()
                }
            }

            Message::ClearSelection => {
                self.file_list.clear_selection();
                Task::none()
            }

            // -- Contexto --------------------------------------------------
            Message::ContextMenuOpen(ctx) => {
                self.context_menu = Some(ctx);
                Task::none()
            }
            Message::ContextMenuClose => {
                self.context_menu = None;
                Task::none()
            }

            // -- Operacoes --------------------------------------------------
            Message::OpenSelected => {
                let paths = self.file_list.selected_paths();
                if let Some(path) = paths.into_iter().next() {
                    if path.is_dir() {
                        self.update(Message::Navigate(path))
                    } else {
                        let p = path.clone();
                        Task::perform(xdg_open(p), |r| {
                            if let Err(e) = r {
                                Message::OpError(e)
                            } else {
                                Message::Refresh
                            }
                        })
                    }
                } else {
                    Task::none()
                }
            }

            Message::DeleteSelected => {
                let paths = self.file_list.selected_paths();
                for path in &paths {
                    if let Err(e) = ops::move_to_trash(path) {
                        self.status = format!("Erro lixeira: {e}");
                        return Task::none();
                    }
                }
                self.file_list.clear_selection();
                self.context_menu = None;
                self.update(Message::Refresh)
            }

            Message::RenameStart(idx) => {
                self.file_list.start_rename(idx);
                self.context_menu = None;
                Task::none()
            }

            Message::RenameInputChanged(s) => {
                self.file_list.rename_input = s;
                Task::none()
            }

            Message::RenameConfirm => {
                if let Some(idx) = self.file_list.renaming {
                    let new_name = self.file_list.rename_input.clone();
                    if let Some(path) = self.file_list.entries.get(idx).cloned() {
                        match ops::rename(&path, &new_name) {
                            Ok(_) => {
                                self.file_list.cancel_rename();
                                return self.update(Message::Refresh);
                            }
                            Err(e) => self.status = format!("Renomear falhou: {e}"),
                        }
                    }
                }
                self.file_list.cancel_rename();
                Task::none()
            }

            Message::RenameCancel => {
                self.file_list.cancel_rename();
                Task::none()
            }

            Message::CopySelected => {
                let paths = self.file_list.selected_paths();
                if !paths.is_empty() {
                    self.clipboard = Some(ClipboardOp::Copy(paths));
                    self.status = "Copiado".to_string();
                }
                self.context_menu = None;
                Task::none()
            }

            Message::CutSelected => {
                let paths = self.file_list.selected_paths();
                if !paths.is_empty() {
                    self.clipboard = Some(ClipboardOp::Cut(paths));
                    self.status = "Recortado".to_string();
                }
                self.context_menu = None;
                Task::none()
            }

            Message::Paste => {
                let dest = self.current_dir.clone();
                match self.clipboard.clone() {
                    Some(ClipboardOp::Copy(paths)) => {
                        for p in &paths {
                            if let Err(e) = ops::copy_to(p, &dest) {
                                self.status = format!("Colar falhou: {e}");
                                return Task::none();
                            }
                        }
                    }
                    Some(ClipboardOp::Cut(paths)) => {
                        for p in &paths {
                            if let Err(e) = ops::move_to(p, &dest) {
                                self.status = format!("Mover falhou: {e}");
                                return Task::none();
                            }
                        }
                        self.clipboard = None;
                    }
                    None => {}
                }
                self.context_menu = None;
                self.update(Message::Refresh)
            }

            Message::NewFolder => {
                self.new_folder_input = Some("Nova pasta".to_string());
                self.context_menu = None;
                Task::none()
            }

            Message::NewFolderInputChanged(s) => {
                self.new_folder_input = Some(s);
                Task::none()
            }

            Message::NewFolderConfirm => {
                if let Some(name) = self.new_folder_input.take() {
                    match ops::mkdir(&self.current_dir, &name) {
                        Ok(_) => return self.update(Message::Refresh),
                        Err(e) => self.status = format!("Criar pasta falhou: {e}"),
                    }
                }
                Task::none()
            }

            Message::NewFolderCancel => {
                self.new_folder_input = None;
                Task::none()
            }

            Message::Refresh => {
                let dir = self.current_dir.clone();
                let dir2 = dir.clone();
                Task::perform(load_dir(dir, self.show_hidden), move |r| match r {
                    Ok(entries) => Message::DirLoaded(dir2.clone(), entries),
                    Err(e) => Message::OpError(e),
                })
            }

            // -- Teclado ---------------------------------------------------
            Message::KeyPressed(key, modifiers) => {
                self.context_menu = None;
                match key.as_ref() {
                    Key::Named(keyboard::key::Named::Delete) => {
                        return self.update(Message::DeleteSelected);
                    }
                    Key::Named(keyboard::key::Named::F2) => {
                        if let Some(&idx) = self.file_list.selected.iter().next() {
                            return self.update(Message::RenameStart(idx));
                        }
                    }
                    Key::Named(keyboard::key::Named::Enter) => {
                        return self.update(Message::OpenSelected);
                    }
                    Key::Named(keyboard::key::Named::Escape) => {
                        self.file_list.clear_selection();
                        self.file_list.cancel_rename();
                        self.new_folder_input = None;
                        if self.search_visible {
                            self.search_visible = false;
                            self.search_query.clear();
                        }
                    }
                    Key::Named(keyboard::key::Named::Backspace) => {
                        return self.update(Message::NavigateUp);
                    }
                    Key::Character(c) if modifiers.control() => match c {
                        "c" => return self.update(Message::CopySelected),
                        "x" => return self.update(Message::CutSelected),
                        "v" => return self.update(Message::Paste),
                        "n" => return self.update(Message::NewFolder),
                        _ => {}
                    },
                    _ => {}
                }
                Task::none()
            }

            // -- Async results ---------------------------------------------
            Message::DirLoaded(path, entries) => {
                self.current_dir = path;
                self.file_list.set_entries(entries);
                self.file_list.sort(self.sort_by, self.sort_ascending);
                self.status.clear();
                Task::none()
            }

            Message::OpError(e) => {
                self.status = e;
                Task::none()
            }

            // -- AppMenu actions (dispatchados por appmenu_subscription) --
            Message::AppMenuNewWindow => {
                let exe = std::env::current_exe().unwrap_or_default();
                Task::perform(
                    async move {
                        let _ = tokio::process::Command::new(&exe).spawn();
                    },
                    |_| Message::Refresh,
                )
            }
            Message::AppMenuQuit => iced::exit(),
            Message::AppMenuSelectAll => {
                let n = self.file_list.entries.len();
                for i in 0..n { self.file_list.selected.insert(i); }
                Task::none()
            }
            Message::AppMenuToggleHidden => {
                self.show_hidden = !self.show_hidden;
                self.update(Message::Refresh)
            }
            Message::AppMenuShowAbout => Task::none(),
            Message::AppMenuShowShortcuts => Task::none(),

            Message::ToggleSearch => {
                self.search_visible = !self.search_visible;
                if !self.search_visible { self.search_query.clear(); }
                Task::none()
            }
            Message::SearchChanged(s) => {
                self.search_query = s;
                Task::none()
            }
            Message::SetViewMode(m) => {
                self.view_mode = m;
                Task::none()
            }
            Message::SetSortBy(s) => {
                if self.sort_by == s {
                    self.sort_ascending = !self.sort_ascending;
                } else {
                    self.sort_by = s;
                    self.sort_ascending = true;
                }
                self.file_list.sort(self.sort_by, self.sort_ascending);
                Task::none()
            }
            Message::ToggleSortOrder => {
                self.sort_ascending = !self.sort_ascending;
                self.file_list.sort(self.sort_by, self.sort_ascending);
                Task::none()
            }
        }
    }

    // -----------------------------------------------------------------------
    // Subscriptions
    // -----------------------------------------------------------------------

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            iced::keyboard::on_key_press(|key, modifiers| {
                Some(Message::KeyPressed(key, modifiers))
            }),
            appmenu::appmenu_subscription(),
        ])
    }

    // -----------------------------------------------------------------------
    // View
    // -----------------------------------------------------------------------

    pub fn view(&self) -> Element<Message> {
        let bg = LumoTheme::bg();
        let panel = LumoTheme::panel();
        let panel_hi = LumoTheme::panel_hi();
        let fg = LumoTheme::fg();
        let muted = LumoTheme::muted();
        let accent = LumoTheme::accent();
        let sep = LumoTheme::sep();

        // -- Toolbar -----------------------
        let segs = breadcrumb::segments(&self.current_dir);
        let mut breadcrumb_row = row![].spacing(2);
        for (i, (label, path)) in segs.iter().enumerate() {
            let trunc = breadcrumb::truncate_label(label, 20);
            let is_last = i == segs.len() - 1;
            let p = path.clone();
            let btn = button(
                text(trunc)
                    .size(13)
                    .color(if is_last { accent } else { muted }),
            )
            .on_press(Message::Navigate(p))
            .style(move |_, _| button_style(Color::TRANSPARENT))
            .padding([2, 4]);
            breadcrumb_row = breadcrumb_row.push(btn);
            if !is_last {
                breadcrumb_row = breadcrumb_row.push(text("/").size(12).color(muted));
            }
        }

        let toolbar = crate::toolbar::view(
            !self.back_stack.is_empty(),
            !self.forward_stack.is_empty(),
            self.search_visible,
            &self.search_query,
            self.view_mode,
            breadcrumb_row.into(),
        );

        // -- Status bar ----------------------------------------------------
        let status_bar = if !self.status.is_empty() {
            container(text(&self.status).size(12).color(muted))
                .width(Length::Fill)
                .padding([3, 12])
                .style(move |_| container_style(panel))
        } else {
            container(text("").size(12))
                .width(Length::Fill)
                .padding([3, 12])
                .style(move |_| container_style(panel))
        };

        // -- Sidebar -------------------------------------------------------
        let mut sidebar_col = column![].spacing(2).padding([8, 6]);
        for item in &self.sidebar {
            let is_active = item.path == self.current_dir;
            let label_color = if is_active { accent } else { fg };
            let pill = LumoTheme::pill_bg();
            let path = item.path.clone();
            let kind = item.kind.clone();
            let icon_str = match kind {
                SidebarKind::Home => "[home]",
                SidebarKind::Trash => "[trash]",
                SidebarKind::Drive => "[drv]",
                _ => "[dir]",
            };
            let sep_kind = matches!(kind, SidebarKind::Trash | SidebarKind::Drive);
            if sep_kind && matches!(item.kind, SidebarKind::Trash) {
                sidebar_col = sidebar_col.push(
                    container(horizontal_rule(1))
                        .padding([4, 0])
                        .width(Length::Fill),
                );
            }
            let btn = button(
                row![
                    text(icon_str).size(11).color(muted),
                    text(&item.label).size(13).color(label_color),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .on_press(Message::Navigate(path))
            .style(move |_, _| {
                if is_active {
                    button_style(pill)
                } else {
                    button_style(Color::TRANSPARENT)
                }
            })
            .padding([5, 8])
            .width(Length::Fill);
            sidebar_col = sidebar_col.push(btn);
        }

        let sidebar = container(scrollable(sidebar_col).height(Length::Fill))
            .width(180)
            .height(Length::Fill)
            .style(move |_| container_style(panel));

        // -- File grid -----------------------------------------------------
        let grid = self.view_grid(fg, muted, accent, panel_hi, sep);

        // -- Nova pasta modal inline (toolbar area) ------------------------
        let content_area = if let Some(ref name) = self.new_folder_input {
            let input = text_input("Nome da pasta", name)
                .on_input(Message::NewFolderInputChanged)
                .on_submit(Message::NewFolderConfirm)
                .size(13)
                .padding([6, 10]);
            let btn_ok = button(text("OK").size(12).color(fg))
                .on_press(Message::NewFolderConfirm)
                .style(move |_, _| button_style(panel_hi))
                .padding([4, 10]);
            let btn_cancel = button(text("Cancelar").size(12).color(muted))
                .on_press(Message::NewFolderCancel)
                .style(move |_, _| button_style(Color::TRANSPARENT))
                .padding([4, 10]);
            column![
                container(
                    row![
                        text("Nova pasta:").size(13).color(muted),
                        input,
                        btn_ok,
                        btn_cancel,
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                )
                .padding([6, 12])
                .width(Length::Fill)
                .style(move |_| container_style(panel)),
                grid,
            ]
            .into()
        } else {
            grid
        };

        // -- Layout final --------------------------------------------------
        let body = row![sidebar, content_area].height(Length::Fill);

        let root = container(column![toolbar, body, status_bar].spacing(0))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container_style(bg));

        // -- Context menu overlay ------------------------------------------
        if let Some(ref ctx) = self.context_menu {
            self.view_context_menu(ctx, root.into(), fg, panel_hi, muted, accent)
        } else {
            root.into()
        }
    }

    fn view_grid(
        &self,
        fg: Color,
        muted: Color,
        accent: Color,
        _panel_hi: Color,
        _sep: Color,
    ) -> Element<Message> {
        let entries: Vec<(usize, &PathBuf)> = if self.search_query.is_empty() {
            self.file_list.entries.iter().enumerate().collect()
        } else {
            let q = self.search_query.to_ascii_lowercase();
            self.file_list.entries.iter().enumerate()
                .filter(|(_, p)| {
                    p.file_name().unwrap_or_default()
                        .to_string_lossy().to_ascii_lowercase().contains(&q)
                })
                .collect()
        };

        match self.view_mode {
            crate::toolbar::ViewMode::Grid => self.view_as_grid(&entries, fg, muted, accent),
            crate::toolbar::ViewMode::List => self.view_as_list(&entries, fg, muted, accent),
            crate::toolbar::ViewMode::Columns => self.view_as_columns(&entries, fg, muted, accent),
        }
    }

    fn view_as_grid<'a>(
        &'a self,
        entries: &[(usize, &'a PathBuf)],
        fg: Color,
        muted: Color,
        accent: Color,
    ) -> Element<'a, Message> {
        const CELL_W: f32 = 96.0;
        const COLS: usize = 7;

        let mut grid_rows: Vec<Element<Message>> = Vec::new();
        let chunks: Vec<&[(usize, &PathBuf)]> = entries.chunks(COLS).collect();

        for chunk in chunks.iter() {
            let mut r = row![].spacing(8);
            for (idx, path) in chunk.iter() {
                let idx = *idx;
                let is_selected = self.file_list.selected.contains(&idx);
                let name = FileList::display_name(path);
                let kind = icon_for_path(path);
                let icon_str = icon_svg_label(&kind);
                let cell_bg = if is_selected { LumoTheme::accent_alpha30() } else { Color::TRANSPARENT };
                let border_color = if is_selected { LumoTheme::accent() } else { Color::TRANSPARENT };

                let cell_content: Element<Message> = if self.file_list.renaming == Some(idx) {
                    column![
                        text(icon_str).size(36).color(if matches!(kind, IconKind::Folder) { accent } else { muted }),
                        text_input("nome", &self.file_list.rename_input)
                            .on_input(Message::RenameInputChanged)
                            .on_submit(Message::RenameConfirm)
                            .size(11)
                            .padding([2, 4]),
                    ]
                    .spacing(6)
                    .align_x(Alignment::Center)
                    .into()
                } else {
                    column![
                        text(icon_str).size(36).color(if matches!(kind, IconKind::Folder) { accent } else { muted }),
                        text(name).size(12).color(fg),
                    ]
                    .spacing(6)
                    .align_x(Alignment::Center)
                    .into()
                };

                let cell = button(
                    container(cell_content)
                        .width(Length::Fixed(CELL_W))
                        .height(Length::Fixed(110.0))
                        .padding([10, 6])
                        .align_x(Alignment::Center)
                        .style(move |_| iced::widget::container::Style {
                            background: Some(iced::Background::Color(cell_bg)),
                            border: iced::Border {
                                color: border_color,
                                width: if is_selected { 1.0 } else { 0.0 },
                                radius: 6.0.into(),
                            },
                            ..Default::default()
                        }),
                )
                .on_press(Message::ItemClicked { idx, ctrl: false, shift: false })
                .style(|_, _| iced::widget::button::Style {
                    background: Some(iced::Background::Color(Color::TRANSPARENT)),
                    border: iced::Border { radius: 6.0.into(), ..Default::default() },
                    text_color: LumoTheme::fg(),
                    ..Default::default()
                })
                .padding(0);

                r = r.push(cell);
            }
            grid_rows.push(r.into());
        }

        if grid_rows.is_empty() {
            let empty: Element<Message> = container(
                text("Pasta vazia").size(14).color(muted),
            )
            .padding([60, 0])
            .center_x(Length::Fill)
            .into();
            return container(scrollable(empty).height(Length::Fill))
                .width(Length::Fill).height(Length::Fill)
                .style(move |_| container_style(LumoTheme::bg()))
                .into();
        }

        let col: Element<Message> = column(grid_rows).spacing(8).padding([12, 12]).into();
        container(scrollable(col).height(Length::Fill))
            .width(Length::Fill).height(Length::Fill)
            .style(move |_| container_style(LumoTheme::bg()))
            .into()
    }

    fn view_as_list<'a>(
        &'a self,
        entries: &[(usize, &'a PathBuf)],
        fg: Color,
        muted: Color,
        accent: Color,
    ) -> Element<'a, Message> {
        let header = container(
            row![
                text("Nome").size(12).color(muted).width(Length::Fill),
                text("Tamanho").size(12).color(muted).width(Length::Fixed(80.0)),
                text("Modificado").size(12).color(muted).width(Length::Fixed(140.0)),
                text("Tipo").size(12).color(muted).width(Length::Fixed(60.0)),
            ]
            .spacing(8)
            .padding([4, 12]),
        )
        .width(Length::Fill)
        .style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(LumoTheme::panel())),
            ..Default::default()
        });

        let mut rows: Vec<Element<Message>> = vec![header.into()];

        if entries.is_empty() {
            rows.push(
                container(text("Pasta vazia").size(13).color(muted))
                    .padding([20, 12])
                    .into()
            );
        }

        for (idx, path) in entries {
            let idx = *idx;
            let is_selected = self.file_list.selected.contains(&idx);
            let row_bg = if is_selected { LumoTheme::accent_alpha30() } else { Color::TRANSPARENT };
            let kind = icon_for_path(path);
            let icon_str = icon_svg_label(&kind);
            let name_str = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let size_str = if path.is_dir() { "--".to_string() } else { FileList::human_size(path) };
            let mod_str = FileList::human_modified(path);
            let type_str = path.extension().and_then(|e| e.to_str()).unwrap_or("--").to_string();

            let row_content = row![
                row![
                    text(icon_str).size(14).color(if matches!(kind, IconKind::Folder) { accent } else { muted }),
                    text(name_str).size(13).color(fg),
                ].spacing(6).width(Length::Fill),
                text(size_str).size(12).color(muted).width(Length::Fixed(80.0)),
                text(mod_str).size(12).color(muted).width(Length::Fixed(140.0)),
                text(type_str).size(12).color(muted).width(Length::Fixed(60.0)),
            ]
            .spacing(8)
            .align_y(Alignment::Center);

            let row_btn = button(
                container(row_content)
                    .padding([5, 12])
                    .width(Length::Fill)
                    .style(move |_| iced::widget::container::Style {
                        background: Some(iced::Background::Color(row_bg)),
                        ..Default::default()
                    }),
            )
            .on_press(Message::ItemClicked { idx, ctrl: false, shift: false })
            .style(|_, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(Color::TRANSPARENT)),
                border: iced::Border::default(),
                text_color: LumoTheme::fg(),
                ..Default::default()
            })
            .padding(0)
            .width(Length::Fill);

            rows.push(row_btn.into());
        }

        container(scrollable(column(rows).spacing(0)).height(Length::Fill))
            .width(Length::Fill).height(Length::Fill)
            .style(move |_| container_style(LumoTheme::bg()))
            .into()
    }

    fn view_as_columns<'a>(
        &'a self,
        entries: &[(usize, &'a PathBuf)],
        fg: Color,
        muted: Color,
        accent: Color,
    ) -> Element<'a, Message> {
        const COL_W: f32 = 220.0;
        const ITEMS_PER_COL: usize = 20;

        let col1_entries: Vec<_> = entries.iter().take(ITEMS_PER_COL).collect();
        let col2_entries: Vec<_> = entries.iter().skip(ITEMS_PER_COL).take(ITEMS_PER_COL).collect();
        let col3_entries: Vec<_> = entries.iter().skip(ITEMS_PER_COL * 2).take(ITEMS_PER_COL).collect();

        fn make_col<'b>(
            slf: &'b crate::app::App,
            col_entries: &[&(usize, &'b PathBuf)],
            fg: Color,
            muted: Color,
            accent: Color,
        ) -> Element<'b, crate::app::Message> {
            let mut items: Vec<Element<crate::app::Message>> = Vec::new();
            for (idx, path) in col_entries {
                let idx = *idx;
                let is_selected = slf.file_list.selected.contains(&idx);
                let row_bg = if is_selected { LumoTheme::accent_alpha30() } else { Color::TRANSPARENT };
                let kind = icon_for_path(path);
                let icon_str = icon_svg_label(&kind);
                let name_str = FileList::display_name_max(path, 22);
                let row_content = row![
                    text(icon_str).size(14).color(if matches!(kind, IconKind::Folder) { accent } else { muted }),
                    text(name_str).size(13).color(fg),
                ]
                .spacing(6)
                .align_y(Alignment::Center);

                let row_btn = button(
                    container(row_content)
                        .padding([4, 8])
                        .width(Length::Fill)
                        .style(move |_| iced::widget::container::Style {
                            background: Some(iced::Background::Color(row_bg)),
                            ..Default::default()
                        }),
                )
                .on_press(crate::app::Message::ItemClicked { idx, ctrl: false, shift: false })
                .style(|_, _| iced::widget::button::Style {
                    background: Some(iced::Background::Color(Color::TRANSPARENT)),
                    border: iced::Border::default(),
                    text_color: LumoTheme::fg(),
                    ..Default::default()
                })
                .padding(0)
                .width(Length::Fill);

                items.push(row_btn.into());
            }

            if items.is_empty() {
                items.push(
                    container(iced::widget::horizontal_space())
                        .height(Length::Fixed(1.0)).into()
                );
            }

            container(scrollable(column(items).spacing(2)).height(Length::Fill))
                .width(Length::Fixed(COL_W))
                .height(Length::Fill)
                .style(move |_| iced::widget::container::Style {
                    background: Some(iced::Background::Color(LumoTheme::bg())),
                    ..Default::default()
                })
                .into()
        }

        let sep_col = container(iced::widget::horizontal_space())
            .width(Length::Fixed(1.0))
            .height(Length::Fill)
            .style(move |_| iced::widget::container::Style {
                background: Some(iced::Background::Color(LumoTheme::sep())),
                ..Default::default()
            });

        let sep_col2 = container(iced::widget::horizontal_space())
            .width(Length::Fixed(1.0))
            .height(Length::Fill)
            .style(move |_| iced::widget::container::Style {
                background: Some(iced::Background::Color(LumoTheme::sep())),
                ..Default::default()
            });

        row![
            make_col(self, &col1_entries, fg, muted, accent),
            sep_col,
            make_col(self, &col2_entries, fg, muted, accent),
            sep_col2,
            make_col(self, &col3_entries, fg, muted, accent),
        ]
        .height(Length::Fill)
        .into()
    }

    fn view_context_menu<'a>(
        &'a self,
        ctx: &ContextMenu,
        base: Element<'a, Message>,
        fg: Color,
        panel_hi: Color,
        muted: Color,
        _accent: Color,
    ) -> Element<'a, Message> {
        // Iced 0.13 nao tem overlay nativo fora de custom widgets.
        // Exibimos o menu como coluna flutuante inline no topo da view.
        let _ = ctx;

        let items: Vec<Element<Message>> = match ctx {
            ContextMenu::Item { .. } => vec![
                ctx_btn("Abrir", Message::OpenSelected, fg, panel_hi),
                ctx_btn(
                    "Renomear (F2)",
                    if let Some(&idx) = self.file_list.selected.iter().next() {
                        Message::RenameStart(idx)
                    } else {
                        Message::ContextMenuClose
                    },
                    fg,
                    panel_hi,
                ),
                ctx_btn("Copiar", Message::CopySelected, fg, panel_hi),
                ctx_btn("Recortar", Message::CutSelected, fg, panel_hi),
                ctx_btn("Mover para Lixeira", Message::DeleteSelected, fg, panel_hi),
                ctx_btn("Fechar menu", Message::ContextMenuClose, muted, panel_hi),
            ],
            ContextMenu::Area { .. } => vec![
                ctx_btn("Nova pasta", Message::NewFolder, fg, panel_hi),
                ctx_btn("Colar", Message::Paste, fg, panel_hi),
                ctx_btn("Atualizar", Message::Refresh, fg, panel_hi),
                ctx_btn("Fechar menu", Message::ContextMenuClose, muted, panel_hi),
            ],
        };

        let menu = container(column(items).spacing(2).padding([6, 0]))
            .width(200)
            .style(move |_| iced::widget::container::Style {
                background: Some(iced::Background::Color(panel_hi)),
                border: iced::Border {
                    color: LumoTheme::sep(),
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..Default::default()
            });

        column![base, menu].into()
    }

    // -----------------------------------------------------------------------
    // Helpers internos
    // -----------------------------------------------------------------------

    fn push_back(&mut self) {
        self.back_stack.push_back(self.current_dir.clone());
        if self.back_stack.len() > 50 {
            self.back_stack.pop_front();
        }
    }
}

// ---------------------------------------------------------------------------
// Async helpers
// ---------------------------------------------------------------------------

async fn load_dir(path: PathBuf, show_hidden: bool) -> Result<Vec<PathBuf>, String> {
    tokio::task::spawn_blocking(move || {
        let mut entries = ops::list_dir(&path).map_err(|e| e.to_string())?;
        if !show_hidden {
            entries.retain(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| !n.starts_with('.'))
                    .unwrap_or(true)
            });
        }
        Ok(entries)
    })
    .await
    .map_err(|e| e.to_string())?
}

async fn xdg_open(path: PathBuf) -> Result<(), String> {
    let status = tokio::process::Command::new("xdg-open")
        .arg(&path)
        .status()
        .await
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("xdg-open falhou: {status}"))
    }
}

// ---------------------------------------------------------------------------
// Style helpers
// ---------------------------------------------------------------------------

fn icon_svg_label(kind: &IconKind) -> &'static str {
    match kind {
        IconKind::Folder => "[/]",
        IconKind::Home => "[H]",
        IconKind::Trash => "[T]",
        IconKind::Image => "[I]",
        IconKind::Video => "[V]",
        IconKind::Audio => "[A]",
        IconKind::Document => "[D]",
        IconKind::Archive => "[Z]",
        IconKind::Code => "[{}]",
        IconKind::Executable => "[X]",
        IconKind::Generic => "[F]",
    }
}

fn button_style(bg: Color) -> iced::widget::button::Style {
    iced::widget::button::Style {
        background: Some(iced::Background::Color(bg)),
        border: iced::Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        text_color: LumoTheme::fg(),
        ..Default::default()
    }
}

fn container_style(bg: Color) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(iced::Background::Color(bg)),
        ..Default::default()
    }
}

fn ctx_btn<'a>(
    label: &'a str,
    msg: Message,
    color: Color,
    panel_hi: Color,
) -> Element<'a, Message> {
    button(text(label).size(13).color(color))
        .on_press(msg)
        .style(move |_, _| button_style(panel_hi))
        .padding([6, 14])
        .width(Length::Fill)
        .into()
}

// ---------------------------------------------------------------------------
// Utilitarios de sistema
// ---------------------------------------------------------------------------

fn dirs_home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

fn username() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "user".to_string())
}
