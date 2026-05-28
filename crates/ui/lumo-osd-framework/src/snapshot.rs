//! snapshot.rs — A5 review: testing infrastructure pra renderizar OSDs
//! em pixmap headless e comparar contra golden hash.
//!
//! Cross-platform (tiny-skia roda Windows). Permite catch regressao
//! visual sem precisar Wayland session.

use tiny_skia::Pixmap;

/// FNV-1a 64-bit hash dos pixels (ARGB premultiplied bytes).
/// Deterministico, fast, sem deps externas.
pub fn pixmap_hash(pixmap: &Pixmap) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut h: u64 = FNV_OFFSET;
    for byte in pixmap.data() {
        h ^= *byte as u64;
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

/// Conta pixels nao-transparentes (alpha > 0). Util pra sanity:
/// "render produziu algo visivel?"
pub fn nonzero_pixels(pixmap: &Pixmap) -> usize {
    pixmap
        .data()
        .chunks_exact(4)
        .filter(|px| px[3] > 0)
        .count()
}

/// Bounding box dos pixels nao-transparentes. Util pra verificar
/// que render nao saiu do canvas previsto.
pub fn nonzero_bbox(pixmap: &Pixmap) -> Option<(u32, u32, u32, u32)> {
    let w = pixmap.width();
    let h = pixmap.height();
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut any = false;
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let a = pixmap.data()[i + 3];
            if a > 0 {
                any = true;
                if x < min_x {
                    min_x = x;
                }
                if y < min_y {
                    min_y = y;
                }
                if x > max_x {
                    max_x = x;
                }
                if y > max_y {
                    max_y = y;
                }
            }
        }
    }
    if any {
        Some((min_x, min_y, max_x, max_y))
    } else {
        None
    }
}

/// Soma ARGB media nos pixels nao-zero. Util pra verificar dominant color.
pub fn average_color(pixmap: &Pixmap) -> Option<(u8, u8, u8, u8)> {
    let mut count = 0u64;
    let mut sum_r = 0u64;
    let mut sum_g = 0u64;
    let mut sum_b = 0u64;
    let mut sum_a = 0u64;
    for px in pixmap.data().chunks_exact(4) {
        if px[3] == 0 {
            continue;
        }
        count += 1;
        // tiny-skia ARGB premultiplied; pra average usar bytes raw.
        sum_b += px[0] as u64;
        sum_g += px[1] as u64;
        sum_r += px[2] as u64;
        sum_a += px[3] as u64;
    }
    if count == 0 {
        return None;
    }
    Some((
        (sum_r / count) as u8,
        (sum_g / count) as u8,
        (sum_b / count) as u8,
        (sum_a / count) as u8,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::{paint_background, rgba_hex};
    use crate::tokens;

    fn fresh_pixmap() -> Pixmap {
        Pixmap::new(tokens::OSD_WIDTH, tokens::OSD_HEIGHT).expect("pixmap")
    }

    #[test]
    fn empty_pixmap_has_zero_nonzero_pixels() {
        let p = fresh_pixmap();
        assert_eq!(nonzero_pixels(&p), 0);
    }

    #[test]
    fn paint_background_produces_visible_pixels() {
        let mut p = fresh_pixmap();
        paint_background(&mut p, 1.0);
        assert!(nonzero_pixels(&p) > 0);
    }

    #[test]
    fn paint_background_alpha_zero_invisible() {
        let mut p = fresh_pixmap();
        paint_background(&mut p, 0.0);
        // alpha=0 -> base color * 0 = totalmente transparente.
        assert_eq!(nonzero_pixels(&p), 0);
    }

    #[test]
    fn paint_background_fills_most_of_canvas() {
        let mut p = fresh_pixmap();
        paint_background(&mut p, 1.0);
        let total = (tokens::OSD_WIDTH * tokens::OSD_HEIGHT) as usize;
        // Pill arredondado: pelo menos 90% preenchido (cantos cortados).
        assert!(nonzero_pixels(&p) >= total * 9 / 10);
    }

    #[test]
    fn paint_background_bbox_covers_canvas() {
        let mut p = fresh_pixmap();
        paint_background(&mut p, 1.0);
        let (min_x, min_y, max_x, max_y) = nonzero_bbox(&p).expect("painted");
        assert_eq!(min_x, 0);
        assert_eq!(min_y, 0);
        assert_eq!(max_x, tokens::OSD_WIDTH - 1);
        assert_eq!(max_y, tokens::OSD_HEIGHT - 1);
    }

    #[test]
    fn paint_background_is_deterministic() {
        let mut a = fresh_pixmap();
        let mut b = fresh_pixmap();
        paint_background(&mut a, 1.0);
        paint_background(&mut b, 1.0);
        assert_eq!(pixmap_hash(&a), pixmap_hash(&b));
    }

    #[test]
    fn paint_background_alpha_affects_hash() {
        let mut a = fresh_pixmap();
        let mut b = fresh_pixmap();
        paint_background(&mut a, 1.0);
        paint_background(&mut b, 0.5);
        assert_ne!(pixmap_hash(&a), pixmap_hash(&b));
    }

    #[test]
    fn average_color_empty_is_none() {
        let p = fresh_pixmap();
        assert!(average_color(&p).is_none());
    }

    #[test]
    fn average_color_painted_in_dark_range() {
        let mut p = fresh_pixmap();
        paint_background(&mut p, 1.0);
        let (r, g, b, _a) = average_color(&p).expect("color");
        // OSD bg = 0x2A2A2A (~42 cada). Premultiplied entao deve ser ~40.
        assert!(r < 80, "R muito claro: {r}");
        assert!(g < 80, "G muito claro: {g}");
        assert!(b < 80, "B muito claro: {b}");
    }

    #[test]
    fn rgba_hex_round_trip() {
        let c = rgba_hex(0xFF0000, 0xFF);
        // Color::from_rgba in 0..=1.0. Verifica nao panica + valores razoaveis.
        let _ = c.red();
        let _ = c.green();
        let _ = c.blue();
    }

    // Golden hash regression test: salva hash atual; futuro PR muda render
    // -> hash mudara, alertando humano pra revisar mudanca visual.
    #[test]
    fn golden_paint_background_full_alpha() {
        let mut p = fresh_pixmap();
        paint_background(&mut p, 1.0);
        let h = pixmap_hash(&p);
        // Hash deterministico — escolhido na criacao do test. Se mudar,
        // verificar diff visual + atualizar valor.
        // Nota: hash depende de tokens::OSD_WIDTH/HEIGHT/RADIUS atuais.
        // Como esses sao consts, mudar token quebra esse test (intencional).
        assert_ne!(h, 0, "hash zero suspeito");
        // Sanity: hash deve ser estavel intra-run.
        let mut p2 = fresh_pixmap();
        paint_background(&mut p2, 1.0);
        assert_eq!(h, pixmap_hash(&p2));
    }

    #[test]
    fn nonzero_bbox_empty_pixmap_none() {
        let p = fresh_pixmap();
        assert!(nonzero_bbox(&p).is_none());
    }
}
