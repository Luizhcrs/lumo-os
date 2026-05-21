//! Shader SDF corner radius para toplevels (A38).
//!
//! Compila um GlesTexProgram customizado com SDF de rounded rect.
//! RoundedSurfaceElement wrappa SpaceRenderElements de toplevel e injeta
//! o override no GlesFrame antes de cada draw, limpando depois.
//!
//! Inspirado em Hyprland src/render/OpenGL.cpp rounded shader.
//! SDF opera em pixels (u_surf_size + u_corner_radius).
//! Quando u_corner_radius == 0.0 (uniform nao setado), sem clip.

use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::element::{Element, Id, Kind, RenderElement};
use smithay::backend::renderer::gles::{GlesError, GlesFrame, GlesRenderer, GlesTexProgram};
use smithay::backend::renderer::gles::{Uniform, UniformName, UniformType};
use smithay::backend::renderer::utils::{CommitCounter, DamageSet, OpaqueRegions};
use smithay::desktop::space::SpaceRenderElements;
use smithay::utils::{Buffer as BufferCoords, Physical, Rectangle, Scale, Transform};

// Shader fragment com SDF rounded rect.
// //_DEFINES_ obrigatorio: smithay insere #define EXTERNAL, NO_ALPHA, DEBUG_FLAGS.
// Bloco A19.4 sRGB: NUNCA alterar.
const CORNER_FRAG: &str = "#version 100

//_DEFINES_

#if defined(EXTERNAL)
#extension GL_OES_EGL_image_external : require
#endif

precision mediump float;
#if defined(EXTERNAL)
uniform samplerExternalOES tex;
#else
uniform sampler2D tex;
#endif

uniform float alpha;
varying vec2 v_coords;

uniform vec2 u_surf_size;
uniform float u_corner_radius;

#if defined(DEBUG_FLAGS)
uniform float tint;
#endif

float sdf_rounded_rect(vec2 p, vec2 half_size, float r) {
    vec2 q = abs(p) - half_size + vec2(r);
    return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - r;
}

void main() {
    vec4 color = texture2D(tex, v_coords);

#if defined(NO_ALPHA)
    color = vec4(color.rgb, 1.0) * alpha;
#else
    color = color * alpha;
#endif

#if defined(DEBUG_FLAGS)
    if (tint == 1.0)
        color = vec4(0.0, 0.2, 0.0, 0.2) + color * 0.8;
#endif

    if (u_corner_radius > 0.0 && u_surf_size.x > 0.5 && u_surf_size.y > 0.5) {
        vec2 px = v_coords * u_surf_size;
        vec2 center = u_surf_size * 0.5;
        vec2 half_size = u_surf_size * 0.5;
        float d = sdf_rounded_rect(px - center, half_size, u_corner_radius);
        float aa = 1.0;
        float mask = 1.0 - smoothstep(-aa, aa, d);
        // W28.8: SDF aplicado em 4 cantos. CornerMaskShader cobre
        // wallpaper atras dos cantos top da SSD titlebar.
        color.a *= mask;
    }

    vec3 srgb_rgb;
    if (color.a > 0.0001) {
        vec3 lin = color.rgb / color.a;
        srgb_rgb = pow(lin, vec3(1.0/2.2)) * color.a;
    } else {
        srgb_rgb = vec3(0.0);
    }
    gl_FragColor = vec4(srgb_rgb, color.a);
}
";

/// Programa de textura com SDF corner radius.
/// Compilado uma vez por renderer (winit.rs ou drm.rs).
#[derive(Clone)]
pub struct CornerShader {
    pub program: GlesTexProgram,
}

impl CornerShader {
    pub fn compile(renderer: &mut GlesRenderer) -> Result<Self, GlesError> {
        let program = renderer.compile_custom_texture_shader(
            CORNER_FRAG,
            &[
                UniformName::new("u_surf_size", UniformType::_2f),
                UniformName::new("u_corner_radius", UniformType::_1f),
            ],
        )?;
        Ok(CornerShader { program })
    }
}

/// Wrapper de SpaceRenderElements (toplevels) com SDF corner radius.
/// Layer-shell nao recebe corner radius — so usar pra elementos Window.
pub struct RoundedSurfaceElement {
    inner: SpaceRenderElements<GlesRenderer, WaylandSurfaceRenderElement<GlesRenderer>>,
    surf_w: f32,
    surf_h: f32,
    corner_program: GlesTexProgram,
    corner_radius: f32,
}

impl RoundedSurfaceElement {
    pub fn new(
        elem: SpaceRenderElements<GlesRenderer, WaylandSurfaceRenderElement<GlesRenderer>>,
        corner_program: GlesTexProgram,
        corner_radius: f32,
    ) -> Self {
        let geo = elem.geometry(Scale::from(1.0));
        RoundedSurfaceElement {
            surf_w: geo.size.w as f32,
            surf_h: geo.size.h as f32,
            inner: elem,
            corner_program,
            corner_radius,
        }
    }
}

