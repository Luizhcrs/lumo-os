//! lumo-ipc - tipos e helpers do canal de IPC do Lumo OS.
//!
//! Canal: socket unix em `$XDG_RUNTIME_DIR/lumo-wm.sock`.
//! Protocolo: linhas JSON ('\n' terminator). Justificativa do
//! line-delimited JSON em vez de length-prefixed/CBOR:
//! - debug trivial (tail / nc -U / socat)
//! - sem framing custom = menos bug nos primeiros dias
//! - throughput esperado < 100 msg/s (UI), zero pressao por bytes
//!
//! Memory feedback_design_lapidado: cada decisao com motivo.
//! Memory feedback_input_feedback_imediato: clients devem aplicar
//! eventos no proximo frame, drop quando lag > 100ms.

use serde::{Deserialize, Serialize};

pub const SOCKET_BASENAME: &str = "lumo-wm.sock";

/// Numero maximo de workspaces na MVP. Fixo em 5 por enquanto
/// (alinhado com lumo-bar que ja desenha 5 pills).
pub const MAX_WORKSPACES: u8 = 5;

/// Eventos emitidos pelo compositor (server) pros clientes.
/// Tag `type` em snake_case pra parse trivial via jq/grep.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LumoEvent {
    /// Estado completo de workspaces. Emitido no connect + a cada
    /// mudanca. Idempotente (vale o ultimo).
    Workspaces { active: u8, total: u8 },
    /// Pedido pra fechar dropdowns ativos em clients (ex: lumo-bar
    /// fecha dropdown de bateria). Emitido pelo compositor quando
    /// click esquerdo no desktop (lumo-desktop) em area vazia.
    /// A21: clientes que nao tem dropdown ignoram silenciosamente.
    CloseDropdowns,
    /// A26: pedido pra fechar menu contextual do lumo-desktop. Emitido
    /// pelo compositor quando bar abre dropdown (mutex: so um popup
    /// aberto na tela por vez). Clients sem menu ativo ignoram.
    CloseDesktopMenu,
    /// A40: pedido pra abrir o item selecionado no desktop (equivalente
    /// a duplo-click). Emitido pelo compositor quando Return e pressionado
    /// sem toplevel ativo. lumo-desktop chama xdg-open no icone selecionado.
    DesktopOpenSelected,
}

/// Comandos enviados pelos clientes (lumo-bar, lumoctl, etc) ao
/// compositor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LumoCommand {
    /// Troca workspace ativo. `to` em 1..=MAX_WORKSPACES.
    /// Compositor valida e ignora fora do range.
    Switch { to: u8 },
    /// Pede pro compositor avisar todos clients pra fecharem seus
    /// dropdowns ativos. A21: enviado por lumo-desktop quando ha
    /// click esquerdo em area vazia da area de trabalho.
    /// Compositor traduz em broadcast LumoEvent::CloseDropdowns.
    CloseDropdowns,
    /// A26: pede pro compositor avisar lumo-desktop pra fechar seu menu
    /// contextual. Enviado por lumo-bar quando abre dropdown (mutex).
    /// Compositor traduz em broadcast LumoEvent::CloseDesktopMenu.
    CloseDesktopMenu,
}

/// Path padrao do socket. Falha se `XDG_RUNTIME_DIR` ausente.
pub fn default_socket_path() -> Option<std::path::PathBuf> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")?;
    let mut p = std::path::PathBuf::from(dir);
    p.push(SOCKET_BASENAME);
    Some(p)
}

/// Serializa um evento em uma linha JSON pronta pra enviar
/// (inclui '\n' final).
pub fn encode_event(ev: &LumoEvent) -> String {
    let mut s = serde_json::to_string(ev).expect("LumoEvent sempre serializa");
    s.push('\n');
    s
}

/// Tenta parsear uma linha em LumoCommand. Linhas em branco
/// retornam None (sem erro).
pub fn parse_command(line: &str) -> Option<Result<LumoCommand, serde_json::Error>> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(serde_json::from_str(trimmed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_roundtrip() {
        let ev = LumoEvent::Workspaces { active: 3, total: 5 };
        let line = encode_event(&ev);
        assert!(line.ends_with('\n'));
        let parsed: LumoEvent = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(parsed, ev);
    }

    #[test]
    fn switch_command_parses() {
        let raw = r#"{"type":"switch","to":2}"#;
        let cmd = parse_command(raw).unwrap().unwrap();
        assert_eq!(cmd, LumoCommand::Switch { to: 2 });
    }

    #[test]
    fn blank_line_is_skipped() {
        assert!(parse_command("").is_none());
        assert!(parse_command("   \n").is_none());
    }
}
