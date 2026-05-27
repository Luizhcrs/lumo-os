//! lumo-bar binario - entry point trivial que delega pra biblioteca.
//!
//! Refactor A-REFACTOR: monolito 2648 LoC quebrado em modulos por feature
//! em `shell/src/bar/`. Este arquivo so chama `lumo_shell::bar::run()`.

fn main() {
    lumo_error::hook::install_panic_hook("lumo-bar", lumo_error::Domain::Shell);
    lumo_shell::bar::run();
}
