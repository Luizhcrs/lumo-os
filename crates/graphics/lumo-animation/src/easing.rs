//! easing.rs - Curvas de easing cubic-bezier + presets Apple/iOS.
//!
//! `LACurve` representa qualquer curva definida por 4 parametros
//! cubic-bezier. `eval(t)` avalia pelo metodo Newton-Raphson: dado t (0..1
//! no tempo), resolve para o parametro u da curva e retorna o valor y(u).
//!
//! Presets matcham os valores usados pelo sistema iOS/macOS; validados
//! contra o Apple Fluid Lab (lab/apple-fluid-demo/).

/// Curva cubic-bezier 1D parametrizada por P1 e P2 (P0=0,0 e P3=1,1 fixos).
///
/// Equivalente a CSS `cubic-bezier(x1, y1, x2, y2)`.
#[derive(Clone, Copy, Debug)]
pub struct LACurve {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl LACurve {
    pub const fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { x1, y1, x2, y2 }
    }

    /// standard CSS ease-in-out.
    pub const fn ease_in_out() -> Self {
        Self::new(0.42, 0.0, 0.58, 1.0)
    }

    /// quadratica acelerada pra desacelerada.
    pub const fn ease_out_quad() -> Self {
        Self::new(0.25, 0.46, 0.45, 0.94)
    }

    /// Cubica sutil mais rapida que quad.
    pub const fn ease_out_cubic() -> Self {
        Self::new(0.215, 0.61, 0.355, 1.0)
    }

    /// Overshoot leve (back easing). Parametro de overshoot embutido
    /// nos control points — sem formula separada.
    pub const fn ease_out_back() -> Self {
        Self::new(0.175, 0.885, 0.32, 1.275)
    }

    /// Apple "smooth" = ease-in-out suavizado. Usado em transicoes neutras.
    pub const fn apple_smooth() -> Self {
        Self::new(0.42, 0.0, 0.58, 1.0)
    }

    /// Apple spring default cubic approx. Arranca rapido, desacelera
    /// suavemente. Usado em dropdowns + sheets iOS.
    pub const fn apple_spring_default() -> Self {
        Self::new(0.32, 0.72, 0.0, 1.0)
    }

    /// Avalia a curva em `t` (0.0..=1.0). Retorna o valor de saida y (0..=1).
    ///
    /// Usa Newton-Raphson pra inverter a componente x do bezier, entao
    /// calcula y com o parametro u encontrado.
    pub fn eval(&self, t: f32) -> f32 {
        if t <= 0.0 {
            return 0.0;
        }
        if t >= 1.0 {
            return 1.0;
        }

        // Achar u tal que bezier_x(u) == t via Newton-Raphson (6 iters suficiente).
        let mut u = t; // chute inicial
        for _ in 0..6 {
            let bx = self.bezier_component(u, self.x1, self.x2);
            let dbx = self.bezier_derivative(u, self.x1, self.x2);
            if dbx.abs() < 1e-6 {
                break;
            }
            u -= (bx - t) / dbx;
            u = u.clamp(0.0, 1.0);
        }

        self.bezier_component(u, self.y1, self.y2)
    }

    // Componente 1D de bezier cubico com P0=0, P1=c1, P2=c2, P3=1.
    #[inline]
    fn bezier_component(&self, t: f32, c1: f32, c2: f32) -> f32 {
        let mt = 1.0 - t;
        3.0 * mt * mt * t * c1 + 3.0 * mt * t * t * c2 + t * t * t
    }

    // Derivada da componente 1D.
    #[inline]
    fn bezier_derivative(&self, t: f32, c1: f32, c2: f32) -> f32 {
        let mt = 1.0 - t;
        3.0 * mt * mt * c1 + 6.0 * mt * t * (c2 - c1) + 3.0 * t * t * (1.0 - c2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_approx() {
        // Curve (0,0,1,1) = linear.
        let c = LACurve::new(0.0, 0.0, 1.0, 1.0);
        assert!((c.eval(0.5) - 0.5).abs() < 0.01, "eval={}", c.eval(0.5));
    }

    #[test]
    fn endpoints() {
        let c = LACurve::ease_out_cubic();
        assert!((c.eval(0.0) - 0.0).abs() < 1e-5);
        assert!((c.eval(1.0) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn apple_spring_monotonic_ish() {
        // apple_spring_default deve alcancar ~0.8 em t=0.3 (arranca rapido).
        let c = LACurve::apple_spring_default();
        let mid = c.eval(0.3);
        assert!(mid > 0.5, "spring deve arrancar rapido, got {mid}");
    }
}
