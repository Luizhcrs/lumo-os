# Estado dos Testes — Lumo OS

> Ultima verificacao: 2026-05-26

## Resumo

| Categoria | Testes | Passando | Falhando | Cobertura |
|-----------|--------|----------|----------|-----------|
| lumo-foundation | 54 | 54 | 0 | Alta (tokens, cores, tema) |
| lumo-wm | 140 | 138 | **2** | Media (animacao quebrada) |
| lumo-shell | 28 | 28 | 0 | Baixa (apenas desktop icons + bar tray) |
| lumo-animation | 43 | 43 | 0 | Alta (spring, easing, interpolate) |
| lumo-ipc | 10 | 10 | 0 | Alta (mensagens, roundtrip) |
| lumo-sensors | 37 | 37 | 0 | Alta (thermal, platform, battery) |
| lumo-telemetry | 0 | 0 | 0 | Nenhuma |
| **Total** | **312** | **310** | **2** | **Media** |

## Testes quebrados

### lumo-wm: window_anim

```
failures:
    window_anim::tests::closing_animates_to_done
    window_anim::tests::opening_animates_to_idle

thread 'window_anim::tests::closing_animates_to_done' panicked:
assertion failed: s.is_animating()
```

**Arquivo**: `crates/compositor/lumo-wm/src/window_anim.rs:158`

Causa provavel: a animacao de janela (open/close) nao esta sendo iniciada
automaticamente no construtor. O teste espera `is_animating()` logo apos
`WindowAnimState::new(WindowAnim::Opening)` mas o estado comeca como idle.

## Crates sem testes

| Crate | Testes | Justificativa |
|-------|--------|---------------|
| lumo-beam | 0 | Wrapper wgpu — testes seriam de integracao GPU |
| lumo-graphics | 0 | Render pipeline — precisa de contexto GPU |
| lumo-text | 0 | Shaping cosmic-text — testes visuais |
| lumo-kit | 0 | Componentes UI — em desenvolvimento |
| lumo-gfx-core | 0 | Crate placeholder/futuro |
| lumo-input | 0 | Eventos normalizados — simples demais? |
| lumoctl | 0 | CLI — testes seriam de integracao |
| apps/* | 0 | Apps Iced — sem testes unitarios |

## O que falta cobrir

### Critico (sem testes)
- **Compositor handlers**: xdg_shell, layer_shell, input, compositor
- **DRM backend**: page-flip, render loop, damage tracking
- **IPC runtime**: socket unix, broadcast, consumo
- **Shell/bar**: paint_frame, hit-test, dropdowns, appmenu
- **Shell/desktop**: wallpaper render, menu overlay, rubber-band

### Medio (poucos testes)
- **Perf/telemetry**: histogramas, sampling (apenas estrutura testada)
- **Sensors**: apenas thermal mapeado (falta battery real, lid)

## Como rodar

```bash
# Todos os testes (demora ~3min)
cargo test --workspace

# Apenas crates especificas
cargo test --lib -p lumo-foundation
cargo test --lib -p lumo-wm
cargo test --lib -p lumo-shell
cargo test --lib -p lumo-animation

# Ignorando os quebrados temporariamente
cargo test --lib -p lumo-wm -- --skip window_anim
```

## Recomendacoes

1. **Fixar os 2 testes de window_anim** antes de qualquer refactor
2. **Adicionar testes de integracao** para IPC (mock socket)
3. **Testes visuais** para shell (snapshot PNG comparado)
4. **Testes de propiedade** (proptest) para parsers (TOML, etc)
5. **CI**: rodar `cargo test --workspace` em todo PR
