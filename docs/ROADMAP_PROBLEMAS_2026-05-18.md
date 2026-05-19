# Roadmap Problemas Atuais — 2026-05-18

## P0 — Bloqueia uso basico

### 1. Wifi nao muda rede mesmo apos polkit
**Status**: polkit rule + nm_connect con up ja em `a3391e7`. Luiz reporta ainda nao funciona.
**Acao**: log empirico fresh apos click rede saved. Verificar SSID exato passado. Pode ser truncate_ssid retornando truncated em vez de full.

### 2. Right-click apps sem context menu
**Status**: Q2 grab_popup implementado.
**Acao**: testar com mousepad (GTK3 puro). Se funcionar = thunar GTK4 limitacao. Se nao = bug grab.

### 3. Pill bar demora aparecer
**Status**: A39 boot curtain sincroniza apos bar pronto. Mas bar spawn pos compositor = ~500ms delay.
**Acao**: systemd user unit pre-spawn bar antes compositor. Bar idle aguardando WAYLAND_DISPLAY.

## P1 — UX feia mas funcional

### 4. S2 pill dropdown click ainda nao testado empiricamente
**Status**: T1.2 implementou click → IPC CloseFocusedToplevel.
**Acao**: testar empirico.

### 5. S1 titlebar menu render
**Status**: T1.1 implementou menu 5 itens.
**Acao**: testar empirico apps SSD (Qt5).

### 6. Cantos bottom janelas ainda quadrados?
**Status**: R3 fix dst.size em vez geometry.
**Acao**: testar empirico foot/thunar.

## P2 — Backlog

### 7. Apps GTK4 sem appmenu pill (limitacao protocolo)
**Acao**: pill fallback S2 cobre — mostra AppName ▾ pra TODOS apps.

### 8. Multi-monitor nao testado
**Acao**: pos M1.

### 9. HiDPI hardcoded 1.0
**Acao**: pos M2.

## P3 — Papers research aplicar

### 10. Late-render scheduler presentation-time (-8ms latencia)
### 11. Cursor HW plane atomic async (desacopla cursor FPS)
### 12. Damage merge heuristica (-20-40% draw calls)
