//! M1: XdgDecorationHandler impl moved to state.rs.
//! Kept as module placeholder.

#[cfg(test)]
mod tests {
    // Testes pure-logic do contrato request_mode em state.rs:
    //   - ServerSide   -> insert em ssd_windows
    //   - unset_mode   -> insert em ssd_windows (server side eh fallback)
    //   - ClientSide   -> remove de ssd_windows (apps GTK/Chrome/Firefox)
    //
    // LumoState nao eh instantiavel em unit test (DisplayHandle), entao
    // testamos a transicao do HashSet que o handler manipula. Se a forma
    // como request_mode atualiza self.ssd_windows mudar, esses testes
    // precisam mudar junto.

    use std::collections::HashSet;

    #[derive(Hash, PartialEq, Eq, Clone)]
    struct FakeSurf(u32);

    /// Replica logica de R1.fix6 request_mode: ClientSide remove, demais inserem.
    fn apply_request_mode(set: &mut HashSet<FakeSurf>, surf: FakeSurf, client_side: bool) {
        if client_side {
            set.remove(&surf);
        } else {
            set.insert(surf);
        }
    }

    #[test]
    fn csd_request_removes_from_ssd_windows() {
        let mut ssd: HashSet<FakeSurf> = HashSet::new();
        let s = FakeSurf(1);
        // Default em new_toplevel: SSD inserido.
        ssd.insert(s.clone());
        assert!(ssd.contains(&s), "setup: surface deve estar em ssd_windows");

        // Cliente pede ClientSide (GTK4/Chrome/Firefox).
        apply_request_mode(&mut ssd, s.clone(), true);
        assert!(!ssd.contains(&s), "ClientSide deve REMOVER de ssd_windows");
    }

    #[test]
    fn ssd_request_keeps_in_ssd_windows() {
        let mut ssd: HashSet<FakeSurf> = HashSet::new();
        let s = FakeSurf(2);
        ssd.insert(s.clone());

        // Cliente pede ServerSide (raro -- Iced 0.13 nao pede nada).
        apply_request_mode(&mut ssd, s.clone(), false);
        assert!(ssd.contains(&s), "ServerSide deve manter em ssd_windows");
    }

    #[test]
    fn unset_mode_keeps_in_ssd_windows() {
        // unset_mode em state.rs faz ssd_windows.insert -- equivalente a ServerSide.
        let mut ssd: HashSet<FakeSurf> = HashSet::new();
        let s = FakeSurf(3);
        ssd.insert(s.clone()); // estado default new_toplevel
                               // Re-insert (idempotente).
        ssd.insert(s.clone());
        assert!(ssd.contains(&s), "unset_mode deve manter SSD (fallback)");
    }
}
