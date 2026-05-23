//! Struct App + Message enum + update + view.
//!
//! Entry point logico da aplicacao Iced.
//! Organizado por feature: sidebar, grid, toolbar/breadcrumb, context menu.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use iced::keyboard::{self, Key, Modifiers};
use iced::mouse;
use iced::widget::svg::Handle as SvgHandle;
use iced::widget::{
    button, column, container, horizontal_rule, row, scrollable, text, text_input, Svg,
};
use iced::{Alignment, Border, Color, Element, Length, Subscription, Task};

use crate::breadcrumb;
use crate::ctxmenu;
use crate::filelist::{FileList, SortBy};
use crate::icons;
use crate::icons::{icon_for_path, IconKind};
use crate::toolbar::ViewMode;
use crate::appmenu;
use crate::ops;
use crate::sidebar::{build_sidebar, SidebarItem, SidebarKind};
use crate::statusbar;
use crate::tabs as tabs_view;
use crate::theme::{LumoTheme, ThemeSnapshot};
use crate::toast::{Toast, ToastKind, ToastQueue};

// ---------------------------------------------------------------------------
// Tab state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Tab {
    pub current_dir: std::path::PathBuf,
    pub file_list: crate::filelist::FileList,
    pub back_stack: std::collections::VecDeque<std::path::PathBuf>,
    pub forward_stack: std::collections::VecDeque<std::path::PathBuf>,
    pub label: String,
}

impl Tab {
    pub fn new(dir: std::path::PathBuf) -> Self {
        let label = dir.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "/".to_string());
        Self {
            current_dir: dir,
            file_list: crate::filelist::FileList::default(),
            back_stack: std::collections::VecDeque::new(),
            forward_stack: std::collections::VecDeque::new(),
            label,
        }
    }
}

// ---------------------------------------------------------------------------
// Properties dialog state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PropertiesState {
    pub path: std::path::PathBuf,
    pub name_edit: String,
}

// ThumbCache: use crate::thumbs::ThumbCache (canonical source)
use crate::thumbs::ThumbCache;

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

    // Preview pane
    TogglePreview,
    // Drives
    DrivesRefreshed(Vec<crate::sidebar::SidebarItem>),
    // Properties dialog
    OpenProperties,
    CloseProperties,
    PropertiesNameChanged(String),
    PropertiesApply,
    // Tabs
    NewTab,
    CloseTab(usize),
    SwitchTab(usize),
    TabNavigate(usize, PathBuf),
    TabDirLoaded(usize, PathBuf, Vec<PathBuf>),
    DriveUnmount(PathBuf),
    EmptyTrash,
    // Tick para refresh periódico
    Tick,
    ThumbLoaded { path: PathBuf, key: String, data: Vec<u8> },

    // Teclado
    KeyPressed(Key, Modifiers),

    // Resultado async de listagem
    DirLoaded(PathBuf, Vec<PathBuf>),
    OpError(String),

    // Polish v2 — toasts e disk usage
    ToastTick,
    DiskUsageLoaded(PathBuf, u64, u64),

    // W21: sidebar arvore
    ToggleSidebarExpand(PathBuf),

    // [debug] raw event listener para diagnosricar hit-test
    RawEvent(iced::Event),
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

