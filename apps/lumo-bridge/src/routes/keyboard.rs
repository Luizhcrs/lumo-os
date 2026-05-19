//! Keyboard routes:
//!  POST /keyboard/type {text}        -- texto literal, typewriter via SyntheticKeyCombo char-a-char
//!                                        com fallback wtype/ydotool quando IPC indisponivel.
//!  POST /keyboard/key  {sequence}    -- ex: "ctrl+alt+t", "super", "return", "f5"
//!                                        -> SyntheticKeyCombo (compositor faz press-all/release-reverse).
//!
//! SI.1: pivot ydotool -> IPC sintetico. Codigos evdev KEY_* sao consumidos
//! pelo compositor que converte internamente para xkb Keycode (+8).

use axum::{http::StatusCode, response::Json};
use lumo_ipc::LumoCommand;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::exec;
use crate::lumo_ipc::{send_command_async, ydotool_fallback_enabled};

#[derive(Deserialize)]
pub struct TypeText {
    pub text: String,
}

#[derive(Deserialize)]
pub struct KeySeq {
    pub sequence: String,
}

/// Mapeia nome de tecla (lowercase) -> KEY_* code de linux/input-event-codes.h.
pub fn keysym(name: &str) -> Option<u16> {
    let n = name.to_ascii_lowercase();
    Some(match n.as_str() {
        // Modifiers
        "ctrl" | "control" | "lctrl" | "leftctrl" => 29,
        "rctrl" | "rightctrl" => 97,
        "shift" | "lshift" | "leftshift" => 42,
        "rshift" | "rightshift" => 54,
        "alt" | "lalt" | "leftalt" => 56,
        "ralt" | "rightalt" | "altgr" => 100,
        "super" | "meta" | "lsuper" | "win" | "leftmeta" => 125,
        "rsuper" | "rightmeta" => 126,
        // Whitespace/control
        "escape" | "esc" => 1,
        "tab" => 15,
        "return" | "enter" => 28,
        "space" => 57,
        "backspace" | "bksp" => 14,
        "delete" | "del" => 111,
        "insert" | "ins" => 110,
        "home" => 102,
        "end" => 107,
        "pageup" | "pgup" => 104,
        "pagedown" | "pgdn" => 109,
        "capslock" | "caps" => 58,
        // Arrows
        "left" => 105,
        "right" => 106,
        "up" => 103,
        "down" => 108,
        // Symbols
        "minus" | "-" => 12,
        "equal" | "=" => 13,
        "leftbrace" | "[" => 26,
        "rightbrace" | "]" => 27,
        "semicolon" | ";" => 39,
        "apostrophe" | "'" => 40,
        "grave" | "`" => 41,
        "backslash" | "\\" => 43,
        "comma" | "," => 51,
        "dot" | "period" | "." => 52,
        "slash" | "/" => 53,
        // Numbers (top row)
        "0" => 11,
        "1" => 2,
        "2" => 3,
        "3" => 4,
        "4" => 5,
        "5" => 6,
        "6" => 7,
        "7" => 8,
        "8" => 9,
        "9" => 10,
        // Letters
        "a" => 30, "b" => 48, "c" => 46, "d" => 32, "e" => 18,
        "f" => 33, "g" => 34, "h" => 35, "i" => 23, "j" => 36,
        "k" => 37, "l" => 38, "m" => 50, "n" => 49, "o" => 24,
        "p" => 25, "q" => 16, "r" => 19, "s" => 31, "t" => 20,
        "u" => 22, "v" => 47, "w" => 17, "x" => 45, "y" => 21,
        "z" => 44,
        // Function
        "f1" => 59, "f2" => 60, "f3" => 61, "f4" => 62,
        "f5" => 63, "f6" => 64, "f7" => 65, "f8" => 66,
        "f9" => 67, "f10" => 68, "f11" => 87, "f12" => 88,
        _ => return None,
    })
}

/// Parse "ctrl+alt+t" -> Vec<u16> de keycodes (modifiers primeiro, key final).
pub fn parse_sequence(seq: &str) -> Result<Vec<u16>, String> {
    let parts: Vec<&str> = seq.split('+').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return Err("empty sequence".into());
    }
    let mut out = Vec::with_capacity(parts.len());
    for p in parts {
        let code = keysym(p).ok_or_else(|| format!("unknown key: {}", p))?;
        out.push(code);
    }
    Ok(out)
}

/// SI.1: roundtrip via ydotool key (fallback). Args: down todos, up reverso.
pub fn build_ydotool_key_args(codes: &[u16]) -> Vec<String> {
    let mut args: Vec<String> = Vec::with_capacity(codes.len() * 2);
    for c in codes {
        args.push(format!("{}:1", c));
    }
    for c in codes.iter().rev() {
        args.push(format!("{}:0", c));
    }
    args
}

