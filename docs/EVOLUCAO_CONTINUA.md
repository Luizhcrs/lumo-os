# Evolucao Continua Lumo OS

Diretriz Luiz 2026-05-18: pesquisar Google a cada wave + aplicar best practices RAM/CPU/GPU/energia.

## Pesquisa permanente (research-driven)

A cada wave, agent-research busca:
- Papers academicos venue: SIGGRAPH, SIGCHI, OSDI, USENIX ATC, UIST, CHI
- Blog posts: emersion, ppaalanen, Hugl/KWin, Collabora, System76, GNOME devs
- Patches/MRs Linux kernel + Mesa
- Docs Wayland-protocols staging recentes

Output: novos paper notes em `docs/research/wave_N_papers.md`.

## Categorias evolucao continua

### RAM gerenciamento

Pendente investigar/aplicar:
- Memory profiling heaptrack/valgrind massif em compositor + apps
- panic=abort + lto=fat (Cargo.toml release profile)
- Arena allocators bumpalo pra short-lived alocacoes (render frame state)
- String interning (lasso crate) pra strings frequentes (paths, names)
- Huge pages 2MB (transparent_hugepage=madvise) pra texture buffers
- Reduzir libstd footprint: no_std parts onde possivel
- Target: shell completo < 200MB residente (Mac < 500MB Finder + Dock)

Aplicacao:
- M1 shell completo: medir RAM antes/depois cada feature
- M2 perf baseline: documentar p50/p95/p99 RAM

### CPU gerenciamento

Pendente investigar/aplicar:
- Per-core affinity P-cores (1) + E-cores (4) Raptor Lake-U U300
- CFS niceness compositor (alta prioridade) vs apps
- Polling vs interrupt input (libinput timer otimo)
- Hot loops branch hints (likely!/unlikely! macros)
- SIMD AVX2 pra blits tiny-skia (futuro)
- Workqueue scheduling

Target: idle CPU < 1% compositor (Mac compositor idle ~0.5%)

### GPU gerenciamento

Roadmap papers ja fila:
- P1 late-render presentation-time (-8ms latencia)
- P2 cursor HW plane atomic async
- P4 damage merge heuristica (-20-40% draw calls)

Pendente investigar:
- i915 atomic plane primary+overlay+cursor assignment optimal
- GLES uniform buffer object pra reduzir draw call setup
- Texture atlas glyphs (lumo-text ja faz)
- Zero-copy dmabuf scanout direct (ja implementado A10)
- Mesa GLES2 vs GLES3 tradeoff
- Vulkan WSI fallback (futuro M3+)
- VRR adaptive sync (laptop panel suporta?)

Target: frame time p95 < 8ms (60Hz = 16.6ms budget)

### Energia gerenciamento

Done:
- charge_control_end_threshold 80% (P5)
- platform_profile cycle 4-modos
- Lid close handler dim + suspend (L5)
- Charge cell balance semanal sex 22h

Pendente investigar/aplicar:
- DPMS off apos N idle (default 5min)
- s2idle vs s3 deep suspend (Galaxy Book 4 specs)
- Wake-on-typing fast resume
- Backlight dim adaptativo (sem ALS — usa hora dia)
- WiFi powersave dynamic
- CPU governor switch (performance quando AC, powersave quando bateria)
- USB autosuspend
- Audio codec D3 idle

Target: idle 8h+ battery (Galaxy Book 4 54Wh / consumo 6W idle = ~9h teorico)

## Workflow autonomo

Cada wave:
1. **Pesquisa** (agent-research, max 30min): 5 refs novas + insights
2. **Plano** (main thread): incorpora insights na wave atual
3. **Implementacao** (agent-dev): codigo + tests
4. **Validacao** (build limpo + screenshot quando aplicavel)
5. **Documentacao** (push docs/ + memory permanente atualizado)

## Memory permanente

