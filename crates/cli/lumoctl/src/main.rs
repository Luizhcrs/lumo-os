//! lumoctl - CLI de controle do Lumo OS.
//!
//! Comandos disponiveis:
//!   lumoctl theme set --accent #FF6B35
//!   lumoctl theme mode dark
//!   lumoctl theme reset
//!
//! Escreve ~/.config/lumo/theme.toml e envia LumoCommand::ReloadTheme
//! via socket unix (XDG_RUNTIME_DIR/lumo-wm.sock).

use std::io::Write;
use std::os::unix::net::UnixStream;

use lumo_foundation::{LumoTheme, LumoTokens};
use lumo_ipc::{default_socket_path, LumoCommand};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("uso: lumoctl <subcomando> [args]");
        eprintln!("  theme set --accent #RRGGBB");
        eprintln!("  theme mode light|dark");
        eprintln!("  theme reset");
        std::process::exit(1);
    }

    match args[0].as_str() {
        "theme" => cmd_theme(&args[1..]),
        "layout" => cmd_layout(&args[1..]),
        other => {
            eprintln!("subcomando desconhecido: {other}");
            std::process::exit(1);
        }
    }
}

fn cmd_theme(args: &[String]) {
    if args.is_empty() {
        eprintln!("uso: lumoctl theme <set|mode|reset> [args]");
        std::process::exit(1);
    }

    let mut tokens = LumoTokens::load_from_disk();

    match args[0].as_str() {
        "set" => {
            // lumoctl theme set --accent #FF6B35
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--accent" => {
                        i += 1;
                        if i >= args.len() {
                            eprintln!("--accent requer um valor hex");
                            std::process::exit(1);
                        }
                        let hex = args[i].trim_start_matches('#');
                        match u32::from_str_radix(hex, 16) {
                            Ok(v) if hex.len() == 6 => tokens.accent = Some(v),
                            _ => {
                                eprintln!("valor invalido para --accent: {}", args[i]);
                                std::process::exit(1);
                            }
                        }
                    }
                    "--ink-deep" | "--ink_deep" => {
                        i += 1;
                        if i >= args.len() {
                            eprintln!("--ink-deep requer um valor hex");
                            std::process::exit(1);
                        }
                        let hex = args[i].trim_start_matches('#');
                        match u32::from_str_radix(hex, 16) {
                            Ok(v) if hex.len() == 6 => tokens.ink_deep = Some(v),
                            _ => {
                                eprintln!("valor invalido para --ink-deep: {}", args[i]);
                                std::process::exit(1);
                            }
                        }
                    }
                    "--pill-bg" | "--pill_bg" => {
                        i += 1;
                        if i >= args.len() {
                            eprintln!("--pill-bg requer um valor hex");
                            std::process::exit(1);
                        }
                        let hex = args[i].trim_start_matches('#');
                        match u32::from_str_radix(hex, 16) {
                            Ok(v) if hex.len() == 6 => tokens.pill_bg = Some(v),
                            _ => {
                                eprintln!("valor invalido para --pill-bg: {}", args[i]);
                                std::process::exit(1);
                            }
                        }
                    }
                    other => {
                        eprintln!("flag desconhecida: {other}");
                        std::process::exit(1);
                    }
                }
                i += 1;
            }
        }
        "mode" => {
            // lumoctl theme mode dark|light
            if args.len() < 2 {
                eprintln!("uso: lumoctl theme mode light|dark");
                std::process::exit(1);
            }
            tokens.mode = match args[1].as_str() {
                "dark" | "Dark" | "DARK" => LumoTheme::Dark,
                "light" | "Light" | "LIGHT" => LumoTheme::Light,
                other => {
                    eprintln!("modo invalido: {other} (esperado: light ou dark)");
                    std::process::exit(1);
                }
            };
        }
        "reset" => {
            // Remove overrides, mantem modo atual
            tokens.accent = None;
            tokens.ink_deep = None;
            tokens.pill_bg = None;
        }
        other => {
            eprintln!("subcomando theme desconhecido: {other}");
            std::process::exit(1);
        }
    }

    if let Err(e) = tokens.save_to_disk() {
        eprintln!("erro ao salvar theme.toml: {e}");
        std::process::exit(1);
    }
    eprintln!("[lumoctl] theme.toml salvo");

    send_reload_theme();
}

/// Envia LumoCommand::ReloadTheme via socket unix. Falha silenciosa
/// se compositor nao estiver rodando (standalone = so toml atualizado).
fn cmd_layout(args: &[String]) {
    if args.is_empty() || args[0] != "reload" {
        eprintln!("uso: lumoctl layout reload");
        std::process::exit(1);
    }
    // Garante que layout.toml existe (copia default se ausente).
    use lumo_foundation::BarLayout;
    let path = match BarLayout::config_path() {
        Some(p) => p,
        None => {
            eprintln!("[lumoctl] HOME nao definido");
            std::process::exit(1);
        }
    };
    if !path.exists() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let default_toml = include_str!("../../../../scripts/install/lumo-layout.default.toml");
        if let Err(e) = std::fs::write(&path, default_toml) {
            eprintln!("[lumoctl] erro ao criar layout.toml: {e}");
        } else {
            eprintln!("[lumoctl] layout.toml criado com valores padrao");
        }
    } else {
        eprintln!("[lumoctl] layout.toml existe, enviando reload");
    }
    send_reload_theme(); // reutiliza o mesmo canal IPC
}

fn send_reload_theme() {
    let Some(path) = default_socket_path() else {
        eprintln!("[lumoctl] XDG_RUNTIME_DIR ausente, nao enviou ReloadTheme");
        return;
    };
    let stream = match UnixStream::connect(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[lumoctl] compositor nao acessivel ({}): theme.toml salvo, reload na proxima sessao", e);
            return;
        }
    };
    let cmd = LumoCommand::ReloadTheme;
    let mut payload = match serde_json::to_string(&cmd) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[lumoctl] serialize erro: {e}");
            return;
        }
    };
    payload.push('\n');
    let mut s = stream;
    if let Err(e) = s.write_all(payload.as_bytes()) {
        eprintln!("[lumoctl] write erro: {e}");
    } else {
        eprintln!("[lumoctl] ReloadTheme enviado ao compositor");
    }
}
