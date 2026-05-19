# Code Review Lumo OS W3-W7 -- 2026-05-19

Reviewer: senior code reviewer agent
Escopo: commits 8d54063..b7741d1 (18 commits, ~10h auto-pilot)
Host de revisao: luizhcrds@192.168.0.106:~/Projects/lumo-shell
HEAD: b7741d1
Workspace: 9 apps novos + 3 papers compositor + GTK theme + splash + wifi modal + perf baseline + Samsung pitch material

---

## Sumario executivo

Build release VERDE (`cargo build --release --workspace` completa). 121 warnings (maior parte dead_code/unused). Tests release: 1 P0 bloqueador (lumo-foundation lib test nao compila por struct field drift) -- alem disso 220+ tests passam sem failures. Apple-refs: 3 hits nao-publicos (1 doc interno, 2 docstrings em easing.rs) -- nenhum atinge pitch/Samsung. Seguranca: 1 P0 (senha Wi-Fi vaza em argv via nmcli). Compositor W3 papers solidos com 7 tests focados, mas cursor-only path tem 1 race com animacoes de splash/dropdown. Apps com testes leves: dock/launcher/notif/term = 0 tests. Recomendacao: Approve com fixes obrigatorios P0 antes M2.

---

## P0 bloqueadores

### P0-1. lumo-foundation lib test nao compila apos add font_sans/font_mono

`crates/foundation/lumo-foundation/src/lib.rs:692` e `:708`

Testes inline `roundtrip_toml` e `resolve_applies_overrides` constroem LumoTokens sem os campos novos `font_sans: Option<String>` e `font_mono: Option<String>` que foram adicionados a struct (linhas 411, 414).

```
error[E0063]: missing fields `font_mono` and `font_sans` in initializer of `LumoTokens`
   --> crates/foundation/lumo-foundation/src/lib.rs:692:17
```

Impacto: `cargo test --workspace` ABORTA na crate foundation, bloqueando CI pipeline e a propria validacao do auto-pilot (memory `feedback_validar_local_antes_push.md` foi violado).

Fix em ~30s: adicionar `font_sans: None, font_mono: None` nos dois inits de teste.

### P0-2. Senha Wi-Fi em argv do nmcli (W6.A)

`shell/src/bar/system_info.rs:483-494`

```rust
std::process::Command::new("nmcli")
    .args(["dev", "wifi", "connect", &ssid, "password", &password, ...])
```

Argv eh world-readable via `/proc/<pid>/cmdline` para qualquer processo do mesmo UID (e em alguns kernels para outros UIDs do mesmo PID namespace). Senhas Wi-Fi vazam em:
- `ps aux | grep nmcli`
- Logs de auditd / sysdig / strace
- Logs de bash -x se invocado via shell wrapper

Fix recomendado: usar `nmcli --ask` com stdin redirecionado, ou criar conexao via DBus em `org.freedesktop.NetworkManager` (AddAndActivateConnection) onde a senha vai em property nao-visivel. Patch minimo (stdin):

```rust
let mut child = std::process::Command::new("nmcli")
    .args(["--ask", "dev", "wifi", "connect", &ssid])
    .stdin(std::process::Stdio::piped())
    .spawn()?;
use std::io::Write;
child.stdin.as_mut().unwrap().write_all(format!("{password}\n").as_bytes())?;
```

Ainda nao perfeito (env passwd visivel pra root) mas elimina exposicao por usuario nao-root.

### P0-3. Apple/iOS refs em arquivos do projeto (memory violation)

```
docs/EVOLUCAO_CONTINUA.md:186:- Curvas Material M3 (renamed Apple)
crates/graphics/lumo-animation/src/easing.rs:1://! easing.rs - Curvas de easing cubic-bezier + presets Material/iOS.
crates/graphics/lumo-animation/src/easing.rs:53:    /// suavemente. Usado em dropdowns + sheets iOS.
```

Memory `feedback_lumo_zero_apple_refs_em_publico.md` exige grep limpo. Pitch deck/onepager/script estao OK (auditados), mas:
- easing.rs eh codigo de runtime que pode aparecer em rustdoc publico
- EVOLUCAO_CONTINUA.md esta no repo, fonte para reviewers Samsung

