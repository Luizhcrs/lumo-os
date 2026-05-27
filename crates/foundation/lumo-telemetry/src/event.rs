//! Event types for Lumo telemetry.

use std::collections::HashMap;

/// Timestamp in nanoseconds (CLOCK_MONOTONIC).
pub type TsNs = u64;

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Click,
    AppLaunch,
    FrameRender,
    IpcCall,
    WindowMap,
    WindowUnmap,
    KeyPress,
    GestureSwipe,
    WorkspaceSwitch,
}

impl std::fmt::Display for EventKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            EventKind::Click => "click",
            EventKind::AppLaunch => "app_launch",
            EventKind::FrameRender => "frame_render",
            EventKind::IpcCall => "ipc_call",
            EventKind::WindowMap => "window_map",
            EventKind::WindowUnmap => "window_unmap",
            EventKind::KeyPress => "key_press",
            EventKind::GestureSwipe => "gesture_swipe",
            EventKind::WorkspaceSwitch => "workspace_switch",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Event {
    pub ts_ns: TsNs,
    pub kind: EventKind,
    pub meta: HashMap<String, String>,
}

impl Event {
    pub fn new(kind: EventKind, meta: HashMap<String, String>) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let ts_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        Self { ts_ns, kind, meta }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_kind_display_snake_case() {
        assert_eq!(EventKind::Click.to_string(), "click");
        assert_eq!(EventKind::AppLaunch.to_string(), "app_launch");
        assert_eq!(EventKind::FrameRender.to_string(), "frame_render");
        assert_eq!(EventKind::WindowMap.to_string(), "window_map");
        assert_eq!(EventKind::WorkspaceSwitch.to_string(), "workspace_switch");
        assert_eq!(EventKind::GestureSwipe.to_string(), "gesture_swipe");
    }

    #[test]
    fn event_kind_serde_snake_case() {
        let json = serde_json::to_string(&EventKind::IpcCall).expect("serialize");
        assert_eq!(json, "\"ipc_call\"");
        let back: EventKind = serde_json::from_str("\"window_unmap\"").expect("deserialize");
        assert_eq!(back, EventKind::WindowUnmap);
    }

    #[test]
    fn event_new_sets_recent_timestamp() {
        let before_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        let ev = Event::new(EventKind::Click, HashMap::new());
        let after_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        assert!(ev.ts_ns >= before_ns);
        assert!(ev.ts_ns <= after_ns);
    }

    #[test]
    fn event_new_preserves_meta() {
        let mut meta = HashMap::new();
        meta.insert("app_id".into(), "com.lumo.files".into());
        let ev = Event::new(EventKind::AppLaunch, meta.clone());
        assert_eq!(ev.kind, EventKind::AppLaunch);
        assert_eq!(ev.meta.get("app_id"), Some(&"com.lumo.files".to_string()));
    }

    #[test]
    fn event_kind_eq_hash() {
        // Hash trait derivado -> permite uso em HashMap.
        let mut counts: HashMap<EventKind, u32> = HashMap::new();
        *counts.entry(EventKind::Click).or_insert(0) += 1;
        *counts.entry(EventKind::Click).or_insert(0) += 1;
        *counts.entry(EventKind::KeyPress).or_insert(0) += 1;
        assert_eq!(counts.get(&EventKind::Click), Some(&2));
        assert_eq!(counts.get(&EventKind::KeyPress), Some(&1));
    }
}
