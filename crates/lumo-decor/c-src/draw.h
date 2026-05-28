// draw.h — pixel rendering helpers pra titlebar Lumo.
// Sync com src/lib.rs constants (TITLEBAR_HEIGHT, BUTTON_SIZE, etc).

#ifndef LUMO_DECOR_DRAW_H
#define LUMO_DECOR_DRAW_H

#include <stdint.h>

#define LUMO_TITLEBAR_HEIGHT 32
#define LUMO_BUTTON_SIZE 14
#define LUMO_BUTTON_GAP 8
#define LUMO_BUTTON_MARGIN_RIGHT 12

// Cores Lumo dark theme.
#define LUMO_TITLEBAR_BG 0xFF2A2A2A
#define LUMO_TITLEBAR_FG 0xFFE0E0E0
#define LUMO_BTN_CLOSE 0xFFE74C3C
#define LUMO_BTN_MIN 0xFFF1C40F
#define LUMO_BTN_MAX 0xFF2ECC71

// F1-1 review: hover state colors (highlight no botao com cursor em cima).
#define LUMO_BTN_CLOSE_HOVER 0xFFFF6B5B
#define LUMO_BTN_MIN_HOVER 0xFFFFD93D
#define LUMO_BTN_MAX_HOVER 0xFF52E08C

// Hover index: -1 = nenhum, 0 = close, 1 = min, 2 = max.
#define LUMO_HOVER_NONE -1
#define LUMO_HOVER_CLOSE 0
#define LUMO_HOVER_MIN 1
#define LUMO_HOVER_MAX 2

// Paint titlebar em buffer ARGB8888 (wl_shm).
// data = pointer to first pixel (linear, stride = width * 4)
// width, height = buffer dims (height >= LUMO_TITLEBAR_HEIGHT)
// title = utf-8 nul-terminated, pode ser NULL
// active = 1 quando window has focus (titlebar bg + colored btns)
//          0 quando inactive (greyed out)
// hover_btn = LUMO_HOVER_NONE / _CLOSE / _MIN / _MAX
void lumo_paint_titlebar(uint32_t *data,
                         int width,
                         int height,
                         const char *title,
                         int active,
                         int hover_btn);

#endif // LUMO_DECOR_DRAW_H
