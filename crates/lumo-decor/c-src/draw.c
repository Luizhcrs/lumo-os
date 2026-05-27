// draw.c — pixel painting puro pra titlebar.
// ARGB8888 little-endian wl_shm format.

#include "draw.h"
#include <string.h>

static inline void fill_rect(uint32_t *data, int stride_px, int x, int y,
                             int w, int h, uint32_t color)
{
    for (int j = y; j < y + h; j++) {
        uint32_t *row = data + j * stride_px;
        for (int i = x; i < x + w; i++) {
            row[i] = color;
        }
    }
}

// Anti-aliased filled circle (simples, sem MSAA — sample center distance).
static inline void fill_circle(uint32_t *data, int stride_px,
                                int cx, int cy, int radius, uint32_t color)
{
    int r2 = radius * radius;
    for (int j = cy - radius; j <= cy + radius; j++) {
        if (j < 0) continue;
        uint32_t *row = data + j * stride_px;
        int dy = j - cy;
        int dy2 = dy * dy;
        for (int i = cx - radius; i <= cx + radius; i++) {
            if (i < 0) continue;
            int dx = i - cx;
            int d2 = dx * dx + dy2;
            if (d2 <= r2) {
                row[i] = color;
            }
        }
    }
}

void lumo_paint_titlebar(uint32_t *data, int width, int height,
                        const char *title, int active)
{
    if (!data || width <= 0 || height < LUMO_TITLEBAR_HEIGHT) {
        return;
    }
    int stride_px = width;

    // Fill titlebar bg.
    uint32_t bg = active ? LUMO_TITLEBAR_BG : 0xFF1F1F1F;
    fill_rect(data, stride_px, 0, 0, width, LUMO_TITLEBAR_HEIGHT, bg);

    // Botoes 3 circulos colored a direita.
    int btn_y = LUMO_TITLEBAR_HEIGHT / 2;
    int total_btns = LUMO_BUTTON_SIZE * 3 + LUMO_BUTTON_GAP * 2;
    int start_x = width - LUMO_BUTTON_MARGIN_RIGHT - total_btns;
    int btn_radius = LUMO_BUTTON_SIZE / 2;

    uint32_t colors[3] = {
        active ? LUMO_BTN_CLOSE : 0xFF555555,
        active ? LUMO_BTN_MIN : 0xFF555555,
        active ? LUMO_BTN_MAX : 0xFF555555,
    };
    for (int b = 0; b < 3; b++) {
        int bx = start_x + (LUMO_BUTTON_SIZE + LUMO_BUTTON_GAP) * b + btn_radius;
        fill_circle(data, stride_px, bx, btn_y, btn_radius, colors[b]);
    }

    // TODO: render title text. Requer font system. Placeholder por enquanto
    // pra confirmar build + load funciona. Adicionar via cairo ou cosmic-text
    // em proxima iteracao.
    (void)title;

    // Separador sutil entre titlebar e content (1px linha embaixo).
    fill_rect(data, stride_px, 0, LUMO_TITLEBAR_HEIGHT - 1, width, 1, 0xFF000000);
}
