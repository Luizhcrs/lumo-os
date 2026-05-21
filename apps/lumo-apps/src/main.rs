//! lumo-apps - dispatcher unico Lumo apps (W33).

use std::process::ExitCode;

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    let basename = std::path::Path::new(&argv[0])
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let sub: String = if basename != "lumo-apps" && basename.starts_with("lumo-") {
        basename.trim_start_matches("lumo-").to_string()
    } else {
        argv.get(1).cloned().unwrap_or_default()
    };

    let res = match sub.as_str() {
        "about"    => lumo_about::run(),
        "calc"     => lumo_calc::run(),
        "notes"    => lumo_notes::run(),
        "monitor"  => lumo_monitor::run(),
        "editor"   => lumo_editor::run(),
        "files"    => lumo_files::run(),
        "settings" => lumo_settings::run(),
        "store"    => lumo_store::run(),
        "" => {
            eprintln!("lumo-apps: argv[0]={} sem subcommand", basename);
            return ExitCode::from(2);
        }
        _ => {
            eprintln!("lumo-apps: app desconhecido '{}'", sub);
            return ExitCode::from(2);
        }
    };

    match res {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => { eprintln!("lumo-apps {}: {}", sub, e); ExitCode::FAILURE }
    }
}
