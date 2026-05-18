//! animator.rs - LAAnimator<T>: driver de animacao duration-based.
//!
//! `LAAnimator` combina um `from`, `to`, uma curva (`AnimCurve`) e tempo
//! decorrido pra produzir um valor interpolado `T: LAInterpolable` a cada
//! tick. Nao mantem estado proprio alem do elapsed — simples, composavel.
//!
//! Diferente de `Spring` (que persegue target via fisica), `LAAnimator` tem
//! duracao fixa e curva deterministica — bom pra animacoes de UI discretas
//! (abre/fecha dropdown, fade-in).

use crate::closed_form::ClosedFormSpring;
use crate::easing::LACurve;
use crate::interpolate::LAInterpolable;
use crate::spring::Spring;

/// Tipo de curva que dirige o animator.
#[derive(Clone, Copy, Debug)]
pub enum AnimCurve {
    /// Curva cubic-bezier deterministia (duracao fixa).
    Bezier { curve: LACurve, duration: f32 },
    /// Spring fisica Euler semi-implicito (duracao intrinseca, sem upper bound exceto settled()).
    Spring(Spring),
    /// Spring analitica closed-form (3 ramos: under/critical/overdamped).
    /// `from_val` e `initial_velocity` armazenam condicoes iniciais fixas em t=0.
    ClosedForm {
        spring: ClosedFormSpring,
        from_val: f32,
        to_val: f32,
        initial_velocity: f32,
    },
}

/// Driver de animacao duration-based (ou spring) generico sobre qualquer T.
///
/// Usage:
/// ```ignore
/// let mut anim = LAAnimator::new(0.0f32, 1.0f32,
///     AnimCurve::Bezier { curve: LACurve::apple_spring_default(), duration: 0.28 });
/// // por frame:
/// let v: f32 = anim.tick(dt);
/// if anim.is_done() { /* parar 60Hz tick */ }
/// ```
#[derive(Clone, Debug)]
pub struct LAAnimator<T: LAInterpolable> {
    pub from: T,
    pub to: T,
    pub curve: AnimCurve,
    pub elapsed: f32,
}

impl<T: LAInterpolable> LAAnimator<T> {
    pub fn new(from: T, to: T, curve: AnimCurve) -> Self {
        Self { from, to, curve, elapsed: 0.0 }
    }

    /// Avanca o animator por `dt` segundos e retorna o valor atual.
    pub fn tick(&mut self, dt: f32) -> T {
        match &mut self.curve {
            AnimCurve::Bezier { curve, duration } => {
                self.elapsed = (self.elapsed + dt).min(*duration + 0.001);
                let t = (self.elapsed / *duration).clamp(0.0, 1.0);
                let progress = curve.eval(t);
                T::lerp(self.from, self.to, progress)
            }
            AnimCurve::Spring(spring) => {
                spring.tick(dt);
                // Spring mantem value; mapeia [0..1] interno pra [from..to].
                T::lerp(self.from, self.to, spring.value.clamp(0.0, 1.5))
            }
            AnimCurve::ClosedForm { spring, from_val, to_val, initial_velocity } => {
                self.elapsed += dt;
                // value_at retorna a posicao fisica; mapeia [from_val..to_val] -> [from..to].
                let phys = spring.value_at(self.elapsed, *from_val, *to_val, *initial_velocity);
                // Normaliza phys no intervalo [from_val..to_val] pra obter progress em [0..1].
                let span = *to_val - *from_val;
                let progress = if span.abs() > 1e-6 {
                    (phys - *from_val) / span
                } else {
                    1.0
                };
                T::lerp(self.from, self.to, progress.clamp(-0.5, 1.5))
            }
        }
    }

