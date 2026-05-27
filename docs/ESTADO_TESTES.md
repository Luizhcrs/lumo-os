# Estado dos Testes — Lumo OS

> Ultima verificacao: 2026-05-27 (pos W37.18 Chromium fix)

## Resumo

| Categoria | Testes | Passando | Falhando | Cobertura |
|-----------|--------|----------|----------|-----------|
| lumo-foundation | 53 | 53 | 0 | Alta (tokens, cores, tema) |
| lumo-wm | 189 | 189 | 0 | Alta (W37.11-18 +4 testes gating protocols) |
| lumo-shell | 36 | 36 | 0 | Media (W37.6 menu unificado) |
| lumo-files | 66 | 66 | 0 | Alta (W37 ctx menu) |
| lumo-animation | 43 | 43 | 0 | Alta (spring, easing, interpolate) |
| lumo-ipc | 10 | 10 | 0 | Alta (mensagens, roundtrip) |
| lumo-sensors | 37 | 37 | 0 | Alta (thermal, platform, battery) |
| lumo-telemetry | 0 | 0 | 0 | Nenhuma |
| **Total** | **434** | **434** | **0** | **Media-Alta** |

## W37 — Chromium broken pipe (RESOLVIDO 2026-05-27)

Detalhes em `docs/W37_CHROMIUM_RESOLVED_2026-05-27.md`.

Tests novos lumo-wm (W37.4 / 5 / 7 / 8 / 11-18):
- gating protocols: `w37_18_toplevel_icon_*`, `w37_15_color_manager_*`
- decoration: `w37_8_client_side_respeitado`, `w37_8_server_side_default`
- focus broadcast: `w37_5_*` (5 testes)
- CSD detection: `w37_8_*csd_*` (6 testes)
- ssd: `w37_7_titlebar_bg_full_width`
- maximize: `w37_4_*` (3 testes)
- menu dyn: `w37_6_*` (4 testes)

Tests novos lumo-shell (W37.2-6):
- desktop ctx: `w37_3_ctx_menu_usa_menu_item_unificado`
- desktop ctx width: `w37_3_ctx_menu_width_igual_menu_w_desktop`
- ipc gating: `w37_close_dropdowns_nao_seta_close_menu`
- `close_desktop_menu_seta_close_menu`, `desktop_open_selected_seta_flag`
- menu dyn: 4 testes

Tests novos lumo-files (W37.0 / 3):
- radius unificado: `w37_3_radius_unificado_com_desktop_menus`
- context: `test_context_menu_*` (4 testes)
- items: `unified_tem_destrutivo`, `unified_tem_nova_pasta_e_colar`
- enable: `sem_selecao_desabilita_itens_de_arquivo`
- enable: `com_selecao_habilita_itens_de_arquivo`

## Testes quebrados

Nenhum. Todos os 434 testes passam.

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