Fix:
- s/Apple/Material M3 + custom/g em easing.rs linha 1
- s/sheets iOS/sheets modais/g em easing.rs linha 53
- s/(renamed Apple)/(curvas proprias)/g em EVOLUCAO_CONTINUA.md

---

## P1 importantes

### P1-1. cursor-only HW plane path nao detecta animacoes em curso

`crates/compositor/lumo-wm/src/backend/drm.rs:985`

```rust
let all_elements = if cursor_moved && surface.pending_flip == false {
    let cursor_only = collect_cursor_only_elements(...);
    if !cursor_only.is_empty() { cursor_only } else { collect_drm_elements(...) }
} else {
    collect_drm_elements(...)
};
```

Falta o guard contra animacoes ativas. Quando `splash_phase != 3` ou `boot_curtain_alpha > 0` ou qualquer dropdown anima, o path cursor-only descarta os outros elementos animados, congelando a animacao no frame em que o cursor se mexer. Sintoma esperado: durante boot, mexer o mouse "para" o splash; durante slide-down de dropdown, mover cursor faz dropdown pular.

Fix:
```rust
let animations_active = state.boot_curtain_alpha > 0.0
    || state.splash_phase < 3
    || state.has_active_dropdown_anim();
let all_elements = if cursor_moved && !surface.pending_flip && !animations_active {
    ...
}
```

### P1-2. GTK theme accent dessincronizado da paleta Lumo

`scripts/install/lumo-gtk-theme/gtk-{3,4}.0/gtk.css`

Theme GTK declara accent `#3b82f6` (blue-500) em 30+ pontos, mas o resto do Lumo (lumo-foundation EMERALD_500_SRGB, lumo-calc theme, splash logo) usa emerald `#10b981`/`#059669`. Resultado pratico: apps GTK (Files, gedit, Firefox prefs) aparecem com botoes/focus azuis em meio a shell verde. Dissonancia visual obvia em screenshots demo Samsung.

Fix: replace_all `#3b82f6` -> `#10b981` em ambos CSS. Tambem trocar `#2563eb` (hover variant) -> `#059669` para manter shade.

### P1-3. revision DBus AppMenu sempre = 1 (todas 5 apps Iced)

`apps/lumo-{calc,editor,notes,monitor,settings}/src/appmenu.rs`

Todas as 5 implementacoes inicializam `AtomicU32::new(1)` e nunca chamam `fetch_add`. Como os menus sao 100% estaticos hoje isso eh tolerable, mas:
- Bloqueia hot-reload de menu (se em algum momento abrir items dinamicos tipo lista de arquivos recentes)
- Cliente plasma-appmenu cacheia layout pela revision; se ID for igual nunca refaz fetch mesmo se servidor mudar

Fix opcional (defensivo): incrementar revision em qualquer mutacao de self.items futura. Por ora aceitavel.

### P1-4. Note.preview() panic em texto UTF-8 multibyte

`apps/lumo-notes/src/note.rs:21`

```rust
if body.len() > 80 { format!("{}...", &body[..80]) } else { body }
```

`body.len()` retorna bytes, nao chars. Texto PT-BR com cedilha/acentos: 41 caracteres "ç" (82 bytes) panicaria no slice se o 80o byte cair entre os 2 bytes UTF-8 do cedilha.

Fix:
```rust
let truncated: String = body.chars().take(80).collect();
if truncated.chars().count() < body.chars().count() { format!("{truncated}...") } else { body }
```

Tests inline (test_preview_long) usam apenas ASCII, nao expoem o bug. Adicionar test com texto multibyte.

### P1-5. lumo-notif TOAST_ID reuso apos replaces_id=0 + wraparound

`apps/lumo-notif/src/dbus.rs:24`

```rust
let id = if replaces_id != 0 { replaces_id } else {
    self.counter.fetch_add(1, Ordering::Relaxed) + 1
};
```

