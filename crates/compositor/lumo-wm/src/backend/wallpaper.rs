//! Wallpaper: imagem de fundo carregada uma vez no startup do backend
//! (winit OU drm), uploadada como textura GL via GlesRenderer e desenhada
//! como element atras da Space (toplevels + layer-shell) a cada frame.
//!
//! A19: substitui clear color solido como fundo principal -- clear color
//! continua sendo desenhado pelo damage tracker em areas nao cobertas
//! pelo wallpaper. Como a textura cobre o output inteiro (stretch),
//! clear color so vira fallback se load do wallpaper falhar.
//!
//! Memory feedback_design_lapidado: justificar -- por que imagem em vez
//! de gradient procedural? Resposta: clear_color ja simulava cor solida;
//! textura permite personalizacao real via env LUMO_WALLPAPER. Stretch
//! escolhido porque wallpaper padrao 1999x1124 e output 1920x1080 sao
//! ambos ~16:9 (diferenca 0.02 = invisivel). Memory feedback_zero_neon_glow:
//! wallpaper externo nao acrescenta saturacao neon (depende do arquivo).
//!
//! Memory feedback_input_feedback_imediato: load acontece UMA VEZ no
//! startup (custo ~120KB jpeg decode = single-digit ms), nao reabre
//! arquivo a cada frame. TextureBuffer e clonado entre frames -- handle
//! GL nao migra.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use image::ImageReader;
use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::texture::{TextureBuffer, TextureRenderElement};
use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::backend::renderer::ImportMem;
use smithay::utils::{Logical, Physical, Point, Rectangle, Size, Transform};

/// Wallpaper carregado: TextureBuffer GL + dimensoes originais da imagem.
///
/// TextureBuffer carrega ContextId do renderer -- portanto deve ser
/// construido APOS o GlesRenderer estar pronto. Logo, esse load acontece
/// dentro do init de cada backend (winit/drm), nao no construtor de
/// LumoState.
pub struct LumoWallpaper {
    pub buffer: TextureBuffer<GlesTexture>,
    pub size: (i32, i32),
}

impl LumoWallpaper {
    /// Tenta carregar imagem do path. Decode via image crate,
    /// upload via renderer.import_memory (Abgr8888 = RGBA little-endian).
    ///
    /// Retorna erro se: arquivo nao existe (caller checa antes), decode
    /// falha, ou upload GL falha. Caller decide se loga warn + segue
    /// sem wallpaper.
    pub fn load(renderer: &mut GlesRenderer, path: &Path) -> Result<Self> {
        let img = ImageReader::open(path)
            .with_context(|| format!("abrir wallpaper {}", path.display()))?
            .with_guessed_format()
            .with_context(|| format!("detect format {}", path.display()))?
            .decode()
            .with_context(|| format!("decode wallpaper {}", path.display()))?
            .into_rgba8();
        let (w, h) = img.dimensions();
        let pixels = img.into_raw();
        let size: Size<i32, smithay::utils::Buffer> = (w as i32, h as i32).into();

        // Abgr8888 = ordem byte [R,G,B,A] em memoria little-endian. Bate
        // exato com layout do image::Rgba8 (R primeiro). Mesmo formato
        // usado pro cursor xcursor em A7 (bug Argb->Abgr resolvido la).
        let texture = renderer
            .import_memory(&pixels, Fourcc::Abgr8888, size, false)
            .map_err(|e| anyhow!("import_memory wallpaper: {e:?}"))?;

        // TextureBuffer scale=1, transform=Normal -- queremos coordenadas
        // 1:1 com output. opaque_regions=None: image::Rgba8 pode ter alpha
        // (PNG transparente). Default seguro.
        let buffer = TextureBuffer::from_texture(
            renderer,
            texture,
            1,
            Transform::Normal,
            None,
        );

        Ok(Self {
            buffer,
            size: (w as i32, h as i32),
        })
    }

    /// Resolve path do wallpaper: env LUMO_WALLPAPER se setado,
    /// senao $HOME/.config/lumo-wallpaper.jpg. Retorna None se HOME
    /// nao definido E env tambem nao setado.
    pub fn resolve_path() -> Option<PathBuf> {
        if let Ok(p) = std::env::var("LUMO_WALLPAPER") {
            return Some(PathBuf::from(p));
        }
        let home = std::env::var("HOME").ok()?;
        Some(PathBuf::from(home).join(".config/lumo-wallpaper.jpg"))
    }

    /// Wrapper: tenta resolver+carregar. Loga warn em qualquer falha
    /// e retorna None (caller usa clear color como fallback).
    pub fn try_load(renderer: &mut GlesRenderer) -> Option<Self> {
        let path = Self::resolve_path()?;
        if !path.exists() {
            tracing::warn!(path = %path.display(), "wallpaper nao encontrado, usando clear color");
            return None;
        }
        match Self::load(renderer, &path) {
            Ok(w) => {
                tracing::info!(
                    path = %path.display(),
                    w = w.size.0,
                    h = w.size.1,
                    "wallpaper carregado"
                );
                Some(w)
            }
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "falha load wallpaper");
                None
            }
        }
    }

    /// Constroi TextureRenderElement escalado pra cobrir output_w x output_h.
    /// Posicao = (0,0) (topo esquerdo do framebuffer fisico).
    ///
    /// Strategy: "stretch" (forcar size logical = output size). Wallpaper
    /// padrao 1999x1124 e display 1920x1080 sao 16:9 muito proximos --
    /// distorcao desprezivel. Pra suporte multi-output ou aspect ratios
    /// diferentes, futuro: implementar cover/contain com crop.
    pub fn element(&self, output_w: i32, output_h: i32) -> TextureRenderElement<GlesTexture> {
        // A19.11: src_rect explicito (full buffer) + size logical (output) =
        // forca scale 7680x4320 -> 1920x1080. Sem src explicito, smithay
        // pode renderizar 1:1 sem scale.
        let logical_size: Size<i32, Logical> = (output_w, output_h).into();
        let src: Rectangle<f64, Logical> = Rectangle::from_size(
            (self.size.0 as f64, self.size.1 as f64).into(),
        );
        TextureRenderElement::from_texture_buffer(
            Point::<f64, Physical>::from((0.0, 0.0)),
            &self.buffer,
            None,
            Some(src),
            Some(logical_size),
            Kind::Unspecified,
        )
    }
}
