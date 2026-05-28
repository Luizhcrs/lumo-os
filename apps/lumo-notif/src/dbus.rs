//! dbus.rs - implementa org.freedesktop.Notifications via zbus.
//!
//! Security (review fixes):
//! - H2: clamp body/summary/app_name; rate-limit por sender DBus.
//! - H3: NAO anunciar `body-markup` capability — sanitize ja escapa.
//! - H5: Mutex::lock().unwrap_or_else(into_inner) pra resistir poisoning.
//! - L1: remover use dead em main.rs.
//! - L3: capability "persistence" mantida (real); markup dropped.

use lumo_notif::rate_limit::{rate_limit_check, safe_lock};
use lumo_notif::sanitize::clamp;
use lumo_notif::urgency::Urgency;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::mpsc;
use zbus::{connection, interface, message::Header};

const MAX_SUMMARY_CHARS: usize = 256;
const MAX_BODY_CHARS: usize = 4096;
const MAX_APP_NAME_CHARS: usize = 64;
const MAX_SENDER_MAP: usize = 1024;
/// Rate limit por sender DBus: max NOTIF_BURST notifs em NOTIF_WINDOW_MS.
const NOTIF_BURST: usize = 20;
const NOTIF_WINDOW_MS: u64 = 5_000;

pub enum NotifEvent {
    Notify {
        id: u32,
        app_name: String,
        summary: String,
        body: String,
        timeout_ms: i32,
        urgency: Urgency,
    },
    CloseNotification {
        id: u32,
    },
}

/// F1.5-B1: extrai byte do hint "urgency" -> Urgency enum. Spec freedesktop.
pub fn parse_urgency_hint(
    hints: &std::collections::HashMap<String, zbus::zvariant::Value<'_>>,
) -> Urgency {
    let Some(v) = hints.get("urgency") else {
        return Urgency::default();
    };
    // Spec diz byte (u8); clients reais as vezes mandam i32/u32. Tenta varios.
    if let Ok(b) = u8::try_from(v) {
        return Urgency::from_byte(b);
    }
    if let Ok(n) = i32::try_from(v) {
        if let Ok(b) = u8::try_from(n) {
            return Urgency::from_byte(b);
        }
    }
    if let Ok(n) = u32::try_from(v) {
        if let Ok(b) = u8::try_from(n) {
            return Urgency::from_byte(b);
        }
    }
    Urgency::default()
}

type SenderMap = Arc<Mutex<HashMap<u32, String>>>;
type RateMap = Arc<Mutex<HashMap<String, Vec<Instant>>>>;