Aceita `replaces_id` arbitrario do cliente sem validar que esse id existe na lista de toasts ativos. Cliente malicioso (qualquer app DBus) pode passar `replaces_id=999999` e sobrescrever toast inexistente do ponto de vista do daemon -- ou pior, se daemon enfileira por id, criar id-collision ao reaproveitar id descartado.

Fix minimo: validar `replaces_id` esta em self.history antes de aceitar; caso contrario alocar novo id.

---

## P2 melhorias

### P2-1. 121 warnings dead_code/unused

Top offenders:
- lumo-files 21 warnings (bin)
- lumo-shell lib 21 warnings
- lumo-settings 16 warnings
- lumo-monitor 10
- lumo-editor 7
- lumo-notes 7
- lumo-dock 5

Maior parte: imports nao-usados (OutputState, RegistryState, SeatState em lumo-dock/main.rs:14-16) e fields nao-lidos (SlotConfig.label, DockConfig.autohide, LumoDock.pointer_y). Rodar `cargo fix --workspace --allow-dirty` resolve metade automatico.

### P2-2. Splash include_bytes!() = 1570 bytes no binario

`crates/compositor/lumo-wm/src/backend/wallpaper.rs:226`

Aceitavel (1.5KB), mas se splash crescer ou virar animado, considerar carregar via XDG `$XDG_DATA_DIRS/lumo/splash.png` em runtime. Fallback embedded ainda OK.

### P2-3. PerfTracker sem cap em samples.len()

`crates/compositor/lumo-wm/src/perf.rs:23`

`Vec::with_capacity(4096)` mas record() faz push ilimitado. Em sessao 60Hz nao logada por 1h: 216000 samples = ~1.5MB. Nao critico, mas se watchdog impede log, vaza memoria continuamente. Adicionar guard: `if samples.len() > 16384 { samples.drain(..8192); }`.

### P2-4. perf log_and_reset trigger acoplado a L2 60s

`crates/compositor/lumo-wm/src/backend/drm.rs:1075`

```rust
if surface.last_timing_log.elapsed() < Duration::from_secs(1) {
    state.perf.log_and_reset();
}
```

Logica fragil: depende de "acabou de logar L2 ha < 1s". Se L2 log perde uma janela (ex: paused), perf nunca loga. Melhor mover para timer dedicado de 60s ou unificar com L2 num so log block.

### P2-5. 78 unwraps em apps/

Maioria em testes (tempdir, fs::write -- justificavel) e construcao de `OwnedValue::try_from(Value::from(static_str))` que sao infallible. Mas 2 quentes ficam fora do hot-path imediato:
- apps/lumo-launcher/src/paint.rs:26 `pb.finish().unwrap()` -- path builder com move/line/quad sempre fecha
- apps/lumo-launcher/src/main.rs:73 `PollTimeout::try_from(16i32).unwrap()` -- 16 eh literal valido

Aceitaveis mas inconsistente com style do shell/bar/main_loop.rs que usa `.expect("msg literal valido")`. Padronizar comentario inline.

### P2-6. lumo-term apenas alacritty config copy (W4.D)

`apps/lumo-term/src/main.rs` (118 LOC, 0 tests). Wrap fino sobre alacritty. Aceitavel para M2 mas sem testes nem fallback se alacritty ausente -- adicionar `which alacritty` check + erro UX claro.

### P2-7. wifi nm_connect spawn sem timeout

`shell/src/bar/system_info.rs:483`

`std::thread::spawn` sem timeout no nmcli subprocess. Se NetworkManager travar/lentidao, thread fica viva indefinidamente. Adicionar `nmcli --wait 15` ou wait_timeout em wrapper.

### P2-8. notif paint usa "circulos" no lugar de glyphs reais

`apps/lumo-notif/src/paint.rs:64-72`

```rust
for (j, _) in toast.summary.chars().take(32).enumerate() {
    fill_circle(canvas, tx + 14.0 + j as f32 * 7.0, y + 42.0, 2.2, pearl);
}
```

Cada caractere renderizado como circulo cinza/branco. Funciona como placeholder mas eh ofensivo na captura para video Samsung. Substituir por cosmic-text igual launcher faz. Decisao: aceitavel para M2-DEV mas P0 antes de gravacao video demo (W7 onepager menciona "demo capturas").