/// Converte um char ASCII em (evdev_codes em ordem). Modifiers + key.
/// Suporta minusculas direto e MAIUSCULAS via Shift. Outros pulam.
fn char_to_codes(c: char) -> Option<Vec<u32>> {
    if c.is_ascii_lowercase() {
        keysym(&c.to_string()).map(|k| vec![k as u32])
    } else if c.is_ascii_uppercase() {
        let lo = c.to_ascii_lowercase().to_string();
        keysym(&lo).map(|k| vec![42u32, k as u32]) // shift + key
    } else if c == ' ' {
        Some(vec![57])
    } else if c == '\n' {
        Some(vec![28])
    } else if c == '\t' {
        Some(vec![15])
    } else if c.is_ascii_digit() {
        keysym(&c.to_string()).map(|k| vec![k as u32])
    } else {
        // Pontuacao simples sem shift.
        let s = c.to_string();
        keysym(&s).map(|k| vec![k as u32])
    }
}

pub async fn type_text(Json(p): Json<TypeText>) -> Result<Json<Value>, (StatusCode, String)> {
    // SI.1: tenta IPC char-a-char. Erro -> fallback ydotool/wtype quando habilitado.
    let mut emitted = 0usize;
    for ch in p.text.chars() {
        let Some(codes) = char_to_codes(ch) else {
            tracing::debug!(ch = ?ch, "SI.1: char nao mapeado, pulado");
            continue;
        };
        let codes_u32: Vec<u32> = codes.iter().copied().collect();
        match send_command_async(LumoCommand::SyntheticKeyCombo { keys: codes_u32 }).await {
            Ok(()) => emitted += 1,
            Err(ipc_err) => {
                if ydotool_fallback_enabled() {
                    // Tenta wtype direto pra resto do texto + abandona loop.
                    let _ = wtype_fallback(&p.text).await;
                    return Ok(Json(
                        json!({"ok": true, "len": p.text.chars().count(), "via": "wtype"}),
                    ));
                }
                return Err((StatusCode::SERVICE_UNAVAILABLE, ipc_err.to_string()));
            }
        }
    }
    Ok(Json(json!({"ok": true, "len": emitted, "via": "ipc"})))
}

async fn wtype_fallback(text: &str) -> Result<(), String> {
    let out = exec::run("/usr/bin/wtype", &[text]).await.map_err(|e| e.to_string())?;
    if out.status != 0 {
        let out2 = exec::run("/usr/bin/ydotool", &["type", "--", text])
            .await
            .map_err(|e| e.to_string())?;
        if out2.status != 0 {
            return Err(format!(
                "wtype exit {}, ydotool exit {}",
                out.status, out2.status
            ));
        }
    }
    Ok(())
}

pub async fn key_sequence(Json(p): Json<KeySeq>) -> Result<Json<Value>, (StatusCode, String)> {
    let codes = parse_sequence(&p.sequence).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let codes_u32: Vec<u32> = codes.iter().map(|c| *c as u32).collect();
    match send_command_async(LumoCommand::SyntheticKeyCombo { keys: codes_u32.clone() }).await {
        Ok(()) => Ok(Json(json!({
            "ok": true, "sequence": p.sequence, "codes": codes, "via": "ipc"
        }))),
        Err(ipc_err) => {
            if ydotool_fallback_enabled() {
                tracing::warn!(err = %ipc_err, "SI.1: IPC falhou, fallback ydotool key");
                let arg_strings = build_ydotool_key_args(&codes);
                let mut args: Vec<&str> = Vec::with_capacity(arg_strings.len() + 1);
                args.push("key");
                for a in &arg_strings {
                    args.push(a.as_str());
                }
                let out = exec::run("/usr/bin/ydotool", &args)
                    .await
                    .map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, e.to_string()))?;
                if out.status != 0 {
                    return Err((
                        StatusCode::SERVICE_UNAVAILABLE,
                        format!(
                            "ydotool key exit {}: {}",
                            out.status,
                            String::from_utf8_lossy(&out.stderr)
                        ),
                    ));
                }
                Ok(Json(json!({
                    "ok": true, "sequence": p.sequence, "codes": codes, "via": "ydotool"
                })))
            } else {
                Err((StatusCode::SERVICE_UNAVAILABLE, ipc_err.to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_modifier_combo() {
        let codes = parse_sequence("ctrl+alt+t").unwrap();
        assert_eq!(codes, vec![29, 56, 20]);
    }

    #[test]
    fn parse_single_key() {
        assert_eq!(parse_sequence("return").unwrap(), vec![28]);
        assert_eq!(parse_sequence("F5").unwrap(), vec![63]);
        assert_eq!(parse_sequence("super").unwrap(), vec![125]);
    }

    #[test]
    fn parse_unknown_errors() {
        assert!(parse_sequence("ctrl+zzzz").is_err());
        assert!(parse_sequence("").is_err());
    }

    #[test]
    fn ydotool_args_down_then_up_reversed() {
        let args = build_ydotool_key_args(&[29, 56, 20]);
        assert_eq!(args, vec!["29:1", "56:1", "20:1", "20:0", "56:0", "29:0"]);
    }

    /// SI.1: char_to_codes mapeia lowercase, uppercase (com shift) e space.
    #[test]
    fn char_to_codes_basic() {
        assert_eq!(char_to_codes('a'), Some(vec![30]));
        assert_eq!(char_to_codes('A'), Some(vec![42, 30]));
        assert_eq!(char_to_codes(' '), Some(vec![57]));
        assert_eq!(char_to_codes('\n'), Some(vec![28]));
        assert_eq!(char_to_codes('5'), Some(vec![6]));
    }
}
