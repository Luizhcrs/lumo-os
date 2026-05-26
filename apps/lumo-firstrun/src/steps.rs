//! steps.rs -- enum de etapas e estado de cada tela do wizard.

use serde::{Deserialize, Serialize};

/// Telas do wizard em ordem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Welcome,
    Language,
    Account,
    Wifi,
    Done,
}

impl Step {
    /// Total de telas (exclui Done como pagina separada).
    pub const COUNT: usize = 4;

    pub fn index(self) -> usize {
        match self {
            Step::Welcome => 0,
            Step::Language => 1,
            Step::Account => 2,
            Step::Wifi => 3,
            Step::Done => 4,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Step::Welcome => Step::Language,
            Step::Language => Step::Account,
            Step::Account => Step::Wifi,
            Step::Wifi => Step::Done,
            Step::Done => Step::Done,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Step::Welcome => "Bem-vindo",
            Step::Language => "Idioma",
            Step::Account => "Conta",
            Step::Wifi => "Wi-Fi",
            Step::Done => "Pronto",
        }
    }
}

/// Locale disponivel para selecao.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Locale {
    PtBr,
    EnUs,
}

impl Locale {
    pub const ALL: &'static [Locale] = &[Locale::PtBr, Locale::EnUs];

    pub fn label(self) -> &'static str {
        match self {
            Locale::PtBr => "Portugues (Brasil)",
            Locale::EnUs => "English (US)",
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Locale::PtBr => "pt_BR",
            Locale::EnUs => "en_US",
        }
    }
}

/// Rede Wi-Fi descoberta via nmcli.
#[derive(Debug, Clone)]
pub struct WifiNetwork {
    pub ssid: String,
    pub signal: u8,
    pub secured: bool,
    pub connected: bool,
}

impl WifiNetwork {
    /// Cria instancia de teste (nao requer nmcli).
    pub fn stub(ssid: impl Into<String>, signal: u8, secured: bool) -> Self {
        WifiNetwork {
            ssid: ssid.into(),
            signal,
            secured,
            connected: false,
        }
    }
}

/// Estado da tela de conta.
#[derive(Debug, Clone, Default)]
pub struct AccountState {
    pub username: String,
    pub password: String,
    pub password_confirm: String,
    pub error: Option<String>,
}

impl AccountState {
    /// Valida os campos. Retorna Err com mensagem de erro se invalido.
    pub fn validate(&self) -> Result<(), String> {
        if self.username.trim().is_empty() {
            return Err("Nome de usuario nao pode ser vazio.".into());
        }
        if self.username.len() < 3 {
            return Err("Nome de usuario precisa ter pelo menos 3 caracteres.".into());
        }
        if !self
            .username
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err("Nome de usuario: apenas letras, numeros, _ e -.".into());
        }
        if self.password.len() < 6 {
            return Err("Senha precisa ter pelo menos 6 caracteres.".into());
        }
        if self.password != self.password_confirm {
            return Err("Senhas nao conferem.".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn step_order() {
        assert_eq!(Step::Welcome.next(), Step::Language);
        assert_eq!(Step::Language.next(), Step::Account);
        assert_eq!(Step::Account.next(), Step::Wifi);
        assert_eq!(Step::Wifi.next(), Step::Done);
        assert_eq!(Step::Done.next(), Step::Done); // sem avanco alem do fim
    }

    #[test]
    fn step_index_monotonic() {
        assert!(Step::Welcome.index() < Step::Language.index());
        assert!(Step::Language.index() < Step::Account.index());
        assert!(Step::Account.index() < Step::Wifi.index());
        assert!(Step::Wifi.index() < Step::Done.index());
    }

    #[test]
    fn account_validate_ok() {
        let s = AccountState {
            username: "luiz".into(),
            password: "secret123".into(),
            password_confirm: "secret123".into(),
            error: None,
        };
        assert!(s.validate().is_ok());
    }

    #[test]
    fn account_validate_short_username() {
        let s = AccountState {
            username: "lu".into(),
            password: "secret123".into(),
            password_confirm: "secret123".into(),
            error: None,
        };
        assert!(s.validate().is_err());
    }

    #[test]
    fn account_validate_password_mismatch() {
        let s = AccountState {
            username: "luiz".into(),
            password: "abc123".into(),
            password_confirm: "abc124".into(),
            error: None,
        };
        let err = s.validate().unwrap_err();
        assert!(err.contains("conferem"));
    }

    #[test]
    fn account_validate_invalid_chars() {
        let s = AccountState {
            username: "luiz rs".into(),
            password: "abc123".into(),
            password_confirm: "abc123".into(),
            error: None,
        };
        assert!(s.validate().is_err());
    }

    #[test]
    fn locale_code() {
        assert_eq!(Locale::PtBr.code(), "pt_BR");
        assert_eq!(Locale::EnUs.code(), "en_US");
    }
}