struct NotificationsServer {
    tx: mpsc::Sender<NotifEvent>,
    counter: std::sync::atomic::AtomicU32,
    sender_map: SenderMap,
    rate_map: RateMap,
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
        hints: std::collections::HashMap<String, zbus::zvariant::Value<'_>>,
        expire_timeout: i32,
    ) -> u32 {
        let sender = hdr.sender().map(|s| s.to_string()).unwrap_or_default();

        // H2: rate-limit por sender. Sem sender (raro) -> bypass mas ainda
        // sujeito ao bound do sender_map.
        if !sender.is_empty() {
            let mut rates = safe_lock(&self.rate_map);
            let history = rates.entry(sender.clone()).or_default();
            if !rate_limit_check(history, Instant::now(), NOTIF_BURST, NOTIF_WINDOW_MS) {
                return 0;
            }
        }

        let urgency = parse_urgency_hint(&hints);

        // H2: clamp inputs.
        let app_name = clamp(&app_name, MAX_APP_NAME_CHARS);
        let summary = clamp(&summary, MAX_SUMMARY_CHARS);
        let body = clamp(&body, MAX_BODY_CHARS);

        let id = if replaces_id != 0 {
            let map = safe_lock(&self.sender_map);
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

        // L1+H2: cap sender_map evita memory leak por spam.
        {
            let mut map = safe_lock(&self.sender_map);
            if map.len() >= MAX_SENDER_MAP {
                // Evict arbitrario (HashMap order). Aceitavel: limite alto.
                if let Some(k) = map.keys().next().cloned() {
                    map.remove(&k);
                }
            }
            map.insert(id, sender);
        }

        let _ = self
            .tx
            .send(NotifEvent::Notify {
                id,
                app_name,
                summary,
                body,
                timeout_ms: expire_timeout,
                urgency,
            })
            .await;
        id
    }

    async fn close_notification(&self, id: u32) {
        safe_lock(&self.sender_map).remove(&id);
        let _ = self.tx.send(NotifEvent::CloseNotification { id }).await;
    }

    fn get_capabilities(&self) -> Vec<String> {
        // H3: NAO anunciar body-markup. Sanitize escapa por padrao.
        vec!["body".to_string(), "persistence".to_string()]
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
        rate_map: Arc::new(Mutex::new(HashMap::new())),
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
    use super::*;

    // M5: parse_urgency_hint cobertura
    #[test]
    fn parse_urgency_missing_hint_returns_normal() {
        let h = HashMap::new();
        assert_eq!(parse_urgency_hint(&h), Urgency::Normal);
    }

    #[test]
    fn parse_urgency_byte_0_low() {
        let mut h = HashMap::new();
        h.insert("urgency".to_string(), zbus::zvariant::Value::U8(0));
        assert_eq!(parse_urgency_hint(&h), Urgency::Low);
    }

    #[test]
    fn parse_urgency_byte_2_critical() {
        let mut h = HashMap::new();
        h.insert("urgency".to_string(), zbus::zvariant::Value::U8(2));
        assert_eq!(parse_urgency_hint(&h), Urgency::Critical);
    }

    #[test]
    fn parse_urgency_i32_critical() {
        let mut h = HashMap::new();
        h.insert("urgency".to_string(), zbus::zvariant::Value::I32(2));
        assert_eq!(parse_urgency_hint(&h), Urgency::Critical);
    }

    #[test]
    fn parse_urgency_u32_normal() {
        let mut h = HashMap::new();
        h.insert("urgency".to_string(), zbus::zvariant::Value::U32(1));
        assert_eq!(parse_urgency_hint(&h), Urgency::Normal);
    }

    #[test]
    fn parse_urgency_wrong_type_falls_to_default() {
        let mut h = HashMap::new();
        h.insert(
            "urgency".to_string(),
            zbus::zvariant::Value::Str("critical".into()),
        );
        assert_eq!(parse_urgency_hint(&h), Urgency::Normal);
    }

    #[test]
    fn parse_urgency_out_of_range_byte_falls_to_normal() {
        let mut h = HashMap::new();
        h.insert("urgency".to_string(), zbus::zvariant::Value::U8(99));
        assert_eq!(parse_urgency_hint(&h), Urgency::Normal);
    }

    // H2: rate_limit_check
    #[test]
    fn rate_limit_allows_below_burst() {
        let mut h = vec![];
        let now = Instant::now();
        for _ in 0..5 {
            assert!(rate_limit_check(&mut h, now, 10, 1000));
        }
    }

    #[test]
    fn rate_limit_blocks_after_burst() {
        let mut h = vec![];
        let now = Instant::now();
        for _ in 0..10 {
            assert!(rate_limit_check(&mut h, now, 10, 1000));
        }
        assert!(!rate_limit_check(&mut h, now, 10, 1000));
    }

    #[test]
    fn rate_limit_evicts_old_entries() {
        let mut h = vec![];
        let t0 = Instant::now();
        // 10 antigos
        for _ in 0..10 {
            rate_limit_check(&mut h, t0, 10, 100);
        }
        // Agora avancou 200ms — todos antigos saem do window.
        let later = t0 + std::time::Duration::from_millis(200);
        assert!(rate_limit_check(&mut h, later, 10, 100));
    }

    #[test]
    fn rate_limit_zero_burst_blocks_everything() {
        let mut h = vec![];
        assert!(!rate_limit_check(&mut h, Instant::now(), 0, 1000));
    }

    // replaces_id sender check
    #[test]
    fn replaces_id_rejected_different_sender() {
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
        assert_ne!(id, replaces_id);
    }

    #[test]
    fn replaces_id_accepted_same_sender() {
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
        assert_eq!(id, replaces_id);
    }
}
