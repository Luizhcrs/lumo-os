//! lumo-notes - thin bin shim. Logic em lib.rs.

fn main() -> iced::Result {
    lumo_error::hook::install_panic_hook("lumo-notes", lumo_error::Domain::App);
    lumo_notes::run()
}
