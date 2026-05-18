//! Hardware target detection via DMI.
//!
//! Lumo eh otimizado pra Samsung Galaxy Book 4 U300 mas roda em qualquer
//! Linux compativel (filosofia "primary target, nao locked"). Este modulo
//! detecta hardware no init pra aplicar tunings especificos quando
//! detecta Galaxy.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwareTarget {
    /// Samsung Galaxy Book4 (qualquer modelo da familia).
    /// Aplica tunings: FRC dither bypass, libinput accel adaptive,
    /// DRM card1 priority, painel 1920x1080 default.
    GalaxyBook4,
    /// Outro hardware Linux generico. Defaults conservadores.
    GenericLinux,
}

impl HardwareTarget {
    pub fn detect() -> Self {
        let family = read_dmi("product_family");
        let vendor = read_dmi("sys_vendor");

        if family == "Galaxy Book4" {
            return HardwareTarget::GalaxyBook4;
        }
        if vendor.to_uppercase().contains("SAMSUNG") {
            // outras Samsung Galaxy futuras
            return HardwareTarget::GalaxyBook4;
        }
        HardwareTarget::GenericLinux
    }

    pub fn label(&self) -> &'static str {
        match self {
            HardwareTarget::GalaxyBook4 => "Samsung Galaxy Book 4",
            HardwareTarget::GenericLinux => "Linux generico",
        }
    }
}

fn read_dmi(key: &str) -> String {
    std::fs::read_to_string(format!("/sys/class/dmi/id/{}", key))
        .unwrap_or_default()
        .trim()
        .to_string()
}
