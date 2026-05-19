//! tests.rs -- testes de integracao do lumo-firstrun.

#[cfg(test)]
mod integration {
    use crate::steps::{AccountState, Locale, Step, WifiNetwork};
    use crate::system::mark_first_run_done;
    use crate::locale::LocaleConfig;
    use tempfile::tempdir;

    #[test]
    fn step_full_sequence() {
        let mut s = Step::Welcome;
        let order = [Step::Welcome, Step::Language, Step::Account, Step::Wifi, Step::Done];
        for expected in order {
            assert_eq!(s, expected);
            s = s.next();
        }
    }

    #[test]
    fn account_valid_user() {
        let acc = AccountState {
            username: "galaxy".into(),
            password: "lumos123".into(),
            password_confirm: "lumos123".into(),
            error: None,
        };
        assert!(acc.validate().is_ok());
    }

    #[test]
    fn account_rejects_empty() {
        let acc = AccountState::default();
        assert!(acc.validate().is_err());
    }

    #[test]
    fn locale_all_has_codes() {
        for &loc in Locale::ALL {
            assert!(!loc.code().is_empty());
            assert!(!loc.label().is_empty());
        }
    }

    #[test]
    fn wifi_stub_fields() {
        let net = WifiNetwork::stub("LumoNet", 85, true);
        assert_eq!(net.ssid, "LumoNet");
        assert_eq!(net.signal, 85);
        assert!(net.secured);
        assert!(!net.connected);
    }

    #[test]
    fn mark_done_creates_flag() {
        let dir = tempdir().unwrap();
        let flag = dir.path().join("subdir").join("first-run-done");
        mark_first_run_done(flag.to_str().unwrap()).unwrap();
        assert!(flag.exists());
    }

    #[test]
    fn locale_write_read() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("locale.toml");
        let cfg = LocaleConfig::new("en_US");
        cfg.write_to(&path).unwrap();
        let loaded = LocaleConfig::read_from(&path).unwrap();
        assert_eq!(loaded.locale, "en_US");
    }
}
