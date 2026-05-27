//! logs.rs — subcomando lumoctl logs
//!
//! Tail de /tmp/lumo-*.log (cada bin escreve em /tmp/lumo-<bin>.log via
//! tracing-subscriber). Filtra por subsystem se --subsystem dado.
//!
//! Implementacao simples: cat dos arquivos + filter linha por linha.
//! Streaming via journalctl-style fica pra futuro.

use std::path::PathBuf;

pub fn run(args: &[String]) {
    let mut subsystem: Option<String> = None;
    let mut lines: usize = 200;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--subsystem" => {
                i += 1;
                if i < args.len() {
                    subsystem = Some(args[i].clone());
                }
            }
            "--lines" | "-n" => {
                i += 1;
                if i < args.len() {
                    lines = args[i].parse().unwrap_or(200);
                }
            }
            _ => {}
        }
        i += 1;
    }
    tail_lumo_logs(subsystem.as_deref(), lines);
}

fn log_files() -> Vec<PathBuf> {
    let mut v = Vec::new();
    let Ok(entries) = std::fs::read_dir("/tmp") else {
        return v;
    };
    for ent in entries.flatten() {
        let Some(name) = ent.file_name().to_str().map(String::from) else {
            continue;
        };
        if name.starts_with("lumo-") && name.ends_with(".log") {
            v.push(ent.path());
        }
    }
    v
}

fn tail_lumo_logs(subsystem: Option<&str>, lines: usize) {
    let files = log_files();
    if files.is_empty() {
        println!("(sem logs em /tmp/lumo-*.log)");
        return;
    }
    for f in files {
        let name = f
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .trim_end_matches(".log");
        if let Some(sub) = subsystem {
            if !name.contains(sub) {
                continue;
            }
        }
        let content = match std::fs::read_to_string(&f) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let all_lines: Vec<&str> = content.lines().collect();
        let start = all_lines.len().saturating_sub(lines);
        println!("=== {} ===", name);
        for line in &all_lines[start..] {
            println!("{}", line);
        }
        println!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_files_returns_paths_or_empty() {
        let v = log_files();
        for p in &v {
            assert!(p.to_string_lossy().contains("lumo-"));
            assert!(p.to_string_lossy().ends_with(".log"));
        }
    }
}
