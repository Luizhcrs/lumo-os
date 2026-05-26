//! closed_form.rs - Spring amortecida 1D com solucao analitica fechada.
//!
//! Tres ramos de amortecimento:
//!   Underdamped  (zeta < 1)  — oscila em torno do target com envelope exp
//!   Critically   (zeta == 1) — converge sem overshoot na taxa maxima
//!   Overdamped   (zeta > 1)  — converge mais devagar que o critico, sem bounce
//!
//! Referencia: Inman, D.J. "Engineering Vibration", 3a ed., Secoes 1.3-1.4.
//!
//! Vantagem sobre Euler semi-implicito: determinismo total — value_at(t) e
//! identico independente de passo de integracao; sem drift acumulado.

/// Regime de amortecimento calculado dos parametros m, k, c.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DampingRegime {
    /// zeta < 1: oscila com envelope decrescente.
    Underdamped,
    /// zeta ~= 1 (diferenca < CRITICAL_EPS): converge sem overshoot ideal.
    Critical,
    /// zeta > 1: convergencia monotonica mais lenta que o critico.
    Overdamped,
}

/// Tolerancia para considerar zeta == 1 (critico).
const CRITICAL_EPS: f32 = 1e-4;

/// Spring amortecida com solucao fechada.
///
/// Campos publicos permitem ajuste de preset sem recriar o struct;
/// chame `recalc()` apos modificar mass/stiffness/damping.
#[derive(Clone, Copy, Debug)]
pub struct ClosedFormSpring {
    pub mass: f32,
    pub stiffness: f32,
    pub damping: f32,
    // --- cache calculado por recalc() ---
    omega_n: f32,
    zeta: f32,
    regime: DampingRegime,
}

/// Presets de uso comum na bar e no shell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpringPreset {
    /// Feedback tatil rapido. m=1, k=300, c=30. zeta ~0.87.
    TapFeedback,
    /// Abertura/fechamento de janela. m=1, k=170, c=22. zeta ~0.85.
    WindowOpenClose,
    /// Sheet que desliza. m=1, k=200, c=28. zeta ~0.99 (quase critico).
    SheetSlide,
    /// Drag-to-reveal. m=1, k=400, c=40. zeta ~1.0 (critico).
    DragToReveal,
}

impl ClosedFormSpring {
    /// Constroi a partir de parametros fisicos e cache omega_n/zeta/regime.
    pub fn new(mass: f32, stiffness: f32, damping: f32) -> Self {
        let mut s = Self {
            mass,
            stiffness,
            damping,
            omega_n: 0.0,
            zeta: 0.0,
            regime: DampingRegime::Underdamped,
        };
        s.recalc();
        s
    }

    /// Constroi a partir de um preset predefinido.
    pub fn from_preset(preset: SpringPreset) -> Self {
        match preset {
            SpringPreset::TapFeedback => Self::new(1.0, 300.0, 30.0),
            SpringPreset::WindowOpenClose => Self::new(1.0, 170.0, 22.0),
            SpringPreset::SheetSlide => Self::new(1.0, 200.0, 28.0),
            SpringPreset::DragToReveal => Self::new(1.0, 400.0, 40.0),
        }
    }

    /// Recalcula cache omega_n, zeta, regime.
    /// Deve ser chamado apos alterar mass/stiffness/damping manualmente.
    pub fn recalc(&mut self) {
        // omega_n = sqrt(k / m)
        self.omega_n = (self.stiffness / self.mass.max(1e-6)).sqrt();
        // zeta = c / (2 * sqrt(k * m))
        let denom = 2.0 * (self.stiffness * self.mass).sqrt();
        self.zeta = self.damping / denom.max(1e-6);
        self.regime = if (self.zeta - 1.0).abs() < CRITICAL_EPS {
            DampingRegime::Critical
        } else if self.zeta < 1.0 {
            DampingRegime::Underdamped
        } else {
            DampingRegime::Overdamped
        };
    }

