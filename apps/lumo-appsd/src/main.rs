//! lumo-appsd - daemon Iced multi-window (W34).

use iced::{daemon, window, Size, Task, Theme};
use iced::window::Id as WinId;
use std::collections::HashMap;
use std::path::PathBuf;

const SOCKET_FILENAME: &str = "lumo-appsd.sock";

fn socket_path() -> PathBuf {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(runtime).join(SOCKET_FILENAME)
}

#[derive(Debug, Clone)]
pub enum AppKind { About, Calc }

impl AppKind {
    fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "about" => Some(Self::About),
            "calc"  => Some(Self::Calc),
            _ => None,
        }
    }
}

enum WindowState {
    About(lumo_about::app::App),
    Calc(lumo_calc::app::App),
}

#[derive(Debug, Clone)]
enum Msg {
    OpenApp(AppKind),
    Opened(AppKind, WinId),
    About(WinId, lumo_about::app::Message),
    Calc(WinId, lumo_calc::app::Message),
    WindowClosed(WinId),
}

struct State {
    windows: HashMap<WinId, WindowState>,
}

fn main() -> iced::Result {
    daemon(title, update, view)
        .theme(|_state, _id| Theme::Dark)
        .run_with(init)
}

fn init() -> (State, Task<Msg>) {
    let sock = socket_path();
    let _ = std::fs::remove_file(&sock);
    std::thread::spawn(move || ipc_listener(sock));
    eprintln!("[appsd] ready (W34)");
    let state = State { windows: HashMap::new() };
    // Abre About inicial pra Iced runtime persistir + warm-up.
    let task = Task::done(Msg::OpenApp(AppKind::About));
    (state, task)
}

fn ipc_listener(path: PathBuf) {
    use std::io::Read;
    use std::os::unix::net::UnixListener;
    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => { eprintln!("[appsd] bind: {}", e); return; }
    };
    for stream in listener.incoming() {
        let mut s = match stream { Ok(s) => s, Err(_) => continue };
        let mut buf = String::new();
        if s.read_to_string(&mut buf).is_err() { continue; }
        eprintln!("[appsd] IPC: {:?}", buf.trim());
        if let Some(kind) = AppKind::parse(&buf) {
            // Append to pending file - daemon Iced thread reads in subscription.
            let _ = std::fs::write(format!("/tmp/lumo-appsd-pending.{:?}", kind), "1");
        }
    }
}

fn title(state: &State, id: WinId) -> String {
    match state.windows.get(&id) {
        Some(WindowState::About(_)) => "Sobre este Galaxy Book".into(),
        Some(WindowState::Calc(_))  => "Lumo Calc".into(),
        None => "Lumo Apps".into(),
    }
}

fn update(state: &mut State, msg: Msg) -> Task<Msg> {
    match msg {
        Msg::OpenApp(kind) => {
            let settings = match kind {
                AppKind::About => window::Settings {
                    size: Size::new(540.0, 600.0),
                    min_size: Some(Size::new(480.0, 520.0)),
                    resizable: false,
                    decorations: true,
                    position: window::Position::Centered,
                    ..Default::default()
                },
                AppKind::Calc => window::Settings {
                    size: Size::new(460.0, 560.0),
                    min_size: Some(Size::new(380.0, 480.0)),
                    decorations: true,
                    position: window::Position::Centered,
                    ..Default::default()
                },
            };
            let (id, open_task) = window::open(settings);
            let kind_clone = kind.clone();
            open_task.map(move |_| Msg::Opened(kind_clone.clone(), id))
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
        Msg::WindowClosed(id) => {
            state.windows.remove(&id);
            Task::none()
        }
    }
}

fn view(state: &State, id: WinId) -> iced::Element<'_, Msg> {
    match state.windows.get(&id) {
        Some(WindowState::About(app)) => app.view().map(move |m| Msg::About(id, m)),
        Some(WindowState::Calc(app))  => app.view().map(move |m| Msg::Calc(id, m)),
        None => iced::widget::text("lumo-appsd loading...").into(),
    }
}
