# Code Review — Megarodada 2026-05-18

## Veredito: Request changes

3 bloqueadores P0/P1 obrigatorios antes de tag/release. Apos isso, approve. Demais P1/P2 viram issues incrementais.

## P0 — Bloqueadores

### 1. M1 SSD close button: nao implementado (`1cf33d6`)
Commit promete handlers click close + drag titlebar. Diff so adiciona render. Botao **decorativo, nao clicavel**. Arquivo: `crates/compositor/lumo-wm/src/handlers/input.rs:150`.

### 2. M1 xdg-decoration: aceita ClientSide sem validar (`1cf33d6`)
`state.rs:457-464` honra `Mode::ClientSide` enviado pelo cliente. Qt5/Qt6 mismatch -> janela **sem decoracao alguma**. Force ServerSide ou log warning.

## P1 — Importantes

### 3. `lid_handler.lock().unwrap()` race envenenamento
`handlers/lid.rs:73,103`. Thread polling panica -> mutex envenena -> compositor crash. Trocar por `lock().map().unwrap_or_default()`.

### 4. Boot curtain frame-rate-coupled (`e9296b2`)
`drm.rs:805` `alpha - 0.067` por frame. 144Hz = 100ms em vez de 250ms. Usar delta tempo real.

### 5. Boot curtain duplicado 3 lugares
Refactor `build_curtain_only(output_w, output_h, alpha)`.

### 6. M2 sombras clip assume z-order linear via `space.elements()` iter
Smithay nao garante back-to-front consistente. Documentar premissa + test com `raise_element`.

### 7. `next_tile_position` hardcoded 1920x1080 + BAR_H=40
Multi-monitor/rotacao quebra. Ler `self.space.outputs().next()` real.

### 8. `set_brightness_pct` duplicada wm + bar
Mover so pra `lumo-sensors`.

### 9. N5 close-focus pega `space.elements().next()` arbitrario
Iterator order, nao MRU. Usar FocusManager.prev tracker.

### 10. OSD process zombie em erros nao-EOF
Loop swallow protocol errors. Adicionar timeout.

## P2 — Melhorias

11. F5 ThemeReloaded hardcoded Light
12. Snapshot test sempre-passa primeira run (UPDATE_SNAPSHOTS env)
13. Snapshot byte-a-byte: instavel cross-distro (PSNR/dssim)
14. CI deny so safety.yml, fmt sem cache
15. osd.rs linhas densas (rustfmt)
16. `init_xdg_decoration` apos `new()` -> inline em new
17. udev rule `/bin/chgrp` -> `/usr/bin/chgrp`
18. Sombras alpha 0.4+bleed: halo em OLED

## Tests gaps

- ZERO teste click-handler SSD close (consistente: nao existe)
- `shadow_subtract_rect` sem test recursivo geometrico
- `boot_clients_ready` mockavel mas nao testado
- FocusManager testa transicoes enum, zero WlSurface real
- OSD tick state machine sem teste
- Snapshot so 2 frames estaticos (sem dropdown/animacao)
- N2 scroll brightness sem clamp/rate limiting test

## Recomendacao

Fix P0 #1 + P0 #2 + P1 #3 + P1 #4 antes de proximo push significativo. Resto vira issues backlog.
