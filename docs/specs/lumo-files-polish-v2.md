# Lumo Files Polish v2 — specification

Status: in progress. Owner: luizhcrs. Target: 2026-05-19.
Stack: Iced 0.13 + lumo-foundation LFTokens bridge.

Goal: take the file manager from "feio e incompleto" to a polished, daily-driver-quality experience consistent with the Lumo design language (Material M3 + Lumo ease curves, accent emerald, light + dark, zero neon/glow).

## Design constraints (hard rules)

- Zero emoji in source, comments, docs, commit messages, README.
- Zero Apple/macOS/SwiftUI references — Samsung is the pitch target.
- Zero neon/glow — only neutral black drop shadows + solid accent. Box-shadow with accent color is banned.
- Light and dark theme both supported. Theme passed via `LumoTheme::variant()` snapshot at view time — no env reads in render path.
- Tokens come from `lumo-foundation::LumoColors` (bg / bg_subtle / fg / fg_subtle / accent / accent_subtle / border / pill_bg / pill_fg / pill_sep). Hardcoded RGB only in `theme.rs`.
- Animations follow Lumo ease: 200–280 ms cubic-bezier(0.32, 0.72, 0.0, 1.0). Iced 0.13 has no animation primitives for arbitrary state, so we limit animation to text_input cursor and built-in scroll inertia. State transitions (hover/active) are instant — Iced re-renders on event.
- SVG inline via `include_bytes!` from `apps/lumo-files/icons/`. No filesystem lookup at runtime.

## Color tokens (used)

| Token | Light hex | Dark hex | Usage |
|------|-----------|----------|-------|
| bg | `#FAFAFA` | `#0F1419` | window bg, content area |
| bg_subtle | `#F0F0F2` | `#1F2024` | sidebar, toolbar, hover wash |
| fg | `#18181B` | `#F5F5F7` | primary text |
| fg_subtle | `#6B7280` | `#9CA3AF` | secondary text |
| accent | `#10B981` | `#10B981` | active state, sort indicator |
| accent_subtle | `#10B98120` | `#10B98130` | hover pill, selected row bg |
| border | `#E5E7EB` | `#2A2A2E` | separators |
| shadow | `#00000040` | `#00000060` | dropdown / dialog elevation |

Implementation: re-uses `LumoTheme` bridge (already in `theme.rs`); new helpers add `bg_subtle()`, `border()`, `shadow()`, `accent_subtle()`, `accent_10()` (10% opacity hover wash).

## Module layout (after polish)

```
apps/lumo-files/src/
  app.rs           — App, Message, update, view orchestration
  main.rs          — entry
  theme.rs         — LumoTheme bridge + helpers (extended)
  icons.rs         — SVG bytes + IconKind + handle helpers (extended)
  sidebar.rs       — build + render sidebar
  toolbar.rs       — nav + view toggle + search (polished)
  breadcrumb.rs    — segments + chevron-right separator
  tabs.rs          — NEW: tab bar render + active underline
  statusbar.rs     — NEW: bottom status bar (items, selection, free disk)
  ctxmenu.rs       — NEW: floating context menu render
  toast.rs         — NEW: bottom-right toast queue + auto-fade
  filelist.rs      — state machine (sort, filter, selection, rename)
  filelist_view.rs — NEW: list + grid render isolated from app.rs
  ops.rs           — IO ops (unchanged)
  thumbs.rs        — thumbnail cache (unchanged)
  appmenu.rs       — dbusmenu (unchanged)
```

Net change: app.rs shrinks (extract render to filelist_view / tabs / statusbar / ctxmenu / toast).

## 1. Sidebar

- 220 px fixed width (was 180, too tight for "Documentos" label).
- Group headers `INICIO`, `DRIVES`, `SISTEMA`: 10 px, fg_subtle, letter-spacing visual via uppercase, padding `[10, 12]`.
- Item row: padding `[6, 10]`, row spacing 10 px, height ~32 px.
- Item icon: 16 px SVG, currentColor = fg_subtle (inactive) / accent (active).
- Item label: 13 px medium, fg (inactive) / fg + accent left-bar (active).
- Hover: bg_subtle pill, radius 8 px.
- Active: accent_subtle bg + 3 px accent vertical bar on the left edge (using a 3 px container, not border-left to avoid Iced layout shift).
- Scroll smooth when overflow.

## 2. Toolbar

- Height 44 px (was ~32, cramped).
- Layout: `[ back fwd up refresh ]  [ breadcrumb (flex) ]  [ search input ]  [ view-toggle ]`.
- Icon buttons: 32 × 32, 16 px icon, radius 8 px, hover bg_subtle, padding `[6, 8]`.
- Disabled state: 40% opacity, no hover.
- Search input: 220 px when expanded, prefix search icon (12 px), placeholder "Buscar nesta pasta…", radius 8, bg = bg_subtle, padding `[6, 10]`.
- View toggle: 2-segment switch (list / grid). Active segment has accent_subtle bg + accent fg.
- Bottom 1 px hairline border using `border` token.

## 3. Breadcrumb

