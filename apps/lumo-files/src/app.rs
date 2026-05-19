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
use crate::filelist::FileList;
use crate::icons::{icon_for_path, icon_label, IconKind};
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
        };
        let task = Task::perform(load_dir(home.clone()), move |r| match r {
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
                Task::perform(load_dir(path), move |r| match r {
                    Ok(entries) => Message::DirLoaded(p2.clone(), entries),
                    Err(e) => Message::OpError(e),
                })
            }

            Message::NavigateBack => {
                if let Some(prev) = self.back_stack.pop_back() {
                    self.forward_stack.push_back(self.current_dir.clone());
                    let p2 = prev.clone();
                    Task::perform(load_dir(prev), move |r| match r {
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
                    Task::perform(load_dir(next), move |r| match r {
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
                Task::perform(load_dir(dir), move |r| match r {
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
                self.status.clear();
                Task::none()
            }

            Message::OpError(e) => {
                self.status = e;
                Task::none()
            }
        }
    }

    // -----------------------------------------------------------------------
    // Subscriptions
    // -----------------------------------------------------------------------

    pub fn subscription(&self) -> Subscription<Message> {
        iced::keyboard::on_key_press(|key, modifiers| {
            Some(Message::KeyPressed(key, modifiers))
        })
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

        // -- Toolbar -------------------------------------------------------
        let btn_back = button(text("<").color(if self.back_stack.is_empty() { muted } else { fg }))
            .on_press_maybe(if self.back_stack.is_empty() {
                None
            } else {
                Some(Message::NavigateBack)
            })
            .style(move |_, _| button_style(panel_hi))
            .padding([4, 10]);

        let btn_fwd = button(text(">").color(if self.forward_stack.is_empty() { muted } else { fg }))
            .on_press_maybe(if self.forward_stack.is_empty() {
                None
            } else {
                Some(Message::NavigateForward)
            })
            .style(move |_, _| button_style(panel_hi))
            .padding([4, 10]);

        let btn_up = button(text("^").color(fg))
            .on_press(Message::NavigateUp)
            .style(move |_, _| button_style(panel_hi))
            .padding([4, 10]);

        // breadcrumb
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

        let btn_new = button(text("[+]").size(13).color(accent))
            .on_press(Message::NewFolder)
            .style(move |_, _| button_style(panel_hi))
            .padding([4, 10]);

        let toolbar = container(
            row![btn_back, btn_fwd, btn_up, breadcrumb_row, btn_new]
                .spacing(6)
                .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .padding([6, 12])
        .style(move |_| container_style(panel));

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
        panel_hi: Color,
        _sep: Color,
    ) -> Element<Message> {
        const CELL_W: u16 = 90;
        const COLS: usize = 8;

        let mut grid_rows: Vec<Element<Message>> = Vec::new();
        let chunks: Vec<&[PathBuf]> = self.file_list.entries.chunks(COLS).collect();

        for (row_i, chunk) in chunks.iter().enumerate() {
            let mut r = row![].spacing(4);
            for (col_i, path) in chunk.iter().enumerate() {
                let idx = row_i * COLS + col_i;
                let is_selected = self.file_list.selected.contains(&idx);
                let name = FileList::display_name(path);
                let kind = icon_for_path(path);
                let icon = icon_label(&kind);
                let cell_bg = if is_selected {
                    LumoTheme::pill_bg()
                } else {
                    Color::TRANSPARENT
                };

                let cell_content = if self.file_list.renaming == Some(idx) {
                    // inline rename input
                    let rename_el: Element<Message> = column![
                        text(icon).size(28).color(if kind == IconKind::Folder {
                            accent
                        } else {
                            muted
                        }),
                        text_input("nome", &self.file_list.rename_input)
                            .on_input(Message::RenameInputChanged)
                            .on_submit(Message::RenameConfirm)
                            .size(11)
                            .padding([2, 4]),
                    ]
                    .spacing(4)
                    .align_x(Alignment::Center)
                    .into();
                    rename_el
                } else {
                    column![
                        text(icon).size(28).color(if kind == IconKind::Folder {
                            accent
                        } else {
                            muted
                        }),
                        text(name).size(11).color(fg),
                    ]
                    .spacing(4)
                    .align_x(Alignment::Center)
                    .into()
                };

                let cell = button(
                    container(cell_content)
                        .width(CELL_W)
                        .padding([8, 4])
                        .align_x(Alignment::Center),
                )
                .on_press(Message::ItemClicked {
                    idx,
                    ctrl: false,
                    shift: false,
                })
                .style(move |_, _| button_style(cell_bg))
                .padding(0);

                r = r.push(cell);
            }
            grid_rows.push(r.into());
        }

        if grid_rows.is_empty() {
            let empty: Element<Message> = container(
                text("Pasta vazia").size(14).color(muted),
            )
            .padding([40, 0])
            .into();
            grid_rows.push(empty);
        }

        let col: Element<Message> = column(grid_rows).spacing(4).padding([8, 12]).into();

        container(scrollable(col).height(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container_style(LumoTheme::bg()))
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

async fn load_dir(path: PathBuf) -> Result<Vec<PathBuf>, String> {
    tokio::task::spawn_blocking(move || ops::list_dir(&path).map_err(|e| e.to_string()))
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
