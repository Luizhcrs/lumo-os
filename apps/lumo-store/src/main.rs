//! lumo-store - thin shim.

fn main() -> iced::Result {
    lumo_error::hook::install_panic_hook("lumo-store", lumo_error::Domain::App);
    lumo_store::run()
}
