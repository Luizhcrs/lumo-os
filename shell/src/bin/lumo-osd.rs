//! lumo-osd binario - entry point. Delega pra `lumo_shell::osd::run()`.

fn main() {
    lumo_error::hook::install_panic_hook("lumo-osd", lumo_error::Domain::Shell);
    lumo_shell::osd::run();
}
