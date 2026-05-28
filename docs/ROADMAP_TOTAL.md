# Lumo OS — Roadmap Total

Snapshot 2026-05-27. Critical path completo ate release publica. ROADMAP.md (atual) cobre milestones tecnicos historicos; este doc define **6 fases** ate Lumo OS daily-driver + Samsung pitch + release.

## Estado base (2026-05-27)

- Compositor lumo-wm ~92% funcional (DRM + winit backends)
- Shell 3-process (bar, desktop, osd) ~50%
- 18 binarios compilam release
- Error handling stack completo (lumo-error + crash dump + ADR-006)
- IPC stable + lumo-bridge HTTP rate-limited
- 13 apps Iced + 5 utils
- ~558 testes unidade workspace
- Apps GTK3 (Mousepad/Firefox) com identidade Lumo parcial via env CSD-suppress
- Apps Qt6 (Kate) com SSD Lumo limpa
- Scroll universal funcionando (PointerAxis handler)
- Focus steal protection Windows-style
- Subsurface focus → root toplevel (Chrome compat)

## Gaps criticos conhecidos

| Item | Impacto | Fase |
|---|---|---|
| Cursor custom surface Chrome bug residual | Clicks erram alvos pequenos | F0 |
| Sem XWayland | 30% apps comuns quebram (Discord/GIMP/Steam) | F0 |
| Multi-monitor robusto | Hot-plug + scaling | F0 |
| Lumo SSD sem text render | App nome nao aparece SSD | F1 |
| HeaderBar GTK3 interno persiste | Mousepad/Inkscape duas titlebars | F1 |
| Sem greeter login | Boot direto sessao user | F2 |
| Sem session save/restore | Logout perde apps abertos | F2 |
| Settings panel incompleto | Falta brightness/displays/sound | F3 |
| Files: sem mounts/smb/search | Daily-driver gap | F3 |
| Sem ISO instalavel | Distribui = manual git clone | F4 |
| Sem update mechanism | Atualizacao via pacman cru | F4 |
| Sem demo video | Pitch Samsung sem material | F5 |

## Fase 0 — Daily Driver Estavel

**Saida**: Luiz usa Lumo OS 8h/dia sem queda critica.

Sprint 1:
- Fix cursor custom surface Chrome (debug + hotspot correct)
- Fix gestures forward client (`zwp_pointer_gestures_v1`)
- XWayland integration (smithay `XWaylandShellState` + binario)
- Multi-monitor hot-plug robusto (output add/remove sem freeze)
- DRM device-lost recovery (re-open + re-init renderer)

Sprint 2:
- Audit silent-ignores prod (S7 sprint completion)
- catch_unwind em handler boundaries (S3 completion)
- Wayland protocols completos: tablet-v2, idle-inhibit, content-type
- Stress test: 30 windows + screen-record + bench

**Saida criterio**: 4h sessao sem crash. lumoctl crash list vazio.

## Fase 1 — Identidade Visual Universal

**Saida**: Qualquer app spawned tem titlebar Lumo + tema consistente.

Sprint 3:
- `lumo-decor` plugin libdecor M2: render real titlebar via wl_shm_pool
- M3: font rendering pra title text (cosmic-text C binding)
- M4: pointer events + button hit pra close/min/max
- Test com Firefox, mpv, Blender, GTK4 apps

Sprint 4:
- SSD compositor text rendering retry (lessons learned do M2 reverted)
- Tema GTK custom Lumo (SCSS → gtk.css) cores match SSD
- Tema Qt5/6 (qt5ct config) cores match
- Icon theme Lumo (SVG set)
- Animacoes uniformes (window open/close, focus pulse)

**Saida criterio**: Screenshot 5 apps (Mousepad, Kate, Firefox, Chrome, lumo-files) com identidade visual identica.

## Fase 2 — Sessao Production

**Saida**: Boot → login → sessao persistente → suspend/resume → logout limpo.

Sprint 5:
- Greeter (greetd + lumo-greeter cliente). Login PAM.
- Lock screen production (lumo-lock hardened. LOCK-RUNTIME errors investigados.)
- Idle pipeline: dim brightness → screen off → suspend
- Session save: snapshot windows abertos + restore on login (xdg activation tokens)

Sprint 6:
- Power management profiles (Performance / Balanced / Power saver)
- Auto-mount removable drives via udisks2
- Bluetooth manager (bluez integration)
- Audio config (pipewire + wireplumber profiles)
- Network manager UI completo

**Saida criterio**: Cold boot ate login screen <8s. Lock/unlock funcional. Suspend recovery <2s.

## Fase 3 — Apps Essenciais

**Saida**: Daily-driver completo sem precisar instalar app externa.

Sprint 7 (paralelo com F4 Sprint 9):
- Files: drag-drop entre painels, mounts udisks2, busca recursiva, tags, trash
- Settings: paineis displays/sound/network/power/keyboard/mouse/accessibility
- Terminal nativo (vte ou cosmic-term port)

