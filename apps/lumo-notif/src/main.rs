//! lumo-notif - daemon DBus org.freedesktop.Notifications + toast overlay.

mod center;
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
    state::run(rx).await;
}