    /// Posicao analitica em t segundos.
    ///
    /// Condicoes iniciais: posicao `from`, velocidade `initial_velocity`.
    /// Retorna posicao interpolada — quando t -> infinito converge para `to`.
    pub fn value_at(&self, t_secs: f32, from: f32, to: f32, initial_velocity: f32) -> f32 {
        // Deslocamento inicial relativo ao equilibrio (target).
        let x0 = from - to;
        let v0 = initial_velocity;

        let displacement = match self.regime {
            DampingRegime::Underdamped => {
                // omega_d = omega_n * sqrt(1 - zeta^2)
                let omega_d = self.omega_n * (1.0 - self.zeta * self.zeta).sqrt();
                // x(t) = exp(-zeta*omega_n*t) * (A*cos(omega_d*t) + B*sin(omega_d*t))
                // A = x0
                // B = (v0 + zeta*omega_n*x0) / omega_d
                let a = x0;
                let b = (v0 + self.zeta * self.omega_n * x0) / omega_d.max(1e-6);
                let envelope = (-self.zeta * self.omega_n * t_secs).exp();
                envelope * (a * (omega_d * t_secs).cos() + b * (omega_d * t_secs).sin())
            }
            DampingRegime::Critical => {
                // x(t) = exp(-omega_n*t) * (A + B*t)
                // A = x0
                // B = v0 + omega_n*x0
                let a = x0;
                let b = v0 + self.omega_n * x0;
                let envelope = (-self.omega_n * t_secs).exp();
                envelope * (a + b * t_secs)
            }
            DampingRegime::Overdamped => {
                // omega_d = omega_n * sqrt(zeta^2 - 1)
                let omega_d = self.omega_n * (self.zeta * self.zeta - 1.0).sqrt();
                // Raizes: r1 = -zeta*omega_n + omega_d, r2 = -zeta*omega_n - omega_d
                // x(t) = A*exp(r1*t) + B*exp(r2*t)
                // Condicoes: A + B = x0, A*r1 + B*r2 = v0
                // => A = (v0 - r2*x0) / (r1 - r2)
                //    B = x0 - A
                let r1 = -self.zeta * self.omega_n + omega_d;
                let r2 = -self.zeta * self.omega_n - omega_d;
                let denom = r1 - r2; // = 2*omega_d, sempre > 0
                let a = (v0 - r2 * x0) / denom.max(1e-6);
                let b = x0 - a;
                a * (r1 * t_secs).exp() + b * (r2 * t_secs).exp()
            }
        };

        to + displacement
    }

    /// Velocidade analitica em t segundos (derivada de value_at).
    pub fn velocity_at(&self, t_secs: f32, from: f32, to: f32, initial_velocity: f32) -> f32 {
        let x0 = from - to;
        let v0 = initial_velocity;

        match self.regime {
            DampingRegime::Underdamped => {
                let omega_d = self.omega_n * (1.0 - self.zeta * self.zeta).sqrt();
                let a = x0;
                let b = (v0 + self.zeta * self.omega_n * x0) / omega_d.max(1e-6);
                let zn = self.zeta * self.omega_n;
                let envelope = (-zn * t_secs).exp();
                // d/dt [envelope * (A*cos + B*sin)]
                //   = -zn*envelope*(A*cos + B*sin) + envelope*(-A*omega_d*sin + B*omega_d*cos)
                let cos_t = (omega_d * t_secs).cos();
                let sin_t = (omega_d * t_secs).sin();
                envelope * (-zn * (a * cos_t + b * sin_t) + omega_d * (-a * sin_t + b * cos_t))
            }
            DampingRegime::Critical => {
                let a = x0;
                let b = v0 + self.omega_n * x0;
                let envelope = (-self.omega_n * t_secs).exp();
                // d/dt [envelope * (A + B*t)]
                //   = -omega_n*envelope*(A + B*t) + envelope*B
                envelope * (-self.omega_n * (a + b * t_secs) + b)
            }
            DampingRegime::Overdamped => {
                let omega_d = self.omega_n * (self.zeta * self.zeta - 1.0).sqrt();
                let r1 = -self.zeta * self.omega_n + omega_d;
                let r2 = -self.zeta * self.omega_n - omega_d;
                let denom = r1 - r2;
                let a = (v0 - r2 * x0) / denom.max(1e-6);
                let b = x0 - a;
                // d/dt [A*exp(r1*t) + B*exp(r2*t)] = A*r1*exp(r1*t) + B*r2*exp(r2*t)
                a * r1 * (r1 * t_secs).exp() + b * r2 * (r2 * t_secs).exp()
            }
        }
    }

    /// Retorna true se o sistema esta assentado em t segundos com tolerancia epsilon.
    ///
    /// Verifica deslocamento E velocidade abaixo de epsilon.
    pub fn settled(
        &self,
        t_secs: f32,
        from: f32,
        to: f32,
        initial_velocity: f32,
        epsilon: f32,
    ) -> bool {
        let pos_err = (self.value_at(t_secs, from, to, initial_velocity) - to).abs();
        let vel = self.velocity_at(t_secs, from, to, initial_velocity).abs();
        pos_err < epsilon && vel < epsilon
    }