- Each segment is a button. 13 px, fg_subtle inactive, fg active (last).
- Separator: chevron-right SVG 12 px, fg_subtle, no padding.
- Hover segment: bg_subtle pill radius 6 px.
- Smart truncation: if total width estimate > 60 chars, drop middle segments and show `..` between root and last-3. (Heuristic; no Iced layout measurement available.)

## 4. Filelist — list view

- Header row: 32 px, sticky-like (just a top container with `border` bottom).
- Columns: Nome (flex) / Tamanho (96) / Modificado (160) / Tipo (80).
- Sort indicator: chevron-up / chevron-down 10 px next to active column label.
- Row height 32 px, padding `[6, 12]`.
- Hover: bg_subtle.
- Selected: accent_subtle bg, 2 px left accent bar (consistent with sidebar active).
- Icon prefix: 14 px SVG from `icon_handle_for_kind(kind)`. Folder, FileText, FileImage, FileVideo, FileAudio, FileArchive, FileCode, FilePdf, FileGeneric.
- Size: human, 1 decimal. `--` for folders.
- Modificado: relative "X minuto(s) atras", "X hora(s) atras", "X dia(s) atras" if < 7 days else absolute `YYYY-MM-DD HH:MM`.
- Tipo: extension uppercase or "Pasta".

## 5. Filelist — grid view

- Cell 112 × 128, padding `[12, 8]`.
- Image thumb 96 × 96 area centred, real Lanczos3 thumb when available.
- Folder uses `folder.svg` at 56 px tinted accent_subtle on bg, with the folder icon centered.
- Label: 11 px, fg, max 2 lines (Iced clips; we truncate to 18 chars).
- Hover: bg_subtle pill radius 12.
- Selected: accent_subtle pill radius 12.
- Responsive: cols = max(3, available_w / 120). We approximate via fixed 7 cols since Iced 0.13 grid is a row-of-rows.

## 6. Tabs

- Tab bar 36 px tall, just below toolbar, bg = bg_subtle (sub-toolbar surface).
- Tab pill: padding `[8, 14]`, radius 8 top corners only, gap 2 px.
- Inactive: fg_subtle text, transparent bg.
- Active: fg text, bg = bg, plus 2 px accent underline.
- Close button on hover only (Iced limitation: we show always at 60% opacity, full on hover-equivalent which is just always-on with neutral color).
- `+` button at far right with `plus.svg`.

## 7. Status bar (bottom)

- Height 26 px, padding `[4, 12]`, bg = bg_subtle, border-top 1 px.
- Text: 11 px fg_subtle.
- Format: `"N itens"` or `"N de M selecionados"` + separator dot + `"X livre de Y"`.
- Free space computed via `statvfs` for current_dir; cached 5 s.

## 8. Empty state

- Folder icon 64 px fg_subtle.
- Text "Esta pasta esta vazia" 13 px fg.
- Subtext "Arraste arquivos para aqui ou use Ctrl+N para criar uma pasta" 11 px fg_subtle.
- Centred vertically and horizontally.

## 9. Loading state

- Skeleton: 6 placeholder rows in list view, 12 placeholder cells in grid.
- Each placeholder is a container bg_subtle with rounded 6 px.
- No spinner — progressive reveal when `DirLoaded` arrives.
- Shown when `loading == true` (new App field).

## 10. Right-click context menu

- Floating panel: bg = bg, border 1 px border token, radius 10 px, shadow neutral.
- Item: padding `[8, 14]`, 13 px fg, hover bg_subtle.
- Separator: 1 px border between groups (Open / Edit / Trash / Properties).
- Items: Abrir, Abrir com…, Renomear (F2), Copiar (Ctrl+C), Recortar (Ctrl+X), Colar (Ctrl+V), Mover para Lixeira (Del), Propriedades (Ctrl+I).

## 11. Toasts

- Bottom-right anchor. Position computed via padding inside root container.
- Toast: bg = bg, border 1 px border, radius 10 px, padding `[10, 14]`, max-width 320 px.
- Text: 12 px fg. Icon prefix per kind (info / warning / error).
- Auto-fade after 4 s. Iced has no opacity animation; we just dismiss the toast on a `Tick` after 4 s.
- Queue: max 3 visible, FIFO drop.

## Test plan

Add `tests` covering:
1. `theme::accent_subtle` returns 10 % alpha (light + dark).
2. `tabs::Tab` add/close/switch invariants.
3. `statusbar::format_status(0/N, 12/100)` -> "12 de 100 selecionados" string.
4. `filelist::human_modified_relative` returns "minuto(s) atras" / "dia(s) atras" boundaries.
5. `toast::ToastQueue::push` keeps max 3 most recent.
6. `ctxmenu::items_for_item` and `items_for_area` return expected message variants.

## Items deferred to v3

- True drag-and-drop reorder of tabs (Iced 0.13 lacks drag events).
- Animated transitions (fade/slide on tab open, dialog show). Iced 0.13 has no `animation::value` for arbitrary state; would need a custom widget or wait for 0.14.
- Multi-pane / split view side-by-side.