pub struct App {
    /// Tabs: lista de tabs abertas. Estado canonico de navegacao.
    pub tabs: Vec<Tab>,
    /// Indice da tab ativa.
    pub active_tab: usize,
    pub sidebar: Vec<SidebarItem>,
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
    pub preview_visible: bool,
    pub thumb_cache: ThumbCache,
    /// Properties dialog state.
    pub properties: Option<PropertiesState>,
    /// Polish v2: tema snapshot (Light/Dark fixo no startup).
    pub theme: ThemeSnapshot,
    /// Polish v2: toast queue para erros nao-criticos.
    pub toasts: ToastQueue,
    /// Polish v2: loading state durante enumeracao de pasta.
    pub loading: bool,
    /// Polish v2: cache de disk usage (free, total) por mountpoint, refresh 5 s.
    pub disk_cache: Option<(PathBuf, u64, u64, Instant)>,
    /// W21: paths expandidos na sidebar tree.
    pub expanded: std::collections::HashSet<PathBuf>,
    /// W21: cache de subdirs de $HOME pra render tree sem hit FS por frame.
    pub home_subdirs: Vec<PathBuf>,
    /// Manual hit-test: ultima posicao do cursor (window-relative).
    pub last_cursor_pos: iced::Point,
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        Self::new_with_dir(dirs_home())
    }

    pub fn new_with_dir(initial: PathBuf) -> (Self, Task<Message>) {
        let home = initial;
        let sidebar = build_sidebar(&username());

        // Carrega diretorio inicial de forma sincrona para eliminar race
        // entre Task::perform e o primeiro frame do compositor Wayland.
        // Task::perform do Iced e encadeado apos window::open() -- o primeiro
        // frame e pintado com loading=true/entries=[] antes do DirLoaded
        // chegar, causando skeleton permanente se o compositor nao pedir
        // redraw depois do DirLoaded.
        let initial_show_hidden = false;
        let initial_entries: Vec<PathBuf> = ops::list_dir(&home)
            .unwrap_or_default()
            .into_iter()
            .filter(|p| {
                initial_show_hidden
                    || p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| !n.starts_with('.'))
                        .unwrap_or(true)
            })
            .collect();

        let mut initial_tab = Tab::new(home.clone());
        initial_tab.file_list.set_entries(initial_entries);
        initial_tab.file_list.sort(SortBy::Name, true);

        let app = Self {
            tabs: vec![initial_tab],
            active_tab: 0,
            sidebar,
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
            preview_visible: false,
            thumb_cache: ThumbCache::new(),
            properties: None,
            theme: ThemeSnapshot::from_env(),
            toasts: ToastQueue::new(),
            loading: false,
            disk_cache: None,
            expanded: std::collections::HashSet::new(),
            home_subdirs: load_immediate_subdirs(&home),
            last_cursor_pos: iced::Point::ORIGIN,
        };
        // Task::none(): dados ja carregados sincronamente acima.
        // Breadcrumb e grid corretos desde o primeiro frame.
        (app, Task::none())
    }

    /// Referencia imutavel para a tab ativa.
    pub fn current_tab(&self) -> &Tab {
        &self.tabs[self.active_tab]
    }

    /// Referencia mutavel para a tab ativa.
    pub fn current_tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active_tab]
    }

    // -----------------------------------------------------------------------
    // Update
    // -----------------------------------------------------------------------

    pub fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            // -- Navegacao --------------------------------------------------
            Message::Navigate(path) => {
                eprintln!("[hit] Navigate handler called path={:?}", path);
                {
                    let cur = self.current_tab().current_dir.clone();
                    self.current_tab_mut().back_stack.push_back(cur);
                    if self.current_tab().back_stack.len() > 50 { self.current_tab_mut().back_stack.pop_front(); }
                }
                self.current_tab_mut().forward_stack.clear();
                let p2 = path.clone();
                Task::perform(load_dir(path, self.show_hidden), move |r| match r {
                    Ok(entries) => Message::DirLoaded(p2.clone(), entries),
                    Err(e) => Message::OpError(e),
                })
            }

            Message::NavigateBack => {
                if let Some(prev) = self.current_tab_mut().back_stack.pop_back() {
                    let cur = self.current_tab().current_dir.clone();
                    self.current_tab_mut().forward_stack.push_back(cur);
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
                if let Some(next) = self.current_tab_mut().forward_stack.pop_back() {
                    let cur = self.current_tab().current_dir.clone();
                    self.current_tab_mut().back_stack.push_back(cur);
                    if self.current_tab().back_stack.len() > 50 { self.current_tab_mut().back_stack.pop_front(); }
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
                if let Some(parent) = self.current_tab().current_dir.parent().map(|p| p.to_path_buf()) {
                    self.update(Message::Navigate(parent))
                } else {
                    Task::none()
                }
            }

            // -- Grid ----------------------------------------------------------
            Message::ItemClicked { idx, ctrl, shift } => {
                if shift {
                    self.current_tab_mut().file_list.shift_click(idx);
                } else if ctrl {
                    self.current_tab_mut().file_list.ctrl_click(idx);
                } else {
                    self.current_tab_mut().file_list.click(idx);
                }
                Task::none()
            }

            Message::ItemDoubleClicked(idx) => {
                if let Some(path) = self.current_tab().file_list.entries.get(idx).cloned() {
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
                self.current_tab_mut().file_list.clear_selection();
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
                let paths = self.current_tab().file_list.selected_paths();
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
                let paths = self.current_tab().file_list.selected_paths();
                for path in &paths {
                    if let Err(e) = ops::move_to_trash(path) {
                        self.status = format!("Erro lixeira: {e}");
                        return Task::none();
                    }
                }
                self.current_tab_mut().file_list.clear_selection();
                self.context_menu = None;
                self.update(Message::Refresh)
            }

            Message::RenameStart(idx) => {
                self.current_tab_mut().file_list.start_rename(idx);
                self.context_menu = None;
                Task::none()
            }

            Message::RenameInputChanged(s) => {
                self.current_tab_mut().file_list.rename_input = s;
                Task::none()
            }

            Message::RenameConfirm => {
                if let Some(idx) = self.current_tab().file_list.renaming {
                    let new_name = self.current_tab().file_list.rename_input.clone();
                    if let Some(path) = self.current_tab().file_list.entries.get(idx).cloned() {
                        match ops::rename(&path, &new_name) {
                            Ok(_) => {
                                self.current_tab_mut().file_list.cancel_rename();
                                return self.update(Message::Refresh);
                            }
                            Err(e) => self.status = format!("Renomear falhou: {e}"),
                        }
                    }
                }
                self.current_tab_mut().file_list.cancel_rename();
                Task::none()
            }

            Message::RenameCancel => {
                self.current_tab_mut().file_list.cancel_rename();
                Task::none()
            }

            Message::CopySelected => {
                let paths = self.current_tab().file_list.selected_paths();
                if !paths.is_empty() {
                    self.clipboard = Some(ClipboardOp::Copy(paths));
                    self.status = "Copiado".to_string();
                }
                self.context_menu = None;
                Task::none()
            }

            Message::CutSelected => {
                let paths = self.current_tab().file_list.selected_paths();
                if !paths.is_empty() {
                    self.clipboard = Some(ClipboardOp::Cut(paths));
                    self.status = "Recortado".to_string();
                }
                self.context_menu = None;
                Task::none()
            }

            Message::Paste => {
                let dest = self.current_tab().current_dir.clone();
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
                    match ops::mkdir(&self.current_tab().current_dir.clone(), &name) {
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
                let dir = self.current_tab().current_dir.clone();
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
                        if let Some(&idx) = self.current_tab().file_list.selected.iter().next() {
                            return self.update(Message::RenameStart(idx));
                        }
                    }
                    Key::Named(keyboard::key::Named::Enter) => {
                        return self.update(Message::OpenSelected);
                    }
                    Key::Named(keyboard::key::Named::Escape) => {
                        self.current_tab_mut().file_list.clear_selection();
                        self.current_tab_mut().file_list.cancel_rename();
                        self.new_folder_input = None;
                        self.properties = None;
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
                        "f" => return self.update(Message::ToggleSearch),
                        "p" => return self.update(Message::TogglePreview),
                        "i" => return self.update(Message::OpenProperties),
                        "t" => return self.update(Message::NewTab),
                        "w" => {
                            if self.tabs.len() > 1 {
                                let idx = self.active_tab;
                                return self.update(Message::CloseTab(idx));
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
                Task::none()
            }

            // -- Async results ---------------------------------------------
            Message::DirLoaded(path, entries) => {
                eprintln!("[hit] DirLoaded path={:?} entries={}", path, entries.len());
                let label = path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "/".to_string());
                self.current_tab_mut().current_dir = path.clone();
                self.current_tab_mut().label = label;
                let sb = self.sort_by; let sa = self.sort_ascending;
                self.current_tab_mut().file_list.set_entries(entries);
                self.current_tab_mut().file_list.sort(sb, sa);
                self.status.clear();
                self.loading = false;

                let mut tasks: Vec<Task<Message>> = Vec::new();

                // Polish v2: disk usage refresh (cache 5 s).
                let needs_disk = match &self.disk_cache {
                    None => true,
                    Some((p, _, _, when)) => {
                        p != &path || when.elapsed() > Duration::from_secs(5)
                    }
                };
                if needs_disk {
                    let p_path = path.clone();
                    let p_msg = path.clone();
                    tasks.push(Task::perform(
                        tokio::task::spawn_blocking(move || statusbar::disk_usage(&p_path)),
                        move |r| {
                            let (free, total) = r.unwrap_or((0, 0));
                            Message::DiskUsageLoaded(p_msg.clone(), free, total)
                        },
                    ));
                }

                // P0.1: pre-render thumbs for image files in background.
                if self.view_mode == crate::toolbar::ViewMode::Grid {
                    let image_paths: Vec<_> = self.tabs[self.active_tab].file_list.entries
                        .iter()
                        .filter(|p| crate::thumbs::is_image(p))
                        .cloned()
                        .collect();
                    for p in image_paths {
                        let key = crate::thumbs::cache_key(&p);
                        tasks.push(Task::perform(
                            tokio::task::spawn_blocking(move || {
                                crate::thumbs::generate_thumb(&p, &key)
                                    .map(|data| (p, key, data))
                            }),
                            |r| match r {
                                Ok(Some((path, key, data))) => {
                                    Message::ThumbLoaded { path, key, data }
                                }
                                _ => Message::Refresh,
                            },
                        ));
                    }
                }

                if tasks.is_empty() {
                    Task::none()
                } else {
                    Task::batch(tasks)
                }
            }

            Message::OpError(e) => {
                self.status = e.clone();
                self.toasts.push(Toast::new(ToastKind::Error, e));
                Task::none()
            }

            Message::ToastTick => {
                self.toasts.evict_expired(Duration::from_secs(4));
                Task::none()
            }

            Message::DiskUsageLoaded(path, free, total) => {
                self.disk_cache = Some((path, free, total, Instant::now()));
                Task::none()
            }

            Message::ToggleSidebarExpand(p) => {
                if self.expanded.contains(&p) {
                    self.expanded.remove(&p);
                } else {
                    self.expanded.insert(p);
                }
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
                let n = self.current_tab().file_list.entries.len();
                for i in 0..n { self.current_tab_mut().file_list.selected.insert(i); }
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
                let sb = self.sort_by; let sa = self.sort_ascending;
                self.current_tab_mut().file_list.sort(sb, sa);
                Task::none()
            }
            Message::ToggleSortOrder => {
                self.sort_ascending = !self.sort_ascending;
                let sb = self.sort_by; let sa = self.sort_ascending;
                self.current_tab_mut().file_list.sort(sb, sa);
                Task::none()
            }

            Message::TogglePreview => {
                self.preview_visible = !self.preview_visible;
                Task::none()
            }

            Message::ThumbLoaded { path: _, key, data } => {
                self.thumb_cache.insert(key, data);
                Task::none()
            }

            Message::DrivesRefreshed(items) => {
                // Atualiza drives na sidebar mantendo itens fixos
                self.sidebar.retain(|it| it.kind != crate::sidebar::SidebarKind::Drive);
                self.sidebar.extend(items);
                Task::none()
            }

            Message::DriveUnmount(path) => {
                let path_str = path.to_string_lossy().to_string();
                Task::perform(
                    async move {
                        tokio::process::Command::new("udisksctl")
                            .args(["unmount", "-b", &path_str])
                            .output()
                            .await
                            .ok();
                    },
                    |_| Message::Refresh,
                )
            }

            Message::EmptyTrash => {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
                let trash_files = std::path::PathBuf::from(&home).join(".local/share/Trash/files");
                let trash_info = std::path::PathBuf::from(&home).join(".local/share/Trash/info");
                Task::perform(
                    async move {
                        let _ = tokio::fs::remove_dir_all(&trash_files).await;
                        let _ = tokio::fs::create_dir_all(&trash_files).await;
                        let _ = tokio::fs::remove_dir_all(&trash_info).await;
                        let _ = tokio::fs::create_dir_all(&trash_info).await;
                    },
                    |_| Message::Refresh,
                )
            }

            Message::OpenProperties => {
                let paths = self.current_tab().file_list.selected_paths();
                if let Some(path) = paths.into_iter().next() {
                    let name = path.file_name()
                        .unwrap_or_default().to_string_lossy().to_string();
                    self.properties = Some(PropertiesState { path, name_edit: name });
                }
                Task::none()
            }

            Message::CloseProperties => {
                self.properties = None;
                Task::none()
            }

            Message::PropertiesNameChanged(s) => {
                if let Some(ref mut p) = self.properties {
                    p.name_edit = s;
                }
                Task::none()
            }

            Message::PropertiesApply => {
                if let Some(props) = self.properties.take() {
                    let _ = ops::rename(&props.path, &props.name_edit);
                    return self.update(Message::Refresh);
                }
                Task::none()
            }

            Message::NewTab => {
                let dir = self.current_tab().current_dir.clone();
                let tab = Tab::new(dir.clone());
                self.tabs.push(tab);
                self.active_tab = self.tabs.len() - 1;
                let idx = self.active_tab;
                let show_hidden = self.show_hidden;
                Task::perform(load_dir(dir.clone(), show_hidden), move |r| match r {
                    Ok(entries) => Message::TabDirLoaded(idx, dir.clone(), entries),
                    Err(e) => Message::OpError(e),
                })
            }

            Message::CloseTab(idx) => {
                if self.tabs.len() > 1 {
                    self.tabs.remove(idx);
                    self.active_tab = self.active_tab.min(self.tabs.len() - 1);
                }
                Task::none()
            }

            Message::SwitchTab(idx) => {
                if idx < self.tabs.len() {
                    self.active_tab = idx;
                    if let Some(tab) = self.tabs.get(idx) {
                        let dir = tab.current_dir.clone();
                        let dir2 = dir.clone();
                        let show_hidden = self.show_hidden;
                        return Task::perform(load_dir(dir, show_hidden), move |r| match r {
                            Ok(entries) => Message::DirLoaded(dir2.clone(), entries),
                            Err(e) => Message::OpError(e),
                        });
                    }
                }
                Task::none()
            }

            Message::TabNavigate(idx, path) => {
                if idx < self.tabs.len() {
                    self.tabs[idx].current_dir = path.clone();
                    let p2 = path.clone();
                    let show_hidden2 = self.show_hidden;
                    return Task::perform(load_dir(path, show_hidden2), move |r| match r {
                        Ok(entries) => Message::TabDirLoaded(idx, p2.clone(), entries),
                        Err(e) => Message::OpError(e),
                    });
                }
                Task::none()
            }

            Message::TabDirLoaded(idx, path, entries) => {
                if let Some(tab) = self.tabs.get_mut(idx) {
                    tab.label = path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "/".to_string());
                    tab.current_dir = path;
                    tab.file_list.set_entries(entries);
                    tab.file_list.sort(self.sort_by, self.sort_ascending);
                }
                Task::none()
            }

            Message::Tick => {
                let username = crate::app::username();
                Task::perform(
                    async move { crate::sidebar::build_sidebar(&username) },
                    |items| {
                        let drives: Vec<_> = items.into_iter()
                            .filter(|it| it.kind == crate::sidebar::SidebarKind::Drive)
                            .collect();
                        Message::DrivesRefreshed(drives)
                    },
                )
            }

            // Manual hit-test workaround: Iced widget hit-test nao dispara handlers
            // em algumas configuracoes Wayland. Rastreamos cursor via RawEvent e
            // disparamos mensagens corretas manualmente no ButtonPressed.
            Message::RawEvent(event) => {
                match &event {
                    iced::Event::Mouse(mouse::Event::CursorMoved { position }) => {
                        self.last_cursor_pos = *position;
                        eprintln!("[hit] CursorMoved {},{}", position.x, position.y);
                    }
                    iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                        let pos = self.last_cursor_pos;
                        eprintln!("[hit] ButtonPressed dispatch x={} y={}", pos.x, pos.y);
                        return self.dispatch_manual_click(pos);
                    }
                    _ => {}
                }
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
            iced::time::every(std::time::Duration::from_secs(5)).map(|_| Message::Tick),
            iced::time::every(std::time::Duration::from_secs(1)).map(|_| Message::ToastTick),
            // [debug] captura todos eventos brutos para diagnostico hit-test
            iced::event::listen_raw(|event, _status, _win| Some(event)).map(Message::RawEvent),
        ])
    }

    // -----------------------------------------------------------------------
    // View
    // -----------------------------------------------------------------------

    pub fn view(&self) -> Element<Message> {
        let th = &self.theme;
        let bg = th.bg;
        let panel = th.bg_subtle;
        let panel_hi = th.bg_subtle;
        let fg = th.fg;
        let muted = th.fg_subtle;
        let accent = th.accent;
        let sep = th.border;

        // -- Breadcrumb (polish v2: chevron separator + smart truncate) ----
        let segs = breadcrumb::segments(&self.current_tab().current_dir);
        let entries = breadcrumb::smart_truncate(&segs, 64);
        let mut breadcrumb_row = row![].spacing(2).align_y(Alignment::Center);
        let last_idx = entries.len().saturating_sub(1);
        for (i, entry) in entries.iter().enumerate() {
            match entry {
                breadcrumb::BreadcrumbEntry::Segment(label, path) => {
                    let is_last = i == last_idx;
                    let lbl = breadcrumb::truncate_label(label, 24);
                    let color = if is_last { fg } else { muted };
                    let p = path.clone();
                    let bd_color = th.border;
                    let btn = button(
                        text(lbl).size(13).color(color),
                    )
                    .on_press(Message::Navigate(p))
                    .style(move |_, status| {
                        let bg = if status == iced::widget::button::Status::Hovered {
                            panel_hi
                        } else {
                            Color::TRANSPARENT
                        };
                        iced::widget::button::Style {
                            background: Some(iced::Background::Color(bg)),
                            border: Border { radius: 6.0.into(), ..Default::default() },
                            text_color: fg,
                            ..Default::default()
                        }
                    });
                    let _ = bd_color;
                    breadcrumb_row = breadcrumb_row.push(btn);
                }
                breadcrumb::BreadcrumbEntry::Ellipsis => {
                    breadcrumb_row = breadcrumb_row.push(
                        text("...").size(13).color(muted),
                    );
                }
            }
            if i != last_idx {
                let chev = Svg::new(SvgHandle::from_memory(icons::CHEVRON_RIGHT))
                    .width(Length::Fixed(10.0))
                    .height(Length::Fixed(10.0))
                    .style(move |_, _| iced::widget::svg::Style { color: Some(muted) });
                breadcrumb_row = breadcrumb_row.push(
                    container(chev).padding([0, 2]),
                );
            }
        }

        let toolbar = crate::toolbar::view(
            th,
            !self.current_tab().back_stack.is_empty(),
            !self.current_tab().forward_stack.is_empty(),
            self.search_visible,
            &self.search_query,
            self.view_mode,
            breadcrumb_row.into(),
        );

        // -- Tab bar (polish v2) -------------------------------------------
        let tab_bar: iced::Element<Message> = if self.tabs.len() > 1 || self.tabs.len() == 1 {
            tabs_view::view(th, &self.tabs[..], self.active_tab)
        } else {
            container(iced::widget::horizontal_space())
                .height(Length::Fixed(0.0))
                .into()
        };

        // -- Status bar (polish v2: itens / selecionados / disco) ----------
        let (free, total) = self.disk_cache
            .as_ref()
            .map(|(_, f, t, _)| (*f, *t))
            .unwrap_or((0, 0));
        let total_items = self.current_tab().file_list.entries.len();
        let selected_n = self.current_tab().file_list.selected.len();
        let status_bar = statusbar::view(th, selected_n, total_items, free, total, &self.status);

        // -- Sidebar (polish v2: SVG icons, hover pill, active accent bar) -
        let mut locais_col: Vec<iced::Element<Message>> = Vec::new();
        let mut drives_col: Vec<iced::Element<Message>> = Vec::new();

        let render_row = |label: String,
                          path: PathBuf,
                          svg_bytes: &'static [u8],
                          is_active: bool,
                          indent: u16,
                          chevron: Option<(bool, PathBuf)>|
         -> iced::Element<Message> {
            let icon_color = if is_active { accent } else { muted };
            // active_bar (3px left) ja sinaliza item ativo; bg transparent pra menos peso visual.
            let selected_bg = Color::TRANSPARENT;

            let icon = Svg::new(SvgHandle::from_memory(svg_bytes))
                .width(Length::Fixed(16.0))
                .height(Length::Fixed(16.0))
                .style(move |_, _| iced::widget::svg::Style { color: Some(icon_color) });

            let active_bar = container(iced::widget::horizontal_space())
                .width(Length::Fixed(3.0))
                .height(Length::Fixed(18.0))
                .style(move |_| iced::widget::container::Style {
                    background: Some(iced::Background::Color(
                        if is_active { accent } else { Color::TRANSPARENT },
                    )),
                    border: Border { radius: 2.0.into(), ..Default::default() },
                    ..Default::default()
                });

            let chevron_elem: iced::Element<Message> = if let Some((expanded, toggle_path)) = chevron {
                let glyph = if expanded { "v" } else { ">" };
                button(text(glyph.to_string()).size(11).color(muted))
                    .on_press(Message::ToggleSidebarExpand(toggle_path))
                    .style(move |_, _| iced::widget::button::Style {
                        background: Some(iced::Background::Color(Color::TRANSPARENT)),
                        border: Border { radius: 4.0.into(), ..Default::default() },
                        text_color: muted,
                        ..Default::default()
                    })
                    .padding([0u16, 4u16])
                    .width(Length::Fixed(16.0))
                    .into()
            } else {
                container(iced::widget::horizontal_space())
                    .width(Length::Fixed(16.0))
                    .into()
            };

            let btn = button(
                row![
                    active_bar,
                    chevron_elem,
                    container(icon).width(Length::Fixed(16.0)).height(Length::Fixed(16.0)),
                    text(label).size(13).color(fg),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .on_press(Message::Navigate(path))
            .style(move |_, status| {
                let bg = if is_active {
                    selected_bg
                } else if status == iced::widget::button::Status::Hovered {
                    panel_hi
                } else {
                    Color::TRANSPARENT
                };
                iced::widget::button::Style {
                    background: Some(iced::Background::Color(bg)),
                    border: Border { radius: 8.0.into(), ..Default::default() },
                    text_color: fg,
                    ..Default::default()
                }
            })
            .padding([6, 10])
            .width(Length::Fill);

            container(btn)
                .padding([0u16, 4u16 + indent])
                .width(Length::Fill)
                .into()
        };

        for item in &self.sidebar {
            let is_active = item.path == self.current_tab().current_dir;
            let path = item.path.clone();
            let kind = item.kind.clone();
            let expandable = matches!(kind, SidebarKind::Home | SidebarKind::Drive);
            let is_expanded = self.expanded.contains(&path);
            let chevron = if expandable {
                Some((is_expanded, path.clone()))
            } else {
                None
            };
            let btn_wrapped = render_row(
                item.label.clone(),
                path.clone(),
                kind.svg_bytes(),
                is_active,
                0,
                chevron,
            );

            if kind == SidebarKind::Drive {
                drives_col.push(btn_wrapped);
            } else {
                locais_col.push(btn_wrapped);
            }

            // W21: subdirs nivel 1 quando Inicio expandido.
            if matches!(kind, SidebarKind::Home) && is_expanded {
                for sub in &self.home_subdirs {
                    let sub_active = *sub == self.current_tab().current_dir;
                    let sub_label = sub.file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_string();
                    let sub_row = render_row(
                        sub_label,
                        sub.clone(),
                        crate::icons::FOLDER,
                        sub_active,
                        20,
                        None,
                    );
                    locais_col.push(sub_row);
                }
            }
        }

        let group_header = |label: &'static str| -> iced::Element<Message> {
            container(text(label).size(10).color(muted))
                .padding([10u16, 12u16])
                .into()
        };

        let mut sidebar_col = column![].spacing(2).padding([8, 6]);
        sidebar_col = sidebar_col.push(group_header("INÍCIO"));
        for btn in locais_col {
            sidebar_col = sidebar_col.push(btn);
        }
        if !drives_col.is_empty() {
            sidebar_col = sidebar_col.push(
                container(horizontal_rule(1)).padding([8, 0]).width(Length::Fill),
            );
            sidebar_col = sidebar_col.push(group_header("DRIVES"));
            for btn in drives_col {
                sidebar_col = sidebar_col.push(btn);
            }
        }

        let sidebar = container(scrollable(sidebar_col).height(Length::Fill))
            .width(220)
            .height(Length::Fill)
            .style({
                let bg = th.bg_subtle;
                let bd = th.border;
                move |_| iced::widget::container::Style {
                    background: Some(iced::Background::Color(bg)),
                    border: Border { color: bd, width: 0.0, ..Default::default() },
                    ..Default::default()
                }
            });


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
        let body = if self.preview_visible {
            let selected_paths = self.current_tab().file_list.selected_paths();
            let preview: iced::Element<Message> = if let Some(path) = selected_paths.first() {
                self.view_preview(path, fg, muted, accent)
            } else {
                container(text("Selecione um item").size(13).color(muted))
                    .padding([20, 12])
                    .width(Length::Fixed(300.0))
                    .height(Length::Fill)
                    .style(move |_| iced::widget::container::Style {
                        background: Some(iced::Background::Color(LumoTheme::panel())),
                        ..Default::default()
                    })
                    .into()
            };
            row![sidebar, content_area, preview].height(Length::Fill)
        } else {
            row![sidebar, content_area].height(Length::Fill)
        };

        let root = container(column![toolbar, tab_bar, body, status_bar].spacing(0))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container_style(bg));

        // -- Properties dialog overlay ------------------------------------
        let root: iced::Element<Message> = if let Some(ref props) = self.properties {
            let name_input = text_input("Nome", &props.name_edit)
                .on_input(Message::PropertiesNameChanged)
                .on_submit(Message::PropertiesApply)
                .size(13)
                .padding([6, 10]);

            let path = &props.path;
            let size_str = crate::filelist::FileList::human_size(path);
            let mod_str = crate::filelist::FileList::human_modified(path);
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("--").to_string();
            let perms = {
                use std::os::unix::fs::PermissionsExt;
                path.metadata().map(|m| format!("{:o}", m.permissions().mode() & 0o777)).unwrap_or_else(|_| "--".to_string())
            };

            let btn_apply = button(text("Aplicar").size(13).color(fg))
                .on_press(Message::PropertiesApply)
                .style(move |_, _| iced::widget::button::Style {
                    background: Some(iced::Background::Color(LumoTheme::accent())),
                    border: iced::Border { radius: 4.0.into(), ..Default::default() },
                    text_color: LumoTheme::bg(),
                    ..Default::default()
                })
                .padding([6, 12]);

            let btn_cancel = button(text("Cancelar").size(13).color(muted))
                .on_press(Message::CloseProperties)
                .style(move |_, _| iced::widget::button::Style {
                    background: Some(iced::Background::Color(Color::TRANSPARENT)),
                    border: iced::Border { radius: 4.0.into(), ..Default::default() },
                    text_color: LumoTheme::muted(),
                    ..Default::default()
                })
                .padding([6, 12]);

            let dialog = container(
                column![
                    text("Propriedades").size(16).color(fg),
                    container(horizontal_rule(1)).padding([4, 0]).width(Length::Fill),
                    text("Nome:").size(12).color(muted),
                    name_input,
                    text("Tamanho:").size(12).color(muted),
                    text(size_str.clone()).size(13).color(fg),
                    text("Modificado:").size(12).color(muted),
                    text(mod_str.clone()).size(13).color(fg),
                    text("Tipo:").size(12).color(muted),
                    text(ext.clone()).size(13).color(fg),
                    text("Permissoes:").size(12).color(muted),
                    text(perms.clone()).size(13).color(fg),
                    container(horizontal_rule(1)).padding([4, 0]).width(Length::Fill),
                    row![btn_apply, btn_cancel].spacing(8),
                ]
                .spacing(6)
                .padding([20, 24]),
            )
            .width(Length::Fixed(400.0))
            .style(move |_| iced::widget::container::Style {
                background: Some(iced::Background::Color(LumoTheme::panel_hi())),
                border: iced::Border {
                    color: LumoTheme::sep(),
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..Default::default()
            });

            // P1.5: Dialog substituye root inteiro para ser overlay real.
            // Iced 0.13 nao tem Stack nativo; substituir body e a solucao correta.
            let _ = root; // root nao renderizado quando dialog ativo
            container(
                container(dialog)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| iced::widget::container::Style {
                background: Some(iced::Background::Color(Color { a: 0.6, ..LumoTheme::bg() })),
                ..Default::default()
            })
            .into()
        } else {
            root.into()
        };

        // -- Context menu overlay ------------------------------------------
        let with_ctx: Element<Message> = if let Some(ref ctx) = self.context_menu {
            self.view_context_menu(ctx, root, fg, panel_hi, muted, accent)
        } else {
            root
        };

        // -- Toasts overlay (bottom-right, inline column) ------------------
        if self.toasts.is_empty() {
            with_ctx
        } else {
            // Embaixo da base, alinha a direita.
            column![
                with_ctx,
                crate::toast::view(&self.theme, &self.toasts),
            ]
            .into()
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
            self.current_tab().file_list.entries.iter().enumerate().collect()
        } else {
            let q = self.search_query.to_ascii_lowercase();
            self.current_tab().file_list.entries.iter().enumerate()
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
        const CELL_W: f32 = 112.0;
        const COLS: usize = 7;
        let th = &self.theme;

        // W19 BUG-FIX: empty wins over skeleton. Vazio = empty-state limpo,
        // nunca placeholders fantasmas sobrepostos com texto "Pasta vazia".
        if entries.is_empty() {
            if self.loading {
                return self.view_grid_skeleton();
            }
            return self.view_empty_state();
        }

        let mut grid_rows: Vec<Element<Message>> = Vec::new();
        let chunks: Vec<&[(usize, &PathBuf)]> = entries.chunks(COLS).collect();

        for chunk in chunks.iter() {
            let mut r = row![].spacing(10);
            for (idx, path) in chunk.iter() {
                let idx = *idx;
                let is_selected = self.current_tab().file_list.selected.contains(&idx);
                let name = FileList::display_name_max(path, 18);
                let kind = icon_for_path(path);
                let icon_color = if matches!(kind, IconKind::Folder) { accent } else { muted };
                let cell_bg = if is_selected { th.accent_subtle } else { Color::TRANSPARENT };

                let svg_icon = Svg::new(SvgHandle::from_memory(icons::svg_bytes_for_kind(&kind)))
                    .width(Length::Fixed(56.0))
                    .height(Length::Fixed(56.0))
                    .style(move |_, _| iced::widget::svg::Style { color: Some(icon_color) });

                let cell_content: Element<Message> = if self.current_tab().file_list.renaming == Some(idx) {
                    column![
                        container(svg_icon).width(Length::Fixed(56.0)).height(Length::Fixed(56.0)),
                        text_input("nome", &self.current_tab().file_list.rename_input)
                            .on_input(Message::RenameInputChanged)
                            .on_submit(Message::RenameConfirm)
                            .size(11)
                            .padding([2, 4]),
                    ]
                    .spacing(6)
                    .align_x(Alignment::Center)
                    .into()
                } else if matches!(kind, IconKind::Image) {
                    let thumb_key = crate::thumbs::cache_key(path);
                    if let Some(bytes) = self.thumb_cache.get(&thumb_key) {
                        column![
                            iced::widget::image::Image::new(
                                iced::widget::image::Handle::from_bytes(bytes.clone())
                            )
                                .width(Length::Fixed(80.0))
                                .height(Length::Fixed(80.0)),
                            text(name).size(11).color(fg),
                        ]
                        .spacing(8)
                        .align_x(Alignment::Center)
                        .into()
                    } else {
                        column![
                            container(svg_icon).width(Length::Fixed(56.0)).height(Length::Fixed(56.0)),
                            text(name).size(11).color(fg),
                        ]
                        .spacing(8)
                        .align_x(Alignment::Center)
                        .into()
                    }
                } else {
                    column![
                        container(svg_icon).width(Length::Fixed(56.0)).height(Length::Fixed(56.0)),
                        text(name).size(11).color(fg),
                    ]
                    .spacing(8)
                    .align_x(Alignment::Center)
                    .into()
                };

                let panel_hi_local = th.bg_subtle;
                let cell = button(
                    container(cell_content)
                        .width(Length::Fixed(CELL_W))
                        .height(Length::Fixed(128.0))
                        .padding([12, 8])
                        .align_x(Alignment::Center)
                        .style(move |_| iced::widget::container::Style {
                            background: Some(iced::Background::Color(cell_bg)),
                            border: Border { radius: 12.0.into(), ..Default::default() },
                            ..Default::default()
                        }),
                )
                .on_press(Message::ItemClicked { idx, ctrl: false, shift: false })
                .style(move |_, status| {
                    let bg = if is_selected {
                        cell_bg
                    } else if status == iced::widget::button::Status::Hovered {
                        panel_hi_local
                    } else {
                        Color::TRANSPARENT
                    };
                    iced::widget::button::Style {
                        background: Some(iced::Background::Color(bg)),
                        border: Border { radius: 12.0.into(), ..Default::default() },
                        text_color: fg,
                        ..Default::default()
                    }
                })
                .padding(0);

                r = r.push(cell);
            }
            grid_rows.push(r.into());
        }

        if grid_rows.is_empty() {
            return self.view_empty_state();
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
        let th = &self.theme;

        // W19 BUG-FIX: empty wins over skeleton.
        if entries.is_empty() {
            if self.loading {
                return self.view_list_skeleton();
            }
            return self.view_empty_state();
        }

        let sort_indicator = |col: SortBy| -> Element<'static, Message> {
            if self.sort_by != col {
                return iced::widget::horizontal_space().into();
            }
            let bytes = if self.sort_ascending {
                icons::ARROW_UP
            } else {
                icons::ARROW_UP // visualmente invertido nao precisa de DIFFERENT svg
            };
            // Pra ascending mostramos a seta como-eh; pra descending viramos o glifo
            // via text("v") como fallback simples
            let s = if self.sort_ascending { " " } else { " " };
            let _ = (bytes, s);
            container(text(if self.sort_ascending { "^" } else { "v" }).size(10).color(accent))
                .padding([0, 4])
                .into()
        };

        let header_cell = |label: &'static str, col: SortBy, w: Length| -> Element<Message> {
            let is_active = self.sort_by == col;
            let color = if is_active { fg } else { muted };
            button(
                row![
                    text(label).size(12).color(color),
                    sort_indicator(col),
                ]
                .spacing(4)
                .align_y(Alignment::Center),
            )
            .on_press(Message::SetSortBy(col))
            .padding([6, 12])
            .style(move |_, status| {
                let bg = if status == iced::widget::button::Status::Hovered {
                    Color { a: 0.04, ..fg }
                } else {
                    Color::TRANSPARENT
                };
                iced::widget::button::Style {
                    background: Some(iced::Background::Color(bg)),
                    border: Border { radius: 4.0.into(), ..Default::default() },
                    text_color: fg,
                    ..Default::default()
                }
            })
            .width(w)
            .into()
        };

        let header = container(
            row![
                header_cell("Nome", SortBy::Name, Length::Fill),
                header_cell("Tamanho", SortBy::Size, Length::Fixed(96.0)),
                header_cell("Modificado", SortBy::ModifiedDate, Length::Fixed(160.0)),
                header_cell("Tipo", SortBy::Type, Length::Fixed(80.0)),
            ]
            .spacing(0)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .style({
            let bg = th.bg_subtle;
            let bd = th.border;
            move |_| iced::widget::container::Style {
                background: Some(iced::Background::Color(bg)),
                border: Border {
                    color: bd,
                    width: 0.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            }
        });

        let mut rows: Vec<Element<Message>> = vec![header.into()];

        if entries.is_empty() {
            rows.push(self.view_empty_state_inline());
            return container(scrollable(column(rows).spacing(0)).height(Length::Fill))
                .width(Length::Fill).height(Length::Fill)
                .style({
                    let bg = th.bg;
                    move |_| iced::widget::container::Style {
                        background: Some(iced::Background::Color(bg)),
                        ..Default::default()
                    }
                })
                .into();
        }

        for (idx, path) in entries {
            let idx = *idx;
            let is_selected = self.current_tab().file_list.selected.contains(&idx);
            let row_bg = if is_selected { th.accent_subtle } else { Color::TRANSPARENT };
            let kind = icon_for_path(path);
            let icon_color = if matches!(kind, IconKind::Folder) { accent } else { muted };
            let name_str = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let size_str = if path.is_dir() { "--".to_string() } else { FileList::human_size(path) };
            let mod_str = FileList::human_modified_relative(path);
            let type_str = FileList::human_type(path);

            let svg_icon = Svg::new(SvgHandle::from_memory(icons::svg_bytes_for_kind(&kind)))
                .width(Length::Fixed(14.0))
                .height(Length::Fixed(14.0))
                .style(move |_, _| iced::widget::svg::Style { color: Some(icon_color) });

            let active_bar = container(iced::widget::horizontal_space())
                .width(Length::Fixed(2.0))
                .height(Length::Fixed(20.0))
                .style({
                    let c = if is_selected { accent } else { Color::TRANSPARENT };
                    move |_| iced::widget::container::Style {
                        background: Some(iced::Background::Color(c)),
                        ..Default::default()
                    }
                });

            let row_content = row![
                active_bar,
                row![
                    container(svg_icon).width(Length::Fixed(14.0)).height(Length::Fixed(14.0)),
                    text(name_str).size(13).color(fg),
                ].spacing(8).align_y(Alignment::Center).width(Length::Fill),
                text(size_str).size(12).color(muted).width(Length::Fixed(96.0)),
                text(mod_str).size(12).color(muted).width(Length::Fixed(160.0)),
                text(type_str).size(12).color(muted).width(Length::Fixed(80.0)),
            ]
            .spacing(10)
            .align_y(Alignment::Center);

            let panel_hi_local = th.bg_subtle;
            let row_btn = button(
                container(row_content)
                    .padding([6, 12])
                    .width(Length::Fill)
                    .style(move |_| iced::widget::container::Style {
                        background: Some(iced::Background::Color(row_bg)),
                        ..Default::default()
                    }),
            )
            .on_press(Message::ItemClicked { idx, ctrl: false, shift: false })
            .style(move |_, status| {
                let bg = if is_selected {
                    row_bg
                } else if status == iced::widget::button::Status::Hovered {
                    panel_hi_local
                } else {
                    Color::TRANSPARENT
                };
                iced::widget::button::Style {
                    background: Some(iced::Background::Color(bg)),
                    border: Border::default(),
                    text_color: fg,
                    ..Default::default()
                }
            })
            .padding(0)
            .width(Length::Fill);

            rows.push(row_btn.into());
        }

        container(scrollable(column(rows).spacing(0)).height(Length::Fill))
            .width(Length::Fill).height(Length::Fill)
            .style({
                let bg = th.bg;
                move |_| iced::widget::container::Style {
                    background: Some(iced::Background::Color(bg)),
                    ..Default::default()
                }
            })
            .into()
    }

    fn view_grid_skeleton(&self) -> Element<Message> {
        let th = &self.theme;
        const COLS: usize = 7;
        const ROWS: usize = 2;
        let mut grid_rows: Vec<Element<Message>> = Vec::new();
        for _ in 0..ROWS {
            let mut r = row![].spacing(10);
            for _ in 0..COLS {
                let placeholder = container(iced::widget::horizontal_space())
                    .width(Length::Fixed(112.0))
                    .height(Length::Fixed(128.0))
                    .style({
                        let bg = th.bg_subtle;
                        move |_| iced::widget::container::Style {
                            background: Some(iced::Background::Color(bg)),
                            border: Border { radius: 12.0.into(), ..Default::default() },
                            ..Default::default()
                        }
                    });
                r = r.push(placeholder);
            }
            grid_rows.push(r.into());
        }
        container(column(grid_rows).spacing(10).padding([12, 12]))
            .width(Length::Fill)
            .height(Length::Fill)
            .style({
                let bg = th.bg;
                move |_| iced::widget::container::Style {
                    background: Some(iced::Background::Color(bg)),
                    ..Default::default()
                }
            })
            .into()
    }

    fn view_list_skeleton(&self) -> Element<Message> {
        let th = &self.theme;
        let mut rows: Vec<Element<Message>> = Vec::new();
        for _ in 0..6 {
            let placeholder = container(iced::widget::horizontal_space())
                .width(Length::Fill)
                .height(Length::Fixed(20.0))
                .style({
                    let bg = th.bg_subtle;
                    move |_| iced::widget::container::Style {
                        background: Some(iced::Background::Color(bg)),
                        border: Border { radius: 4.0.into(), ..Default::default() },
                        ..Default::default()
                    }
                });
            rows.push(container(placeholder).padding([6, 12]).into());
        }
        container(column(rows).spacing(0))
            .width(Length::Fill)
            .height(Length::Fill)
            .style({
                let bg = th.bg;
                move |_| iced::widget::container::Style {
                    background: Some(iced::Background::Color(bg)),
                    ..Default::default()
                }
            })
            .into()
    }

    fn view_empty_state(&self) -> Element<Message> {
        let th = &self.theme;
        let icon = Svg::new(SvgHandle::from_memory(icons::FOLDER_OPEN))
            .width(Length::Fixed(64.0))
            .height(Length::Fixed(64.0))
            .style({
                let c = th.fg_subtle;
                move |_, _| iced::widget::svg::Style { color: Some(c) }
            });
        let body = column![
            container(icon).padding([0, 0]),
            text("Esta pasta esta vazia").size(14).color(th.fg_subtle),
            text("Arraste arquivos aqui ou use Ctrl+N para criar pasta")
                .size(11)
                .color(th.fg_subtle),
        ]
        .spacing(12)
        .align_x(Alignment::Center);
        container(body)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .width(Length::Fill)
            .height(Length::Fill)
            .style({
                let bg = th.bg;
                move |_| iced::widget::container::Style {
                    background: Some(iced::Background::Color(bg)),
                    ..Default::default()
                }
            })
            .into()
    }

    fn view_empty_state_inline(&self) -> Element<Message> {
        let th = &self.theme;
        let icon = Svg::new(SvgHandle::from_memory(icons::FOLDER_OPEN))
            .width(Length::Fixed(64.0))
            .height(Length::Fixed(64.0))
            .style({
                let c = th.fg_subtle;
                move |_, _| iced::widget::svg::Style { color: Some(c) }
            });
        container(
            column![
                container(icon).padding([0, 0]),
                text("Esta pasta esta vazia").size(13).color(th.fg_subtle),
                text("Use Ctrl+N para criar uma pasta").size(11).color(th.fg_subtle),
            ]
            .spacing(12)
            .align_x(Alignment::Center),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .width(Length::Fill)
        .height(Length::Fill)
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
                let is_selected = slf.current_tab().file_list.selected.contains(&idx);
                let row_bg = if is_selected { slf.theme.accent_subtle } else { Color::TRANSPARENT };
                let kind = icon_for_path(path);
                let icon_color = if matches!(kind, IconKind::Folder) { accent } else { muted };
                let svg_icon = Svg::new(SvgHandle::from_memory(icons::svg_bytes_for_kind(&kind)))
                    .width(Length::Fixed(14.0))
                    .height(Length::Fixed(14.0))
                    .style(move |_, _| iced::widget::svg::Style { color: Some(icon_color) });
                let name_str = FileList::display_name_max(path, 22);
                let row_content = row![
                    container(svg_icon).width(Length::Fixed(14.0)).height(Length::Fixed(14.0)),
                    text(name_str).size(13).color(fg),
                ]
                .spacing(8)
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

    fn view_preview(
        &self,
        path: &PathBuf,
        fg: Color,
        muted: Color,
        _accent: Color,
    ) -> iced::Element<Message> {
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let size_str = if path.is_dir() { "--".to_string() } else { crate::filelist::FileList::human_size(path) };
        let mod_str = crate::filelist::FileList::human_modified(path);
        let kind = crate::icons::icon_for_path(path);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("--").to_string();
        let icon_color = muted;
        let preview_icon = Svg::new(SvgHandle::from_memory(icons::svg_bytes_for_kind(&kind)))
            .width(Length::Fixed(40.0))
            .height(Length::Fixed(40.0))
            .style(move |_, _| iced::widget::svg::Style { color: Some(icon_color) });

        let text_preview: Option<String> = if matches!(ext.as_str(), "txt" | "md" | "json") {
            std::fs::read_to_string(path).ok().map(|s| {
                s.chars().take(200).collect()
            })
        } else {
            None
        };

        let mut info_col = column![
            container(preview_icon).width(Length::Fixed(40.0)).height(Length::Fixed(40.0)),
            text(name).size(13).color(fg),
            text("Tamanho:").size(11).color(muted),
            text(size_str).size(12).color(fg),
            text("Modificado:").size(11).color(muted),
            text(mod_str).size(12).color(fg),
            text("Tipo:").size(11).color(muted),
            text(ext).size(12).color(fg),
        ]
        .spacing(4)
        .padding([12, 12]);

        if let Some(preview_text) = text_preview {
            info_col = info_col.push(text("Conteudo:").size(11).color(muted));
            info_col = info_col.push(
                container(text(preview_text).size(11).color(fg))
                    .padding([6, 8])
                    .style(move |_| iced::widget::container::Style {
                        background: Some(iced::Background::Color(LumoTheme::panel_hi())),
                        border: iced::Border { radius: 4.0.into(), ..Default::default() },
                        ..Default::default()
                    })
            );
        }

        container(scrollable(info_col).height(Length::Fill))
            .width(Length::Fixed(300.0))
            .height(Length::Fill)
            .style(move |_| iced::widget::container::Style {
                background: Some(iced::Background::Color(LumoTheme::panel())),
                border: iced::Border {
                    color: LumoTheme::sep(),
                    width: 1.0,
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    }

    fn view_context_menu<'a>(
        &'a self,
        ctx: &ContextMenu,
        base: Element<'a, Message>,
        _fg: Color,
        _panel_hi: Color,
        _muted: Color,
        _accent: Color,
    ) -> Element<'a, Message> {
        // Iced 0.13 nao tem overlay nativo fora de custom widgets.
        // Empilhamos o menu como coluna abaixo da base view.
        let rename_msg = if let Some(&idx) = self.current_tab().file_list.selected.iter().next() {
            Message::RenameStart(idx)
        } else {
            Message::ContextMenuClose
        };
        let menu = ctxmenu::view(&self.theme, ctx, rename_msg);
        column![base, menu].into()
    }

    // -----------------------------------------------------------------------
    // Helpers internos
    // -----------------------------------------------------------------------

    fn push_back(&mut self) {
    }

    // -----------------------------------------------------------------------
    // Manual hit-test (workaround: Iced widget handlers nao disparam no
    // compositor atual — usamos RawEvent + geometria estatica da sidebar)
    // -----------------------------------------------------------------------

    /// Constantes de layout da sidebar (coordenadas window-relative, px).
    /// Toolbar ~44px + tab_bar ~34px = ~78px offset ate o corpo.
    const SIDEBAR_BODY_Y: f32 = 78.0;
    /// padding-top da sidebar column.
    const SIDEBAR_COL_PAD_TOP: f32 = 8.0;
    /// Altura do group_header INICIO (padding [10,12] + text ~10px).
    const SIDEBAR_HEADER_H: f32 = 30.0;
    /// Altura de cada item de sidebar (button padding [6,10] 16px icon + spacing 2).
    const SIDEBAR_ROW_H: f32 = 30.0;
    /// Largura maxima da sidebar (container width 220).
    const SIDEBAR_W: f32 = 220.0;

    fn dispatch_manual_click(&mut self, pos: iced::Point) -> Task<Message> {
        let x = pos.x;
        let y = pos.y;

        // Ignora cliques fora da sidebar
        if x >= Self::SIDEBAR_W {
            return Task::none();
        }

        // y relativo ao inicio da coluna sidebar (abaixo do toolbar + tabbar)
        let y_rel = y - Self::SIDEBAR_BODY_Y - Self::SIDEBAR_COL_PAD_TOP;
        if y_rel < 0.0 {
            return Task::none();
        }

        // Pula o header INICIO
        let y_items = y_rel - Self::SIDEBAR_HEADER_H;
        if y_items < 0.0 {
            return Task::none();
        }

        // Calcula indice do item clicado
        let raw_idx = (y_items / Self::SIDEBAR_ROW_H) as usize;

        // Constroi lista flat de itens visiveis (igual ao render loop em view())
        let mut flat: Vec<(std::path::PathBuf, bool)> = Vec::new();
        // bool = true se e item expandivel (Home ou Drive)
        for item in &self.sidebar {
            let expandable = matches!(item.kind, crate::sidebar::SidebarKind::Home | crate::sidebar::SidebarKind::Drive);
            flat.push((item.path.clone(), expandable));
            if matches!(item.kind, crate::sidebar::SidebarKind::Home) && self.expanded.contains(&item.path) {
                for sub in &self.home_subdirs {
                    flat.push((sub.clone(), false));
                }
            }
        }

        if let Some((path, expandable)) = flat.get(raw_idx) {
            let path = path.clone();
            let expandable = *expandable;
            // Chevron zone: container_pad(4) + btn_pad(10) + active_bar(3) + spacing(8) = 25..41
            // active_bar(3) + spacing(8) + chevron(16) = chevron at x=[25, 41] from sidebar edge
            let is_chevron_zone = expandable && x >= 25.0 && x <= 45.0;
            if is_chevron_zone {
                let p = path.clone();
                return Task::perform(async move { p }, Message::ToggleSidebarExpand);
            } else {
                let p = path.clone();
                return Task::perform(async move { p }, Message::Navigate);
            }
        }

        Task::none()
    }
}

// ---------------------------------------------------------------------------
// Async helpers
// ---------------------------------------------------------------------------

pub(crate) async fn load_dir(path: PathBuf, show_hidden: bool) -> Result<Vec<PathBuf>, String> {
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

#[allow(dead_code)]
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
        IconKind::Pdf => "[P]",
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

/// W21: lista subdirs imediatos pra tree sidebar. Sync, blocking (~1ms).
fn load_immediate_subdirs(root: &PathBuf) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                    if name.starts_with('.') {
                        continue;
                    }
                }
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_app() -> App {
        let home = PathBuf::from("/tmp");
        let initial_tab = Tab::new(home.clone());
        App {
            tabs: vec![initial_tab],
            active_tab: 0,
            sidebar: vec![],
            clipboard: None,
            context_menu: None,
            status: String::new(),
            new_folder_input: None,
            show_hidden: false,
            search_visible: false,
            search_query: String::new(),
            view_mode: crate::toolbar::ViewMode::Grid,
            sort_by: crate::filelist::SortBy::Name,
            sort_ascending: true,
            preview_visible: false,
            thumb_cache: ThumbCache::new(),
            properties: None,
            theme: crate::theme::ThemeSnapshot::dark(),
            toasts: crate::toast::ToastQueue::new(),
            loading: false,
            disk_cache: None,
            expanded: std::collections::HashSet::new(),
            home_subdirs: Vec::new(),
            last_cursor_pos: iced::Point::ORIGIN,
        }
    }

    fn set_entries(app: &mut App, paths: Vec<PathBuf>) {
        app.current_tab_mut().file_list.set_entries(paths);
    }

    #[test]
    fn test_open_dir_sets_tab_current_dir() {
        let mut app = make_app();
        let dir = PathBuf::from("/tmp");
        // Simulate DirLoaded
        app.current_tab_mut().current_dir = dir.clone();
        assert_eq!(app.current_tab().current_dir, dir);
    }

    #[test]
    fn test_change_view_mode() {
        let mut app = make_app();
        app.view_mode = crate::toolbar::ViewMode::List;
        assert_eq!(app.view_mode, crate::toolbar::ViewMode::List);
        app.view_mode = crate::toolbar::ViewMode::Grid;
        assert_eq!(app.view_mode, crate::toolbar::ViewMode::Grid);
    }

    #[test]
    fn test_search_filter_empty_returns_all() {
        let mut app = make_app();
        let paths: Vec<PathBuf> = vec![
            PathBuf::from("/tmp/foo.txt"),
            PathBuf::from("/tmp/bar.txt"),
        ];
        set_entries(&mut app, paths.clone());
        app.search_query = String::new();
        let tab = app.current_tab();
        assert_eq!(tab.file_list.entries.len(), 2);
    }

    #[test]
    fn test_tab_cycle_next() {
        let mut app = make_app();
        // Add a second tab
        let tab2 = Tab::new(PathBuf::from("/home"));
        app.tabs.push(tab2);
        assert_eq!(app.tabs.len(), 2);
        app.active_tab = 0;
        // Cycle to next
        app.active_tab = (app.active_tab + 1) % app.tabs.len();
        assert_eq!(app.active_tab, 1);
        // Cycle wraps
        app.active_tab = (app.active_tab + 1) % app.tabs.len();
        assert_eq!(app.active_tab, 0);
    }

    #[test]
    fn test_close_tab_reduces_count() {
        let mut app = make_app();
        app.tabs.push(Tab::new(PathBuf::from("/home")));
        assert_eq!(app.tabs.len(), 2);
        app.tabs.remove(1);
        app.active_tab = app.active_tab.min(app.tabs.len() - 1);
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.active_tab, 0);
    }
}