    // --- Acessores do cache (somente leitura) ---

    /// Frequencia natural omega_n = sqrt(k/m).
    pub fn omega_n(&self) -> f32 {
        self.omega_n
    }

    /// Razao de amortecimento zeta = c / (2 * sqrt(k*m)).
    pub fn zeta(&self) -> f32 {
        self.zeta
    }

    /// Regime de amortecimento calculado.
    pub fn regime(&self) -> DampingRegime {
        self.regime
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Helpers ---

    fn assert_near(a: f32, b: f32, tol: f32, msg: &str) {
        assert!(
            (a - b).abs() < tol,
            "{msg}: got {a}, expected ~{b}, tol={tol}"
        );
    }

    // --- Testes de convergencia ---

    #[test]
    fn underdamped_converge_para_target() {
        // TapFeedback: zeta ~0.87, underdamped.
        let s = ClosedFormSpring::from_preset(SpringPreset::TapFeedback);
        assert_eq!(s.regime(), DampingRegime::Underdamped);
        let v = s.value_at(2.0, 0.0, 1.0, 0.0);
        assert_near(v, 1.0, 0.001, "underdamped deve convergir em 2s");
    }

    #[test]
    fn critically_damped_sem_overshoot() {
        // DragToReveal: zeta ~1.0, critico.
        let s = ClosedFormSpring::from_preset(SpringPreset::DragToReveal);
        assert!(
            s.regime() == DampingRegime::Critical || s.regime() == DampingRegime::Overdamped,
            "DragToReveal deve ser critico ou overdamped"
        );
        // Sem overshoot: value_at nunca cruza alem de to=1.0 vindo de from=0.0.
        let mut max_v = 0.0f32;
        let from = 0.0f32;
        let to = 1.0f32;
        for i in 1..=200 {
            let t = i as f32 * 0.01;
            let v = s.value_at(t, from, to, 0.0);
            if v > max_v {
                max_v = v;
            }
        }
        assert!(
            max_v <= 1.001,
            "critically-damped nao deve overshoot: max={max_v}"
        );
    }

    #[test]
    fn overdamped_sem_overshoot_e_lento() {
        // m=1, k=100, c=30 => zeta = 30/(2*sqrt(100)) = 1.5 => overdamped.
        let s = ClosedFormSpring::new(1.0, 100.0, 30.0);
        assert_eq!(s.regime(), DampingRegime::Overdamped);
        let from = 0.0f32;
        let to = 1.0f32;
        // Sem overshoot.
        let mut max_v = 0.0f32;
        for i in 1..=500 {
            let t = i as f32 * 0.01;
            let v = s.value_at(t, from, to, 0.0);
            if v > max_v {
                max_v = v;
            }
        }
        assert!(max_v <= 1.001, "overdamped nao deve overshoot: max={max_v}");
        // Ainda nao chegou em 0.3s.
        let v_at_03 = s.value_at(0.3, from, to, 0.0);
        assert!(
            v_at_03 < 0.99,
            "overdamped deve ser lento em 0.3s: v={v_at_03}"
        );
    }

    #[test]
    fn velocidade_em_t0_igual_initial_velocity() {
        // Para qualquer regime, velocity_at(0) deve ser igual a initial_velocity.
        let presets = [
            SpringPreset::TapFeedback,
            SpringPreset::WindowOpenClose,
            SpringPreset::SheetSlide,
            SpringPreset::DragToReveal,
        ];
        for preset in presets {
            let s = ClosedFormSpring::from_preset(preset);
            let v_init = 3.5f32;
            let vel = s.velocity_at(0.0, 0.0, 1.0, v_init);
            assert_near(
                vel,
                v_init,
                0.01,
                &format!("velocity_at(0) preset={preset:?}"),
            );
        }
    }

    #[test]
    fn posicao_em_t0_igual_from() {
        // value_at(0) deve ser igual a from para qualquer regime.
        let presets = [
            SpringPreset::TapFeedback,
            SpringPreset::WindowOpenClose,
            SpringPreset::SheetSlide,
            SpringPreset::DragToReveal,
        ];
        for preset in presets {
            let s = ClosedFormSpring::from_preset(preset);
            let from = 0.3f32;
            let to = 1.0f32;
            let v = s.value_at(0.0, from, to, 0.0);
            assert_near(v, from, 1e-5, &format!("value_at(0) preset={preset:?}"));
        }
    }

    #[test]
    fn settled_em_t_grande() {
        // Todos presets devem estar assentados em 5 segundos com epsilon=0.001.
        let presets = [
            SpringPreset::TapFeedback,
            SpringPreset::WindowOpenClose,
            SpringPreset::SheetSlide,
            SpringPreset::DragToReveal,
        ];
        for preset in presets {
            let s = ClosedFormSpring::from_preset(preset);
            assert!(
                s.settled(5.0, 0.0, 1.0, 0.0, 0.001),
                "preset {preset:?} nao assentou em 5s"
            );
        }
    }

    #[test]
    fn preset_tap_feedback_regime_underdamped() {
        let s = ClosedFormSpring::from_preset(SpringPreset::TapFeedback);
        // m=1, k=300, c=30 => zeta = 30/(2*sqrt(300)) ~= 0.866
        assert_eq!(s.regime(), DampingRegime::Underdamped);
        assert!(
            (s.zeta() - 0.866).abs() < 0.01,
            "zeta TapFeedback: {}",
            s.zeta()
        );
    }

    #[test]
    fn preset_window_open_close_regime_underdamped() {
        let s = ClosedFormSpring::from_preset(SpringPreset::WindowOpenClose);
        // m=1, k=170, c=22 => zeta = 22/(2*sqrt(170)) ~= 0.844
        assert_eq!(s.regime(), DampingRegime::Underdamped);
        assert!(
            (s.zeta() - 0.844).abs() < 0.01,
            "zeta WindowOpenClose: {}",
            s.zeta()
        );
    }

    #[test]
    fn preset_sheet_slide_regime_near_critical() {
        let s = ClosedFormSpring::from_preset(SpringPreset::SheetSlide);
        // m=1, k=200, c=28 => zeta = 28/(2*sqrt(200)) ~= 0.990
        assert!(
            s.zeta() > 0.95,
            "SheetSlide deve ter zeta alto (near-critical): {}",
            s.zeta()
        );
    }

    #[test]
    fn preset_drag_to_reveal_regime_critical_or_overdamped() {
        let s = ClosedFormSpring::from_preset(SpringPreset::DragToReveal);
        // m=1, k=400, c=40 => zeta = 40/(2*sqrt(400)) = 40/40 = 1.0 (exato critico)
        assert!(
            s.regime() == DampingRegime::Critical || s.regime() == DampingRegime::Overdamped,
            "DragToReveal deve ser critico: regime={:?}",
            s.regime()
        );
        assert_near(s.zeta(), 1.0, 0.001, "DragToReveal zeta deve ser 1.0");
    }

    #[test]
    fn underdamped_com_velocity_inicial_positiva_chega_antes() {
        // Com velocidade inicial em direcao ao target, deve atingir target mais rapido.
        let s = ClosedFormSpring::from_preset(SpringPreset::TapFeedback);
        let from = 0.0f32;
        let to = 1.0f32;
        // Sem velocity: tempo para chegar perto do target.
        let mut t_reach_no_vel = 1.0f32;
        for i in 1..=100 {
            let t = i as f32 * 0.01;
            if (s.value_at(t, from, to, 0.0) - to).abs() < 0.05 {
                t_reach_no_vel = t;
                break;
            }
        }
        // Com velocity inicial em direcao ao target.
        let mut t_reach_with_vel = 1.0f32;
        for i in 1..=100 {
            let t = i as f32 * 0.01;
            if (s.value_at(t, from, to, 5.0) - to).abs() < 0.05 {
                t_reach_with_vel = t;
                break;
            }
        }
        assert!(
            t_reach_with_vel <= t_reach_no_vel,
            "com velocidade inicial deve chegar mais rapido: {t_reach_with_vel} vs {t_reach_no_vel}"
        );
    }

    #[test]
    fn overdamped_converge_em_tempo_suficiente() {
        let s = ClosedFormSpring::new(1.0, 100.0, 30.0);
        assert_eq!(s.regime(), DampingRegime::Overdamped);
        let v = s.value_at(10.0, 0.0, 1.0, 0.0);
        assert_near(v, 1.0, 0.001, "overdamped deve convergir em 10s");
    }

    #[test]
    fn recalc_atualiza_cache() {
        let mut s = ClosedFormSpring::new(1.0, 100.0, 20.0);
        let zeta_antes = s.zeta();
        s.damping = 40.0;
        s.recalc();
        let zeta_depois = s.zeta();
        assert!(zeta_depois > zeta_antes, "recalc deve atualizar zeta");
    }
}
