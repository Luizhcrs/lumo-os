//! math.rs - detecta e avalia expressoes matematicas via meval.

pub fn try_eval(query: &str) -> Option<String> {
    let has_op = query.contains('+') || query.contains('-') || query.contains('*')
        || query.contains('/') || query.contains('^')
        || query.to_lowercase().contains("sqrt") || query.to_lowercase().contains("sin")
        || query.to_lowercase().contains("cos") || query.to_lowercase().contains("log");
    if !has_op { return None; }
    match meval::eval_str(query) {
        Ok(val) => {
            if val.fract() == 0.0 && val.abs() < 1e15 {
                Some(format!("{}", val as i64))
            } else {
                Some(format!("{val:.6}").trim_end_matches('0').trim_end_matches('.').to_string())
            }
        }
        Err(_) => None,
    }
}
