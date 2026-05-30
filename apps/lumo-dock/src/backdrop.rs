//! backdrop.rs -- frosted backdrop do dock (wallpaper borrado atras do pill).
//!
//! Mesma tecnica da bar (shell/src/bar/backdrop.rs): o dock vive numa
//! layer-shell Bottom com exclusive_zone, sempre sobre o WALLPAPER (janelas
//! nao entram na zona do dock). Entao pre-borramos a faixa INFERIOR do
//! wallpaper UMA VEZ no startup e clipamos ao pill arredondado. Custo zero por
//! frame, sem pass offscreen no compositor (zero risco no path DRM) e identico
//! visualmente a um backdrop-blur real enquanto so houver wallpaper atras.
//!
//! Fonte: cache RGBA8 output-res /dev/shm/lumo-wallpaper.cache (gerado por
//! lumo-prewarm.service, mesmo formato lido pelo compositor). Sem cache ->
//! None -> paint cai no pill solido (degradacao graciosa).

use tiny_skia::{
    BlendMode, FillRule, FilterQuality, IntSize, Mask, PathBuilder, Pixmap, PixmapMut, PixmapPaint,
    Transform,
};

const CACHE_MAGIC: &[u8; 4] = b"LMWP";
const CACHE_VERSION: u32 = 1;
const CACHE_PATH: &str = "/dev/shm/lumo-wallpaper.cache";
/// Raio do blur (px) + passes (box blur ~ gaussiano). Um pouco mais forte que
/// a bar (14) pra leitura limpa atras de icones pequenos.
const BLUR_RADIUS: usize = 16;
const BLUR_PASSES: u32 = 3;

/// Faixa INFERIOR do wallpaper (largura da tela x strip_h) ja borrada.
pub struct Backdrop {
    strip: Pixmap,
}

impl Backdrop {
    /// Carrega o cache, escala pra largura `screen_w` mantendo aspecto, recorta
    /// as ULTIMAS `strip_h` linhas (faixa inferior = zona do dock) e borra.
    pub fn load(screen_w: u32, strip_h: u32) -> Option<Backdrop> {
        if screen_w == 0 || strip_h == 0 {
            return None;
        }
        let (pixels, cw, ch) = read_cache()?;
        let cache_px = Pixmap::from_vec(pixels, IntSize::from_wh(cw, ch)?)?;
        // Cache e output-res; escala pra screen_w mantendo aspecto.
        let sx = screen_w as f32 / cw as f32;
        let screen_h = (ch as f32 * sx).round() as u32;
        if screen_h < strip_h {
            return None;
        }
        let mut strip = Pixmap::new(screen_w, strip_h)?;
        // Desloca o cache escalado pra cima de (screen_h - strip_h) VIA TRANSFORM
        // (post_translate): so a faixa inferior do wallpaper cai nas strip_h
        // linhas do strip. (O param (x,y) do draw_pixmap nao desloca de forma
        // confiavel quando ha transform -- usar a translacao no proprio transform.)
        let dy = -((screen_h - strip_h) as f32);
        strip.draw_pixmap(
            0,
            0,
            cache_px.as_ref(),
            &PixmapPaint {
                blend_mode: BlendMode::Source,
                opacity: 1.0,
                quality: FilterQuality::Bilinear,
            },
            Transform::from_scale(sx, sx).post_translate(0.0, dy),
            None,
        );
        box_blur_rgba(
            strip.data_mut(),
            screen_w as usize,
            strip_h as usize,
            BLUR_RADIUS,
            BLUR_PASSES,
        );
        Some(Backdrop { strip })
    }

    /// Pinta o trecho borrado clipado ao pill (px,py,w,h, raio r). A surface do
    /// dock == faixa inferior da tela, logo strip[x,y] == surface[x,y]: o strip
    /// e desenhado em (0,0) e o pixel (px,py) bate.
    pub fn paint_pill(&self, canvas: &mut PixmapMut, px: f32, py: f32, w: f32, h: f32, r: f32) {
        let cw = canvas.width();
        let chh = canvas.height();
        let Some(mut mask) = Mask::new(cw, chh) else {
            return;
        };
        let Some(path) = rounded_rect_path(px, py, w, h, r) else {
            return;
        };
        mask.fill_path(&path, FillRule::Winding, true, Transform::identity());
        canvas.draw_pixmap(
            0,
            0,
            self.strip.as_ref(),
            &PixmapPaint {
                blend_mode: BlendMode::SourceOver,
                opacity: 1.0,
                quality: FilterQuality::Nearest,
            },
            Transform::identity(),
            Some(&mask),
        );
    }
}

