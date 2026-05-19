//! dbus.rs - implementa org.freedesktop.Notifications via zbus.

use tokio::sync::mpsc;
use zbus::{connection, interface};

pub enum NotifEvent {
    Notify { id: u32, app_name: String, summary: String, body: String, timeout_ms: i32 },
    CloseNotification { id: u32 },
}

struct NotificationsServer {
    tx: mpsc::Sender<NotifEvent>,
    counter: std::sync::atomic::AtomicU32,
}

#[interface(name = "org.freedesktop.Notifications")]
impl NotificationsServer {
    async fn notify(
        &self, app_name: String, replaces_id: u32, _app_icon: String,
        summary: String, body: String, _actions: Vec<String>,
        _hints: std::collections::HashMap<String, zbus::zvariant::Value<'_>>,
        expire_timeout: i32,
    ) -> u32 {
        let id = if replaces_id != 0 { replaces_id } else {
            self.counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
        };
        let _ = self.tx.send(NotifEvent::Notify { id, app_name, summary, body, timeout_ms: expire_timeout }).await;
        id
    }
    async fn close_notification(&self, id: u32) {
        let _ = self.tx.send(NotifEvent::CloseNotification { id }).await;
    }
    fn get_capabilities(&self) -> Vec<String> {
        vec!["body".to_string(), "body-markup".to_string(), "persistence".to_string()]
    }
    fn get_server_information(&self) -> (&str, &str, &str, &str) {
        ("lumo-notif", "lumo-os", "0.1.0", "1.2")
    }
}

pub async fn serve(tx: mpsc::Sender<NotifEvent>) -> zbus::Result<()> {
    let server = NotificationsServer { tx, counter: std::sync::atomic::AtomicU32::new(0) };
    let _conn = connection::Builder::session()?
        .name("org.freedesktop.Notifications")?
        .serve_at("/org/freedesktop/Notifications", server)?
        .build().await?;
    std::future::pending::<()>().await;
    Ok(())
}
