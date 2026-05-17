//! Spring physics (Layer 4.1.9).
//!
//! Animacao Apple-style baseada em sistema massa-mola amortecida. Cada
//! `Spring` mantem `value` perseguindo `target` integrando aceleracao por
//! frame (Euler semi-implicito — barato e estavel pra dt < 16ms).
//!
//! Presets pensados em "feel":
//!   - `snappy`   : reage rapido, settle curto (botoes, tabs)
//!   - `smooth`   : critically damped, zero overshoot (transicoes neutras)
//!   - `bouncy`   : overshoot pronunciado (badges, celebracoes)
//!
//! Convencao: dt em segundos (delta de Instant::now()). Stiffness em
//! "rad/s ao quadrado", damping em "rad/s". Para critically damped
//! `damping = 2 * sqrt(stiffness * mass)`.

/// Mola amortecida 1D.
#[derive(Clone, Copy, Debug)]
pub struct Spring {
    pub stiffness: f32,
    pub damping: f32,
    pub mass: f32,
    pub value: f32,
    pub velocity: f32,
    pub target: f32,
}

impl Default for Spring {
    fn default() -> Self {
        Self::smooth()
    }
}

impl Spring {
    /// Constroi com stiffness/damping arbitrarios. `mass = 1.0`, value/target
    /// = 0 (caller pode setar via `set_value` / `set_target`).
    pub fn new(stiffness: f32, damping: f32) -> Self {
        Self {
            stiffness,
            damping,
            mass: 1.0,
            value: 0.0,
            velocity: 0.0,
            target: 0.0,
        }
    }

    /// Reage rapido, settle curto (~250ms pra delta 0→1).
    pub fn snappy() -> Self {
        Self::new(400.0, 26.0)
    }

    /// Critically damped, zero overshoot. Damping = 2*sqrt(stiffness*mass)
    /// para stiffness=300, mass=1 daria ~34.6; usamos 30 pra leve underdamp
    /// quase imperceptivel (~1% overshoot) que da "vida" ao movimento.
    pub fn smooth() -> Self {
        Self::new(300.0, 30.0)
    }

    /// Overshoot pronunciado, settle longo. Bom pra badges / "pop".
    pub fn bouncy() -> Self {
        Self::new(280.0, 18.0)
    }

    pub fn set_value(&mut self, v: f32) {
        self.value = v;
    }

    pub fn set_target(&mut self, t: f32) {
        self.target = t;
    }

    /// Pula direto pro target sem animar (util pra inicializacao).
    pub fn snap_to(&mut self, v: f32) {
        self.value = v;
        self.target = v;
        self.velocity = 0.0;
    }

    /// Integra uma iteracao com `dt` em segundos. Usa Euler semi-implicito:
    /// 1) calcula aceleracao com forca atual
    /// 2) atualiza velocity primeiro
    /// 3) usa nova velocity pra atualizar value
    /// Isso e mais estavel que Euler explicito pra molas rigidas.
    pub fn tick(&mut self, dt: f32) {
        // Clamp dt pra evitar explosao quando o frame demora muito (resize,
        // breakpoint, etc.) — limite ~32ms (~30fps).
        let dt = dt.min(0.032);

        let displacement = self.value - self.target;
        let force = -self.stiffness * displacement - self.damping * self.velocity;
        let accel = force / self.mass.max(0.0001);
        self.velocity += accel * dt;
        self.value += self.velocity * dt;
    }

    /// `true` quando velocity e displacement estao abaixo do threshold;
    /// caller pode pular o tick pra economizar CPU.
    pub fn settled(&self) -> bool {
        self.velocity.abs() < 0.001 && (self.value - self.target).abs() < 0.001
    }
}


// ----------------------------------------------------------------------------
// LA* aliases (A9-rename) -- Apple CoreAnimation-style namespace.
// ----------------------------------------------------------------------------

/// Alias Apple-style. Prefira `LASpring` em call sites novos.
pub type LASpring = Spring;

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    fn run_until_settled(s: &mut Spring, max_iters: usize) -> usize {
        for i in 0..max_iters {
            s.tick(1.0 / 60.0);
            if s.settled() {
                return i;
            }
        }
        max_iters
    }

    #[test]
    fn snap_to_clears_velocity() {
        let mut s = Spring::smooth();
        s.velocity = 5.0;
        s.snap_to(0.5);
        assert_eq!(s.value, 0.5);
        assert_eq!(s.target, 0.5);
        assert_eq!(s.velocity, 0.0);
        assert!(s.settled());
    }

    #[test]
    fn smooth_settles_under_one_second() {
        let mut s = Spring::smooth();
        s.set_target(1.0);
        let iters = run_until_settled(&mut s, 240);
        // 60fps * 1s = 60 frames. Smooth deve assentar bem antes.
        assert!(iters < 60, "smooth nao assentou em 1s: {iters} frames");
        assert!((s.value - 1.0).abs() < 0.01);
    }

    #[test]
    fn bouncy_overshoots() {
        let mut s = Spring::bouncy();
        s.set_target(1.0);
        let mut max_value: f32 = 0.0;
        for _ in 0..120 {
            s.tick(1.0 / 60.0);
            if s.value > max_value {
                max_value = s.value;
            }
        }
        assert!(max_value > 1.0, "bouncy nao deu overshoot: max={max_value}");
    }

    #[test]
    fn snappy_faster_than_smooth() {
        let mut a = Spring::snappy();
        let mut b = Spring::smooth();
        a.set_target(1.0);
        b.set_target(1.0);
        let ia = run_until_settled(&mut a, 240);
        let ib = run_until_settled(&mut b, 240);
        assert!(ia < ib, "snappy {ia} deveria assentar antes de smooth {ib}");
    }
}
