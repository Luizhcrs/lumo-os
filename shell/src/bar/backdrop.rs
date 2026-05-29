//! bar/backdrop.rs - Fundo frosted (wallpaper borrado) da ilha flutuante.
//!
//! A bar virou uma ILHA flutuante (vide tokens ISLAND_*). Pra o efeito
//! "frosted glass" da referencia (Luiz 2026-05), o painel mostra o
//! wallpaper BORRADO atras dos pills, em vez de uma faixa solida.
//!
//! IDEIA-CHAVE (custo zero por frame): blur real de backdrop e caro porque
//! o que esta atras e dinamico. MAS atras da bar (faixa do topo) o backdrop
//! e quase sempre SO o wallpaper -- janelas ficam abaixo do exclusive_zone +
//! gap (clamp min_y impede janela subir sob a bar). Entao pre-borramos o
//! wallpaper UMA VEZ no startup e desenhamos a faixa borrada clipada ao
//! painel arredondado. Resultado identico ao backdrop-blur, sem pass
//! offscreen no compositor (zero risco no path DRM) e sem custo por frame.
//!
//! Alinhamento: a surface da bar e deslocada por (ISLAND_MARGIN_X,
//! ISLAND_MARGIN_TOP) via set_margin. A faixa borrada e screen-space (top
//! da tela); ao pintar amostramos com esse offset pra casar pixel-a-pixel
//! com o wallpaper sharp que o compositor desenha atras dos pixels
//! transparentes da surface.
//!
//! Fonte do wallpaper: cache pre-aquecido /dev/shm/lumo-wallpaper.cache
//! (RGBA8 ja escalado pra resolucao do output, gerado por
//! lumo-prewarm.service -- mesmo formato lido pelo compositor em
//! backend/wallpaper.rs). Sem cache -> None -> state.rs cai no painel
//! translucido solido (degradacao graciosa, sem blur).

use tiny_skia::{
    BlendMode, FillRule, FilterQuality, IntSize, Mask, PathBuilder, Pixmap, PixmapMut, PixmapPaint,
    Transform,
};

/// Magic bytes do header do cache (igual backend/wallpaper.rs).
const CACHE_MAGIC: &[u8; 4] = b"LMWP";
/// Versao de formato suportada.
const CACHE_VERSION: u32 = 1;
/// Path do cache em tmpfs (gerado por lumo-prewarm.service).
const CACHE_PATH: &str = "/dev/shm/lumo-wallpaper.cache";

/// Raio do blur (px) e numero de passes (box blur ~ gaussiano).
const BLUR_RADIUS: usize = 14;
const BLUR_PASSES: u32 = 3;

/// Faixa do wallpaper borrada, screen-space (largura da tela x strip_h).
pub struct Backdrop {
    /// Pixmap RGBA premultiplicado (alpha=255, wallpaper opaco) cobrindo
    /// os primeiros `strip_h` pixels da tela ja borrados.
    strip: Pixmap,
}

