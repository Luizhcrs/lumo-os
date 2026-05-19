# Video Demo 90s — Roteiro Time-Coded

Versao: W7 / 2026-05-19
Duracao: 1m30s (90 segundos)
Formato alvo: 1920x1080, 60fps, sem naracao ao vivo (legenda baked)

---

## Instrucoes de Gravacao

### Ambiente requerido
- Sessao Lumo OS no TTY3 via DRM-KMS (feature `drm-backend`)
- Variavel: `LUMO_THEME=dark` (contraste melhor em video)
- Resolucao: 1920x1080 nativa do painel eDP-1
- Apps pre-lancados mas minimizados: lumo-files, lumo-monitor, lumo-calc

### Gravacao com wf-recorder
```bash
# Gravar tela completa
wf-recorder -f lumo_demo_90s.mp4 -c libx264 -x yuv420p -r 60

# Para quando atingir 90s ou via Ctrl+C
# Verificar duracao:
ffprobe -v quiet -show_entries format=duration -of csv=p=0 lumo_demo_90s.mp4
```

### Gravacao com OBS (alternativa)
- Source: Screen Capture (Wayland) ou pipewire screen capture
- Encoder: x264, CRF 18, preset slow
- Audio: desligar (demo sem audio)
- Output: 1920x1080 @ 60fps

### Pos-producao minima
```bash
# Adicionar legendas baked via ffmpeg (arquivo .srt gerado a partir deste roteiro)
ffmpeg -i lumo_demo_90s.mp4 -vf subtitles=subtitles.srt -c:v libx264 lumo_demo_final.mp4

# Compressao para envio
ffmpeg -i lumo_demo_final.mp4 -c:v libx264 -crf 23 -preset slow lumo_demo_samsung.mp4
```

---

## Roteiro Time-Coded

### 0:00 — 0:08 | Tela preta -> boot curtain

**Visual:** Tela preta. Aparecer logotipo "Lumo OS" centralizado, fade in suave.
**Legenda:** "Lumo OS — ambiente de desktop Wayland nativo em Rust"
**Acao:** Iniciar compositor (curtain ja deve estar visivel do boot real, ou cortar para momento pos-curtain)
**Notas tecnicas:** Boot curtain anima alpha de 1.0 -> 0.0 com delta tempo real (nao frame-coupled).

---

### 0:08 — 0:18 | Desktop + bar

**Visual:** Desktop aparece. Bar visivel no topo: workspace pills, clock, system tray.
**Legenda:** "Compositor Wayland + shell layer-shell — 13 crates Rust"
**Acao:** Mover cursor devagar pela bar. Mostrar workspace pills 1-5. Pill ativa com accent emerald.
**Notas tecnicas:** Nenhuma interacao ainda — so mostrar a composicao visual.

---

### 0:18 — 0:32 | Dropdown de bateria

**Visual:** Clicar no icone de bateria no system tray. Dropdown abre com animacao spring `smooth`.
**Legenda:** "Integracao nativa: charge limit, platform profile, thermal — via sysfs samsung-galaxybook"
**Acao:**
1. Abrir dropdown bateria (0:18)
2. Mostrar: saude 98.9%, ciclos 47, charge limit toggle
3. Clicar "Platform Profile" — trocar de balanced para performance (0:25)
4. Fechar dropdown (0:30)
**Notas tecnicas:** Escrita real em `/sys/firmware/acpi/platform_profile`. Confirmar visualmente no terminal antes de gravar.

---

### 0:32 — 0:48 | Janelas + SSD

**Visual:** Abrir lumo-files. Janela aparece com server-side decorations (titlebar + botao close).
**Legenda:** "Server-side decorations renderizadas pelo compositor — sem dependencia de toolkit"
**Acao:**
1. Abrir lumo-files (0:32)
2. Navegar por alguns diretorios (0:36)
3. Drag na titlebar para reposicionar janela (0:42)
4. Abrir lumo-monitor ao lado (0:45)
**Notas tecnicas:** Drag move via protocolo xdg_shell move. SSD close button e decorativo neste build (P0 pendente — nao clicar no botao close durante a demo).

---

### 0:48 — 1:02 | Troca de tema

**Visual:** Clicar no toggle de tema na bar (ou usar lumoctl).
**Legenda:** "Hot reload Light/Dark via inotify — sem restart, sem flash"
**Acao:**
1. Sistema esta em dark mode (0:48)
2. Clicar toggle tema -> transicao para light mode (0:50)
3. Mostrar janelas, bar, dropdowns em light mode (0:54)
4. Voltar para dark mode (0:58)
**Notas tecnicas:** Evento `ThemeReloaded` via IPC. Verificar se F5 ThemeReloaded hardcoded Light esta corrigido antes de gravar (P2 do code review). Se nao, gravar so dark->light.

---

### 1:02 — 1:16 | Spring physics animations

**Visual:** Abrir e fechar lumo-calc. Mostrar animacoes de entrada/saida de janela.
**Legenda:** "Animacoes spring massa-mola — sem keyframes, driven por delta tempo real"
**Acao:**
1. Abrir lumo-calc — animar entrada com preset `bouncy` (1:02)
2. Mostrar interacao com calc (1:06)
3. Abrir dropdown de brilho — mostrar animacao smooth (1:10)
4. Ajustar brilho com slider (1:13)
**Notas tecnicas:** Spring presets: `bouncy` (stiffness 300, damping 18) para janelas, `smooth` (stiffness 200, damping 22) para dropdowns.

---

### 1:16 — 1:24 | Workspace switch

**Visual:** Clicar em workspace pill 2. Apps ficam no workspace 1. Workspace 2 limpo.
**Legenda:** "Workspaces via IPC sub-ms — SetWorkspace compositor direto"
**Acao:**
1. Clicar pill 2 (1:16) — workspace vazio, animacao transicao
2. Abrir lumo-launcher (tecla launcher ou lumoctl) (1:18)
3. Digitar "monitor" no fuzzy search (1:20)
4. Fechar launcher (1:22)
**Notas tecnicas:** Launcher e M1 — se nao disponivel, pular esta cena e redistribuir tempo.

---

### 1:24 — 1:30 | Encerramento

**Visual:** Volta para desktop com multiplas janelas. Fade out suave para preto.
**Legenda:** "M4 — Galaxy Book 4 U300 nativo — Nov 2026"
**Sub-legenda:** "luizhcrs@gmail.com"
**Acao:** Nenhuma interacao. So mostrar o desktop completo funcionando.

---

## Cues de Apresentacao (demo ao vivo)

Se apresentando ao vivo (nao video pre-gravado):

| Momento | Cue | Contingencia |
|---------|-----|-------------|
| Antes de comecar | Verificar que lumo-wm esta em DRM real (nao nested) | Se nested, mencionar explicitamente |
| 0:32 | Nao clicar no botao close (P0 pendente) | Usar Alt+F4 ou lumoctl kill se necessario |
| 1:02 | Spring animation pode variar frame rate | Ter wf-recorder gravando para fallback |
| Qualquer travamento | `lumoctl restart-bar` no segundo terminal | Ter TTY2 com terminal aberto |

---

*Gerado em: 2026-05-19 | Wave 7 Samsung pitch material*
