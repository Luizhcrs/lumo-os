//! freeze.rs — detector de freeze de cliente Wayland via xdg ping/pong.
//!
//! xdg_wm_base.ping(serial) builtin: cliente toolkit (winit, sctk) responde
//! com pong(serial) automatico. Se cliente nao responde em PING_TIMEOUT,
//! marcamos como freeze.
//!
//! Estrategia:
//! - Cada toplevel tem `last_ping_sent` + `last_pong_seen`.
//! - Tick periodico (PING_INTERVAL): envia ping novo.
//! - Tick verifica: se pending_ping > PING_TIMEOUT sem pong, freeze.
//! - Recebe pong: cliente OK; se estava freeze, broadcast cleared.
//!
//! Implementacao em compositor:
//! - state.freeze: FreezeTracker
//! - calloop timer chama tick()
//! - xdg ping handler chama on_pong(pid, serial)
//!
//! NOTA: scheduler que envia ping real fica em handlers/xdg_shell.rs.
//! Este modulo so trackeia state + decide quando emit eventos.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use lumo_ipc::LumoEvent;

pub const PING_INTERVAL: Duration = Duration::from_millis(1000);
pub const PING_TIMEOUT: Duration = Duration::from_millis(2000);

#[derive(Debug, Clone)]
struct ClientState {
    app_id: String,
    last_pong: Instant,
    pending_ping_at: Option<Instant>,
    frozen: bool,
}

#[derive(Default)]
pub struct FreezeTracker {
    /// pid -> client state.
    clients: HashMap<u32, ClientState>,
}

impl FreezeTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registra novo toplevel client. Chama ao receber xdg_toplevel.
    pub fn register(&mut self, pid: u32, app_id: &str) {
        self.clients.insert(
            pid,
            ClientState {
                app_id: app_id.to_string(),
                last_pong: Instant::now(),
                pending_ping_at: None,
                frozen: false,
            },
        );
    }

    pub fn unregister(&mut self, pid: u32) {
        self.clients.remove(&pid);
    }

    /// Marca ping enviado pra esse pid neste instante.
    pub fn on_ping_sent(&mut self, pid: u32, when: Instant) {
        if let Some(c) = self.clients.get_mut(&pid) {
            c.pending_ping_at = Some(when);
        }
    }

    /// Pong recebido. Retorna evento se estava freeze.
    pub fn on_pong(&mut self, pid: u32, when: Instant) -> Option<LumoEvent> {
        let Some(c) = self.clients.get_mut(&pid) else {
            return None;
        };
        c.last_pong = when;
        c.pending_ping_at = None;
        if c.frozen {
            c.frozen = false;
            return Some(LumoEvent::AppFreezeCleared { pid });
        }
        None
    }

    /// Tick periodico. Verifica timeout pra cada client.
    /// Retorna Vec de eventos a broadcast (AppFreeze pra novos freezes).
    pub fn tick(&mut self, now: Instant) -> Vec<LumoEvent> {
        let mut events = Vec::new();
        for (pid, c) in self.clients.iter_mut() {
            let Some(sent_at) = c.pending_ping_at else {
                continue;
            };
            if c.frozen {
                continue;
            }
            if now.duration_since(sent_at) >= PING_TIMEOUT {
                c.frozen = true;
                events.push(LumoEvent::AppFreeze {
                    pid: *pid,
                    app_id: c.app_id.clone(),
                });
            }
        }
        events
    }

    pub fn is_frozen(&self, pid: u32) -> bool {
        self.clients.get(&pid).map(|c| c.frozen).unwrap_or(false)
    }

    pub fn pids_due_for_ping(&self, now: Instant) -> Vec<u32> {
        self.clients
            .iter()
            .filter(|(_, c)| {
                c.pending_ping_at.is_none() && now.duration_since(c.last_pong) >= PING_INTERVAL
            })
            .map(|(pid, _)| *pid)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_creates_client() {
        let mut t = FreezeTracker::new();
        t.register(42, "lumo-files");
        assert!(!t.is_frozen(42));
    }

    #[test]
    fn unregister_removes() {
        let mut t = FreezeTracker::new();
        t.register(42, "x");
        t.unregister(42);
        assert!(!t.is_frozen(42));
        assert!(t.pids_due_for_ping(Instant::now()).is_empty());
    }

    #[test]
    fn tick_no_freeze_when_no_ping_sent() {
        let mut t = FreezeTracker::new();
        t.register(1, "x");
        // Sem on_ping_sent, tick nao deve marcar freeze.
        let events = t.tick(Instant::now() + Duration::from_secs(10));
        assert!(events.is_empty());
    }

    #[test]
    fn tick_marks_freeze_after_timeout() {
        let mut t = FreezeTracker::new();
        t.register(1, "lumo-x");
        let t0 = Instant::now();
        t.on_ping_sent(1, t0);
        let events = t.tick(t0 + PING_TIMEOUT + Duration::from_millis(50));
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], LumoEvent::AppFreeze { pid: 1, .. }));
        assert!(t.is_frozen(1));
    }

    #[test]
    fn tick_does_not_re_emit_freeze() {
        let mut t = FreezeTracker::new();
        t.register(1, "x");
        let t0 = Instant::now();
        t.on_ping_sent(1, t0);
        t.tick(t0 + PING_TIMEOUT + Duration::from_millis(50));
        let again = t.tick(t0 + PING_TIMEOUT + Duration::from_secs(2));
        assert!(again.is_empty(), "freeze ja anunciado nao re-emite");
    }

    #[test]
    fn pong_clears_freeze_with_event() {
        let mut t = FreezeTracker::new();
        t.register(1, "x");
        let t0 = Instant::now();
        t.on_ping_sent(1, t0);
        t.tick(t0 + PING_TIMEOUT + Duration::from_millis(50));
        assert!(t.is_frozen(1));
        let ev = t.on_pong(1, t0 + Duration::from_secs(3)).expect("ev");
        assert!(matches!(ev, LumoEvent::AppFreezeCleared { pid: 1 }));
        assert!(!t.is_frozen(1));
    }

    #[test]
    fn pong_when_not_frozen_returns_none() {
        let mut t = FreezeTracker::new();
        t.register(1, "x");
        assert!(t.on_pong(1, Instant::now()).is_none());
    }

    #[test]
    fn pong_for_unknown_pid_returns_none() {
        let mut t = FreezeTracker::new();
        assert!(t.on_pong(999, Instant::now()).is_none());
    }

    #[test]
    fn pids_due_for_ping_excludes_pending() {
        let mut t = FreezeTracker::new();
        t.register(1, "x");
        let t0 = Instant::now();
        t.on_ping_sent(1, t0);
        let due = t.pids_due_for_ping(t0 + Duration::from_millis(100));
        assert!(due.is_empty(), "pending nao deve aparecer em due");
    }

    #[test]
    fn pids_due_for_ping_excludes_recent_pong() {
        let mut t = FreezeTracker::new();
        t.register(1, "x");
        // last_pong = Instant::now() por register; menos de PING_INTERVAL = nao due.
        let due = t.pids_due_for_ping(Instant::now());
        assert!(due.is_empty());
    }

    #[test]
    fn pids_due_for_ping_includes_overdue() {
        let mut t = FreezeTracker::new();
        t.register(1, "x");
        let due = t.pids_due_for_ping(Instant::now() + PING_INTERVAL + Duration::from_millis(50));
        assert_eq!(due, vec![1]);
    }
}
