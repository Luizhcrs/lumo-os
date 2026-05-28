//! lumo-osd — A4 review: daemon OSD unico que substitui lumo-osd-{locks,brightness,volume}.
//!
//! Arquitetura:
//!   - sources/: 4 polling sources (locks, brightness, volume + pactl_parse)
//!   - dispatch.rs: prioridade Caps > Num > Scroll > Brightness > Volume
//!   - render: 1 surface layer-shell unica (Galaxy-only)
//!
//! Vantagens vs N bins:
//!   - 1 systemd-user unit em vez de 3
//!   - 1 layer-shell surface em vez de 3 (memoria, focus contention)
//!   - 1 crash watcher em vez de 3
//!   - Priority arbitration entre eventos quando varios mudam junto

pub mod dispatch;
pub mod sources;
