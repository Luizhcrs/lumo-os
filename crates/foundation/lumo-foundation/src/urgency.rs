//! urgency.rs — DBus org.freedesktop.Notifications urgency level.
//!
//! Compartilhado entre lumo-notif (daemon DBus), lumo-osd-framework (centro
//! de mensagens), futuro lumo-center. Movido de apps/lumo-notif pro foundation
//! pra evitar duplicate enum entre crates (A3 review).
//!
//! Spec: org.freedesktop.Notifications hint "urgency" = byte:
//!   0 = Low      (subtle, easy timeout)
//!   1 = Normal   (default)
//!   2 = Critical (persistente ate user dismiss, visual destacado)

use serde::{Deserialize, Serialize};

/// Default timeouts em ms por urgency.
pub const LOW_TIMEOUT_MS: u64 = 4000;
pub const NORMAL_TIMEOUT_MS: u64 = 5000;
pub const CRITICAL_TIMEOUT_MS: u64 = 0; // 0 = sticky

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Urgency {
    Low,
    #[default]
    Normal,
    Critical,
}

impl Urgency {
    /// Parse byte do hint DBus. Valores fora de 0..=2 cai em Normal.
    pub fn from_byte(b: u8) -> Self {
        match b {
            0 => Urgency::Low,
            2 => Urgency::Critical,
            _ => Urgency::Normal,
        }
    }

    pub fn to_byte(self) -> u8 {
        match self {
            Urgency::Low => 0,
            Urgency::Normal => 1,
            Urgency::Critical => 2,
        }
    }

    /// Critical NUNCA expira por timeout (spec freedesktop).
    pub fn ignores_timeout(self) -> bool {
        matches!(self, Urgency::Critical)
    }

    /// Default timeout em ms se cliente passar -1.
    pub fn default_timeout_ms(self) -> u64 {
        match self {
            Urgency::Low => LOW_TIMEOUT_MS,
            Urgency::Normal => NORMAL_TIMEOUT_MS,
            Urgency::Critical => CRITICAL_TIMEOUT_MS,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_byte_low() {
        assert_eq!(Urgency::from_byte(0), Urgency::Low);
    }

    #[test]
    fn from_byte_normal() {
        assert_eq!(Urgency::from_byte(1), Urgency::Normal);
    }

    #[test]
    fn from_byte_critical() {
        assert_eq!(Urgency::from_byte(2), Urgency::Critical);
    }

    #[test]
    fn from_byte_unknown_falls_to_normal() {
        assert_eq!(Urgency::from_byte(99), Urgency::Normal);
        assert_eq!(Urgency::from_byte(255), Urgency::Normal);
    }

    #[test]
    fn roundtrip_byte() {
        for u in [Urgency::Low, Urgency::Normal, Urgency::Critical] {
            assert_eq!(Urgency::from_byte(u.to_byte()), u);
        }
    }

    #[test]
    fn critical_ignores_timeout() {
        assert!(Urgency::Critical.ignores_timeout());
        assert!(!Urgency::Normal.ignores_timeout());
        assert!(!Urgency::Low.ignores_timeout());
    }

    #[test]
    fn critical_default_timeout_is_sticky() {
        assert_eq!(Urgency::Critical.default_timeout_ms(), 0);
    }

    #[test]
    fn normal_default_timeout_5s() {
        assert_eq!(Urgency::Normal.default_timeout_ms(), 5000);
    }

    #[test]
    fn low_default_timeout_4s() {
        assert_eq!(Urgency::Low.default_timeout_ms(), 4000);
    }

    #[test]
    fn default_is_normal() {
        assert_eq!(Urgency::default(), Urgency::Normal);
    }

    #[test]
    fn debug_format() {
        assert_eq!(format!("{:?}", Urgency::Critical), "Critical");
    }
}
