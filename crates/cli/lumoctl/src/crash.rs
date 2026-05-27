//! crash.rs — subcomando lumoctl crash list / crash <id>
//!
//! crash list: lista ultimos crash dumps em ~/.local/state/lumo/crashes/
//! crash <id>: pretty-print de 1 dump (id = filename ou prefixo)

use std::path::PathBuf;

pub fn crash_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".local/state/lumo/crashes")
}

pub fn run(args: &[String]) {
    if args.is_empty() {
        eprintln!("uso: lumoctl crash <list|show> [id]");
        std::process::exit(1);
    }
    match args[0].as_str() {
        "list" => list(),
        "show" => {
            if args.len() < 2 {
                eprintln!("uso: lumoctl crash show <id-prefix>");
                std::process::exit(1);
            }
            show(&args[1]);
        }
        other => {
            eprintln!("subcomando desconhecido: {other}");
            std::process::exit(1);
        }
    }
}

fn list() {
    let dir = crash_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        println!("(nenhum crash em {})", dir.display());
        return;
    };
    let mut crashes: Vec<(String, String)> = Vec::new();
    for ent in entries.flatten() {
        let Some(name) = ent.file_name().to_str().map(String::from) else {
            continue;
        };
        if !name.ends_with(".json") {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(ent.path()) else {
            continue;
        };
        let v: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let bin = v.get("binary").and_then(|s| s.as_str()).unwrap_or("?");
        let code = v.get("code").and_then(|s| s.as_str()).unwrap_or("?");
        let ts = v.get("ts_unix").and_then(|s| s.as_u64()).unwrap_or(0);
        let summary = format!("{}  {:32}  {}", ts, bin, code);
        crashes.push((name, summary));
    }
    crashes.sort_by(|a, b| b.0.cmp(&a.0));
    if crashes.is_empty() {
        println!("(nenhum crash)");
        return;
    }
    println!("{:50}  {:>10}  {:32}  {}", "FILE", "TS", "BINARY", "CODE");
    for (name, summary) in crashes.iter().take(20) {
        println!("{:50}  {}", name, summary);
    }
}

fn show(id_prefix: &str) {
    let dir = crash_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        eprintln!("crash dir nao existe: {}", dir.display());
        std::process::exit(1);
    };
    for ent in entries.flatten() {
        let Some(name) = ent.file_name().to_str().map(String::from) else {
            continue;
        };
        if !name.contains(id_prefix) {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(ent.path()) {
            println!("=== {} ===", name);
            // Pretty-print se for JSON valido.
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Ok(p) = serde_json::to_string_pretty(&v) {
                    println!("{}", p);
                    return;
                }
            }
            println!("{}", content);
            return;
        }
    }
    eprintln!("crash com prefixo '{}' nao encontrado", id_prefix);
    std::process::exit(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crash_dir_uses_home() {
        std::env::set_var("HOME", "/home/test");
        let dir = crash_dir();
        assert!(dir.to_string_lossy().contains("/home/test"));
        assert!(dir.to_string_lossy().ends_with("lumo/crashes"));
    }
}
