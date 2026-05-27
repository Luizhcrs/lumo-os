//! lumo-editor - thin shim.

fn main() -> iced::Result {
    lumo_error::hook::install_panic_hook("lumo-editor", lumo_error::Domain::App);
    lumo_editor::run()
}
