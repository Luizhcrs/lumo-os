//! Keyboard routes:
//!  POST /keyboard/type {text}        -- wtype <text>, fallback ydotool type --
//!  POST /keyboard/key  {sequence}    -- ex: "ctrl+alt+t", "super", "return", "f5"
//!
//! Mapeamento de keysym -> linux/input-event-codes.h (KEY_*) feito em memoria.
//! ydotool key usa raw keycodes (28:1 28:0).

use axum::{http::StatusCode, response::Json};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::exec;

#[derive(Deserialize)]
pub struct TypeText {
    pub text: String,
}

#[derive(Deserialize)]
pub struct KeySeq {
    pub sequence: String,
}

/// Mapeia nome de tecla (lowercase) -> KEY_* code de linux/input-event-codes.h.
fn keysym(name: &str) -> Option<u16> {
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

/// Constroi args ydotool key: down todos, depois up reverso.
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

pub async fn type_text(Json(p): Json<TypeText>) -> Result<Json<Value>, (StatusCode, String)> {
    // wtype passa texto literal -- arg slice, sem shell.
    let out = exec::run("/usr/bin/wtype", &[p.text.as_str()])
        .await
        .map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, e.to_string()))?;
    if out.status != 0 {
        // Fallback: ydotool type -- <text>
        let out2 = exec::run("/usr/bin/ydotool", &["type", "--", p.text.as_str()])
            .await
            .map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, e.to_string()))?;
        if out2.status != 0 {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                format!(
                    "wtype exit {}, ydotool exit {}: {}",
                    out.status,
                    out2.status,
                    String::from_utf8_lossy(&out2.stderr)
                ),
            ));
        }
    }
    Ok(Json(json!({"ok": true, "len": p.text.chars().count()})))
}

pub async fn key_sequence(Json(p): Json<KeySeq>) -> Result<Json<Value>, (StatusCode, String)> {
    let codes = parse_sequence(&p.sequence).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
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
            format!("ydotool key exit {}: {}", out.status, String::from_utf8_lossy(&out.stderr)),
        ));
    }
    Ok(Json(json!({"ok": true, "sequence": p.sequence, "codes": codes})))
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
}
