# Lumo OS — Agent Team

Lumo OS é desenvolvido por 4 agentes especializados com ownership exclusivo.
Nenhum agente toca o código de outro sem autorização explícita.

## Time

| Agente | Persona | Owns | Especialidade |
|--------|---------|------|---------------|
| **NOVA** | Compositor | `crates/compositor/lumo-wm/` | Smithay, DRM, foco, tiling, input |
| **KAI**  | Shell UX   | `shell/` + `apps/lumo-dock/` | Bar, dock, menus, layer-shell |
| **ECHO** | Apps       | `apps/lumo-*/` (apps produto) | Iced, features, UX de produto |
| **ATLAS**| Platform   | `crates/` (libs + lumo-ipc)  | Contrato IPC, libs compartilhadas |

Cada agente tem um `CLAUDE.md` próprio no diretório que ele owns.

## Arquitetura de interfaces

```
                    ┌─────────────────┐
                    │     ATLAS       │
                    │  lumo-ipc       │
                    │  lumo-foundation│
                    │  lumo-animation │
                    │  lumo-kit       │
                    └────────┬────────┘
                             │ contrato
              ┌──────────────┼──────────────┐
              ▼              │              ▼
        ┌──────────┐   IPC socket    ┌──────────┐
        │   NOVA   │ ─────────────── │   KAI    │
        │ lumo-wm  │ LumoCommand →   │  shell/  │
        │          │ ← LumoEvent     │  dock/   │
        └──────────┘                 └──────────┘
              │                            
     Wayland protocol                     
              │                            
        ┌──────────┐
        │   ECHO   │
        │  apps/*  │
        └──────────┘
```

## Regra de ouro

> Se você está editando um arquivo fora do seu `OWNS`, pare e verifique se é realmente necessário.
> Na maioria dos casos, a solução correta é um ticket para o agente dono.

## Sequência de deploy

```
1. ATLAS   → merge em crates/ (contrato disponível)
2. NOVA    → merge em lumo-wm/ (handler implementado)
3. KAI     → merge em shell/ dock/ (consumer implementado)
4. ECHO    → merge em apps/ (independente, pode ser paralelo ao 2/3)
5. Build   → Galaxy: lumo-wm(drm) + lumo-bar + lumo-desktop + lumo-dock
6. Restart → hot-restart compositor → restart systemd services
7. QA      → screenshot + Gemini visual validation
```

## Stack técnica

- **Linguagem:** Rust 2021 edition
- **Compositor:** Smithay (Wayland server)
- **Apps:** Iced 0.13
- **Shell:** tiny-skia + cosmic-text (layer-shell client)
- **IPC:** Unix socket, line-delimited JSON
- **GPU:** GLES2/EGL (DRM backend), Mesa
- **Hardware alvo:** Samsung Galaxy Book 4 (eDP-1, 2560x1600)

## Convenções universais

- Commits: Conventional Commits (`fix(wm):`, `feat(shell):`, `feat(ipc):`)
- Email: `luizhcrs@gmail.com` (nunca @sidi.org.br)
- Build release: sempre `--release` — debug é 10x mais lento
- Sem emojis em código, commits, logs
- Toda mudança com impacto visual: screenshot antes/depois