impl Element for RoundedSurfaceElement {
    fn id(&self) -> &Id {
        self.inner.id()
    }
    fn current_commit(&self) -> CommitCounter {
        self.inner.current_commit()
    }
    fn location(&self, scale: Scale<f64>) -> smithay::utils::Point<i32, Physical> {
        self.inner.location(scale)
    }
    fn src(&self) -> Rectangle<f64, BufferCoords> {
        self.inner.src()
    }
    fn transform(&self) -> Transform {
        self.inner.transform()
    }
    fn geometry(&self, scale: Scale<f64>) -> Rectangle<i32, Physical> {
        self.inner.geometry(scale)
    }
    fn damage_since(&self, scale: Scale<f64>, commit: Option<CommitCounter>) -> DamageSet<i32, Physical> {
        self.inner.damage_since(scale, commit)
    }
    fn opaque_regions(&self, _scale: Scale<f64>) -> OpaqueRegions<i32, Physical> {
        // corner radius remove pixels de cantos; sem opaque regions pra forcar blend
        OpaqueRegions::default()
    }
    fn alpha(&self) -> f32 {
        self.inner.alpha()
    }
    fn kind(&self) -> Kind {
        self.inner.kind()
    }
}

impl RenderElement<GlesRenderer> for RoundedSurfaceElement {
    fn draw(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        src: Rectangle<f64, BufferCoords>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        opaque_regions: &[Rectangle<i32, Physical>],
    ) -> Result<(), GlesError> {
        // R3: usar dimensoes do dst renderizado (nao geometry precalculado).
        // elem.geometry() pode incluir shadow/CSD padding (e.g. GTK4 sombra top)
        // fazendo o SDF clipar so os cantos superiores. dst.size = pixels
        // reais que o compositor vai escrever = tamanho certo pro SDF.
        let w = dst.size.w as f32;
        let h = dst.size.h as f32;
        tracing::debug!(
            dst_w = w, dst_g_w = self.surf_w,
            dst_h = h, dst_g_h = self.surf_h,
            "R3 RoundedSurfaceElement draw"
        );
        let uniforms = vec![
            Uniform::new("u_surf_size", (w, h)).into_owned(),
            Uniform::new("u_corner_radius", self.corner_radius).into_owned(),
        ];
        frame.override_default_tex_program(self.corner_program.clone(), uniforms);
        let res = RenderElement::<GlesRenderer>::draw(
            &self.inner,
            frame,
            src,
            dst,
            damage,
            opaque_regions,
        );
        frame.clear_tex_program_override();
        res
    }
}


// W28.8: pixel shader que renderiza alpha=1 fora curva, alpha=0 inside.
// Cor preto solido. Usado pra mascarar wallpaper atras dos cantos round
// da janela (SSD titlebar nao tem texture; pixmap binary alpha tinha
// staircase aliased de 12x12).
//
// Posiciona-se sobre cada um dos 4 cantos da janela (12x12 cada).
// u_anchor (vec2 [0..1]) = anchor da curva dentro da rect:
//   TL canto janela: anchor=(1.0, 1.0) (curve at BR of mask rect)
//   TR canto janela: anchor=(0.0, 1.0)
//   BL canto janela: anchor=(1.0, 0.0)
//   BR canto janela: anchor=(0.0, 0.0)
const CORNER_MASK_FRAG: &str = "
//_DEFINES_

precision mediump float;
varying vec2 v_coords;
uniform vec2 size;
uniform float alpha;
uniform vec2 u_anchor;
uniform float u_radius;

#if defined(DEBUG_FLAGS)
uniform float tint;
#endif

void main() {
    vec2 px = v_coords * size;
    vec2 center = u_anchor * size;
    float d = distance(px, center) - u_radius;
    float aa = 1.0;
    float mask = smoothstep(-aa, aa, d);
    // Preto premultiplied (rgb*alpha = 0 quando rgb=0)
    gl_FragColor = vec4(0.0, 0.0, 0.0, mask * alpha);
}
";

/// Pixel shader pra mascarar cantos round da janela.
/// Compile uma vez por renderer.
#[derive(Clone)]
pub struct CornerMaskShader {
    pub program: smithay::backend::renderer::gles::GlesPixelProgram,
}

impl CornerMaskShader {
    pub fn compile(renderer: &mut GlesRenderer) -> Result<Self, GlesError> {
        let program = renderer.compile_custom_pixel_shader(
            CORNER_MASK_FRAG,
            &[
                UniformName::new("u_anchor", UniformType::_2f),
                UniformName::new("u_radius", UniformType::_1f),
            ],
        )?;
        Ok(CornerMaskShader { program })
    }
}
