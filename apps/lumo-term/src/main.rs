//! lumo-term -- wrapper Alacritty com tema Lumo.
//!
//! Instala ~/.config/alacritty/lumo.toml se nao existir e
//! re-exec alacritty com --config-file apontando pro tema.

use std::env;
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

const THEME_TOML: &str = r##"# lumo-term -- tema Alacritty para Lumo OS.
# Gerado por lumo-term. Nao edite; sera sobrescrito na proxima atualizacao.

[env]
TERM = "xterm-256color"

[window]
padding = { x = 12, y = 12 }
opacity = 0.96
decorations = "None"

[scrolling]
history = 10000

[font]
size = 12.0

[font.normal]
family = "JetBrainsMono Nerd Font"
style = "Regular"

[font.bold]
family = "JetBrainsMono Nerd Font"
style = "Bold"

[font.italic]
family = "JetBrainsMono Nerd Font"
style = "Italic"

[colors.primary]
background = "#0a0a0c"
foreground = "#f5f5f7"

[colors.cursor]
text   = "#0a0a0c"
cursor = "#10b981"

[colors.selection]
text       = "CellForeground"
background = "#10b981"

[colors.normal]
black   = "#131318"
red     = "#f87171"
green   = "#34d399"
yellow  = "#fbbf24"
blue    = "#60a5fa"
magenta = "#a78bfa"
cyan    = "#22d3ee"
white   = "#e5e7eb"

[colors.bright]
black   = "#3f3f46"
red     = "#fca5a5"
green   = "#6ee7b7"
yellow  = "#fcd34d"
blue    = "#93c5fd"
magenta = "#c4b5fd"
cyan    = "#67e8f9"
white   = "#f5f5f7"

[cursor]
style = { shape = "Block", blinking = "On" }
blink_interval = 500

[selection]
save_to_clipboard = true

[keyboard]
bindings = [
    { key = "V", mods = "Control|Shift", action = "Paste" },
    { key = "C", mods = "Control|Shift", action = "Copy" },
    { key = "Plus", mods = "Control", action = "IncreaseFontSize" },
    { key = "Minus", mods = "Control", action = "DecreaseFontSize" },
    { key = "Key0", mods = "Control", action = "ResetFontSize" },
]
"##;

fn theme_path() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| "/root".into());
    PathBuf::from(home).join(".config/alacritty/lumo.toml")
}

fn install_theme(path: &PathBuf) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::write(path, THEME_TOML).ok();
}

fn main() {
    let config = theme_path();
    if !config.exists() {
        install_theme(&config);
    }

    let extra_args: Vec<String> = env::args().skip(1).collect();
    let mut cmd = Command::new("alacritty");
    cmd.arg("--config-file").arg(&config);
    for a in &extra_args {
        cmd.arg(a);
    }
    // re-exec: processo lumo-term e substituido por alacritty; sem fork extra.
    let err = cmd.exec();
    eprintln!("[lumo-term] exec alacritty falhou: {err}");
    std::process::exit(1);
}
