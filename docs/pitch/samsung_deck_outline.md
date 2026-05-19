# Samsung Pitch — Deck Outline (10 slides)

Versao: W7 / 2026-05-19
Destinatario: equipe tecnica Samsung BR / diretoria de plataforma
Objetivo: pitch white-label Lumo OS para linha Galaxy Book

---

## Slide 1 — Capa

**Titulo:** Lumo OS: ambiente de desktop Wayland de nova geracao para Galaxy Book

**Bullets:**
- Ambiente construido inteiramente em Rust + Wayland-native
- Hardware target primario: Galaxy Book 4 U300
- Status: M0 (2026-05-25) — compositor funcional, demos disponiveis

**Notas:** Abrir com demo ao vivo se possivel. Nao comecar com slides.

---

## Slide 2 — O Problema

**Titulo:** Desktops Linux existentes nao foram projetados para OEM

**Bullets:**
- GNOME/KDE: projetados para distro generica, nao hardware especifico
- Hyprland: excelente desempenho, mas sem suporte OEM, sem integracao Samsung Knox
- Pipelines de cor incorretas introduzem banding em paineis FRC 6-bit
- Footprint de memoria alto: GNOME idle ~600MB+, KDE idle ~400MB+
- Nenhum framework oferece path claro para integracao com samsung-galaxybook driver

**Notas:** Mencionar experiencia com Galaxy Book 4 U300 especificamente. Citar problema do banding como diferencial observavel.

---

## Slide 3 — Solucao

**Titulo:** Compositor Wayland proprio + shell integrado ao hardware

**Bullets:**
- `lumo-wm`: compositor Wayland DRM-KMS em Rust com smithay 0.7
- Pipeline cor sRGB correta: texturas importadas como SRGB8_ALPHA8, output demultiplied
- `lumo-sensors`: leitura nativa sysfs (bateria, brilho, perfil de plataforma, lid switch)
- IPC proprio via Unix socket: latencia sub-ms entre compositor e shell
- Hot reload de tema via inotify: troca Light/Dark sem restart

**Notas:** Mostrar pipeline cor: tile branca no canto, comparar com Hyprland se possivel.

---

## Slide 4 — Arquitetura

**Titulo:** 13 crates Rust, separacao clara de responsabilidades

**Bullets:**
- `lumo-wm` (compositor) + `lumo-bar` / `lumo-desktop` (shell layer-shell)
- Camadas: Foundation -> Graphics -> UI -> Compositor -> Shell -> Apps
- Dois backends: Winit (dev nested) e DRM-KMS (producao TTY)
- Protocols suportados: xdg_shell, wlr-layer-shell, xdg-decoration, linux-dmabuf, fractional-scale e mais 10
- 113 arquivos Rust / ~24.000 linhas de codigo

**Notas:** Mostrar diagrama de fluxo libinput -> lumo-wm -> DRM-KMS. Enfatizar que tudo e Rust: sem C unsafe direto no caminho critico.

---

## Slide 5 — Integracao Hardware Galaxy Book 4

**Titulo:** Controles nativos via samsung-galaxybook driver

**Bullets:**
- Charge limit 80%: `/sys/class/power_supply/BAT1/charge_control_end_threshold`
- Platform profile 4 modos: low-power / quiet / balanced / performance
- Lid close handler: SW_LID via evdev, dim + suspend em 3s
- 9 thermal zones monitoradas + cooling devices
- Battery health display: 98.9% saude no SKU de teste (47 ciclos)
- Integracao futura: Samsung Knox coexistence (M4)

**Notas:** Demo ao vivo do dropdown de bateria mostrando charge limit toggle e perfil de plataforma.

---

## Slide 6 — Performance (targets M2)

**Titulo:** Targets de performance documentados, a medir em hardware real

**Bullets:**
- Frame time target p95: < 16.7ms (budget 60Hz) — [MEDIR em DRM real]
- Input-to-pixel target p95: < 16ms — [MEDIR em DRM real]
- RAM RSS shell completo target: < 500MB — [MEDIR em DRM real]
- `PerfTracker` implementado: log p50/p95/p99 em microsegundos a cada 60s
- Script `perf-baseline.sh` para captura reproducivel de 5 minutos

**Notas:** Ser honesto: numeros sao targets, medicao em DRM real acontece em M2 (2026-07-20). Mostrar o script e a instrumentacao — isso e diferencialmente maduro para o estagio do projeto.

---

## Slide 7 — Apps Core

**Titulo:** Suite de aplicativos nativos em desenvolvimento

**Bullets (apps existentes no repositorio):**
- `lumo-files`: gerenciador de arquivos
- `lumo-monitor`: monitor de sistema
- `lumo-calc`: calculadora
- `lumo-launcher`: launcher fuzzy search
- `lumo-term`: terminal baseado em foot (M3)
- `lumo-settings`: painel de configuracoes (M3)
- `lumo-notes`, `lumo-editor`, `lumo-dock`, `lumo-notif`: em desenvolvimento

**Notas:** Abrir lumo-files + lumo-monitor ao vivo durante apresentacao.

---

## Slide 8 — Roadmap

**Titulo:** M0 a M4: Galaxy Book 4 rodando Lumo OS nativo em Nov 2026

**Bullets:**
- M0 — Docs + Demo Video: **2026-05-25** (em curso)
- M1 — Shell Completo (dock, launcher, notificacoes): **2026-07-06**
- M2 — Performance Baseline documentado: **2026-07-20**
- M3 — Apps Core (lumo-term, lumo-settings, fingerprint, hotkeys Fn): **2026-10-12**
- M4 — Samsung Pitch Ready (demo hardware, docs OEM, benchmark, proposta Knox): **2026-11-30**

**Notas:** M4 e o momento ideal para discussao de contrato. M0-M3 sao marcos tecnicos mensurave -- cada um com criterios de aceite documentados.

---

## Slide 9 — Proposta White-Label

**Titulo:** Tres modalidades de parceria

**Bullets:**
- **A. OEM Integration**: Samsung customiza tokens de design + integra Knox; Lumo fornece base tecnica
- **B. Co-desenvolvimento**: Samsung contribui com engenheiros para M2-M4; roadmap alinhado com linha Galaxy Book
- **C. Licenciamento**: licenca proprietaria Lumo OS para uso interno Samsung em SKUs especificos

**Diferenciais vs alternativas:**
- Footprint menor que GNOME/KDE
- Pipeline cor correta para paineis Samsung FRC 6-bit
- Driver samsung-galaxybook ja integrado no stack
- Rust: sem CVEs de buffer overflow no caminho critico

**Notas:** Nao fixar preco neste slide. Proximo passo: reuniao tecnica para avaliar integracao Knox.

---

## Slide 10 — Proximo Passo

**Titulo:** Demo hands-on + reuniao tecnica

**Bullets:**
- Demo ao vivo: compositor + shell + apps rodando em Galaxy Book 4 U300
- Documentacao tecnica disponivel: ARCHITECTURE.md, UX_GUIDELINES.md, M2_PERF_BASELINE.md, ROADMAP.md
- Repositorio privado disponivel para due diligence tecnica
- Contato: Luiz Henrique Cavalcanti Ramos da Silva — luizhcrs@gmail.com

**Call to action:** Agendar reuniao tecnica antes de M1 (2026-07-06) para alinhar requisitos OEM

**Notas:** Deixar 10 minutos para perguntas tecnicas. Ter ARCHITECTURE.md aberto no terminal.

---

*Gerado em: 2026-05-19 | Wave 7 Samsung pitch material*
