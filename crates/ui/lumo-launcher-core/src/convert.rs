//! convert.rs — unit conversion pro Spotlight Lumo.
//!
//! Suporta queries naturais: "50 miles to km", "100 f to c", "1 gb to mb".
//! Case insensitive. Aceita variacoes: "→", "in", "to", "para".
//!
//! Categorias:
//! - Comprimento: km, m, cm, mm, mi (miles), ft (feet), in (inch), yd (yards)
//! - Massa: kg, g, mg, lb (pounds), oz (ounces)
//! - Temperatura: c (celsius), f (fahrenheit), k (kelvin)
//! - Storage: tb, gb, mb, kb, b (bytes) — base 1024
//! - Tempo: s, min, h, d, w (week)
//!
//! Output: numero formatado com 4 casas decimais (trim trailing zeros).

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Category {
    Length,
    Mass,
    Temperature,
    Storage,
    Time,
}

/// Conversao normalizada: cada unit tem fator pra unidade base da categoria.
/// Base: m (length), g (mass), s (time), b (storage), c (temperature special).
fn unit_info(unit: &str) -> Option<(Category, f64)> {
    let u = unit.to_lowercase();
    let u = u.as_str();
    match u {
        // Length: base = metro
        "m" | "meter" | "meters" | "metro" | "metros" => Some((Category::Length, 1.0)),
        "km" | "kilometer" | "kilometers" | "quilometro" | "quilometros" => {
            Some((Category::Length, 1000.0))
        }
        "cm" | "centimeter" | "centimeters" => Some((Category::Length, 0.01)),
        "mm" | "millimeter" | "millimeters" => Some((Category::Length, 0.001)),
        "mi" | "mile" | "miles" => Some((Category::Length, 1609.344)),
        "ft" | "feet" | "foot" => Some((Category::Length, 0.3048)),
        "in" | "inch" | "inches" | "polegada" | "polegadas" => Some((Category::Length, 0.0254)),
        "yd" | "yard" | "yards" => Some((Category::Length, 0.9144)),
        // Mass: base = grama
        "g" | "gram" | "grams" | "grama" | "gramas" => Some((Category::Mass, 1.0)),
        "kg" | "kilogram" | "kilograms" | "quilo" | "quilos" => Some((Category::Mass, 1000.0)),
        "mg" | "milligram" | "milligrams" => Some((Category::Mass, 0.001)),
        "lb" | "lbs" | "pound" | "pounds" | "libra" | "libras" => Some((Category::Mass, 453.59237)),
        "oz" | "ounce" | "ounces" => Some((Category::Mass, 28.349523125)),
        // Storage: base = bytes
        "b" | "byte" | "bytes" => Some((Category::Storage, 1.0)),
        "kb" => Some((Category::Storage, 1024.0)),
        "mb" => Some((Category::Storage, 1024.0 * 1024.0)),
        "gb" => Some((Category::Storage, 1024.0 * 1024.0 * 1024.0)),
        "tb" => Some((Category::Storage, 1024.0 * 1024.0 * 1024.0 * 1024.0)),
        // Time: base = segundos
        "s" | "sec" | "second" | "seconds" | "seg" | "segundo" | "segundos" => {
            Some((Category::Time, 1.0))
        }
        "min" | "minute" | "minutes" | "minuto" | "minutos" => Some((Category::Time, 60.0)),
        "h" | "hr" | "hour" | "hours" | "hora" | "horas" => Some((Category::Time, 3600.0)),
        "d" | "day" | "days" | "dia" | "dias" => Some((Category::Time, 86400.0)),
        "w" | "week" | "weeks" | "semana" | "semanas" => Some((Category::Time, 604800.0)),
        // Temperature: nao tem fator simples; trata separado.
        "c" | "celsius" => Some((Category::Temperature, 0.0)),
        "f" | "fahrenheit" => Some((Category::Temperature, 1.0)),
        "k" | "kelvin" => Some((Category::Temperature, 2.0)),
        _ => None,
    }
}

fn convert_temperature(value: f64, from: &str, to: &str) -> Option<f64> {
    let from = from.to_lowercase();
    let to = to.to_lowercase();
    let to_c = |v: f64, u: &str| -> Option<f64> {
        match u {
            "c" | "celsius" => Some(v),
            "f" | "fahrenheit" => Some((v - 32.0) * 5.0 / 9.0),
            "k" | "kelvin" => Some(v - 273.15),
            _ => None,
        }
    };
    let from_c = |v: f64, u: &str| -> Option<f64> {
        match u {
            "c" | "celsius" => Some(v),
            "f" | "fahrenheit" => Some(v * 9.0 / 5.0 + 32.0),
            "k" | "kelvin" => Some(v + 273.15),
            _ => None,
        }
    };
    let c = to_c(value, &from)?;
    from_c(c, &to)
}

fn format_value(v: f64) -> String {
    if v.abs() < 1e-6 {
        return "0".to_string();
    }
    let rounded = (v * 10000.0).round() / 10000.0;
    let s = format!("{}", rounded);
    s
}

