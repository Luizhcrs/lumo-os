//! lumo-appsd - daemon Iced multi-window completo (W34.2).
//!
//! 8 apps Lumo co-existem em runtime Iced unico:
//! about, calc, notes, monitor, editor, files, settings, store.
//!
//! IPC via /run/user/UID/lumo-appsd.sock. Cliente: lumo-appctl <kind>.

use iced::{daemon, window, Size, Task, Theme, Subscription};
use iced::window::Id as WinId;
use std::collections::HashMap;
use std::path::PathBuf;

const SOCKET_FILENAME: &str = "lumo-appsd.sock";

fn socket_path() -> PathBuf {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(runtime).join(SOCKET_FILENAME)
}

#[derive(Debug, Clone, Copy)]
pub enum AppKind {
    About,
    Calc,
    Notes,
    Monitor,
    Editor,
    Files,
    Settings,
    Store,
}

impl AppKind {
    fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "about"    => Some(Self::About),
            "calc"     => Some(Self::Calc),
            "notes"    => Some(Self::Notes),
            "monitor"  => Some(Self::Monitor),
            "editor"   => Some(Self::Editor),
            "files"    => Some(Self::Files),
            "settings" => Some(Self::Settings),
            "store"    => Some(Self::Store),
            _ => None,
        }
    }
    fn app_id(self) -> &'static str {
        match self {
            AppKind::About    => "com.lumo.about",
            AppKind::Calc     => "com.lumo.calc",
            AppKind::Notes    => "com.lumo.notes",
            AppKind::Monitor  => "com.lumo.monitor",
            AppKind::Editor   => "com.lumo.editor",
            AppKind::Files    => "com.lumo.files",
            AppKind::Settings => "com.lumo.settings",
            AppKind::Store    => "com.lumo.store",
        }
    }
    fn settings(self) -> window::Settings {
        use window::Position::Centered;
        let (w, h, min_w, min_h, resize) = match self {
            AppKind::About    => (540.0, 600.0, 480.0, 520.0, false),
            AppKind::Calc     => (460.0, 560.0, 380.0, 480.0, true),
            AppKind::Notes    => (900.0, 620.0, 600.0, 400.0, true),
            AppKind::Monitor  => (900.0, 640.0, 700.0, 480.0, true),
            AppKind::Editor   => (880.0, 600.0, 400.0, 300.0, true),
            AppKind::Files    => (1000.0, 660.0, 600.0, 400.0, true),
            AppKind::Settings => (900.0, 620.0, 700.0, 480.0, true),
            AppKind::Store    => (900.0, 640.0, 720.0, 480.0, true),
        };
        let mut s = window::Settings {
            size: Size::new(w, h),
            min_size: Some(Size::new(min_w, min_h)),
            resizable: resize,
            decorations: true,
            position: Centered,
            ..Default::default()
        };
        s.platform_specific.application_id = self.app_id().to_string();
        s
    }
    fn title(self) -> &'static str {
        match self {
            AppKind::About    => "Sobre este Galaxy Book",
            AppKind::Calc     => "Lumo Calc",
            AppKind::Notes    => "Lumo Notes",
            AppKind::Monitor  => "Lumo Monitor",
            AppKind::Editor   => "Lumo Editor",
            AppKind::Files    => "Lumo Files",
            AppKind::Settings => "Lumo Settings",
            AppKind::Store    => "Lumo Store",
        }
    }
}

enum WindowState {
    About(lumo_about::app::App),
    Calc(lumo_calc::app::App),
    Notes(lumo_notes::app::App),
    Monitor(lumo_monitor::app::App),
    Editor(lumo_editor::app::App),
    Files(lumo_files::app::App),
    Settings(lumo_settings::app::App),
    Store(lumo_store::app::StoreApp),
}

#[derive(Debug, Clone)]
enum Msg {
    OpenApp(AppKind),
    OpenAppArg(AppKind, Option<String>),
    Opened(AppKind, WinId),
    About(WinId, lumo_about::app::Message),
    Calc(WinId, lumo_calc::app::Message),
    Notes(WinId, lumo_notes::app::Message),
    Monitor(WinId, lumo_monitor::app::Message),
    Editor(WinId, lumo_editor::app::Message),
    Files(WinId, lumo_files::app::Message),
    Settings(WinId, lumo_settings::app::Message),
    Store(WinId, lumo_store::app::Msg),
    WindowClosed(WinId),
}

struct State {
    windows: HashMap<WinId, WindowState>,
    pending_files_path: Option<std::path::PathBuf>,
}

fn main() -> iced::Result {
    daemon(title_fn, update, view)
        .theme(|_state, _id| Theme::Dark)
        .subscription(combined_subscription)
        .run_with(init)
}

