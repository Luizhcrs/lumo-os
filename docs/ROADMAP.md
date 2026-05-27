# Lumo OS — Roadmap

## Status Atual

Compositor (`lumo-wm`) ~90% funcional. Shell (`lumo-bar`, `lumo-desktop`) ~35%.
Pipeline grafica e cor completas. IPC operacional. Sensores basicos implementados.

## Milestones

### M0 — Docs + Legal + Demo Video (2026-05-25)

- [done] Docs tecnicos consolidados em `docs/` (F2)
- [wip] Demo video 60s: boot -> compositor -> bar -> dropdown -> janela
- [wip] README publico como entry point
- [no] Licenca definida (closed source por padrao)

### M1 — Shell Completo (2026-07-06)

- Dock com apps fixados e animacao de launch
- Launcher de aplicativos (fuzzy search)
- Notificacoes (overlay layer-shell)
- SSD close button funcional (P0 do code review 2026-05-18)
- xdg-decoration forcando ServerSide (P0 do code review 2026-05-18)
- Boot curtain com delta tempo real (P1 do code review 2026-05-18)

### M2 — Performance Baseline (2026-07-20)

- Frame time medido e documentado (target: < 8ms por frame em DRM)
- Boot curtain refatorado (funcao unica, sem duplicacao)
- Sombras com z-order garantido
- Testes de snapshot com PSNR/dssim (substituir byte-a-byte)
- CI com cache de fmt e deny

### M3 — Apps Core (2026-10-12)

- `lumo-term` — terminal nativo baseado em foot
- `lumo-settings` — painel de configuracoes (tema, brilho, bateria, rede)
- Fingerprint unlock (requer libfprint patch — P2 sensors)
- Hotkeys Fn+F* mapeados no compositor

### M4 — Samsung Pitch Ready (2026-11-30)

- Demo hardware: Galaxy Book 4 U300 rodando Lumo OS nativo
- Documentacao OEM: diferenciais tecnicos vs Hyprland/GNOME/KDE
- Benchmark documentado: latencia input, frame time, consumo RAM
- Proposta white-label: integracao com Samsung Knox / One UI coexistence

## Backlog Tecnico (post-W37)

Gargalos identificados na analise da pipeline 2026-05-27. Numerados pra referencia.

- **#5 XWayland**: smithay `XWaylandShell` + binario `Xwayland`. Spec ~500 LOC. Apps Discord/GIMP/Steam/LibreOffice voltam a funcionar. Risco: override-redirect + focus-stealing quirks. Estimativa 1 semana.
- **#6 Migracao multibinary**: ver ADR-001. Mover `lumo-appsd` + `lumo-appctl` pra `archive/`, criar symlinks no install. Esforco 1 dia.
- **#7 shell tests +150**: bar/render, dropdowns por categoria, desktop/input state machine, paint_ctx_menu. Target 47 -> 200. Esforco 2 dias.
- **#9 focus debounce 50ms**: state.focus_debounce_timer + calloop schedule. Evita pills piscando + flood IPC em transicoes rapidas. Esforco 4h.

## Itens Descartados

| Feature | Motivo |
|---------|--------|
| Auto-brightness via ALS | Galaxy Book 4 U300 nao tem sensor ALS |
| KB backlight ajuste | Sem LED class no SKU testado |
| Auto-rotate | Clamshell tradicional, sem acelerometro |
| Wake-on-approach (IR/ToF) | Sem sensor IR |

Detalhes de hardware em `docs/sensors_galaxy_book4.md`.

## Contexto Estrategico

Roadmap publico. Detalhes de negocio, parcerias e pitch em Obsidian privado:
`1 - Projetos/Projeto - Lumo OS/`.
