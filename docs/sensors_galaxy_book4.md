# Sensores Galaxy Book 4 U300 — auditoria + ideias UX

Doc gerado por agente research + validacao empirica via SSH no Galaxy real (Arch Linux kernel 7.0.7-arch2-1, samsung-galaxybook driver carregado, BAT1 cycle 47).

## Hardware confirmado

- SKU base: Galaxy Book4 NP750XGJ-* serie, 15.6" FHD IPS, clamshell tradicional.
- CPU: Intel Processor U300 (Raptor Lake-U, 1P+4E, ate 4.4 GHz). UHD Xe G4 48 EU. SEM NPU/VPU.
- Implicacao: SEM accelerometer (clamshell, nao 2-em-1) -> auto-rotate descartado.
- Implicacao: SEM IR camera -> wake-on-approach descartado.

## Inventario validado empiricamente (2026-05-18)

| Sensor / device | Path sysfs | Confirmado | Notas |
|---|---|---|---|
| ALS (ambient light) | /sys/bus/iio/devices/iio:device* | **NAO** | IIO devices vazio. Galaxy Book 4 U300 nao expoe ALS via IIO. |
| Accelerometer | /sys/bus/iio/devices | **NAO** | IIO vazio. Clamshell sem sensor. |
| Hall / lid switch | /proc/acpi/button/lid/*/state | **SIM** | "state: open". Evento SW_LID via evdev. |
| Thermal zones | /sys/class/thermal/thermal_zone[0-7] | **SIM** | 8 zonas: INT3400, SNS1-3, TCPU, TCPU_PCI, x86_pkg_temp, iwlwifi_1. |
| Battery health | /sys/class/power_supply/BAT1/{charge_full,charge_full_design,cycle_count} | **SIM** | 3490000/3530000 = 98.9% saude, 47 ciclos. |
| Charge limit | /sys/class/power_supply/BAT1/charge_control_end_threshold | **SIM** | Atual=100. Aceita 1-100 via samsung-galaxybook. |
| Platform profile | /sys/firmware/acpi/platform_profile | **SIM** | Atual=balanced. Opcoes: low-power/quiet/balanced/performance (4 modos). |
| Firmware attributes | /sys/class/firmware-attributes/samsung-galaxybook/ | **SIM** | samsung-galaxybook driver expoe atributos BIOS-like. |
| Backlight (display) | /sys/class/backlight/intel_backlight/ | **SIM** | Padrao Intel, controle direto. |
| KB backlight | /sys/class/leds/*kbd_backlight | **NAO** | LEDs detectados: capslock/numlock/scrolllock + LAN + mmc. SEM samsung-galaxybook::kbd_backlight. Pode nao existir HW neste sku. |
| Cooling devices | /sys/class/thermal/cooling_device[0-8] | **SIM** | 9 cooling devices. |
| Cameras | /dev/video* | NAO VERIFICADO | Requer IPU6 firmware + libcamera. |
| Fingerprint | USB 2808:6553 (FocalTech) | NAO VERIFICADO | Bloqueado por libfprint stock (precisa MR patched). |

## Priorizacao revisada (matriz pos-validacao empirica)

| Feature | Impacto UX | Esforco | HW viavel? | Prioridade |
|---|---|---|---|---|
| Charge limit 80% toggle | Alto | Baixo | SIM | **P0** |
| Lid close -> lock + dim | Alto | Baixo | SIM | **P0** |
| Battery health % display | Medio | Baixo | SIM | **P0** |
| Platform profile cycle (4 modos) | Medio | Baixo | SIM | **P0** |
| Thermal indicator dropdown bateria | Medio | Baixo | SIM | P1 |
| Hotkeys Fn+F* compositor | Alto | Medio | SIM | P1 |
| Auto-brightness via ALS | n/a | n/a | **NAO** | descartado (sem ALS) |
| KB backlight ajuste | n/a | n/a | **NAO** | descartado (sem LED class) |
| Auto-rotate | n/a | n/a | **NAO** | descartado (clamshell) |
| Webcam presence detection RGB | Baixo | Alto | SIM (custo CPU) | P3 |
| Fingerprint unlock | Alto | Alto (libfprint patch) | SIM com patch | P2 |
| Wake-on-approach (IR/ToF) | n/a | n/a | **NAO** | descartado (sem IR) |

## Categorizado por uso UX

### Charge limit toggle (P0)

UI no dropdown bateria: switch "Cuidar bateria" -> escreve 80 em `/sys/class/power_supply/BAT1/charge_control_end_threshold`. Persistir via systemd unit (resetar pos S3).

Diferencial vs concorrentes: Samsung official Battery Saver, mas via Linux nativo. Vendor ja approves.

### Lid close lock (P0)

Evento `SW_LID` via evdev (libinput propaga). Compositor intercepta:
1. Apos lid close: dim 50% imediato + start timer 3s
2. Timer dispara -> lock screen + suspend deferred 30s
3. Lid open antes: cancela tudo

Diferente do default systemd-logind (suspend imediato). UX Apple MacBook.

### Battery health display (P0)

Dropdown bateria mostra:
- "Saude: 98%" (charge_full / charge_full_design * 100)
- "Ciclos: 47"
- "Tempo restante: 4h 23min" (ja implementado parcial)

Cor por threshold: >80% verde, 60-80% amarelo, <60% laranja.

### Platform profile cycle (P0)

Galaxy Book 4 expoe 4 modos: low-power, quiet, balanced, performance.

UI: dropdown bateria adiciona pill "Perfil: balanced" -> click cycle proximo. Escreve em `/sys/firmware/acpi/platform_profile`.

Hotkey: Fn+F11 (samsung-galaxybook consume internamente, mas Lumo pode reagir via WMI event listener pra atualizar UI imediato).

### Thermal indicator (P1)

Le `thermal_zone6/temp` (x86_pkg_temp) periodicamente. Mostra no dropdown bateria:
- <70C verde (oculto)
- 70-85C amarelo "Cpu morno" 
- \>85C laranja "Cpu quente - throttling" + sugere mudar pra quiet

### Hotkeys Fn+F* (P1)

samsung-galaxybook consome F9 (KB backlight), F10 (block recording), F11 (profile).

Lumo deve adicionar handlers userspace pra:
- Fn+F1 -> lumo-launcher
- Fn+F5 -> toggle touchpad enable
- Fn+F12 -> toggle Fn-lock

Implementacao: subscribe a `evdev` em /dev/input/event* + filter scancodes Samsung specificos. WMI events via netlink kernel.

## Riscos tecnicos confirmados

- **ALS confirmado ausente**: dropa auto-brightness do roadmap inteiro. Substituir por: brightness manual em config + presets horario (dia/noite/auto).
- **KB backlight confirmado ausente** neste sku: U300 base pode nao ter KB iluminado em HW. Verificar empiricamente apertando Fn+F9 e observando — se nenhuma luz acende, hardware nao tem.
- **firmware-attributes existe**: explorar opcoes BIOS-tweakable (USB charging quando off, lid open power, mic kill switch). Documentar antes de expor em painel settings.
- **platform_profile tem 4 modos (nao 3)**: docs Kernel mencionam 3, mas Galaxy expoe 4 incluindo "quiet". UI deve listar todos.

## Crate proposto: `crates/system/lumo-sensors/`

API publica (apenas signatures, impl posterior):

```rust
pub struct SensorRegistry { /* sysfs paths cached */ }