/// W34.7: batch sub-app subscriptions + IPC + window close events.
fn combined_subscription(state: &State) -> Subscription<Msg> {
    let mut subs: Vec<Subscription<Msg>> = vec![ipc_subscription(state)];
    // Sub-app subscriptions wrapped per window id.
    for (&id, win) in state.windows.iter() {
        let s = match win {
            WindowState::About(a)    => a.subscription().map(move |m| Msg::About(id, m)),
            WindowState::Calc(a)     => a.subscription().map(move |m| Msg::Calc(id, m)),
            WindowState::Notes(a)    => a.subscription().map(move |m| Msg::Notes(id, m)),
            WindowState::Monitor(a)  => a.subscription().map(move |m| Msg::Monitor(id, m)),
            WindowState::Editor(a)   => a.subscription().map(move |m| Msg::Editor(id, m)),
            WindowState::Files(a)    => a.subscription().map(move |m| Msg::Files(id, m)),
            WindowState::Settings(a) => a.subscription().map(move |m| Msg::Settings(id, m)),
            WindowState::Store(_)    => continue, // lumo-store no subscription()
        };
        subs.push(s);
    }
    // Window close events -> Msg::WindowClosed pra cleanup state.windows.
    subs.push(iced::window::close_events().map(Msg::WindowClosed));
    Subscription::batch(subs)
}

fn init() -> (State, Task<Msg>) {
    let sock = socket_path();
    let _ = std::fs::remove_file(&sock);
    eprintln!("[appsd] ready (W34.2) socket={}", sock.display());
    let state = State { windows: HashMap::new(), pending_files_path: None };
    // W34.8: skip auto-open. Daemon Iced runtime persiste sem windows.
    // Bug: about inicial abria com size errado (renderiza so metade).
    (state, Task::none())
}

fn ipc_subscription(_state: &State) -> Subscription<Msg> {
    use iced::stream::channel;
    use iced::futures::SinkExt;
    use tokio::io::AsyncReadExt;
    use tokio::net::UnixListener;

    Subscription::run_with_id(
        "lumo-appsd-ipc",
        channel(16, |mut output| async move {
            let path = socket_path();
            let listener = match UnixListener::bind(&path) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("[appsd] bind socket: {}", e);
                    std::future::pending::<()>().await;
                    unreachable!();
                }
            };
            eprintln!("[appsd] IPC listening {}", path.display());
            loop {
                let (mut stream, _addr) = match listener.accept().await {
                    Ok(s) => s,
                    Err(e) => { eprintln!("[appsd] accept: {}", e); continue; }
                };
                let mut buf = String::new();
                if stream.read_to_string(&mut buf).await.is_err() { continue; }
                eprintln!("[appsd] IPC recv: {:?}", buf.trim());
                let line = buf.trim();
                let (k_str, arg) = match line.find(':') {
                    Some(i) => (&line[..i], Some(line[i+1..].to_string())),
                    None => (line, None),
                };
                if let Some(kind) = AppKind::parse(k_str) {
                    let _ = output.send(Msg::OpenAppArg(kind, arg)).await;
                }
            }
        }),
    )
}

fn title_fn(state: &State, id: WinId) -> String {
    match state.windows.get(&id) {
        Some(WindowState::About(_))    => AppKind::About.title().into(),
        Some(WindowState::Calc(_))     => AppKind::Calc.title().into(),
        Some(WindowState::Notes(_))    => AppKind::Notes.title().into(),
        Some(WindowState::Monitor(_))  => AppKind::Monitor.title().into(),
        Some(WindowState::Editor(_))   => AppKind::Editor.title().into(),
        Some(WindowState::Files(_))    => AppKind::Files.title().into(),
        Some(WindowState::Settings(_)) => AppKind::Settings.title().into(),
        Some(WindowState::Store(_))    => AppKind::Store.title().into(),
        None => "Lumo Apps".into(),
    }
}

