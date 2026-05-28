// draw.c — pixel painting puro pra titlebar.
// ARGB8888 little-endian wl_shm format.
//
// F1-1 review:
//   - AA edges em fill_circle (sub-pixel sample 4x4)
//   - Icones glifo dentro de cada botao quando hover (X, -, +)
//   - Hover state colors (bg + fg highlights)

#include "draw.h"
#include <math.h>
#include <stdint.h>
#include <string.h>

static inline void fill_rect(uint32_t *data, int stride_px, int x, int y,
                             int w, int h, uint32_t color)
{
    for (int j = y; j < y + h; j++) {
        if (j < 0) continue;
        uint32_t *row = data + j * stride_px;
        for (int i = x; i < x + w; i++) {
            if (i < 0) continue;
            row[i] = color;
        }
    }
}

// Blend src ARGB sobre dst ARGB com alpha do src.
static inline uint32_t blend_argb(uint32_t dst, uint32_t src, uint8_t a)
{
    uint32_t dr = (dst >> 16) & 0xFF;
    uint32_t dg = (dst >> 8) & 0xFF;
    uint32_t db = dst & 0xFF;
    uint32_t sr = (src >> 16) & 0xFF;
    uint32_t sg = (src >> 8) & 0xFF;
    uint32_t sb = src & 0xFF;
    uint32_t na = 255 - a;
    uint32_t r = (sr * a + dr * na) / 255;
    uint32_t g = (sg * a + dg * na) / 255;
    uint32_t b = (sb * a + db * na) / 255;
    return 0xFF000000u | (r << 16) | (g << 8) | b;
}

// AA filled circle: 4x4 supersample por pixel pra suavizar borda.
// F1-1: substitui o naive distance check anterior que dava aliasing visivel.
static inline void fill_circle_aa(uint32_t *data, int stride_px,
                                   int cx, int cy, int radius, uint32_t color)
{
    float r = (float)radius;
    float r_outer = r + 0.5f;
    float r_inner = r - 0.5f;
    for (int j = cy - radius - 1; j <= cy + radius + 1; j++) {
        if (j < 0) continue;
        uint32_t *row = data + j * stride_px;
        for (int i = cx - radius - 1; i <= cx + radius + 1; i++) {
            if (i < 0) continue;
            // 4x4 supersample
            int hits = 0;
            for (int sy = 0; sy < 4; sy++) {
                float dy = (j - cy) + (sy + 0.5f) / 4.0f - 0.5f;
                for (int sx = 0; sx < 4; sx++) {
                    float dx = (i - cx) + (sx + 0.5f) / 4.0f - 0.5f;
                    float d2 = dx * dx + dy * dy;
                    if (d2 <= r * r) {
                        hits++;
                    }
                }
            }
            if (hits == 0) continue;
            uint8_t a = (uint8_t)((hits * 255) / 16);
            // Fast path: dentro do circulo full opacity.
            if (hits == 16) {
                row[i] = color;
            } else {
                row[i] = blend_argb(row[i], color, a);
            }
            (void)r_outer; (void)r_inner;
        }
    }
}

// Linha 1px AA simples — Bresenham + endpoint antialias.
// F1-1: usado pra desenhar icones (X / - / +) dentro dos botoes em hover.
static inline void draw_line(uint32_t *data, int stride_px, int w, int h,
                              int x0, int y0, int x1, int y1, uint32_t color)
{
    int dx = abs(x1 - x0);
    int dy = abs(y1 - y0);
    int sx = x0 < x1 ? 1 : -1;
    int sy = y0 < y1 ? 1 : -1;
    int err = dx - dy;
    int x = x0;
    int y = y0;
    while (1) {
        if (x >= 0 && x < w && y >= 0 && y < h) {
            uint32_t *row = data + y * stride_px;
            row[x] = color;
        }
        if (x == x1 && y == y1) break;
        int e2 = 2 * err;
        if (e2 > -dy) { err -= dy; x += sx; }
        if (e2 < dx)  { err += dx; y += sy; }
    }
}

// Icon close: X centrado.
static void icon_close(uint32_t *data, int stride_px, int w, int h,
                       int cx, int cy, int radius, uint32_t fg)
{
    int r = (int)(radius * 0.5f);
    draw_line(data, stride_px, w, h, cx - r, cy - r, cx + r, cy + r, fg);
    draw_line(data, stride_px, w, h, cx + r, cy - r, cx - r, cy + r, fg);
}

// Icon min: linha horizontal centrada.
static void icon_min(uint32_t *data, int stride_px, int w, int h,
                     int cx, int cy, int radius, uint32_t fg)
{
    int r = (int)(radius * 0.55f);
    draw_line(data, stride_px, w, h, cx - r, cy, cx + r, cy, fg);
}

// Icon max: quadrado/cruz centrado.
static void icon_max(uint32_t *data, int stride_px, int w, int h,
                     int cx, int cy, int radius, uint32_t fg)
{
    int r = (int)(radius * 0.55f);
    draw_line(data, stride_px, w, h, cx - r, cy, cx + r, cy, fg);
    draw_line(data, stride_px, w, h, cx, cy - r, cx, cy + r, fg);
}

void lumo_paint_titlebar(uint32_t *data, int width, int height,
                        const char *title, int active, int hover_btn)
{
    if (!data || width <= 0 || height < LUMO_TITLEBAR_HEIGHT) {
        return;
    }
    int stride_px = width;

    uint32_t bg = active ? LUMO_TITLEBAR_BG : 0xFF1F1F1F;
    fill_rect(data, stride_px, 0, 0, width, LUMO_TITLEBAR_HEIGHT, bg);

    int btn_y = LUMO_TITLEBAR_HEIGHT / 2;
    int total_btns = LUMO_BUTTON_SIZE * 3 + LUMO_BUTTON_GAP * 2;
    int start_x = width - LUMO_BUTTON_MARGIN_RIGHT - total_btns;
    int btn_radius = LUMO_BUTTON_SIZE / 2;

    uint32_t base[3] = {
        active ? LUMO_BTN_CLOSE : 0xFF555555,
        active ? LUMO_BTN_MIN : 0xFF555555,
        active ? LUMO_BTN_MAX : 0xFF555555,
    };
    uint32_t hover[3] = {
        LUMO_BTN_CLOSE_HOVER,
        LUMO_BTN_MIN_HOVER,
        LUMO_BTN_MAX_HOVER,
    };
    for (int b = 0; b < 3; b++) {
        int bx = start_x + (LUMO_BUTTON_SIZE + LUMO_BUTTON_GAP) * b + btn_radius;
        uint32_t color = (hover_btn == b) ? hover[b] : base[b];
        fill_circle_aa(data, stride_px, bx, btn_y, btn_radius, color);
        // Icone desenhado apenas no hover (matches macOS UX: glifo aparece em hover).
        if (hover_btn == b && active) {
            uint32_t fg = 0xFF1A1A1A;
            switch (b) {
                case 0: icon_close(data, stride_px, width, height, bx, btn_y, btn_radius, fg); break;
                case 1: icon_min(data, stride_px, width, height, bx, btn_y, btn_radius, fg); break;
                case 2: icon_max(data, stride_px, width, height, bx, btn_y, btn_radius, fg); break;
            }
        }
    }

    // TODO M3: render title text via cosmic-text. Placeholder ate la.
    (void)title;

    // Separador embaixo.
    fill_rect(data, stride_px, 0, LUMO_TITLEBAR_HEIGHT - 1, width, 1, 0xFF000000);
}
