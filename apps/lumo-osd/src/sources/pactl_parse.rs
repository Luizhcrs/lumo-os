//! pactl_parse.rs — parse output de `pactl get-sink-volume @DEFAULT_SINK@`
//! e `pactl get-sink-mute @DEFAULT_SINK@`.
//!
//! Pure parsing — chamadas pactl via Command em main.rs. Tests usam
//! strings fixtures de pactl real output.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VolumeState {
    /// 0-100+ (pode passar de 100 com boost).
    pub pct: u32,
    pub muted: bool,
}

impl VolumeState {
    /// 0.0-1.5 normalizado (1.0 = 100%, ate 1.5 com boost).
    pub fn ratio(&self) -> f32 {
        (self.pct as f32 / 100.0).clamp(0.0, 1.5)
    }
}

/// Parse stdout de `pactl get-sink-volume @DEFAULT_SINK@`.
/// Formato (PulseAudio + PipeWire):
///   "Volume: front-left: 45875 /  70% / -9.32 dB,   front-right: 45875 /  70% / -9.32 dB"
/// Retorna porcentagem do front-left.
pub fn parse_volume(stdout: &str) -> Option<u32> {
    // Procura primeiro "NNN%" no output.
    let bytes = stdout.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            // Recua pra pegar numero.
            let mut j = i;
            while j > 0 && (bytes[j - 1].is_ascii_digit() || bytes[j - 1] == b' ') {
                j -= 1;
            }
            let slice = std::str::from_utf8(&bytes[j..i]).ok()?;
            return slice.trim().parse().ok();
        }
        i += 1;
    }
    None
}

/// Parse stdout de `pactl get-sink-mute @DEFAULT_SINK@`.
/// Formato: "Mute: yes" ou "Mute: no".
pub fn parse_mute(stdout: &str) -> Option<bool> {
    let lower = stdout.to_lowercase();
    if lower.contains("mute:") {
        if lower.contains("yes") || lower.contains("true") {
            return Some(true);
        }
        if lower.contains("no") || lower.contains("false") {
            return Some(false);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_volume_basic() {
        let s = "Volume: front-left: 45875 /  70% / -9.32 dB";
        assert_eq!(parse_volume(s), Some(70));
    }

    #[test]
    fn parse_volume_zero() {
        let s = "Volume: front-left: 0 /   0% / -inf dB";
        assert_eq!(parse_volume(s), Some(0));
    }

    #[test]
    fn parse_volume_full() {
        let s = "Volume: front-left: 65536 / 100% / 0.00 dB";
        assert_eq!(parse_volume(s), Some(100));
    }

    #[test]
    fn parse_volume_boosted() {
        let s = "Volume: front-left: 98304 / 150% / +8.78 dB";
        assert_eq!(parse_volume(s), Some(150));
    }

    #[test]
    fn parse_volume_stereo_returns_first() {
        let s = "Volume: front-left: 45875 / 45% /  -9.32 dB,   front-right: 65536 / 80% / 0.00 dB";
        assert_eq!(parse_volume(s), Some(45));
    }

    #[test]
    fn parse_volume_no_percent_none() {
        assert_eq!(parse_volume("no percent here"), None);
    }

    #[test]
    fn parse_volume_empty_returns_none() {
        assert_eq!(parse_volume(""), None);
    }

    #[test]
    fn parse_mute_yes() {
        assert_eq!(parse_mute("Mute: yes"), Some(true));
    }

    #[test]
    fn parse_mute_no() {
        assert_eq!(parse_mute("Mute: no"), Some(false));
    }

    #[test]
    fn parse_mute_uppercase() {
        assert_eq!(parse_mute("Mute: YES"), Some(true));
    }

    #[test]
    fn parse_mute_unrelated_none() {
        assert_eq!(parse_mute("Volume: 50%"), None);
    }

    #[test]
    fn parse_mute_empty_none() {
        assert_eq!(parse_mute(""), None);
    }

    #[test]
    fn ratio_clamped_to_one_point_five() {
        let s = VolumeState { pct: 200, muted: false };
        assert_eq!(s.ratio(), 1.5);
    }

    #[test]
    fn ratio_zero_when_pct_zero() {
        let s = VolumeState { pct: 0, muted: false };
        assert_eq!(s.ratio(), 0.0);
    }

    #[test]
    fn ratio_one_at_100_pct() {
        let s = VolumeState { pct: 100, muted: false };
        assert!((s.ratio() - 1.0).abs() < 0.001);
    }

    #[test]
    fn volume_state_eq() {
        assert_eq!(
            VolumeState { pct: 50, muted: false },
            VolumeState { pct: 50, muted: false }
        );
        assert_ne!(
            VolumeState { pct: 50, muted: false },
            VolumeState { pct: 50, muted: true }
        );
    }
}