/// Parse "<num> <unit> [to|in|para|→] <unit>".
/// Retorna "<resultado> <unit_alvo>" formatado.
pub fn try_convert(query: &str) -> Option<String> {
    let q = query.trim().to_lowercase();
    // Substitui delimitadores por "to".
    let q = q
        .replace('→', " to ")
        .replace(" in ", " to ")
        .replace(" para ", " to ");
    let parts: Vec<&str> = q.split_whitespace().collect();
    if parts.len() < 4 {
        return None;
    }
    // [num, from_unit, to, to_unit] OU [num, from_unit, ..., to, to_unit]
    let value: f64 = parts[0].parse().ok()?;
    let from_unit = parts[1];
    let to_idx = parts.iter().position(|s| *s == "to")?;
    let to_unit = parts.get(to_idx + 1)?;
    let (from_cat, from_factor) = unit_info(from_unit)?;
    let (to_cat, to_factor) = unit_info(to_unit)?;
    if from_cat != to_cat {
        return None;
    }
    if from_cat == Category::Temperature {
        let result = convert_temperature(value, from_unit, to_unit)?;
        return Some(format!("{} {}", format_value(result), to_unit));
    }
    let base = value * from_factor;
    let result = base / to_factor;
    Some(format!("{} {}", format_value(result), to_unit))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn miles_to_km() {
        let r = try_convert("50 miles to km").expect("ok");
        // 50 * 1.609344 = 80.4672
        assert!(r.starts_with("80.4672"));
        assert!(r.contains("km"));
    }

    #[test]
    fn km_to_miles() {
        let r = try_convert("100 km to miles").expect("ok");
        assert!(r.starts_with("62.13"));
    }

    #[test]
    fn meters_to_centimeters() {
        let r = try_convert("2 m to cm").expect("ok");
        assert!(r.starts_with("200"));
    }

    #[test]
    fn fahrenheit_to_celsius() {
        let r = try_convert("100 f to c").expect("ok");
        // (100 - 32) * 5/9 = 37.7778
        assert!(r.starts_with("37.7778"));
    }

    #[test]
    fn celsius_to_fahrenheit() {
        let r = try_convert("0 c to f").expect("ok");
        assert!(r.starts_with("32"));
    }

    #[test]
    fn kelvin_to_celsius() {
        let r = try_convert("273.15 k to c").expect("ok");
        assert!(r.starts_with("0"));
    }

    #[test]
    fn kg_to_lb() {
        let r = try_convert("1 kg to lb").expect("ok");
        // 1000 / 453.59237 = 2.2046...
        assert!(r.starts_with("2.20"));
    }

    #[test]
    fn gb_to_mb_storage() {
        let r = try_convert("2 gb to mb").expect("ok");
        // 2 GB = 2048 MB (base 1024).
        assert!(r.starts_with("2048"));
    }

    #[test]
    fn hours_to_seconds() {
        let r = try_convert("1 h to s").expect("ok");
        assert!(r.starts_with("3600"));
    }

    #[test]
    fn days_to_hours() {
        let r = try_convert("1 d to h").expect("ok");
        assert!(r.starts_with("24"));
    }

    #[test]
    fn portuguese_units() {
        let r = try_convert("5 quilometros to metros").expect("ok");
        assert!(r.starts_with("5000"));
    }

    #[test]
    fn arrow_separator() {
        let r = try_convert("100 km → miles").expect("ok");
        assert!(r.starts_with("62.13"));
    }

    #[test]
    fn in_separator() {
        let r = try_convert("100 cm in m").expect("ok");
        assert!(r.starts_with("1"));
    }

    #[test]
    fn para_separator() {
        let r = try_convert("60 min para h").expect("ok");
        assert!(r.starts_with("1"));
    }

    #[test]
    fn category_mismatch_none() {
        assert!(try_convert("50 kg to km").is_none());
    }

    #[test]
    fn unknown_unit_none() {
        assert!(try_convert("50 elephants to km").is_none());
    }

    #[test]
    fn invalid_number_none() {
        assert!(try_convert("abc miles to km").is_none());
    }

    #[test]
    fn missing_to_keyword_none() {
        assert!(try_convert("50 miles km").is_none());
    }

    #[test]
    fn too_short_none() {
        assert!(try_convert("50 km").is_none());
    }

    #[test]
    fn empty_none() {
        assert!(try_convert("").is_none());
    }

    #[test]
    fn case_insensitive() {
        let r = try_convert("100 KM TO MI").expect("ok");
        assert!(r.starts_with("62.13"));
    }

    #[test]
    fn negative_value() {
        let r = try_convert("-40 c to f").expect("ok");
        // -40C = -40F (cross point famoso).
        assert!(r.starts_with("-40"));
    }

    #[test]
    fn decimal_value() {
        let r = try_convert("1.5 km to m").expect("ok");
        assert!(r.starts_with("1500"));
    }

    #[test]
    fn format_value_trims() {
        // Internal helper test.
        assert_eq!(format_value(0.0), "0");
        assert_eq!(format_value(80.4672), "80.4672");
        assert_eq!(format_value(100.0), "100");
    }
}
