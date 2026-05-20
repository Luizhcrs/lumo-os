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
