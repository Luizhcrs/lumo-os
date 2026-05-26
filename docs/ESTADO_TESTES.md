# Estado dos Testes — Lumo OS

> Ultima verificacao: 2026-05-26

## Resumo

| Categoria | Testes | Passando | Falhando | Cobertura |
|-----------|--------|----------|----------|-----------|
| lumo-foundation | 53 | 53 | 0 | Alta (tokens, cores, tema) |
| lumo-wm | 139 | 139 | 0 | Alta (após fix W32.4) |
| lumo-shell | 27 | 27 | 0 | Baixa (apenas desktop icons + bar tray) |
| lumo-animation | 43 | 43 | 0 | Alta (spring, easing, interpolate) |
| lumo-ipc | 10 | 10 | 0 | Alta (mensagens, roundtrip) |
| lumo-sensors | 37 | 37 | 0 | Alta (thermal, platform, battery) |
| lumo-telemetry | 0 | 0 | 0 | Nenhuma |
| **Total** | **309** | **309** | **0** | **Media** |

## Testes quebrados

Nenhum. Todos os 309 testes passam.

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
```

## Recomendacoes

1. **Adicionar testes de integracao** para IPC (mock socket)
2. **Testes visuais** para shell (snapshot PNG comparado)
3. **Testes de propiedade** (proptest) para parsers (TOML, etc)
4. **CI**: rodar `cargo test --workspace` em todo PR
