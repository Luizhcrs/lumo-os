//! spring.rs - Spring: mola amortecida 1D.
//!
//! Euler semi-implicito: barato, estavel para dt < 32ms (clamped).
//! Presets mapeam o "feel" Apple CoreAnimation:
//!   snappy   : resposta rapida, settle ~250ms
//!   smooth   : critically damped, zero overshoot
//!   bouncy   : underdamped, overshoot pronunciado
//!   interactive: alta responsividade, damping forte (tracking dedo/cursor)

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

    /// Resposta rapida, settle curto (~250ms pra delta 0->1).
    /// stiffness=400, damping=26 => zeta~0.65, underdamped rapido.
    pub fn snappy() -> Self {
        Self::new(400.0, 26.0)
    }

    /// Critically damped, zero overshoot (~350ms).
    /// damping ligeiramente abaixo do critico pra dar "vida" sem bounce visivel.
    pub fn smooth() -> Self {
        Self::new(300.0, 30.0)
    }

    /// Underdamped, overshoot pronunciado. Bom pra badges / pop.
    pub fn bouncy() -> Self {
        Self::new(280.0, 18.0)
    }

    /// Tracking de input: response=0.15s, damping alto.
    /// Simula Apple UISpringTimingParameters(mass:1, stiffness:440, damping:74).
    /// Excelente pra cursor follow ou rubber-band.
    pub fn interactive() -> Self {
        Self::new(440.0, 74.0)
    }

    pub fn set_value(&mut self, v: f32) {
        self.value = v;
    }

    pub fn set_target(&mut self, t: f32) {
        self.target = t;
    }

    /// Pula direto pro target sem animar (inicializacao).
    pub fn snap_to(&mut self, v: f32) {
        self.value = v;
        self.target = v;
        self.velocity = 0.0;
    }

    /// Integra um frame. dt em segundos; clamp a 32ms pra evitar explosao.
    pub fn tick(&mut self, dt: f32) {
        let dt = dt.min(0.032);
        let displacement = self.value - self.target;
        let force = -self.stiffness * displacement - self.damping * self.velocity;
        let accel = force / self.mass.max(0.0001);
        self.velocity += accel * dt;
        self.value += self.velocity * dt;
    }

    /// True quando animacao convergiu (caller pode parar 60Hz tick).
    pub fn settled(&self) -> bool {
        self.velocity.abs() < 0.001 && (self.value - self.target).abs() < 0.001
    }
}

/// Alias Apple-style (LASpring = Spring). Prefira este em call sites novos.
pub type LASpring = Spring;

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
        assert_eq!(s.velocity, 0.0);
        assert!(s.settled());
    }

    #[test]
    fn smooth_settles_under_one_second() {
        let mut s = Spring::smooth();
        s.set_target(1.0);
        let iters = run_until_settled(&mut s, 240);
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

    #[test]
    fn interactive_settles_fast() {
        let mut s = Spring::interactive();
        s.set_target(1.0);
        let iters = run_until_settled(&mut s, 240);
        // interactive deve assentar em < 30 frames (~500ms) por design.
        assert!(iters < 120, "interactive demorou: {iters} frames");
    }
}