Memory ja capturada (em `~/.claude/.../memory/`):
- project-lumo-os: visao + status + crates + filosofia
- feedback-subagent-build-validation-obrigatoria
- feedback-lumo-zero-apple-refs-em-publico
- feedback-design-lapidado
- feedback-zero-neon-glow
- feedback-input-feedback-imediato

Nova memory pendente criar:
- evolucao-continua-research-driven (este doc resumido)
- target-perf-metrics (RAM/CPU/GPU/energia thresholds)

## Da raiz ao visual — camadas otimizaveis

### L1 Kernel + drivers
- samsung-galaxybook (charge_control, platform_profile, firmware-attributes)
- i915 GPU (atomic plane, cursor HW, VRR adaptive sync)
- libinput (touchpad accel, gestures, palm rejection)
- libseat/seatd (TTY session takeover)
- ACPI thermal, lid switch, hotkeys Fn+F*

Evolucao:
- Pesquisar patches kernel mainline pendentes pra Galaxy Book 4
- Investigar drivers proprietarios opcionais (Samsung WMI extras)
- Eventual contribuir patches upstream (credibilidade Samsung pitch)

### L2 Userspace runtime
- systemd (user units pre-spawn — D1 done)
- polkit (rules NM + sensors + power)
- udev (rules LED + backlight)
- tmpfiles (perms boot)
- dbus (Registrar appmenu)
- pipewire (audio futuro)

Evolucao:
- systemd-analyze blame: reduzir boot time
- polkit: agent grafico Lumo proprio (em vez gnome-polkit/lxqt-polkit)
- dbus services: lumo-power, lumo-notify, lumo-settings expose proprio

### L3 Compositor (lumo-wm)
- smithay 0.7 vendored (5 patches sRGB)
- GLES via Mesa
- libinput backend
- DRM/KMS direct + winit nested
- IPC unix socket

Evolucao papers:
- P1 late-render presentation-time
- P2 cursor HW plane atomic
- P4 damage merge heuristica
- Color management v1 protocol (Mesa MR done)
- FIFO v1 / commit-timing v1 protocols
- Vulkan WSI fallback (M3+)

### L4 Foundation/Graphics layer
- lumo-foundation tokens (theme reload L6 done)
- lumo-beam (wgpu wrapper)
- lumo-graphics (SDF quad/shadow)
- lumo-text (cosmic-text atlas)
- lumo-animation (LASpring closed-form P3 done)

Evolucao:
- Atlas dinamico (glyph cache evict LRU)
- Shadow GPU shader (substituir CPU SolidColor)
- Spring presets Material M3 curves
- Texture compression formats (BC7) onde aplicavel

### L5 Shell (lumo-bar/desktop/osd/dock/launcher/notif)
- layer-shell clients
- tiny-skia render SHM
- IPC consume

Evolucao:
- Componentes via trait (refactor abstracao A1-A5 documentado)
- Plugin features lifecycle
- TOML data-driven layout (F1 done)
- Event bus pub/sub (futuro)

### L6 Apps
- lumo-files (MVP done, polish E1.2-6 em curso)
- lumo-term (fork alacritty futuro)
- lumo-settings (Iced futuro)
- lumo-text (fork cosmic-edit futuro)
- lumo-calc/notes/monitor (Iced futuro)

Evolucao:
- DBus appmenu export cada app (E1.1 + replica)
- Shared cargo deps lumo-foundation
- Inter-app drag-drop (xdg-foreign)
- App-shared clipboard history

### L7 Visual/UX
- Tokens emerald + ink_deep + Geist/Inter (R4)
- Curvas Material M3 (curvas proprias)
- Spring physics LASpring real
- Sombras drop classicas
- Animations: dropdown spring, window scale-fade, workspace slide
- Acessibilidade: reduced motion, high contrast (futuro)
- Localizacao: pt-BR default + i18n (futuro)

Evolucao:
- GTK theme Lumo CSS (apps externos respeitam)
- Cursor shape protocol (text/resize/hand)
- Icon theme Lumo proprio (substituir Adwaita)
- Sound theme (futuro)
- Splash boot animado
- Lock screen + greeter (Fase 4)
