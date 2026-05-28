//! lumo-notif - daemon DBus org.freedesktop.Notifications + toast overlay.

mod center;
mod crash_watcher;
mod dbus;
mod history;
mod paint;
mod state;

use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    lumo_error::hook::install_panic_hook("lumo-notif", lumo_error::Domain::App);
    let (tx, rx) = mpsc::channel::<dbus::NotifEvent>(64);
    let tx2 = tx.clone();
    tokio::spawn(async move {
        if let Err(e) = dbus::serve(tx2).await {
            eprintln!("[lumo-notif] dbus falhou: {e}");
        }
    });
    // UX1: poll ~/.local/state/lumo/crashes/ + notif por crash novo.
    let tx_crash = tx.clone();
    tokio::spawn(async move {
        crash_watcher::run(tx_crash).await;
    });
    state::run(rx).await;
}
