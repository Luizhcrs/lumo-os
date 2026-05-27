//! lumo-calc - thin bin shim. Logic em lib.rs.

fn main() -> iced::Result {
    lumo_error::hook::install_panic_hook("lumo-calc", lumo_error::Domain::App);
    lumo_calc::run()
}