impl SensorRegistry {
    pub fn discover() -> Result<Self, SensorError>;
    pub fn lid_switch(&self) -> Option<&LidSwitch>;
    pub fn thermal_zones(&self) -> &[ThermalZone];
    pub fn battery(&self) -> &Battery;  // sempre presente
    pub fn platform_profile(&self) -> Option<&PlatformProfile>;
    pub fn firmware_attributes(&self) -> Option<&FirmwareAttrs>;
}

pub struct Battery { /* paths cached */ }
impl Battery {
    pub fn percent(&self) -> Result<u8, SensorError>;
    pub fn health_percent(&self) -> Result<u8, SensorError>; // full/design * 100
    pub fn cycle_count(&self) -> Option<u32>;
    pub fn charge_limit(&self) -> Option<u8>;
    pub fn set_charge_limit(&self, pct: u8) -> Result<(), SensorError>;
    pub fn status(&self) -> ChargingStatus;
}

pub trait PlatformProfile {
    fn current(&self) -> Profile; // LowPower|Quiet|Balanced|Performance
    fn available(&self) -> Vec<Profile>;
    fn set(&self, p: Profile) -> Result<(), SensorError>;
    fn cycle_next(&self) -> Result<Profile, SensorError>;
}

pub trait LidSwitch {
    fn current_state(&self) -> LidState; // Open|Closed
    fn subscribe(&self, cb: Box<dyn Fn(LidState) + Send>) -> Subscription;
}

pub struct ThermalZone {
    pub name: String,
    pub kind: ThermalKind, // Cpu|Soc|Charger|Nvme|Wifi|Other
}
impl ThermalZone {
    pub fn temp_celsius(&self) -> Result<f32, SensorError>;
}
```

Backend sysfs-only. polkit pra escrita em charge_control_end_threshold (precisa root sem). systemd-tmpfiles cria regra no boot pra dar permissao group `lumo` se desejado.

## Diferenciador vs concorrentes

O que Lumo OS no Galaxy Book 4 pode ter que outros sistemas no mesmo hardware NAO tem (ou tem pior):

1. **UI nativa Samsung-aware**: charge limit + perfil + saude bateria + thermal zones lapidados em UI Apple-grade. Windows expoe parcial via Samsung Settings; Linux distros genericas nao expoem.
2. **Lid close UX customizado**: dim+timer em vez de suspend imediato.
3. **Fn+F* binding configuravel**: usuario remap. Windows fixo, macOS fixo, Linux distros sem.
4. **4-modo platform profile**: maioria das implementacoes mostra 3 (low/balanced/perf). Lumo mostra 4 (inclui quiet).

## Fontes

- Linux Kernel Docs — samsung-galaxybook driver: https://docs.kernel.org/admin-guide/laptops/samsung-galaxybook.html
- ArchWiki Samsung laptop: https://wiki.archlinux.org/title/Laptop/Samsung
- Linux thermal sysfs ABI: https://docs.kernel.org/driver-api/thermal/sysfs-api.html
- platform_profile sysfs ABI: https://docs.kernel.org/userspace-api/sysfs-platform_profile.html
- Validacao empirica: SSH luizhcrds@192.168.0.106 em 2026-05-18, kernel 7.0.7-arch2-1, samsung-galaxybook driver carregado.

## Proximo passo

1. Criar crate `crates/system/lumo-sensors/` (subagent dev em sessao futura)
2. Implementar `Battery::set_charge_limit` + UI toggle no dropdown bateria do lumo-bar
3. Polkit rule para autorizar write sysfs sem root
4. Integrar lid close handler no compositor (substituir systemd-logind default)
