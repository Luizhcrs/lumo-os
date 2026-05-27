# Lumo OS — UX Guidelines

## Filosofia

**Input com feedback imediato.** Cada keystroke ou pointer event deve produzir resposta visual no proximo frame. Se o sistema estiver com lag > 100ms, inputs antigos sao descartados — nunca enfileiralos invisivamente.

**Lapidado, nada por acaso.** Cada posicao, escala, timing e token tem justificativa tecnica ou de design. "Parece certo" nao eh criterioda aprovacao. Polir antes de acumular features.

**Sem neon/glow.** Zero `box-shadow` colorido com accent. Sombras sao pretas neutras com alpha. Accent aparece solido, saturacao media.

**Zero emoji** em codigo, docs, commits e UI (salvo conteudo do usuario final).

## Tokens de Design

Definidos em `crates/foundation/lumo-foundation/` (LFColor, LFTokens).

| Token | Valor | Uso |
|-------|-------|-----|
| `accent` | emerald `#34D399` | Pills ativas, toggles on, cursor indicator |
| `ink_deep` | `#0D0D0D` (dark) / `#F5F5F5` (light) | Background primario |
| `ink_mid` | `#1A1A1A` / `#E8E8E8` | Superficies secundarias (dropdowns, cards) |
| `ink_light` | `#2A2A2A` / `#D0D0D0` | Bordas, separadores |
| `text_primary` | `#FFFFFF` / `#111111` | Texto de corpo |
| `text_secondary` | `#A0A0A0` / `#606060` | Labels, metadados |
| `pill_radius` | `14px` | Raio de borda de pills e cards |
| `shadow_alpha` | `0.3` | Alpha de sombras de janela |

## Tipografia

| Papel | Fonte | Tamanho | Peso |
|-------|-------|---------|------|
| UI labels | Geist | 13px | 400 |
| Titulos janela | Geist | 14px | 500 |
| Monospace / code | Geist Mono | 13px | 400 |
| Clock bar | Geist | 14px | 500 |

Shaping via `cosmic-text` com alpha mask only (grayscale AA). Sem subpixel rendering — evita artefatos rainbow no painel FRC 6-bit.

## Animacoes — LASpring Presets

Implementadas em `crates/graphics/lumo-animation/`. Fisica de spring massa-mola amortecida, sem keyframes fixos.

| Preset | Stiffness | Damping | Uso tipico |
|--------|-----------|---------|------------|
| `snappy` | 400 | 28 | Feedback de click, hover |
| `smooth` | 200 | 22 | Transicoes de painel |
| `bouncy` | 300 | 18 | Entrada de janela, dock bounce |
| `interactive` | 600 | 32 | Drag, resize em tempo real |

**Regra**: animacoes driven por delta tempo real — nunca `alpha -= 0.067` por frame (quebra em frequencias diferentes de 60Hz).

## Componentes

### Bar (`lumo-bar`)

Layer-shell `Top`, altura 40px, `exclusive_zone=40`. Regioes:

- Esquerda: workspace pills (1-5), accent na ativa
- Centro: titulo da janela em foco
- Direita: system tray (bateria, wifi, hora, brilho)

Click nas pills: envia `SetWorkspace` via IPC. Click em system tray: abre dropdown correspondente.

**Invariante de tamanho**: surface criada com altura `BAR_HEIGHT + DROPDOWN_H` sempre. Dropdown renderiza abaixo com area transparente quando fechado. Nunca resize dinamico apos init (causa flicker).

### Dropdowns

Abrem "para baixo" da bar. Superficie compartilhada com a bar (area transparente quando fechados). Animacao `smooth` preset na abertura/fechamento.

### OSD (On-Screen Display)

Overlay centralizado para feedback de brilho, volume, platform profile. Aparece 2s e desaparece com animacao `smooth`. Nao recebe input (passthrough).

### Janelas — Server-Side Decorations (SSD)

Titlebars renderizados pelo compositor. Altura 30px, botao close (circulo vermelho 12px) alinhado a esquerda. Drag na titlebar move janela.

**Status atual**: botao close eh decorativo (render implementado, handler click P0 pendente — ver `docs/reviews/code_review_2026-05-18.md`).

### Dock (futuro — M1)

Layer-shell `Bottom`. Icones de apps fixados + apps abertos. Animacao `bouncy` no launch.

## Temas

`light` e `dark`. Toggle via `LUMO_THEME` env ou IPC `SetTheme`. Default: `light`.

Hot reload via filewatcher em `~/.config/lumo/theme.toml` — sem restart.

## Principios de Acessibilidade

- Contraste minimo 4.5:1 (WCAG AA) entre `text_primary` e `ink_deep`.
- Sem informacao transmitida apenas por cor.
- Feedback tativo (animacao spring) complementa feedback visual.
