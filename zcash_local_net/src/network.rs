//! Structs and utility functions associated with local network configuration.

use std::{
    collections::HashSet,
    net::TcpListener,
    sync::{LazyLock, Mutex},
};

/// Process-wide set of ports already returned by [`pick_unused_port`].
///
/// Prevents two concurrent in-process callers from being handed the same port —
/// a race the previous `portpicker`-backed implementation allowed because it
/// picked-and-released the underlying socket *before* returning, so a second
/// caller could land on the just-freed port. Combined with kernel-assigned
/// ephemeral allocation (see [`pick_unused_port`]), this also makes
/// cross-process collisions vanishingly rare.
///
/// Ports are retained for the lifetime of the process. Tests are bounded and
/// the size of `u16` × 65k is negligible.
static RESERVED: LazyLock<Mutex<HashSet<u16>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

/// Acquire the registry lock, recovering from poisoning. The inner state is a
/// `HashSet<u16>` whose only mutations are `insert` / `contains` checks before
/// any panic can fire, so a poisoned lock holds a consistent set.
fn lock_reserved() -> std::sync::MutexGuard<'static, HashSet<u16>> {
    RESERVED.lock().unwrap_or_else(|e| e.into_inner())
}

const PICK_ATTEMPTS: usize = 64;

/// Returns a port that is currently free AND not already returned by another
/// concurrent caller in this process.
///
/// If `fixed_port` is `Some`, that exact port is reserved (panics if it is
/// already in use OR already reserved by another caller in this process).
///
/// Random allocation uses `TcpListener::bind("127.0.0.1:0")`, letting the
/// kernel assign an ephemeral port. Unlike `portpicker::pick_unused_port`
/// (which randomly samples `15000..25000` and bind-checks — a 10 000-port
/// range that suffers measurable birthday-paradox collisions when many
/// allocations run in parallel), kernel allocation is sequential and
/// TIME_WAIT-cooled, so two concurrent callers (even in different processes)
/// effectively never receive the same port.
#[must_use]
pub fn pick_unused_port(fixed_port: Option<u16>) -> u16 {
    let mut reserved = lock_reserved();

    if let Some(port) = fixed_port {
        assert!(
            !reserved.contains(&port),
            "Fixed port {port} already reserved by another caller in this process"
        );
        assert!(
            TcpListener::bind(("127.0.0.1", port)).is_ok(),
            "Fixed port {port} is not free"
        );
        reserved.insert(port);
        return port;
    }

    for _ in 0..PICK_ATTEMPTS {
        let listener =
            TcpListener::bind("127.0.0.1:0").expect("kernel failed to assign an ephemeral port");
        let port = listener.local_addr().expect("local_addr").port();
        // Drop now so the caller's child process can bind. Holding the socket
        // would only narrow the cross-process race but block our own spawn —
        // child processes (zebrad, zcashd, etc.) do not set SO_REUSEADDR.
        drop(listener);
        if reserved.insert(port) {
            return port;
        }
        // The kernel handed back a port we already reserved — possible if an
        // earlier reservation's caller never bound. Try again.
    }
    panic!("could not pick a fresh unreserved port after {PICK_ATTEMPTS} attempts");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    /// Concurrent callers must never receive the same port.
    ///
    /// Reproduces the original flake shape: 10 zebrad tests × 4 ports each
    /// occasionally collided in `portpicker`'s 15000-25000 random range.
    /// With kernel allocation + the in-process registry, the expected
    /// collision count over `THREADS × PICKS_PER_THREAD` allocations is zero.
    #[test]
    fn pick_unused_port_returns_unique_ports_under_concurrency() {
        const THREADS: usize = 32;
        const PICKS_PER_THREAD: usize = 8;

        let observed: Arc<Mutex<HashSet<u16>>> = Arc::new(Mutex::new(HashSet::new()));
        let mut handles = Vec::with_capacity(THREADS);
        for _ in 0..THREADS {
            let observed = Arc::clone(&observed);
            handles.push(thread::spawn(move || {
                for _ in 0..PICKS_PER_THREAD {
                    let port = pick_unused_port(None);
                    let mut o = observed.lock().expect("observed poisoned");
                    assert!(o.insert(port), "duplicate port {port} returned concurrently");
                }
            }));
        }
        for h in handles {
            h.join().expect("worker thread panicked");
        }
        assert_eq!(
            observed.lock().expect("observed poisoned").len(),
            THREADS * PICKS_PER_THREAD,
            "expected every concurrent pick to be unique"
        );
    }

    /// Repeat the concurrent-hammer test many times to catch low-probability
    /// races. The pre-fix collision rate of `portpicker`'s random allocator
    /// was ~5–10% per run; 32 trials of 32 picks would catch that with
    /// overwhelming probability (1 − 0.95^32 ≈ 0.81 → 1 − 0.95^(32·32) → 1).
    #[test]
    fn pick_unused_port_no_collisions_repeated() {
        const TRIALS: usize = 32;
        for trial in 0..TRIALS {
            let observed: Arc<Mutex<HashSet<u16>>> = Arc::new(Mutex::new(HashSet::new()));
            let mut handles = Vec::with_capacity(8);
            for _ in 0..8 {
                let observed = Arc::clone(&observed);
                handles.push(thread::spawn(move || {
                    for _ in 0..4 {
                        let port = pick_unused_port(None);
                        let mut o = observed.lock().expect("observed poisoned");
                        assert!(o.insert(port), "trial {trial}: duplicate port {port}");
                    }
                }));
            }
            for h in handles {
                h.join().expect("worker thread panicked");
            }
        }
    }

    /// Returned ports must be bindable. Catches an impl that hands back ports
    /// the OS still considers in use.
    #[test]
    fn returned_ports_are_bindable() {
        for _ in 0..16 {
            let port = pick_unused_port(None);
            let listener = TcpListener::bind(("127.0.0.1", port))
                .unwrap_or_else(|e| panic!("returned port {port} not bindable: {e}"));
            drop(listener);
        }
    }

    /// Mirrors the actual zebrad/zcashd usage shape: many concurrent
    /// "fake spawns" each pick 4 ports and bind all of them. If any bind
    /// fails or any port is duplicated across spawns, the test fails.
    /// This is the regression test for the reported zebrad RPC-readiness
    /// timeout caused by colliding port picks across parallel test runs.
    #[test]
    fn simulated_concurrent_spawns_have_unique_bindable_ports() {
        const SPAWNS: usize = 12;
        const PORTS_PER_SPAWN: usize = 4;

        let observed: Arc<Mutex<HashSet<u16>>> = Arc::new(Mutex::new(HashSet::new()));
        let mut handles = Vec::with_capacity(SPAWNS);
        for spawn_id in 0..SPAWNS {
            let observed = Arc::clone(&observed);
            handles.push(thread::spawn(move || {
                let mut listeners = Vec::with_capacity(PORTS_PER_SPAWN);
                for which in 0..PORTS_PER_SPAWN {
                    let port = pick_unused_port(None);
                    {
                        let mut o = observed.lock().expect("observed poisoned");
                        assert!(
                            o.insert(port),
                            "spawn {spawn_id} port slot {which}: duplicate port {port}"
                        );
                    }
                    let l = TcpListener::bind(("127.0.0.1", port)).unwrap_or_else(|e| {
                        panic!("spawn {spawn_id} could not bind port {port}: {e}")
                    });
                    listeners.push(l);
                }
                listeners
            }));
        }
        let mut all_listeners = Vec::with_capacity(SPAWNS * PORTS_PER_SPAWN);
        for h in handles {
            all_listeners.extend(h.join().expect("worker thread panicked"));
        }
        assert_eq!(
            all_listeners.len(),
            SPAWNS * PORTS_PER_SPAWN,
            "expected every simulated spawn to bind all its ports"
        );
    }

    /// Fixed-port reservation must reject double-reservation within the
    /// same process — protects against tests accidentally pinning the same
    /// constant port.
    #[test]
    #[should_panic(expected = "already reserved")]
    fn fixed_port_double_reservation_panics() {
        // Acquire a port the kernel says is free, then reserve it twice.
        let port = pick_unused_port(None);
        // First call after a random pick already inserted `port` into the
        // registry. Re-reserving via fixed_port should panic.
        let _ = pick_unused_port(Some(port));
    }
}
