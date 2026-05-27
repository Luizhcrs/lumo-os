# Lumo OS — Error Codes

Mapa central de codigos `<DOMAIN>-<SUBSYS>-<NNN>`. Adicionar novo: append + bump tabela + cite em PR.

## Convencoes

- **DOMAIN**: prefixo curto do dominio (WM, IPC, SHELL, APP, BRIDGE, SENSOR, RENDER, THEME).
- **SUBSYS**: subsistema dentro do dominio (RENDER, CONN, AUTH, CONF...).
- **NNN**: numero sequencial 3-digit dentro do par DOMAIN-SUBSYS.

Codigos sao **append-only**. Removido = reserved (nunca reusar). Mudou semantica = novo codigo + deprecate antigo.

## Codigos reservados

### Generico

| Codigo | Severity | Quando | Recovery hint |
|--------|----------|--------|---------------|
| `PANIC-UNCAUGHT-001` | fatal | Panic Rust nao tratado, capturado por panic_hook | Restart processo |

### Compositor (WM)

| Codigo | Severity | Quando | Recovery hint |
|--------|----------|--------|---------------|
| `WM-INIT-001` | fatal | Falha ao inicializar seat/keyboard | Verificar libinput + seatd |
| `WM-RENDER-001` | fatal | GPU device lost, sem dmabuf | Tentar reabrir DRM device |
| `WM-RENDER-002` | degraded | page-flip falhou 3x consecutivos | Disable vsync, retry |
| `WM-PROTOCOL-001` | recoverable | Cliente Wayland violou protocolo | Kill client, manter compositor |
| `WM-CONFIG-001` | degraded | `~/.config/lumo/displays.toml` parse falhou | Fallback default |

### IPC

| Codigo | Severity | Quando | Recovery hint |
|--------|----------|--------|---------------|
| `IPC-CONN-001` | recoverable | Cliente IPC dropou conexao | Drain + unsub |
| `IPC-CONN-002` | recoverable | Bind socket falhou (in-use) | Remove sock + retry |
| `IPC-FRAME-001` | recoverable | Frame JSON corrompido | Skip line, continue |

### Shell

| Codigo | Severity | Quando | Recovery hint |
|--------|----------|--------|---------------|
| `SHELL-SPAWN-001` | recoverable | Bar/desktop crashou | Respawn + toast |
| `SHELL-RENDER-001` | degraded | tiny-skia draw falhou | Skip frame |

### App (Iced)

| Codigo | Severity | Quando | Recovery hint |
|--------|----------|--------|---------------|
| `APP-CRASH-001` | recoverable | App Iced panicou | Toast + reopen offer |
| `APP-FREEZE-001` | recoverable | App nao respondeu ping em 2s | Force quit dialog |

### Bridge HTTP

| Codigo | Severity | Quando | Recovery hint |
|--------|----------|--------|---------------|
| `BRIDGE-AUTH-001` | user_error | Bearer ausente/invalido | HTTP 401 |
| `BRIDGE-RATE-001` | user_error | Rate limit exceeded | HTTP 429 + Retry-After |
| `BRIDGE-IPC-001` | degraded | Bridge nao conseguiu falar com lumo-wm | HTTP 503 |

### Sensor

| Codigo | Severity | Quando | Recovery hint |
|--------|----------|--------|---------------|
| `SENSOR-READ-001` | recoverable | sysfs read falhou (transient) | Retry next sample |
| `SENSOR-MISSING-001` | degraded | hardware sensor ausente | Hide UI element |

### Theme

| Codigo | Severity | Quando | Recovery hint |
|--------|----------|--------|---------------|
| `THEME-LOAD-001` | recoverable | `theme.toml` parse falhou | Default theme |
| `THEME-WATCH-001` | degraded | inotify watch falhou | Hot-reload desabilitado |

## Como usar em codigo

```rust
use lumo_error::{lumo_err, Domain, Severity, RecoveryHint};

let e = lumo_err!(Domain::Compositor, Severity::Degraded, "WM-RENDER-002",
    "page-flip falhou 3x: {}", io_err)
    .with_recovery(RecoveryHint::DisableFeature { feature: "vsync".into() });

tracing::warn!(code = e.code.as_ref(), "{}", e);
```

## Documentar codigo novo

1. Escolher prefixo correto.
2. Pegar proximo numero livre na tabela.
3. Append linha aqui + descricao do gatilho.
4. Comitar junto com codigo que usa.
