//! sanitize.rs — F1.5-B1 review fixes:
//!   - H2: clamp strings (summary/body/app_name) pra evitar DoS
//!   - H3: markup_escape pra evitar Pango/HTML injection no body
//!   - Strip control chars (exceto \n e \t) pra evitar terminal escapes / ANSI

/// Trunca string em chars (nao bytes). Retorna string nova; cap = max_chars.
pub fn clamp(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max_chars).collect();
    truncated
}

/// Escape Pango markup chars: & < > ' "
/// Mesma logica do glib::markup_escape_text mas sem deps Linux.
pub fn markup_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\'' => out.push_str("&apos;"),
            '"' => out.push_str("&quot;"),
            // Strip control chars (0x00-0x1f) exceto \n (0x0a) e \t (0x09).
            c if (c as u32) < 0x20 && c != '\n' && c != '\t' => {}
            // DEL e ISO controls.
            c if (c as u32) == 0x7f => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_short_string_unchanged() {
        assert_eq!(clamp("hi", 10), "hi");
    }

    #[test]
    fn clamp_exact_length() {
        assert_eq!(clamp("12345", 5), "12345");
    }

    #[test]
    fn clamp_truncates_excess() {
        assert_eq!(clamp("1234567890", 5), "12345");
    }

    #[test]
    fn clamp_respects_chars_not_bytes() {
        // 5 chars (utf-8 multibyte) cabem em max=5 mesmo com bytes >5.
        let s = "ñáéíó";
        assert_eq!(s.chars().count(), 5);
        assert!(s.len() > 5);
        assert_eq!(clamp(s, 5), s);
    }

    #[test]
    fn clamp_truncates_multibyte() {
        let s = "ñáéíó";
        let c = clamp(s, 2);
        assert_eq!(c.chars().count(), 2);
    }

    #[test]
    fn markup_escape_ampersand() {
        assert_eq!(markup_escape("A & B"), "A &amp; B");
    }

    #[test]
    fn markup_escape_lt_gt() {
        assert_eq!(markup_escape("<b>"), "&lt;b&gt;");
    }

    #[test]
    fn markup_escape_quotes() {
        assert_eq!(markup_escape("'x'"), "&apos;x&apos;");
        assert_eq!(markup_escape("\"y\""), "&quot;y&quot;");
    }

    #[test]
    fn markup_escape_preserves_newlines_tabs() {
        assert_eq!(markup_escape("a\nb\tc"), "a\nb\tc");
    }

    #[test]
    fn markup_escape_strips_control_chars() {
        // ANSI escape (ESC = 0x1b) + bell (0x07) sumir.
        let s = "a\x1b[31mRED\x07b";
        let out = markup_escape(s);
        assert!(!out.contains('\x1b'));
        assert!(!out.contains('\x07'));
        assert!(out.contains("a"));
        assert!(out.contains("RED"));
    }

    #[test]
    fn markup_escape_strips_del_char() {
        let s = "a\x7fb";
        let out = markup_escape(s);
        assert_eq!(out, "ab");
    }

    #[test]
    fn markup_escape_pango_injection_neutralized() {
        let attack = "<span foreground=\"red\">FAKE CRITICAL</span>";
        let out = markup_escape(attack);
        assert!(!out.contains("<span"));
        assert!(!out.contains("</span>"));
        assert!(out.contains("&lt;span"));
    }

    #[test]
    fn markup_escape_empty_string() {
        assert_eq!(markup_escape(""), "");
    }

    #[test]
    fn markup_escape_no_change_when_safe() {
        let s = "hello world 123";
        assert_eq!(markup_escape(s), s);
    }
}
