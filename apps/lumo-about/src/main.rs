fn main() -> iced::Result {
    lumo_error::hook::install_panic_hook("lumo-about", lumo_error::Domain::App);
    lumo_about::run()
}