Sprint 8:
- Editor: syntax highlighting (tree-sitter), find-replace, multi-tab
- Notes: backlinks, search, export
- Calc: scientific mode + history persistente
- Browser default: Firefox config + extensions (uBlock Origin pre-installed)
- Mail/IM bridges (optional via flatpak)

**Saida criterio**: Luiz instala Lumo OS limpa + faz 1 semana de trabalho sem instalar pacote extra.

## Fase 4 — Distribuicao

**Saida**: Outra pessoa instala Lumo OS via ISO em <15min.

Sprint 9:
- archiso ISO buildscript (PKGBUILD + profiles)
- First-run wizard (locale, timezone, user creation, theme, wifi)
- Installer (rsync rootfs + grub + systemd config)
- Update mechanism: pacman wrapper + commit-based rollback (snapper btrfs)

Sprint 10:
- Branding: logo, splash boot, login bg
- Driver opt-in: nvidia/amd via DKMS modules
- Hardware probe: detect Galaxy Book 4 → enable specific tweaks
- AUR/repo Lumo public (pkg mirror)

**Saida criterio**: ISO boota em 3 hardware diferentes (Galaxy Book 4 + 1 desktop + 1 VM) sem hang.

## Fase 5 — Samsung Pitch

**Saida**: Material pra Samsung negociar white-label / OEM.

Sprint 11:
- Demo video 60s ("Lumo em 60s")
- Walkthrough 5min completo
- Benchmark documentado: frame time vs XFCE/macOS/Win11, RAM idle, boot time, battery 4h workload
- Pitch deck PDF (estrutura: problema, solucao, mercado, tech, roadmap, ROI)
- White-label proposal: customizacao por OEM (tema, branding, app list)

Sprint 12:
- Samsung Galaxy Book 4 hardware certification (boot tempo, drivers, sensors, ALS)
- Touch + pen drawing tests (S Pen integration backlog)
- Sound profile Galaxy speakers
- Knox coexistence study (security boundaries)

**Saida criterio**: Pitch enviado pra contato Samsung Brasil + Korea HQ.

## Fase 6 — Public Beta

**Saida**: 100 beta testers fora time interno.

Sprint 13:
- Licenca decisao final (closed-source default por ADR; reconsiderar pra open quando comunidade)
- Code-of-conduct + contributing.md (se open)
- Beta program: form + selecao + slack/discord
- Press kit (Phoronix, OMGUbuntu, etc se open)
- External security audit (Trail of Bits ou similar)

Sprint 14:
- Public website (lumo-os.com landing + docs)
- Telemetry opt-in upload (anonymized, sanitized via lumo-bridge endpoint)
- Issue tracker public (GitHub/Gitea)
- v1.0 release

**Saida criterio**: 100 instalacoes ISO + telemetria mostra <0.5% crash/dia + sessoes >2h avg.

## Critical path

```
F0 → F1 → F2 → F3 ─┐
                   ├→ F5 → F6
              F4 ──┘
```

F3 + F4 paralelos. F5 espera F3 + F4. F6 final.

## Cadencia

- 2 semanas por sprint
- Cada sprint = 1 PR principal + 3-5 PRs auxiliares
- Demo internal final sprint + ajuste roadmap
- Maximo 1 fase ativa por vez

## Calendar otimista (2 semanas/sprint)

| Sprint | Fase | Janela |
|---|---|---|
| 1-2 | F0 | Jun 2026 |
| 3-4 | F1 | Jul 2026 |
| 5-6 | F2 | Ago 2026 |
| 7-8 | F3 | Set 2026 |
| 9-10 | F4 | Set-Out 2026 |
| 11-12 | F5 | Nov 2026 |
| 13-14 | F6 | Dez 2026 |

**Targets**:
- Beta interno: Set 2026 (apos F2)
- Samsung pitch: Dez 2026
- v1.0 publica: Q1 2027

## Riscos majores

- **XWayland complexidade** smithay = 30% delay possivel F0
- **Chrome cursor + gesture forward** = scope F0 pode estourar
- **libdecor plugin custom** = scope F1 alto (C cdylib + render)
- **Greeter + PAM** = scope F2 desconhecido
- **Samsung resposta** = F5/F6 dependem de fora
- **Comunidade open** vs closed = decisao binaria F6

## Definicoes

- **Critical bug**: crash compositor / sessao morre / dados perdidos
- **High bug**: feature core nao usavel
- **Medium bug**: workaround existe
- **Low bug**: cosmetico
- **Sprint completed**: PR merged + tests pass + doc updated + demo internal
- **Fase completed**: criterio saida atingido

## Reavaliacao

Atualizar este doc no fim de cada sprint:
- Move done items
- Adiciona novos issues descobertos
- Re-prioriza se necessario
- Atualiza calendar

Sem reavaliacao = doc fica stale e perde poder de decisao.