    /// True quando a animacao terminou e nao precisa mais de tick.
    pub fn is_done(&self) -> bool {
        match &self.curve {
            AnimCurve::Bezier { duration, .. } => self.elapsed >= *duration,
            AnimCurve::Spring(spring) => spring.settled(),
            AnimCurve::ClosedForm { spring, from_val, to_val, initial_velocity } => {
                // Epsilon de 0.002 adequado para animacoes visuais (sub-pixel).
                spring.settled(self.elapsed, *from_val, *to_val, *initial_velocity, 0.002)
            }
        }
    }

    /// Reseta pra reanimar do valor atual ate novo `to`.
    /// Preserva `from` como posicao atual de saida (pra nao pular).
    pub fn restart_to(&mut self, new_to: T) {
        // Captura valor atual antes de resetar.
        let current = self.tick(0.0);
        self.from = current;
        self.to = new_to;
        self.elapsed = 0.0;
        if let AnimCurve::Spring(spring) = &mut self.curve {
            spring.value = 0.0;
            spring.velocity = 0.0;
            spring.target = 1.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::closed_form::SpringPreset;
    use crate::easing::LACurve;

    #[test]
    fn bezier_anim_starts_at_from() {
        let mut a = LAAnimator::new(0.0f32, 1.0f32,
            AnimCurve::Bezier { curve: LACurve::ease_out_cubic(), duration: 0.3 });
        let v = a.tick(0.0);
        assert!(v < 0.05, "v={v}");
    }

    #[test]
    fn bezier_anim_ends_at_to() {
        let mut a = LAAnimator::new(0.0f32, 1.0f32,
            AnimCurve::Bezier { curve: LACurve::ease_out_cubic(), duration: 0.3 });
        a.tick(0.3);
        assert!(a.is_done());
        let v = a.tick(0.0);
        assert!((v - 1.0).abs() < 0.01, "v={v}");
    }

    #[test]
    fn spring_anim_settles() {
        let mut spring = Spring::smooth();
        spring.set_target(1.0);
        let mut a = LAAnimator::new(0.0f32, 1.0f32, AnimCurve::Spring(spring));
        for _ in 0..120 {
            a.tick(1.0 / 60.0);
            if a.is_done() {
                return;
            }
        }
        panic!("spring nao assentou em 120 frames");
    }

    #[test]
    fn closed_form_anim_starts_at_from() {
        let spring = ClosedFormSpring::from_preset(SpringPreset::WindowOpenClose);
        let mut a = LAAnimator::new(
            0.85f32, 1.0f32,
            AnimCurve::ClosedForm {
                spring,
                from_val: 0.85,
                to_val: 1.0,
                initial_velocity: 0.0,
            },
        );
        let v = a.tick(0.0);
        assert!((v - 0.85).abs() < 0.01, "deve comecar em from=0.85, got {v}");
    }

    #[test]
    fn closed_form_anim_converges_to_to() {
        let spring = ClosedFormSpring::from_preset(SpringPreset::WindowOpenClose);
        let mut a = LAAnimator::new(
            0.85f32, 1.0f32,
            AnimCurve::ClosedForm {
                spring,
                from_val: 0.85,
                to_val: 1.0,
                initial_velocity: 0.0,
            },
        );
        // Avanca 3 segundos em passos de frame.
        let mut v = 0.85f32;
        for _ in 0..180 {
            v = a.tick(1.0 / 60.0);
        }
        assert!((v - 1.0).abs() < 0.01, "deve convergir para 1.0, got {v}");
    }

    #[test]
    fn closed_form_anim_is_done_after_settle() {
        let spring = ClosedFormSpring::from_preset(SpringPreset::WindowOpenClose);
        let mut a = LAAnimator::new(
            0.85f32, 1.0f32,
            AnimCurve::ClosedForm {
                spring,
                from_val: 0.85,
                to_val: 1.0,
                initial_velocity: 0.0,
            },
        );
        // Avanca ate assentar (max 600 frames = 10s a 60Hz).
        for _ in 0..600 {
            a.tick(1.0 / 60.0);
            if a.is_done() {
                return;
            }
        }
        panic!("closed_form anim nao assentou em 10s");
    }
}