impl Backdrop {
    /// Carrega o cache de wallpaper, escala pra tela, recorta a faixa do
    /// topo (`strip_h` linhas) e aplica blur. None se cache ausente/invalido.
    pub fn load(screen_w: u32, screen_h: u32, strip_h: u32) -> Option<Backdrop> {
        if screen_w == 0 || screen_h == 0 || strip_h == 0 {
            return None;
        }
        let (pixels, cw, ch) = read_cache()?;
        let cache_px = Pixmap::from_vec(pixels, IntSize::from_wh(cw, ch)?)?;

        // Faixa screen-space. draw_pixmap escala o cache (cw x ch) pra tela
        // (screen_w x screen_h); so as primeiras strip_h linhas caem no dst.
        let mut strip = Pixmap::new(screen_w, strip_h)?;
        let sx = screen_w as f32 / cw as f32;
        let sy = screen_h as f32 / ch as f32;
        strip.draw_pixmap(
            0,
            0,
            cache_px.as_ref(),
            &PixmapPaint {
                blend_mode: BlendMode::Source,
                opacity: 1.0,
                quality: FilterQuality::Bilinear,
            },
            Transform::from_scale(sx, sy),
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

    /// Pinta a faixa borrada clipada ao painel (px,py,w,h) com raios
    /// independentes topo (rt) / baixo (rb) no `canvas`. (src_off_x,
    /// src_off_y) = posicao screen-space do canto top-left do painel pra
    /// amostrar o trecho certo do wallpaper.
    #[allow(clippy::too_many_arguments)]
    pub fn paint_panel(
        &self,
        canvas: &mut PixmapMut,
        px: f32,
        py: f32,
        w: f32,
        h: f32,
        rt: f32,
        rb: f32,
        src_off_x: f32,
        src_off_y: f32,
    ) {
        let cw = canvas.width();
        let chh = canvas.height();
        let Some(mut mask) = Mask::new(cw, chh) else {
            return;
        };
        let Some(path) = rounded_rect_path(px, py, w, h, rt, rb) else {
            return;
        };
        mask.fill_path(&path, FillRule::Winding, true, Transform::identity());

        // strip pixel (src_off_x, src_off_y) deve cair em canvas (px, py).
        let dst_x = (px - src_off_x).round() as i32;
        let dst_y = (py - src_off_y).round() as i32;
        canvas.draw_pixmap(
            dst_x,
            dst_y,
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

/// Le o cache de wallpaper de /dev/shm. Retorna (pixels RGBA8, w, h).
/// Forca alpha=255 (wallpaper opaco) pra premultiplied == straight.
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
    // Wallpaper e opaco: forca alpha=255 pra premultiplied == straight (o
    // tiny_skia Pixmap assume premultiplicado).
    for px in pixels.chunks_exact_mut(4) {
        px[3] = 255;
    }
    Some((pixels, w, h))
}

/// Path de retangulo com raios independentes topo (rt) / baixo (rb). A bar
/// colada usa rt=0 (topo reto) + rb>0 (base curva).
fn rounded_rect_path(x: f32, y: f32, w: f32, h: f32, rt: f32, rb: f32) -> Option<tiny_skia::Path> {
    let x = x.round();
    let y = y.round();
    let half = (w / 2.0).min(h / 2.0);
    let rt = rt.clamp(0.0, half);
    let rb = rb.clamp(0.0, half);
    let mut pb = PathBuilder::new();
    pb.move_to(x + rt, y);
    pb.line_to(x + w - rt, y);
    if rt > 0.0 {
        pb.quad_to(x + w, y, x + w, y + rt);
    } else {
        pb.line_to(x + w, y);
    }
    pb.line_to(x + w, y + h - rb);
    if rb > 0.0 {
        pb.quad_to(x + w, y + h, x + w - rb, y + h);
    } else {
        pb.line_to(x + w, y + h);
    }
    pb.line_to(x + rb, y + h);
    if rb > 0.0 {
        pb.quad_to(x, y + h, x, y + h - rb);
    } else {
        pb.line_to(x, y + h);
    }
    pb.line_to(x, y + rt);
    if rt > 0.0 {
        pb.quad_to(x, y, x + rt, y);
    } else {
        pb.line_to(x, y);
    }
    pb.close();
    pb.finish()
}

/// Box blur separavel (H + V), `passes` iteracoes ~ gaussiano. Opera em
/// RGBA premultiplicado in-place. Clamp nas bordas (extend edge).
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

/// Passe horizontal: src -> dst. Janela deslizante por canal.
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

/// Passe vertical: src -> dst. Janela deslizante por coluna.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Box blur reduz a variancia (suaviza arestas). Padrao xadrez 0/255 ->
    /// apos blur, valores convergem pro meio (sem 0 nem 255 puros no centro).
    #[test]
    fn blur_smooths_high_contrast() {
        let w = 64;
        let h = 64;
        let mut data = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let v = if (x / 8 + y / 8) % 2 == 0 { 255 } else { 0 };
                let o = (y * w + x) * 4;
                data[o] = v;
                data[o + 1] = v;
                data[o + 2] = v;
                data[o + 3] = 255;
            }
        }
        box_blur_rgba(&mut data, w, h, 6, 3);
        // Centro de um bloco antes era 0 ou 255 puro; apos blur deve ter
        // sangrado pro meio (nem 0 nem 255 no canal R do pixel central).
        let center = ((h / 2) * w + (w / 2)) * 4;
        let r = data[center];
        assert!(r > 20 && r < 235, "esperado valor borrado, obtido {r}");
        // Alpha preservado (wallpaper opaco).
        assert_eq!(data[center + 3], 255);
    }

    /// radius=0 e no-op (nao altera dados nem entra em panico).
    #[test]
    fn blur_radius_zero_noop() {
        let mut data = vec![123u8; 16 * 16 * 4];
        let copy = data.clone();
        box_blur_rgba(&mut data, 16, 16, 0, 3);
        assert_eq!(data, copy);
    }

    /// Cache invalido (magic errado / curto) -> read_cache devolve None sem
    /// panico. (Nao escreve arquivo; so valida o guard de tamanho.)
    #[test]
    fn rounded_rect_path_builds() {
        // Topo reto + base curva (formato da bar colada).
        assert!(rounded_rect_path(0.0, 0.0, 100.0, 40.0, 0.0, 16.0).is_some());
        // Todos cantos curvos.
        assert!(rounded_rect_path(0.0, 0.0, 100.0, 40.0, 16.0, 16.0).is_some());
        // Degenerado (w/h zero) nao deve dar panico.
        let _ = rounded_rect_path(0.0, 0.0, 0.0, 0.0, 0.0, 16.0);
    }

    /// Edge: radius MAIOR que a dimensao da faixa. A janela deslizante
    /// satura nas bordas em multiplos indices; o invariante 1-entra/1-sai
    /// mantem `sum` = soma de 2r+1 bytes >=0 (sem underflow/panic). Gradiente
    /// decrescente (255->0) e o pior caso pro subtrai-na-borda-direita.
    #[test]
    fn blur_radius_exceeds_dimension_no_panic() {
        let w = 4;
        let h = 4;
        let mut data = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                // Gradiente 255 (esq/topo) -> 0 (dir/baixo).
                let v = (255 - (x + y) * 36).min(255) as u8;
                let o = (y * w + x) * 4;
                data[o] = v;
                data[o + 1] = v;
                data[o + 2] = v;
                data[o + 3] = 255;
            }
        }
        // radius 14 >> w/h=4. Em debug isto panica se houver underflow u32.
        box_blur_rgba(&mut data, w, h, 14, 3);
        // Resultado finito + alpha preservado (sem corrupcao).
        for px in data.chunks_exact(4) {
            assert_eq!(px[3], 255);
        }
    }

    /// Blur de imagem uniforme preserva o valor (media de constante = const).
    #[test]
    fn blur_uniform_preserves_value() {
        let w = 32;
        let h = 32;
        let mut data = vec![0u8; w * h * 4];
        for px in data.chunks_exact_mut(4) {
            px[0] = 100;
            px[1] = 150;
            px[2] = 200;
            px[3] = 255;
        }
        box_blur_rgba(&mut data, w, h, 5, 2);
        let c = ((h / 2) * w + w / 2) * 4;
        assert_eq!(data[c], 100);
        assert_eq!(data[c + 1], 150);
        assert_eq!(data[c + 2], 200);
        assert_eq!(data[c + 3], 255);
    }
}
