//! Parser CSS subset usando cssparser.

use cssparser::{Parser, ParserInput};
use crate::model::{Stylesheet, Rule, Selector, PropertyValue, parse_color_literal, parse_px_literal};

pub fn parse(src: &str) -> Result<Stylesheet, String> {
    let mut input = ParserInput::new(src);
    let mut parser = Parser::new(&mut input);
    let mut sheet = Stylesheet::default();

    // Manually consume top-level rules. Each rule = selector { decl-list } OR :root.
    loop {
        parser.skip_whitespace();
        if parser.is_exhausted() { break; }
        // Parse until "{".
        let start = parser.position();
        let mut prelude_tokens: Vec<String> = Vec::new();
        loop {
            if parser.is_exhausted() { return Err("EOF antes de `{`".into()); }
            let tok_pos = parser.position();
            match parser.next_including_whitespace() {
                Ok(t) => {
                    if matches!(t, cssparser::Token::CurlyBracketBlock) { break; }
                    let s = parser.slice_from(tok_pos).to_string();
                    prelude_tokens.push(s);
                }
                Err(e) => return Err(format!("prelude erro: {:?}", e)),
            }
        }
        // Re-parse selector from prelude string.
        let prelude_str: String = prelude_tokens.concat();
        let selector_str = prelude_str.trim();
        // {} block reads via parse_nested_block.
        let result = parser.parse_nested_block(|inner| -> Result<(), cssparser::ParseError<'_, ()>> {
            if selector_str == ":root" {
                parse_root_decls(inner, &mut sheet)?;
            } else {
                let sel = match parse_selector(selector_str) {
                    Some(s) => s,
                    None => return Ok(()),
                };
                let props = parse_decls(inner)?;
                sheet.rules.push(Rule { selector: sel, props });
            }
            Ok(())
        });
        if let Err(e) = result {
            return Err(format!("block @ {:?}: {:?}", start, e));
        }
    }
    Ok(sheet)
}

fn parse_selector(s: &str) -> Option<Selector> {
    let s = s.trim();
    // Multi-class only: ".pill.lumo" or ".pill"
    if !s.starts_with('.') { return None; }
    let classes: Vec<String> = s.split('.')
        .filter(|c| !c.is_empty())
        .map(|c| c.trim().to_string())
        .collect();
    if classes.is_empty() { return None; }
    Some(Selector { classes })
}

fn parse_root_decls<'i>(
    parser: &mut Parser<'i, '_>,
    sheet: &mut Stylesheet,
) -> Result<(), cssparser::ParseError<'i, ()>> {
    loop {
        parser.skip_whitespace();
        if parser.is_exhausted() { break; }
        // Read token; expect Ident OR Delim('-') (cssparser pode split `--name`).
        let state = parser.state();
        let tok = match parser.next_including_whitespace_and_comments() {
            Ok(t) => t.clone(),
            Err(_) => break,
        };
        let name: String = match &tok {
            cssparser::Token::Ident(s) => s.to_string(),
            _ => {
                // Re-parse as raw text until `:`. CustomProperty `--foo` lex
                // differs by cssparser version.
                parser.reset(&state);
                let start = parser.position();
                while !parser.is_exhausted() {
                    let p = parser.state();
                    match parser.next_including_whitespace() {
                        Ok(cssparser::Token::Colon) => { parser.reset(&p); break; }
                        Ok(cssparser::Token::Semicolon) => { parser.reset(&p); break; }
                        Ok(_) => continue,
                        Err(_) => break,
                    }
                }
                parser.slice_from(start).trim().to_string()
            }
        };
        if !name.starts_with("--") {
            let _ = consume_until_semi(parser);
            continue;
        }
        if parser.expect_colon().is_err() { break; }
        let value = read_value_text(parser);
        let _ = parser.expect_semicolon();
        sheet.vars.insert(name.trim_start_matches("--").to_string(), value);
    }
    Ok(())
}

