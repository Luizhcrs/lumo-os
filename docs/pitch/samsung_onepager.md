# Lumo OS — One Pager Samsung

**Ambiente de desktop Wayland de nova geracao para Galaxy Book**
Luiz Henrique Cavalcanti Ramos da Silva — luizhcrs@gmail.com

---

## Problema | Solucao | Diferencial

| Problema | Solucao Lumo OS | Diferencial vs concorrentes |
|----------|-----------------|----------------------------|
| Desktops Linux genericos (GNOME/KDE) nao integram hardware Samsung especifico | Leitura nativa sysfs via `lumo-sensors`: charge limit, platform profile, lid switch, thermal | GNOME/KDE requerem extensoes de terceiros ou scripts; Lumo tem suporte de primeira classe |
| Pipeline de cor incorreta em paineis FRC 6-bit causa banding visivel | Texturas importadas como `SRGB8_ALPHA8`, shaders output linear->sRGB demultiplied correto | Hyprland/wlroots: pipeline "naive", sem CM. Lumo corrige no nivel do compositor |
| Footprint RAM alto: GNOME idle ~600MB+, KDE ~400MB+ | Target: shell completo < 500MB RSS — [MEDIR em DRM M2] | Sem daemons extras, sem GObject overhead, sem JavaScript runtime |
| Sem path claro para integracao com Samsung Knox | M4 inclui proposta tecnica Knox coexistence e docs OEM | Unico compositor Wayland com roadmap publico de integracao Galaxy Book |
| Compositor crash-prone em C (Mutter, KWin) | Rust: sem CVEs de buffer overflow no caminho critico; `PerfTracker` para observabilidade | Linguagem de sistemas moderna com garantias de seguranca de memoria |

---

## Numeros Reais

| Metrica | Valor | Fonte |
|---------|-------|-------|
| Arquivos Rust | 113 | `find crates/ apps/ -name '*.rs' \| wc -l` |
| Linhas de codigo | ~24.000 | wc -l crates/ apps/ |
| Crates workspace | 13 | ARCHITECTURE.md |
| Apps no repositorio | 9 | `ls apps/` |
| Protocolos Wayland suportados | 14 | ARCHITECTURE.md |
| Saude bateria SKU de teste | 98,9% | `/sys/class/power_supply/BAT1/` validado |
| Ciclos BAT1 SKU de teste | 47 | sysfs empirico 2026-05-18 |
| Platform profiles disponiveis | 4 | sysfs: low-power/quiet/balanced/performance |
| Frame time target p95 | < 16,7ms | M2_PERF_BASELINE.md — [MEDIR M2] |
| RAM RSS target | < 500MB | M2_PERF_BASELINE.md — [MEDIR M2] |

---

## Roadmap

```
2026-05-25        2026-07-06        2026-07-20        2026-10-12        2026-11-30
     |                  |                  |                  |                  |
    M0                 M1                 M2                 M3                 M4
Docs + Demo       Shell Completo    Perf Baseline       Apps Core         Samsung Pitch
Video 60s         Dock+Launcher     p50/p95/p99         lumo-term         Demo Galaxy Book 4
README pub        Notificacoes      RAM medido          Settings          Docs OEM
Licenca           SSD close fix     CI cache            Fingerprint       Knox proposta
```

### O que esta pronto (M0 em curso)

- Compositor Wayland DRM-KMS funcional (~90%)
- Pipeline cor sRGB correta (5 patches smithay vendor)
- Bar layer-shell com workspaces, system tray, dropdowns (~35% shell)
- IPC Unix socket operacional
- Hot reload de tema via inotify
- Integracao sysfs: bateria, brilho, platform profile, lid close
- `PerfTracker` instrumentado (log p50/p95/p99)

---

## Stack Tecnico

- **Linguagem:** Rust 1.x (todo o codebase)
- **Compositor:** smithay 0.7 (fork vendor com 5 patches CM)
- **Backend grafico:** wgpu + GLES2 (Intel UHD Xe G4 48 EU)
- **Text shaping:** cosmic-text + atlas de glifos proprio
- **Animacoes:** spring massa-mola LASpring (sem keyframes)
- **Hardware target:** Galaxy Book 4 U300 — Intel U300, 15.6" FHD IPS
- **Driver Samsung:** samsung-galaxybook (mainline Linux)
- **Licenca:** proprietaria (closed source)

---

## Proposta de Parceria

Tres modalidades negociaveis para reuniao pos-M1:

**A. OEM Integration** — Samsung personaliza tokens de design + Knox; Lumo fornece base tecnica e suporte
**B. Co-desenvolvimento** — Engenheiros Samsung contribuem para M2-M4; roadmap alinhado com linha Galaxy Book
**C. Licenciamento** — Licenca proprietaria para uso interno em SKUs especificos Galaxy Book

Documentacao tecnica disponivel para due diligence: ARCHITECTURE.md, UX_GUIDELINES.md, ROADMAP.md, M2_PERF_BASELINE.md, sensors_galaxy_book4.md.

---

**Contato:** Luiz Henrique Cavalcanti Ramos da Silva — luizhcrs@gmail.com
**Repositorio:** privado — acesso disponivel sob NDA
**Demo:** video 90s disponivel + demo ao vivo apos M0 (2026-05-25)

*Gerado em: 2026-05-19 | Wave 7 Samsung pitch material*
