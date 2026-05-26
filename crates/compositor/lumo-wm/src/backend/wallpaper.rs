//! Wallpaper: imagem de fundo carregada uma vez no startup do backend
//! (winit OU drm), uploadada como textura GL via GlesRenderer e desenhada
//! como element atras da Space (toplevels + layer-shell) a cada frame.
//!
//! A19: substitui clear color solido como fundo principal -- clear color
//! continua sendo desenhado pelo damage tracker em areas nao cobertas
//! pelo wallpaper. Como a textura cobre o output inteiro (stretch),
//! clear color so vira fallback se load do wallpaper falhar.
//!
//! C4 (boot integration): try_load tenta primeiro o cache pre-aquecido
//! em /dev/shm/lumo-wallpaper.cache (gerado por lumo-prewarm.service).
//! Cache contem RGBA8 pre-decodificado e escalado para 1920x1080,
//! eliminando decode JPEG 8K + scale (~250ms) no hot path de startup.
//! Formato: header 16 bytes LE [LMWP][w u32][h u32][version u32]
//!          seguido de w*h*4 bytes RGBA8.
//! Fallback automatico para decode normal se cache ausente/corrompido.
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

/// Magic bytes no header do cache de wallpaper (lumo-prewarm.sh).
const CACHE_MAGIC: &[u8; 4] = b"LMWP";
/// Versao de formato suportada.
const CACHE_VERSION: u32 = 1;
/// Path do cache em tmpfs (gerado por lumo-prewarm.service).
const CACHE_PATH: &str = "/dev/shm/lumo-wallpaper.cache";

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
        Self::upload(renderer, pixels, w as i32, h as i32)
    }

    /// Tenta ler o cache pre-aquecido de /dev/shm/lumo-wallpaper.cache.
    /// Retorna erro se: arquivo ausente, header invalido, tamanho incorreto.
    /// Caller faz fallback para load() normal em qualquer erro.
    fn load_cache(renderer: &mut GlesRenderer) -> Result<Self> {
        let data = std::fs::read(CACHE_PATH)
            .with_context(|| "abrir cache /dev/shm/lumo-wallpaper.cache")?;

        // Header: 4 magic + 4 width + 4 height + 4 version = 16 bytes.
        if data.len() < 16 {
            return Err(anyhow!("cache muito pequeno ({} bytes)", data.len()));
        }
        let magic = &data[0..4];
        if magic != CACHE_MAGIC {
            return Err(anyhow!("magic invalido: {:?}", magic));
        }
        let w = u32::from_le_bytes(
            data[4..8]
                .try_into()
                .expect("slice 4B garantido pelo bounds check"),
        ) as i32;
        let h = u32::from_le_bytes(
            data[8..12]
                .try_into()
                .expect("slice 4B garantido pelo bounds check"),
        ) as i32;
        let version = u32::from_le_bytes(
            data[12..16]
                .try_into()
                .expect("slice 4B garantido pelo bounds check"),
        );

        if version != CACHE_VERSION {
            return Err(anyhow!("versao de cache incompativel: {version}"));
        }
        if w <= 0 || h <= 0 || w > 7680 || h > 4320 {
            return Err(anyhow!("dimensoes invalidas no cache: {w}x{h}"));
        }
        let expected_pixels = (w as usize) * (h as usize) * 4;
        let actual_pixels = data.len() - 16;
        if actual_pixels != expected_pixels {
            return Err(anyhow!(
                "tamanho de pixels incorreto: esperado {expected_pixels}, encontrado {actual_pixels}"
            ));
        }

        let pixels = data[16..].to_vec();
        tracing::info!(w, h, "wallpaper cache hit: /dev/shm/lumo-wallpaper.cache");
        Self::upload(renderer, pixels, w, h)
    }

    /// Upload de pixels RGBA8 para textura GL. Fatorado para ser chamado
    /// tanto por load() (decode normal) quanto por load_cache() (shm).
    fn upload(renderer: &mut GlesRenderer, pixels: Vec<u8>, w: i32, h: i32) -> Result<Self> {
        let size: Size<i32, smithay::utils::Buffer> = (w, h).into();

        // Abgr8888 = ordem byte [R,G,B,A] em memoria little-endian. Bate
        // exato com layout do image::Rgba8 (R primeiro). Mesmo formato
        // usado pro cursor xcursor em A7 (bug Argb->Abgr resolvido la).
        let texture = renderer
            .import_memory(&pixels, Fourcc::Abgr8888, size, false)
            .map_err(|e| anyhow!("import_memory wallpaper: {e:?}"))?;

        // TextureBuffer scale=1, transform=Normal -- queremos coordenadas
        // 1:1 com output. opaque_regions=None: image::Rgba8 pode ter alpha
        // (PNG transparente). Default seguro.
        let buffer = TextureBuffer::from_texture(renderer, texture, 1, Transform::Normal, None);

        Ok(Self {
            buffer,
            size: (w, h),
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

    /// Wrapper: tenta resolver+carregar. Ordem de tentativa:
    /// 1. Cache pre-aquecido /dev/shm/lumo-wallpaper.cache (lumo-prewarm.service).
    /// 2. Decode normal do arquivo original via image crate.
    /// Loga warn em qualquer falha final e retorna None.
    pub fn try_load(renderer: &mut GlesRenderer) -> Option<Self> {
        // C4: tenta cache primeiro (zero decode cost se prewarm rodou).
        match Self::load_cache(renderer) {
            Ok(w) => return Some(w),
            Err(e) => {
                // Cache ausente e normal antes do prewarm; outro erro = warn.
                if std::path::Path::new(CACHE_PATH).exists() {
                    tracing::warn!(error = %e, "cache corrompido, fallback decode normal");
                } else {
                    tracing::debug!("cache nao disponivel, decode direto do arquivo");
                }
            }
        }

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
                    "wallpaper carregado (decode direto)"
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
        let src: Rectangle<f64, Logical> =
            Rectangle::from_size((self.size.0 as f64, self.size.1 as f64).into());
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

// ============================================================
// W6.C: splash boot logo
// ============================================================

/// Carrega assets/splash.png como MemoryRenderBuffer RGBA8.
/// Embutido em compile-time via include_bytes!.
/// Retorna None se decode falhar (graceful degradation).
pub fn load_splash_buffer(
) -> Option<smithay::backend::renderer::element::memory::MemoryRenderBuffer> {
    use smithay::backend::allocator::Fourcc;
    use smithay::backend::renderer::element::memory::MemoryRenderBuffer;
    use smithay::utils::Transform;

    let bytes = include_bytes!("../../../../../assets/splash.png");
    let img = image::load_from_memory(bytes).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    let pixels = img.into_raw();
    Some(MemoryRenderBuffer::from_slice(
        &pixels,
        Fourcc::Abgr8888,
        (w as i32, h as i32),
        1,
        Transform::Normal,
        None,
    ))
}

/// Constroi MemoryRenderBufferRenderElement do splash centrado no output.
/// Requer renderer pra importar textura (padrao smithay 0.7).
pub fn splash_element(
    renderer: &mut smithay::backend::renderer::gles::GlesRenderer,
    buffer: &smithay::backend::renderer::element::memory::MemoryRenderBuffer,
    output_w: i32,
    output_h: i32,
    alpha: f32,
) -> Option<
    smithay::backend::renderer::element::memory::MemoryRenderBufferRenderElement<
        smithay::backend::renderer::gles::GlesRenderer,
    >,
> {
    use smithay::backend::renderer::element::memory::MemoryRenderBufferRenderElement;
    use smithay::backend::renderer::element::Kind;
    use smithay::utils::{Physical, Point};

    let buf_w = 320_i32;
    let buf_h = 320_i32;
    let cx = (output_w - buf_w) / 2;
    let cy = (output_h - buf_h) / 2;

    MemoryRenderBufferRenderElement::from_buffer(
        renderer,
        Point::<f64, Physical>::from((cx as f64, cy as f64)),
        buffer,
        Some(alpha),
        None,
        None,
        Kind::Unspecified,
    )
    .ok()
}