fn update(state: &mut State, msg: Msg) -> Task<Msg> {
    match msg {
        Msg::OpenApp(kind) => {
            let (_id, open_task) = window::open(kind.settings());
            open_task.map(move |id| Msg::Opened(kind, id))
        }
        Msg::OpenAppArg(kind, arg) => {
            let (_id, open_task) = window::open(kind.settings());
            // Stash arg in pending map keyed by next window id.
            if let (AppKind::Files, Some(path)) = (kind, arg.clone()) {
                state.pending_files_path = Some(path.into());
            }
            open_task.map(move |id| Msg::Opened(kind, id))
        }
        Msg::Opened(AppKind::About, id) => {
            let (app, t) = lumo_about::app::App::new();
            state.windows.insert(id, WindowState::About(app));
            t.map(move |m| Msg::About(id, m))
        }
        Msg::Opened(AppKind::Calc, id) => {
            let (app, t) = lumo_calc::app::App::new();
            state.windows.insert(id, WindowState::Calc(app));
            t.map(move |m| Msg::Calc(id, m))
        }
        Msg::Opened(AppKind::Notes, id) => {
            let (app, t) = lumo_notes::app::App::new();
            state.windows.insert(id, WindowState::Notes(app));
            t.map(move |m| Msg::Notes(id, m))
        }
        Msg::Opened(AppKind::Monitor, id) => {
            let (app, t) = lumo_monitor::app::App::new();
            state.windows.insert(id, WindowState::Monitor(app));
            t.map(move |m| Msg::Monitor(id, m))
        }
        Msg::Opened(AppKind::Editor, id) => {
            let (app, t) = lumo_editor::app::App::new(None);
            state.windows.insert(id, WindowState::Editor(app));
            t.map(move |m| Msg::Editor(id, m))
        }
        Msg::Opened(AppKind::Files, id) => {
            let (app, t) = if let Some(p) = state.pending_files_path.take() {
                lumo_files::app::App::new_with_dir(p)
            } else {
                lumo_files::app::App::new()
            };
            state.windows.insert(id, WindowState::Files(app));
            t.map(move |m| Msg::Files(id, m))
        }
        Msg::Opened(AppKind::Settings, id) => {
            let (app, t) = lumo_settings::app::App::new();
            state.windows.insert(id, WindowState::Settings(app));
            t.map(move |m| Msg::Settings(id, m))
        }
        Msg::Opened(AppKind::Store, id) => {
            let (app, t) = lumo_store::app::StoreApp::new();
            state.windows.insert(id, WindowState::Store(app));
            t.map(move |m| Msg::Store(id, m))
        }
        Msg::About(id, m) => {
            if let Some(WindowState::About(app)) = state.windows.get_mut(&id) {
                app.update(m).map(move |m| Msg::About(id, m))
            } else { Task::none() }
        }
        Msg::Calc(id, m) => {
            if let Some(WindowState::Calc(app)) = state.windows.get_mut(&id) {
                app.update(m).map(move |m| Msg::Calc(id, m))
            } else { Task::none() }
        }
        Msg::Notes(id, m) => {
            if let Some(WindowState::Notes(app)) = state.windows.get_mut(&id) {
                app.update(m).map(move |m| Msg::Notes(id, m))
            } else { Task::none() }
        }
        Msg::Monitor(id, m) => {
            if let Some(WindowState::Monitor(app)) = state.windows.get_mut(&id) {
                app.update(m).map(move |m| Msg::Monitor(id, m))
            } else { Task::none() }
        }
        Msg::Editor(id, m) => {
            if let Some(WindowState::Editor(app)) = state.windows.get_mut(&id) {
                app.update(m).map(move |m| Msg::Editor(id, m))
            } else { Task::none() }
        }
        Msg::Files(id, m) => {
            if let Some(WindowState::Files(app)) = state.windows.get_mut(&id) {
                app.update(m).map(move |m| Msg::Files(id, m))
            } else { Task::none() }
        }
        Msg::Settings(id, m) => {
            if let Some(WindowState::Settings(app)) = state.windows.get_mut(&id) {
                app.update(m).map(move |m| Msg::Settings(id, m))
            } else { Task::none() }
        }
        Msg::Store(id, m) => {
            if let Some(WindowState::Store(app)) = state.windows.get_mut(&id) {
                app.update(m).map(move |m| Msg::Store(id, m))
            } else { Task::none() }
        }
        Msg::WindowClosed(id) => {
            state.windows.remove(&id);
            Task::none()
        }
    }
}

fn view(state: &State, id: WinId) -> iced::Element<'_, Msg> {
    match state.windows.get(&id) {
        Some(WindowState::About(app))    => app.view().map(move |m| Msg::About(id, m)),
        Some(WindowState::Calc(app))     => app.view().map(move |m| Msg::Calc(id, m)),
        Some(WindowState::Notes(app))    => app.view().map(move |m| Msg::Notes(id, m)),
        Some(WindowState::Monitor(app))  => app.view().map(move |m| Msg::Monitor(id, m)),
        Some(WindowState::Editor(app))   => app.view().map(move |m| Msg::Editor(id, m)),
        Some(WindowState::Files(app))    => app.view().map(move |m| Msg::Files(id, m)),
        Some(WindowState::Settings(app)) => app.view().map(move |m| Msg::Settings(id, m)),
        Some(WindowState::Store(app))    => app.view().map(move |m| Msg::Store(id, m)),
        None => iced::widget::text("lumo-appsd loading...").into(),
    }
}
