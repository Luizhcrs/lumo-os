//! toast_logic.rs - F1.5-B1: logic pura de expiry/effective_timeout pra Toast.
//!
//! Extraido de state.rs::Toast pra testar Critical-never-expires sem Wayland deps.

use crate::urgency::Urgency;
use std::time::Duration;

/// Decide o timeout efetivo em ms aplicando regras de urgency + cliente-passed expire_timeout.
///
/// Regras (spec freedesktop + Lumo defaults):
///   - Critical: SEMPRE sticky (retorna 0 = nao expira), ignora cliente.
///   - Cliente passou `expire_timeout = 0`: sticky (nao expira).
///   - Cliente passou `expire_timeout > 0`: usa esse valor.
///   - Cliente passou `expire_timeout = -1` (default): usa urgency.default_timeout_ms().
pub fn effective_timeout_ms(client_expire_timeout: i32, urgency: Urgency) -> u64 {
    if urgency.ignores_timeout() {
        return 0;
    }
    match client_expire_timeout {
        0 => 0,
        n if n > 0 => n as u64,
        _ => urgency.default_timeout_ms(),
    }
}

/// True se toast deve auto-dismiss.
///
/// Regras:
///   - Critical NUNCA expira (mesmo passado a duracao).
///   - timeout_ms == 0: sticky, nunca expira.
///   - elapsed >= timeout_ms: expira.
///   - hover ou dismissing: nao expira ainda.
pub fn should_expire(
    urgency: Urgency,
    timeout_ms: u64,
    elapsed: Duration,
    hover: bool,
    dismissing: bool,
) -> bool {
    if hover || dismissing {
        return false;
    }
    if urgency.ignores_timeout() || timeout_ms == 0 {
        return false;
    }
    elapsed >= Duration::from_millis(timeout_ms)
}

/// Critical toast bypassa o limite MAX_TOASTS quando empilhando (nao desloca outros critical).
///
/// Retorna `Some(idx)` se um toast nao-critical na fila pode ser dismissed pra dar espaco.
/// `None` se todos os existentes sao critical (nesse caso, novo critical aceita stack alem do max).
pub fn slot_to_evict_for_critical(urgencies: &[Urgency]) -> Option<usize> {
    urgencies
        .iter()
        .position(|u| !matches!(u, Urgency::Critical))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn critical_always_sticky() {
        assert_eq!(effective_timeout_ms(5000, Urgency::Critical), 0);
        assert_eq!(effective_timeout_ms(-1, Urgency::Critical), 0);
        assert_eq!(effective_timeout_ms(0, Urgency::Critical), 0);
    }

    #[test]
    fn client_zero_means_sticky_for_non_critical() {
        assert_eq!(effective_timeout_ms(0, Urgency::Normal), 0);
        assert_eq!(effective_timeout_ms(0, Urgency::Low), 0);
    }

    #[test]
    fn client_positive_used_for_non_critical() {
        assert_eq!(effective_timeout_ms(3000, Urgency::Normal), 3000);
        assert_eq!(effective_timeout_ms(1500, Urgency::Low), 1500);
    }

    #[test]
    fn client_negative_falls_to_urgency_default() {
        assert_eq!(effective_timeout_ms(-1, Urgency::Normal), 5000);
        assert_eq!(effective_timeout_ms(-1, Urgency::Low), 4000);
    }

    #[test]
    fn should_expire_critical_never() {
        let huge = Duration::from_secs(3600);
        assert!(!should_expire(
            Urgency::Critical,
            5000,
            huge,
            false,
            false
        ));
    }

    #[test]
    fn should_expire_sticky_zero_timeout() {
        let huge = Duration::from_secs(3600);
        assert!(!should_expire(Urgency::Normal, 0, huge, false, false));
    }

    #[test]
    fn should_expire_after_duration() {
        assert!(should_expire(
            Urgency::Normal,
            1000,
            Duration::from_millis(1500),
            false,
            false
        ));
    }

    #[test]
    fn should_not_expire_before_duration() {
        assert!(!should_expire(
            Urgency::Normal,
            5000,
            Duration::from_millis(500),
            false,
            false
        ));
    }

    #[test]
    fn should_not_expire_when_hover() {
        assert!(!should_expire(
            Urgency::Normal,
            100,
            Duration::from_secs(60),
            true,
            false
        ));
    }

    #[test]
    fn should_not_expire_when_dismissing() {
        assert!(!should_expire(
            Urgency::Normal,
            100,
            Duration::from_secs(60),
            false,
            true
        ));
    }

    #[test]
    fn evict_picks_first_non_critical() {
        let q = [Urgency::Critical, Urgency::Normal, Urgency::Critical];
        assert_eq!(slot_to_evict_for_critical(&q), Some(1));
    }

    #[test]
    fn evict_returns_none_when_all_critical() {
        let q = [Urgency::Critical, Urgency::Critical];
        assert_eq!(slot_to_evict_for_critical(&q), None);
    }

    #[test]
    fn evict_picks_first_when_no_critical() {
        let q = [Urgency::Normal, Urgency::Low];
        assert_eq!(slot_to_evict_for_critical(&q), Some(0));
    }

    #[test]
    fn evict_empty_returns_none() {
        let q: [Urgency; 0] = [];
        assert_eq!(slot_to_evict_for_critical(&q), None);
    }
}
