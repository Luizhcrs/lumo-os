//! lumo-appsd - daemon Iced multi-window completo (W34.9).
//!
//! 8 apps Lumo co-existem em runtime Iced unico:
//! about, calc, notes, monitor, editor, files, settings, store.
//!
//! IPC via /run/user/UID/lumo-appsd.sock. Cliente: lumo-appctl <kind>.
//!
//! W34.9 fixes:
//! - pending args fila VecDeque (era Option, race se 2 OpenAppArg rapid)
//! - accept loop spawn per connection (era serial, cliente lento bloqueava)
//! - bind retry com unlink (era falha hard em EADDRINUSE)
//! - view None retorna empty (era texto "loading..." flicker)
//! - editor recebe path arg tambem

use iced::{daemon, window, Size, Task, Theme, Subscription};
use iced::window::Id as WinId;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

const SOCKET_FILENAME: &str = "lumo-appsd.sock";

fn socket_path() -> PathBuf {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(runtime).join(SOCKET_FILENAME)
}

/// W34.10: notifica lumo-wm que abriu janela com app_id conhecido.
/// Workaround Iced 0.13 (nao emite xdg_toplevel.set_app_id a tempo).
/// Envia LumoCommand::AppActivated via socket lumo-wm.sock.
fn send_wm_app_activated(app_id: &str, title: &str) {
    let app_id = app_id.to_string();
    let title = title.to_string();
    let pid = std::process::id();
    // W34.16: spawn thread + sleep antes drop pra dar tempo WM ler buffered bytes.
    std::thread::spawn(move || {
        use std::io::Write;
        use std::os::unix::net::UnixStream;
        let runtime = match std::env::var("XDG_RUNTIME_DIR") {
            Ok(r) => r,
            Err(_) => return,
        };
        let path = format!("{}/lumo-wm.sock", runtime);
        let mut s = match UnixStream::connect(&path) {
            Ok(s) => s,
            Err(e) => { eprintln!("[appsd] W34.10 connect wm: {}", e); return; }
        };
        let payload = format!(
            "{{\"type\":\"app_activated\",\"app_id\":\"{}\",\"title\":\"{}\",\"pid\":{}}}\n",
            app_id, title, pid
        );
        if let Err(e) = s.write_all(payload.as_bytes()) {
            eprintln!("[appsd] W34.10 write wm: {}", e);
            return;
        }
        // W34.17: hold connection alive ate WM read. 100ms suficiente pro calloop tick (4ms).
        std::thread::sleep(std::time::Duration::from_millis(100));
        eprintln!("[appsd] W34.10 sent AppActivated {} pid={}", app_id, pid);
        // drop fecha socket
    });
}