---

## Tests gaps por crate

| Crate                | Tests | Status     | Comentario |
|----------------------|-------|------------|------------|
| lumo-foundation      | N/A   | NAO COMPILA | P0-1 acima |
| lumo-wm (lib)        | 22    | OK         | W3.P1/P2/P4 cobertos |
| lumo-shell (lib)     | 37    | OK         |  |
| lumo-files           | 25    | OK         |  |
| lumo-notes           | 16    | OK         | Falta UTF-8 edge case |
| lumo-settings        | 12    | OK         |  |
| lumo-calc            | 12    | OK         | Tests appmenu + eval bons |
| lumo-monitor         | 10    | OK         |  |
| lumo-editor          | 9     | OK         |  |
| lumo-graphics        | 7     | OK         |  |
| lumo-text            | 5     | OK         |  |
| lumo-animation       | 2     | LEVE       | Adicionar spring closed-form test |
| lumo-input           | 2     | LEVE       |  |
| lumo-dock            | 0     | GAP        | W4.A merged sem 1 test |
| lumo-launcher        | 0     | GAP        | W4.B merged sem 1 test (fuzzy matcher merece) |
| lumo-notif           | 0     | GAP        | W4.C DBus daemon sem test |
| lumo-term            | 0     | GAP        | Aceitavel (wrapper config) |
| lumo-beam, ipc, gfx-core, sensors, kit | 0 cada | OK | crates utils, baixo risco |

Acao: adicionar 3-5 pure-logic tests para dock (magnify spring), launcher (fuzzy match score), notif (id allocation + replaces_id) antes de fechar M2. Memory `feedback_subagent_build_validation_obrigatoria.md` exigia 5+ tests por app -- 3 apps ainda nao atingem.

---

## Memory safety audit

### unsafe blocks novos (6 total)

| Local | Tipo | Avaliacao |
|-------|------|-----------|
| apps/lumo-monitor/src/app.rs:392 | libc::sysconf(_SC_CLK_TCK) | OK -- syscall thread-safe |
| apps/lumo-monitor/src/proc.rs:158-159 | libc::statvfs64 zero-init + chamada | OK -- pattern POSIX comum |
| crates/compositor/lumo-wm/src/backend/drm.rs:253 | renderer.with_context(\|gl\| unsafe { ... }) | OK -- pre-existente, herdado smithay |
| crates/compositor/lumo-wm/src/backend/drm.rs:389,403 | EGLDisplay::new, GlesRenderer::new | OK -- pre-existente |

Nenhum unsafe NOVO em W3-W7 que merece pushback. unwraps em hot path: nenhum no compositor render. Cargo lock vendor smithay com `nom v1.2.4 will be rejected` -- atualizar smithay vendor antes do Rust 1.85.

### Panic risk

- P1-4: `&body[..80]` em note.rs (P1)
- `pb.finish().unwrap()` -- estatico, sempre OK
- Nenhum panic!() direto novo encontrado

---

## Apple refs audit (grep result)

```
docs/EVOLUCAO_CONTINUA.md:186:- Curvas Material M3 (renamed Apple)
crates/graphics/lumo-animation/src/easing.rs:1://! easing.rs - Curvas de easing cubic-bezier + presets Material/iOS.
crates/graphics/lumo-animation/src/easing.rs:53://     suavemente. Usado em dropdowns + sheets iOS.
```

Status: 3 hits, todos cobertos em P0-3.

`docs/pitch/` e `docs/research_papers_2026-05-18.md` auditados manualmente: zero hits. Material para Samsung esta limpo. Pitch onepager cita "Hyprland/wlroots", "GNOME/KDE", "Knox", "FRC 6-bit" -- vocabulario alvo correto.

---

## Build/CI integrity

