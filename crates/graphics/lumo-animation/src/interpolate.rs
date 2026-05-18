//! interpolate.rs - Trait LAInterpolable + impls concretas.
//!
//! Lerp correto por tipo: f32/f64 trivial, tuplas componente a componente,
//! Color u32 RGBA com sRGB-aware lerp pra evitar muddy mid-colors (perceptual
//! blend via sqrt-aproximacao).

/// Qualquer tipo que possa ser interpolado linearmente.
pub trait LAInterpolable: Copy {
    fn lerp(a: Self, b: Self, t: f32) -> Self;
}

impl LAInterpolable for f32 {
    #[inline]
    fn lerp(a: Self, b: Self, t: f32) -> Self {
        a + (b - a) * t
    }
}

impl LAInterpolable for f64 {
    #[inline]
    fn lerp(a: Self, b: Self, t: f32) -> Self {
        a + (b - a) * t as f64
    }
}

/// (x, y) ponto 2D.
impl LAInterpolable for (f32, f32) {
    #[inline]
    fn lerp(a: Self, b: Self, t: f32) -> Self {
        (f32::lerp(a.0, b.0, t), f32::lerp(a.1, b.1, t))
    }
}

/// Rect (x, y, w, h).
impl LAInterpolable for (f32, f32, f32, f32) {
    #[inline]
    fn lerp(a: Self, b: Self, t: f32) -> Self {
        (
            f32::lerp(a.0, b.0, t),
            f32::lerp(a.1, b.1, t),
            f32::lerp(a.2, b.2, t),
            f32::lerp(a.3, b.3, t),
        )
    }
}

/// Color u32 = 0xRRGGBBAA.
///
/// Blend perceptual: lineariza canais RGB via gamma-2.0 aproximacao
/// (sqrt), blenda, gamma-encode de volta. Evita o "escurecimento no meio"
/// do lerp direto em sRGB comprimido. Alpha: lerp linear (ja linear).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct LAColor(pub u32);

impl LAInterpolable for LAColor {
    fn lerp(a: Self, b: Self, t: f32) -> Self {
        let ar = ((a.0 >> 24) & 0xFF) as f32 / 255.0;
        let ag = ((a.0 >> 16) & 0xFF) as f32 / 255.0;
        let ab = ((a.0 >>  8) & 0xFF) as f32 / 255.0;
        let aa = ( a.0        & 0xFF) as f32 / 255.0;

        let br = ((b.0 >> 24) & 0xFF) as f32 / 255.0;
        let bg = ((b.0 >> 16) & 0xFF) as f32 / 255.0;
        let bb = ((b.0 >>  8) & 0xFF) as f32 / 255.0;
        let ba = ( b.0        & 0xFF) as f32 / 255.0;

        // Lineariza RGB (gamma 2.0 como aproximacao de sRGB).
        let lr_a = ar * ar;
        let lg_a = ag * ag;
        let lb_a = ab * ab;

        let lr_b = br * br;
        let lg_b = bg * bg;
        let lb_b = bb * bb;

        let lr_c = lr_a + (lr_b - lr_a) * t;
        let lg_c = lg_a + (lg_b - lg_a) * t;
        let lb_c = lb_a + (lb_b - lb_a) * t;
        let la_c = aa  + (ba   - aa)   * t;

        // Gamma-encode de volta.
        let r = lr_c.sqrt().clamp(0.0, 1.0);
        let g = lg_c.sqrt().clamp(0.0, 1.0);
        let b_c = lb_c.sqrt().clamp(0.0, 1.0);
        let a = la_c.clamp(0.0, 1.0);

        let packed = ((r * 255.0) as u32) << 24
                   | ((g * 255.0) as u32) << 16
                   | ((b_c * 255.0) as u32) << 8
                   | (a * 255.0) as u32;

        LAColor(packed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f32_lerp_midpoint() {
        assert!((f32::lerp(0.0, 1.0, 0.5) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn color_lerp_opaque_white_black() {
        // Branco 0xFFFFFFFF e preto 0x000000FF, meio = cinza medio perceptual.
        let white = LAColor(0xFFFFFFFF);
        let black = LAColor(0x000000FF);
        let mid = LAColor::lerp(white, black, 0.5);
        let r = (mid.0 >> 24) & 0xFF;
        let g = (mid.0 >> 16) & 0xFF;
        // Perceptual midpoint via gamma-2.0 = ~0x7F (127) nao ~0x80.
        assert!(r > 170 && r < 190, "r={r} (esperado ~180 = sqrt(0.5)*255)");
        assert_eq!(r, g);
    }

    #[test]
    fn color_lerp_t0_is_a() {
        let a = LAColor(0xAABBCCDD);
        let b = LAColor(0x11223344);
        assert_eq!(LAColor::lerp(a, b, 0.0), a);
    }

    #[test]
    fn color_lerp_t1_is_b() {
        let a = LAColor(0xAABBCCDD);
        let b = LAColor(0x11223344);
        // Pequena tolerancia de round-trip pelo float.
        let result = LAColor::lerp(a, b, 1.0);
        let dr = ((result.0 >> 24) & 0xFF) as i32 - ((b.0 >> 24) & 0xFF) as i32;
        assert!(dr.abs() <= 1, "r diff={dr}");
    }
}