fn parse_decls<'i>(
    parser: &mut Parser<'i, '_>,
) -> Result<Vec<(String, PropertyValue)>, cssparser::ParseError<'i, ()>> {
    let mut out = Vec::new();
    loop {
        parser.skip_whitespace();
        if parser.is_exhausted() { break; }
        let name = match parser.expect_ident_cloned() {
            Ok(n) => n.to_string(),
            Err(_) => break,
        };
        if parser.expect_colon().is_err() { break; }
        let value = read_value_text(parser);
        let _ = parser.expect_semicolon();
        let parsed = classify_value(&value);
        out.push((name, parsed));
    }
    Ok(out)
}

fn read_value_text<'i>(parser: &mut Parser<'i, '_>) -> String {
    let start = parser.position();
    loop {
        if parser.is_exhausted() { break; }
        let state = parser.state();
        match parser.next_including_whitespace() {
            Ok(cssparser::Token::Semicolon) => {
                parser.reset(&state);
                break;
            }
            Ok(cssparser::Token::CloseCurlyBracket) => {
                parser.reset(&state);
                break;
            }
            Ok(cssparser::Token::Function(_)) | Ok(cssparser::Token::ParenthesisBlock) => {
                // Consume nested block (e.g. var(--pad), rgba(...)).
                let _ = parser.parse_nested_block(|_p: &mut Parser<'_, '_>| -> Result<(), cssparser::ParseError<'_, ()>> {
                    // Consume tudo dentro do block.
                    while !_p.is_exhausted() {
                        if _p.next_including_whitespace().is_err() { break; }
                    }
                    Ok(())
                });
                continue;
            }
            Ok(_) => continue,
            Err(_) => break,
        }
    }
    parser.slice_from(start).trim().to_string()
}

fn consume_until_semi<'i>(parser: &mut Parser<'i, '_>) -> Result<(), cssparser::ParseError<'i, ()>> {
    loop {
        if parser.is_exhausted() { return Ok(()); }
        let tok = parser.next();
        match tok {
            Ok(cssparser::Token::Semicolon) => return Ok(()),
            Ok(_) => continue,
            Err(_) => return Ok(()),
        }
    }
}

fn classify_value(raw: &str) -> PropertyValue {
    let s = raw.trim();
    // var(--name)
    if let Some(name) = s.strip_prefix("var(--").and_then(|x| x.strip_suffix(")")) {
        return PropertyValue::Var(name.trim().to_string());
    }
    if let Some(c) = parse_color_literal(s) {
        return PropertyValue::Color(c);
    }
    if let Some(p) = parse_px_literal(s) {
        return PropertyValue::Px(p);
    }
    PropertyValue::Str(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_root_vars() {
        let sheet = parse(":root { --pill-h: 28px; --accent: #00C896; }").unwrap();
        assert_eq!(sheet.vars.get("pill-h").map(|s| s.as_str()), Some("28px"));
        assert_eq!(sheet.vars.get("accent").map(|s| s.as_str()), Some("#00C896"));
    }

    #[test]
    fn parses_class_rule() {
        let sheet = parse(".pill { height: 28px; background: #1A1A1C; }").unwrap();
        assert_eq!(sheet.rules.len(), 1);
        assert_eq!(sheet.rules[0].selector.classes, vec!["pill"]);
        let h = sheet.get_px(&["pill"], "height");
        assert_eq!(h, Some(28.0));
        let bg = sheet.get_color(&["pill"], "background");
        assert_eq!(bg, Some(0x1A1A1CFF));
    }

    #[test]
    fn cascade_specificity() {
        let css = ".pill { height: 28px; } .pill.lumo { height: 32px; }";
        let sheet = parse(css).unwrap();
        assert_eq!(sheet.get_px(&["pill"], "height"), Some(28.0));
        assert_eq!(sheet.get_px(&["pill", "lumo"], "height"), Some(32.0));
    }

    #[test]
    fn var_resolution() {
        let css = ":root { --pad: 14px; } .pill { padding: var(--pad); }";
        let sheet = parse(css).unwrap();
        assert_eq!(sheet.get_px(&["pill"], "padding"), Some(14.0));
    }
}