/// W34.11: notifica lumo-wm que todas janelas fecharam. Bar limpa pills.
fn send_wm_app_deactivated() {
    std::thread::spawn(|| {
        use std::io::Write;
        use std::os::unix::net::UnixStream;
        let runtime = match std::env::var("XDG_RUNTIME_DIR") {
            Ok(r) => r,
            Err(_) => { eprintln!("[appsd] W34.11 sem XDG_RUNTIME_DIR"); return; }
        };
        let path = format!("{}/lumo-wm.sock", runtime);
        let mut s = match UnixStream::connect(&path) {
            Ok(s) => s,
            Err(e) => { eprintln!("[appsd] W34.11 connect wm: {}", e); return; }
        };
        let payload = b"{\"type\":\"app_deactivated\"}\n";
        if let Err(e) = s.write_all(payload) {
            eprintln!("[appsd] W34.11 write wm: {}", e);
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
        eprintln!("[appsd] W34.11 sent AppDeactivated to {}", path);
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    /// W34.9 fix #1: fila FIFO de args por AppKind (era Option, race se rapid).
    pending_args: HashMap<AppKind, VecDeque<String>>,
}

fn main() -> iced::Result {
    daemon(title_fn, update, view)
        .theme(|_state, _id| Theme::Dark)
        .subscription(combined_subscription)
        .run_with(init)
}

fn combined_subscription(state: &State) -> Subscription<Msg> {
    let mut subs: Vec<Subscription<Msg>> = vec![ipc_subscription(state)];
    for (&id, win) in state.windows.iter() {
        let s = match win {
            WindowState::About(a)    => a.subscription().map(move |m| Msg::About(id, m)),
            WindowState::Calc(a)     => a.subscription().map(move |m| Msg::Calc(id, m)),
            WindowState::Notes(a)    => a.subscription().map(move |m| Msg::Notes(id, m)),
            WindowState::Monitor(a)  => a.subscription().map(move |m| Msg::Monitor(id, m)),
            WindowState::Editor(a)   => a.subscription().map(move |m| Msg::Editor(id, m)),
            WindowState::Files(a)    => a.subscription().map(move |m| Msg::Files(id, m)),
            WindowState::Settings(a) => a.subscription().map(move |m| Msg::Settings(id, m)),
            WindowState::Store(_)    => continue,
        };
        subs.push(s);
    }
    subs.push(iced::window::close_events().map(Msg::WindowClosed));
    Subscription::batch(subs)
}

fn init() -> (State, Task<Msg>) {
    let sock = socket_path();
    let _ = std::fs::remove_file(&sock);
    eprintln!("[appsd] ready (W34.9) socket={}", sock.display());
    let state = State {
        windows: HashMap::new(),
        pending_args: HashMap::new(),
    };
    (state, Task::none())
}

fn ipc_subscription(_state: &State) -> Subscription<Msg> {
    use iced::stream::channel;
    use iced::futures::SinkExt;
    use tokio::io::AsyncReadExt;
    use tokio::net::UnixListener;

    Subscription::run_with_id(
        "lumo-appsd-ipc",
        channel(16, |output| async move {
            let path = socket_path();
            // W34.9 fix #5: retry unlink+bind se EADDRINUSE.
            let listener = loop {
                match UnixListener::bind(&path) {
                    Ok(l) => break l,
                    Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                        eprintln!("[appsd] socket in use, unlink+retry: {}", path.display());
                        let _ = std::fs::remove_file(&path);
                        continue;
                    }
                    Err(e) => {
                        eprintln!("[appsd] bind socket: {}", e);
                        std::future::pending::<()>().await;
                        unreachable!();
                    }
                }
            };
            eprintln!("[appsd] IPC listening {}", path.display());
            loop {
                let (mut stream, _addr) = match listener.accept().await {
                    Ok(s) => s,
                    Err(e) => { eprintln!("[appsd] accept: {}", e); continue; }
                };
                // W34.9 fix #2: spawn per connection (era serial, lento bloqueava todos).
                let mut tx = output.clone();
                tokio::spawn(async move {
                    let mut buf = String::new();
                    if let Err(e) = stream.read_to_string(&mut buf).await {
                        eprintln!("[appsd] read: {}", e);
                        return;
                    }
                    eprintln!("[appsd] IPC recv: {:?}", buf.trim());
                    let line = buf.trim();
                    let (k_str, arg) = match line.find(':') {
                        Some(i) => (&line[..i], Some(line[i+1..].to_string())),
                        None => (line, None),
                    };
                    if let Some(kind) = AppKind::parse(k_str) {
                        let _ = tx.send(Msg::OpenAppArg(kind, arg)).await;
                    }
                });
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
        None => String::new(),
    }
}

fn update(state: &mut State, msg: Msg) -> Task<Msg> {
    match msg {
        Msg::OpenApp(kind) => {
            let (_id, open_task) = window::open(kind.settings());
            open_task.map(move |id| Msg::Opened(kind, id))
        }
        Msg::OpenAppArg(kind, arg) => {
            // W34.9 fix #1: push em fila FIFO por kind. Msg::Opened pop_front.
            if let Some(a) = arg {
                state.pending_args.entry(kind).or_default().push_back(a);
            }
            let (_id, open_task) = window::open(kind.settings());
            open_task.map(move |id| Msg::Opened(kind, id))
        }
        Msg::Opened(AppKind::About, id) => {
            let (app, t) = lumo_about::app::App::new();
            state.windows.insert(id, WindowState::About(app));
            send_wm_app_activated(AppKind::About.app_id(), AppKind::About.title());
            t.map(move |m| Msg::About(id, m))
        }
        Msg::Opened(AppKind::Calc, id) => {
            let (app, t) = lumo_calc::app::App::new();
            state.windows.insert(id, WindowState::Calc(app));
            send_wm_app_activated(AppKind::Calc.app_id(), AppKind::Calc.title());
            t.map(move |m| Msg::Calc(id, m))
        }
        Msg::Opened(AppKind::Notes, id) => {
            let (app, t) = lumo_notes::app::App::new();
            state.windows.insert(id, WindowState::Notes(app));
            send_wm_app_activated(AppKind::Notes.app_id(), AppKind::Notes.title());
            t.map(move |m| Msg::Notes(id, m))
        }
        Msg::Opened(AppKind::Monitor, id) => {
            let (app, t) = lumo_monitor::app::App::new();
            state.windows.insert(id, WindowState::Monitor(app));
            send_wm_app_activated(AppKind::Monitor.app_id(), AppKind::Monitor.title());
            t.map(move |m| Msg::Monitor(id, m))
        }
        Msg::Opened(AppKind::Editor, id) => {
            // W34.9: editor aceita path arg.
            let arg = state.pending_args.get_mut(&AppKind::Editor)
                .and_then(|q| q.pop_front());
            let (app, t) = lumo_editor::app::App::new(arg);
            state.windows.insert(id, WindowState::Editor(app));
            send_wm_app_activated(AppKind::Editor.app_id(), AppKind::Editor.title());
            t.map(move |m| Msg::Editor(id, m))
        }
        Msg::Opened(AppKind::Files, id) => {
            let arg = state.pending_args.get_mut(&AppKind::Files)
                .and_then(|q| q.pop_front());
            let (app, t) = if let Some(p) = arg {
                lumo_files::app::App::new_with_dir(PathBuf::from(p))
            } else {
                lumo_files::app::App::new()
            };
            state.windows.insert(id, WindowState::Files(app));
            send_wm_app_activated(AppKind::Files.app_id(), AppKind::Files.title());
            t.map(move |m| Msg::Files(id, m))
        }
        Msg::Opened(AppKind::Settings, id) => {
            let arg = state.pending_args.get_mut(&AppKind::Settings)
                .and_then(|q| q.pop_front());
            let (app, t) = lumo_settings::app::App::new();
            if let Some(tab) = arg {
                eprintln!("[appsd] settings arg ignorado (sem tab API): {}", tab);
            }
            state.windows.insert(id, WindowState::Settings(app));
            send_wm_app_activated(AppKind::Settings.app_id(), AppKind::Settings.title());
            t.map(move |m| Msg::Settings(id, m))
        }
        Msg::Opened(AppKind::Store, id) => {
            // W34.9 fix #8: tab arg pass-through (era ignorado).
            let arg = state.pending_args.get_mut(&AppKind::Store)
                .and_then(|q| q.pop_front());
            let (app, t) = lumo_store::app::StoreApp::new();
            if let Some(tab) = arg {
                eprintln!("[appsd] store tab={} (StoreApp::new sem tab arg ainda)", tab);
            }
            state.windows.insert(id, WindowState::Store(app));
            send_wm_app_activated(AppKind::Store.app_id(), AppKind::Store.title());
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
            // W34.11: se nenhuma janela mais, notifica bar pra limpar pills.
            if state.windows.is_empty() {
                send_wm_app_deactivated();
            }
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
        // W34.9 fix #10: empty space em vez de "loading..." flicker.
        None => iced::widget::Space::with_width(iced::Length::Fill).into(),
    }
}
