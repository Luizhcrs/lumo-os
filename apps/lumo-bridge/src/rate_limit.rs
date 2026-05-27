//! rate_limit.rs — token bucket per-peer simples pra evitar flood IPC.
//!
//! Aplica-se a TODAS rotas protegidas. Health publico nao limita.
//! Limites: 100 req/s sustained, burst 50. Configuravel via env:
//!   LUMO_BRIDGE_RPS (default 100)
//!   LUMO_BRIDGE_BURST (default 50)
//!
//! Bucket por peer SocketAddr. GC periodico nao implementado:
//! HashMap cresce O(n_peers). Bridge e local-only entao n e baixo.
//! Se virar problema, adicionar LRU cap.
//!
//! Retorna 429 Too Many Requests quando exausto.
//! Header `Retry-After: 1` indica wait time.
//!
//! 0.0.0.0 bind = qualquer cliente local pode chamar; rate limit
//! reduz risco de spike CPU/IPC por cliente buggy ou malicioso local.
//! Nao substitui auth bearer; soma camada.
//!
//! Justificativa per-IP em vez de global: cliente unico bugado nao
//! pode comer quota de outros clientes.

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Mutex;
use std::time::Instant;

pub struct Bucket {
    tokens: f64,
    last_refill: Instant,
}

pub struct RateLimiter {
    rps: f64,
    burst: f64,
    buckets: Mutex<HashMap<SocketAddr, Bucket>>,
}

impl RateLimiter {
    pub fn new(rps: f64, burst: f64) -> Self {
        Self {
            rps,
            burst,
            buckets: Mutex::new(HashMap::new()),
        }
    }

    pub fn from_env() -> Self {
        let rps = std::env::var("LUMO_BRIDGE_RPS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(100.0);
        let burst = std::env::var("LUMO_BRIDGE_BURST")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(50.0);
        Self::new(rps, burst)
    }

    /// Tenta consumir 1 token. Retorna true se permitido.
    /// Resilient a mutex poison (review R3): se outra thread panicou
    /// dentro do lock, recovery via into_inner em vez de propagar panic.
    pub fn check(&self, peer: SocketAddr) -> bool {
        let now = Instant::now();
        let mut buckets = match self.buckets.lock() {
            Ok(g) => g,
            Err(poison) => {
                tracing::warn!("rate-limit mutex envenenado, recovering inner");
                poison.into_inner()
            }
        };
        let bucket = buckets.entry(peer).or_insert_with(|| Bucket {
            tokens: self.burst,
            last_refill: now,
        });
        let elapsed = now.duration_since(bucket.last_refill).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * self.rps).min(self.burst);
        bucket.last_refill = now;
        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

pub async fn require_rate_limit(
    peer_info: Option<ConnectInfo<SocketAddr>>,
    axum::extract::Extension(limiter): axum::extract::Extension<std::sync::Arc<RateLimiter>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let peer = peer_info
        .map(|ConnectInfo(p)| p)
        .unwrap_or_else(|| SocketAddr::from(([127, 0, 0, 1], 0)));
    if limiter.check(peer) {
        next.run(req).await
    } else {
        (
            StatusCode::TOO_MANY_REQUESTS,
            [("Retry-After", "1")],
            "rate limit exceeded\n",
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn peer(p: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), p)
    }

    #[test]
    fn burst_allows_n_requests() {
        let rl = RateLimiter::new(1.0, 5.0);
        let p = peer(1000);
        for _ in 0..5 {
            assert!(rl.check(p));
        }
        assert!(!rl.check(p), "6th deve exceder");
    }

    #[test]
    fn separate_peers_dont_share_bucket() {
        let rl = RateLimiter::new(1.0, 3.0);
        let a = peer(1001);
        let b = peer(1002);
        for _ in 0..3 {
            assert!(rl.check(a));
        }
        assert!(!rl.check(a));
        for _ in 0..3 {
            assert!(rl.check(b));
        }
    }

    #[test]
    fn refill_after_time() {
        let rl = RateLimiter::new(100.0, 2.0);
        let p = peer(1003);
        assert!(rl.check(p));
        assert!(rl.check(p));
        assert!(!rl.check(p));
        std::thread::sleep(std::time::Duration::from_millis(30));
        assert!(rl.check(p), "depois de 30ms a 100rps deve ter ~3 tokens");
    }

    #[test]
    fn refill_caps_at_burst() {
        let rl = RateLimiter::new(1000.0, 4.0);
        let p = peer(1004);
        std::thread::sleep(std::time::Duration::from_millis(50));
        // Burst nao excede 4 mesmo apos longo idle.
        for _ in 0..4 {
            assert!(rl.check(p));
        }
        assert!(!rl.check(p));
    }

    #[test]
    fn from_env_defaults_when_unset() {
        // Garante que sem env vars nao panica.
        std::env::remove_var("LUMO_BRIDGE_RPS");
        std::env::remove_var("LUMO_BRIDGE_BURST");
        let rl = RateLimiter::from_env();
        let p = peer(1005);
        assert!(rl.check(p));
    }

    #[test]
    fn check_recovers_from_poisoned_mutex() {
        use std::sync::Arc;
        let rl = Arc::new(RateLimiter::new(10.0, 2.0));
        // Forca poison: thread panica enquanto segura lock.
        let rl_clone = Arc::clone(&rl);
        let _ = std::thread::spawn(move || {
            let _guard = rl_clone.buckets.lock().expect("first lock");
            panic!("envenena");
        })
        .join();
        // Apos poison, check ainda funciona via into_inner recovery.
        let p = peer(1006);
        assert!(rl.check(p), "check apos poison deve continuar funcionando");
    }
}
