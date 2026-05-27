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

// Paint titlebar em buffer ARGB8888 (wl_shm).
// data = pointer to first pixel (linear, stride = width * 4)
// width, height = buffer dims (height >= LUMO_TITLEBAR_HEIGHT)
// title = utf-8 nul-terminated, pode ser NULL
// active = 1 quando window has focus (titlebar bg + colored btns)
//          0 quando inactive (greyed out)
void lumo_paint_titlebar(uint32_t *data,
                         int width,
                         int height,
                         const char *title,
                         int active);

#endif // LUMO_DECOR_DRAW_H
