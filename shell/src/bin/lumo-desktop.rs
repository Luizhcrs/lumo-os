//! lumo-desktop binario - entry point trivial que delega pra biblioteca.
//!
//! Refactor A-REFACTOR: split em modulos `shell/src/desktop/`. Este arquivo
//! so chama `lumo_shell::desktop::run()`.

fn main() {
    lumo_shell::desktop::run();
}
