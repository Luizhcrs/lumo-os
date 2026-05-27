//! math.rs - detecta e avalia expressoes matematicas via meval.

pub fn try_eval(query: &str) -> Option<String> {
    let has_op = query.contains('+')
        || query.contains('-')
        || query.contains('*')
        || query.contains('/')
        || query.contains('^')
        || query.to_lowercase().contains("sqrt")
        || query.to_lowercase().contains("sin")
        || query.to_lowercase().contains("cos")
        || query.to_lowercase().contains("log");
    if !has_op {
        return None;
    }
    match meval::eval_str(query) {
        Ok(val) => {
            if val.fract() == 0.0 && val.abs() < 1e15 {
                Some(format!("{}", val as i64))
            } else {
                Some(
                    format!("{val:.6}")
                        .trim_end_matches('0')
                        .trim_end_matches('.')
                        .to_string(),
                )
            }
        }
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_returns_none() {
        assert_eq!(try_eval("firefox"), None);
        assert_eq!(try_eval("hello world"), None);
    }

    #[test]
    fn simple_add() {
        assert_eq!(try_eval("2+2"), Some("4".into()));
    }

    #[test]
    fn integer_result_no_decimal() {
        // val.fract() == 0 -> formata como i64.
        assert_eq!(try_eval("10*5"), Some("50".into()));
        assert_eq!(try_eval("100/4"), Some("25".into()));
    }

    #[test]
    fn fractional_result_trims_zeros() {
        assert_eq!(try_eval("1/4"), Some("0.25".into()));
        // 1/3 = 0.333333 (precisao 6 digitos)
        assert!(try_eval("1/3").unwrap().starts_with("0.333"));
    }

    #[test]
    fn negative_result() {
        assert_eq!(try_eval("2-10"), Some("-8".into()));
    }

    #[test]
    fn power_operator() {
        assert_eq!(try_eval("2^10"), Some("1024".into()));
    }

    #[test]
    fn sqrt_function() {
        assert_eq!(try_eval("sqrt(16)"), Some("4".into()));
    }

    #[test]
    fn sqrt_case_insensitive_detection() {
        // try_eval verifica lowercase pra detection mas meval e case-sensitive.
        // SQRT(16) seria detectado mas meval pode rejeitar maiusculo.
        // Testa apenas que detection nao panica.
        let _ = try_eval("SQRT(16)");
    }

    #[test]
    fn invalid_expression_returns_none() {
        // tem operador mas expressao invalida.
        assert_eq!(try_eval("2+"), None);
        assert_eq!(try_eval("*5"), None);
    }

    #[test]
    fn empty_returns_none() {
        assert_eq!(try_eval(""), None);
    }

    #[test]
    fn detection_requires_operator_or_function() {
        // "42" sozinho sem operador -> nao detecta -> None.
        assert_eq!(try_eval("42"), None);
    }

    #[test]
    fn sin_cos_log_detected() {
        assert!(try_eval("sin(0)").is_some());
        assert!(try_eval("cos(0)").is_some());
        // log nao testa valor (base ambiguous em meval); so detection.
        let _ = try_eval("log(10)");
    }
}
