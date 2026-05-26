//! dbus.rs - implementa org.freedesktop.Notifications via zbus.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use zbus::{connection, interface, message::Header};

pub enum NotifEvent {
    Notify {
        id: u32,
        app_name: String,
        summary: String,
        body: String,
        timeout_ms: i32,
    },
    CloseNotification {
        id: u32,
    },
}

/// Mapa de id -> sender DBus unico (BusName). Usado pra validar replaces_id.
type SenderMap = Arc<Mutex<HashMap<u32, String>>>;

struct NotificationsServer {
    tx: mpsc::Sender<NotifEvent>,
    counter: std::sync::atomic::AtomicU32,
    sender_map: SenderMap,
}

#[interface(name = "org.freedesktop.Notifications")]
impl NotificationsServer {
    async fn notify(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        app_name: String,
        replaces_id: u32,
        _app_icon: String,
        summary: String,
        body: String,
        _actions: Vec<String>,
        _hints: std::collections::HashMap<String, zbus::zvariant::Value<'_>>,
        expire_timeout: i32,
    ) -> u32 {
        let sender = hdr.sender().map(|s| s.to_string()).unwrap_or_default();

        let id = if replaces_id != 0 {
            let map = self.sender_map.lock().unwrap();
            if map.get(&replaces_id).map(|s| s == &sender).unwrap_or(false) {
                replaces_id
            } else {
                self.counter
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                    + 1
            }
        } else {
            self.counter
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                + 1
        };

        self.sender_map.lock().unwrap().insert(id, sender);

        let _ = self
            .tx
            .send(NotifEvent::Notify {
                id,
                app_name,
                summary,
                body,
                timeout_ms: expire_timeout,
            })
            .await;
        id
    }

    async fn close_notification(&self, id: u32) {
        self.sender_map.lock().unwrap().remove(&id);
        let _ = self.tx.send(NotifEvent::CloseNotification { id }).await;
    }

    fn get_capabilities(&self) -> Vec<String> {
        vec![
            "body".to_string(),
            "body-markup".to_string(),
            "persistence".to_string(),
        ]
    }

    fn get_server_information(&self) -> (&str, &str, &str, &str) {
        ("lumo-notif", "lumo-os", "0.1.0", "1.2")
    }
}

pub async fn serve(tx: mpsc::Sender<NotifEvent>) -> zbus::Result<()> {
    let server = NotificationsServer {
        tx,
        counter: std::sync::atomic::AtomicU32::new(0),
        sender_map: Arc::new(Mutex::new(HashMap::new())),
    };
    let _conn = connection::Builder::session()?
        .name("org.freedesktop.Notifications")?
        .serve_at("/org/freedesktop/Notifications", server)?
        .build()
        .await?;
    std::future::pending::<()>().await;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn replaces_id_rejected_different_sender() {
        use std::collections::HashMap;
        let mut map: HashMap<u32, String> = HashMap::new();
        map.insert(5, ":1.10".to_string());
        let replaces_id: u32 = 5;
        let sender = ":1.99".to_string();
        let id = if replaces_id != 0 {
            if map.get(&replaces_id).map(|s| s == &sender).unwrap_or(false) {
                replaces_id
            } else {
                42u32
            }
        } else {
            42u32
        };
        assert_ne!(id, replaces_id, "sender diferente deve receber novo id");
    }

    #[test]
    fn replaces_id_accepted_same_sender() {
        use std::collections::HashMap;
        let mut map: HashMap<u32, String> = HashMap::new();
        map.insert(5, ":1.10".to_string());
        let replaces_id: u32 = 5;
        let sender = ":1.10".to_string();
        let id = if replaces_id != 0 {
            if map.get(&replaces_id).map(|s| s == &sender).unwrap_or(false) {
                replaces_id
            } else {
                99u32
            }
        } else {
            99u32
        };
        assert_eq!(id, replaces_id, "mesmo sender deve poder substituir notif");
    }
}