fn read_cache() -> Option<(Vec<u8>, u32, u32)> {
    let data = std::fs::read(CACHE_PATH).ok()?;
    if data.len() < 16 || &data[0..4] != CACHE_MAGIC {
        return None;
    }
    let w = u32::from_le_bytes(data[4..8].try_into().ok()?);
    let h = u32::from_le_bytes(data[8..12].try_into().ok()?);
    let version = u32::from_le_bytes(data[12..16].try_into().ok()?);
    if version != CACHE_VERSION || w == 0 || h == 0 || w > 7680 || h > 4320 {
        return None;
    }
    let expected = (w as usize).checked_mul(h as usize)?.checked_mul(4)?;
    if data.len() - 16 != expected {
        return None;
    }
    let mut pixels = data[16..].to_vec();
    // Wallpaper opaco: forca alpha=255 (premultiplied == straight no tiny_skia).
    // Swap R<->B: o cache vem na ordem do wl_shm (Argb8888 = bytes B,G,R,A) que
    // o compositor consome direto; o tiny_skia Pixmap espera R,G,B,A. Sem o swap
    // o azul do lago vira marrom. (px[0]<->px[2].)
    for px in pixels.chunks_exact_mut(4) {
        px.swap(0, 2);
        px[3] = 255;
    }
    Some((pixels, w, h))
}

fn rounded_rect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<tiny_skia::Path> {
    let x = x.round();
    let y = y.round();
    let r = r.min(w / 2.0).min(h / 2.0);
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    pb.finish()
}

/// Box blur separavel (H + V), `passes` iteracoes ~ gaussiano. RGBA
/// premultiplicado in-place, clamp nas bordas.
fn box_blur_rgba(data: &mut [u8], w: usize, h: usize, radius: usize, passes: u32) {
    if radius == 0 || w == 0 || h == 0 || data.len() < w * h * 4 {
        return;
    }
    let mut tmp = vec![0u8; data.len()];
    for _ in 0..passes {
        blur_h(data, &mut tmp, w, h, radius);
        blur_v(&tmp, data, w, h, radius);
    }
}

#[inline]
fn clamp_idx(i: isize, n: usize) -> usize {
    i.max(0).min(n as isize - 1) as usize
}

fn blur_h(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize) {
    let win = (2 * r + 1) as u32;
    for y in 0..h {
        let row = y * w * 4;
        for c in 0..4 {
            let mut sum: u32 = 0;
            for k in 0..=(2 * r) {
                let xi = clamp_idx(k as isize - r as isize, w);
                sum += src[row + xi * 4 + c] as u32;
            }
            for x in 0..w {
                dst[row + x * 4 + c] = (sum / win) as u8;
                let add_i = clamp_idx(x as isize + r as isize + 1, w);
                let rem_i = clamp_idx(x as isize - r as isize, w);
                sum = sum + src[row + add_i * 4 + c] as u32 - src[row + rem_i * 4 + c] as u32;
            }
        }
    }
}

fn blur_v(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize) {
    let win = (2 * r + 1) as u32;
    let stride = w * 4;
    for x in 0..w {
        let col = x * 4;
        for c in 0..4 {
            let mut sum: u32 = 0;
            for k in 0..=(2 * r) {
                let yi = clamp_idx(k as isize - r as isize, h);
                sum += src[col + yi * stride + c] as u32;
            }
            for y in 0..h {
                dst[col + y * stride + c] = (sum / win) as u8;
                let add_i = clamp_idx(y as isize + r as isize + 1, h);
                let rem_i = clamp_idx(y as isize - r as isize, h);
                sum = sum + src[col + add_i * stride + c] as u32 - src[col + rem_i * stride + c] as u32;
            }
        }
    }
}