| Comando                                   | Resultado                                              |
|-------------------------------------------|--------------------------------------------------------|
| cargo build --release --workspace         | OK (0.39s incremental, 121 warnings, sem errors)       |
| cargo test --workspace --no-fail-fast     | FAIL em lumo-foundation lib test (P0-1)                |
| cargo test --workspace --exclude lumo-foundation | OK -- 222 tests passam, 0 failed, 1 ignored   |
| cargo audit                               | Nao rodado -- adicionar ao CI                          |
| Apple refs grep apps/ crates/ docs/       | 3 hits (P0-3)                                          |

Warnings duplicados entre lib/test (`lumo-shell lib generated 21 warnings (21 duplicates)`) indicam que #[allow(dead_code)] em modulos nao foi propagado nos test targets.

Future-incompat warning: nom v1.2.4 vendored em smithay. Bloqueante em Rust ~1.90 (Q4 2026). Atualizar vendor.

---

## Compositor papers W3 -- analise focada

### W3.P1 late-render scheduler (drm.rs:831)

Logica: compute_render_timeout retorna Some(sleep_for) se `frame_age < deadline_age` (~13.667ms a 60Hz). 4 tests cobertos.

Race verificada:
- last_vblank_ts setado em VBlank event (drm.rs:649) -- single producer
- last_frame_time setado em queue_frame Ok branch (drm.rs:1044) -- single producer
- Ambos lidos no timer 4ms (compute_render_timeout linha 832) -- single consumer
- &LumoState imut access OK, sem corrida

Concern menor: last_vblank_ts: Option<Duration> armazena DrmEventTime::Monotonic(ts) mas a logica compute_render_timeout USA `surf.last_frame_time.elapsed()` (Instant), nao last_vblank_ts. O campo vblank esta capturado para uso futuro mas a logica atual usa frame_time -- doc inline poderia clarificar.

### W3.P2 cursor HW plane (drm.rs:985)

P1-1 acima: falta guard de animacoes ativas. Senao OK.

Result `cursor_element.is_some()` -> log "HW plane state changed": correto.

### W3.P4 damage merge (backend/damage.rs)

Heuristica: bbox merge se len > 8 ou coverage < 0.6. 4 tests cobrindo empty/single/many/low-coverage. Bem implementado.

Edge case nao testado: rects com largura/altura zero. coverage_area retorna 0 para esse rect, mas bounding_box ainda os inclui no min/max -- pode inflar bbox desnecessariamente. Aceitavel pois renderer ignora rects degenerados, mas adicionar `filter(|r| r.size.w > 0 && r.size.h > 0)` em bounding_box garante higiene.

Heuristica nao chama queue_frame com bbox merged -- atualmente o elem_damage calculado eh **descartado** apos merge (apenas tracing::trace logado). Ver drm.rs:1010: `// Resultado logado via tracing::trace dentro de merge_if_complex.` Significa que a otimizacao W3.P4 nao tem efeito real ainda -- so observa. Isto eh por design (heuristica observacional) ou bug? Vale clarificar no commit message ou aplicar de fato em render_frame.

---

## Recomendacao final

**Approve com fixes obrigatorios P0 antes M2 release.**

P0-1 (foundation test) eh trivial (~30s patch) e bloqueia CI. P0-2 (senha argv) eh trivial via stdin pipe e elimina vulnerabilidade real. P0-3 (Apple refs) eh sed em 3 linhas.

Apos P0:
- P1-1 (cursor-only animation guard) antes de gravar video demo
- P1-2 (GTK accent emerald) antes de screenshots Samsung
- P1-4 (UTF-8 preview) baixo risco mas trivial

P2 sao melhorias incrementais; aceitar nessa rodada.

Build/test pipeline solido apos P0-1. Compositor W3 esta production-quality. Apps W4-W5 funcionais mas com gaps de teste em dock/launcher/notif. GTK theme e splash sao polish W6 funcional. Pitch W7 esta entregavel sem ajustes maiores -- apenas validar que demos sincronizam com fixes P1.

Volume entregue em ~10h auto-pilot eh notavel; defeitos sao tipicos de iteracao rapida e nenhum compromete arquitetura.

---

Reviewer: code-reviewer agent
Data: 2026-05-19
Tempo total review: ~25 min via SSH em luizhcrds@192.168.0.106
