//! lumo-settings - thin shim.

fn main() -> iced::Result {
    lumo_error::hook::install_panic_hook("lumo-settings", lumo_error::Domain::App);
    lumo_settings::run()
}
