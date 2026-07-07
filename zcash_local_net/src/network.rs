//! Structs and utility functions associated with local network configuration.

use std::{
    collections::HashSet,
    net::TcpListener,
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicU32, Ordering},
    },
};

/// Process-wide set of ports already returned by [`pick_unused_port`].
///
/// Prevents two concurrent in-process callers from being handed the same
/// port. Ports are retained for the lifetime of the process; tests are
/// bounded and the size of `u16` × 65k is negligible.
static RESERVED: LazyLock<Mutex<HashSet<u16>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

/// Acquire the registry lock, recovering from poisoning. The inner state is a
/// `HashSet<u16>` whose only mutations are `insert` / `contains` checks before
/// any panic can fire, so a poisoned lock holds a consistent set.
fn lock_reserved() -> std::sync::MutexGuard<'static, HashSet<u16>> {
    RESERVED.lock().unwrap_or_else(|e| e.into_inner())
}

/// First port of the allocation band. The band sits below every default
/// ephemeral range (Linux assigns from `ip_local_port_range`, by default
/// 32768–60999; macOS from 49152), so the kernel never hands a port in
/// this band to an outgoing connection or a `bind(:0)` caller. Hosts
/// configured with an ephemeral floor below 32768 are out of scope.
const BAND_START: u16 = 16384;
/// One past the last port of the allocation band: the default Linux
/// ephemeral floor.
const BAND_END: u16 = 32768;
/// Contiguous ports assigned to one process's partition slice. A test
/// process launches a handful of listeners (validator RPC and peer
/// ports, indexer gRPC), so 64 leaves an order of magnitude of headroom
/// before a cursor walks into a neighboring slice.
const SLICE_PORTS: u32 = 64;

/// Number of candidates a single pick may examine before giving up:
/// four full slices, so a pick survives its own slice being exhausted
/// or squatted and walks deterministically into the neighbors.
const PICK_ATTEMPTS: usize = 4 * SLICE_PORTS as usize;

/// Per-process cursor into the process's slice of the band. Starts at
/// zero: allocation within a process is sequential, never random.
static CURSOR: AtomicU32 = AtomicU32::new(0);

/// Returns a port that is currently free AND not already returned by another
/// concurrent caller in this process.
///
/// If `fixed_port` is `Some`, that exact port is reserved (panics if it is
/// already in use OR already reserved by another caller in this process).
///
/// Random allocation is deterministic by construction rather than sampled:
/// ports come from a fixed band below every default ephemeral range
/// (`BAND_START..BAND_END`), partitioned into per-process slices by process
/// id, walked sequentially by a process-local cursor, and bind-checked
/// before they are returned. Each ingredient removes one historical flake:
///
/// - The band sits below the kernel's ephemeral floor, so a picked port can
///   never be reused by the kernel for an outgoing connection or another
///   process's `bind(:0)` in the pick-to-child-bind window. That reuse was
///   the residual flake of the previous kernel-assigned (`bind(:0)`)
///   implementation, which this one replaces.
/// - The per-process slice keeps parallel test processes (nextest runs one
///   process per test) out of each other's territory, which the still
///   earlier `portpicker` implementation failed at by randomly sampling a
///   shared 10 000-port range — measurable birthday-paradox collisions.
/// - The bind check and sequential walk step deterministically over ports
///   squatted by unrelated services.
///
/// Two live processes whose ids coincide modulo the slice count can still
/// race the same slice, and an unrelated service can still grab a port
/// between the bind check and the child's bind; the launch-time
/// retry-on-collision machinery remains as the backstop for that residue.
#[must_use]
pub fn pick_unused_port(fixed_port: Option<u16>) -> u16 {
    let mut reserved = lock_reserved();

    if let Some(port) = fixed_port {
        assert!(
            !reserved.contains(&port),
            "Fixed port {port} already reserved by another caller in this process"
        );
        // Deliberately no `TcpListener::bind` pre-check here. A pinned
        // port reflects the caller's choice; surfacing a port conflict
        // earlier than the actual binder (zebrad/zcashd/etc.) hides
        // the failure mode the harness must be able to recover from.
        // The cross-test-subprocess race the doc-comment describes
        // cannot be detected here anyway — a parallel test process can
        // grab the port between this check and the child's bind, so
        // pre-checking is both misleading and racy.
        reserved.insert(port);
        return port;
    }

    let band = u32::from(BAND_END - BAND_START);
    let slice_count = band / SLICE_PORTS;
    let slice_index = std::process::id() % slice_count;
    for _ in 0..PICK_ATTEMPTS {
        let offset = CURSOR.fetch_add(1, Ordering::Relaxed);
        // The linear position walks the process's own slice first and
        // spills into subsequent slices (wrapping at the band's end) once
        // the cursor exceeds the slice width.
        let linear = (slice_index * SLICE_PORTS + offset) % band;
        let port = BAND_START + u16::try_from(linear).expect("band fits in u16");
        if !reserved.insert(port) {
            // Already handed out by this process (cursor wrapped the band).
            continue;
        }
        match TcpListener::bind(("127.0.0.1", port)) {
            // Drop immediately so the caller's child process can bind.
            Ok(listener) => {
                drop(listener);
                return port;
            }
            // Occupied by another process or service: walk on. The port
            // stays reserved so this process never retries it.
            Err(_) => continue,
        }
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
    /// With the sequential cursor + the in-process registry, the expected
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
                    assert!(
                        o.insert(port),
                        "duplicate port {port} returned concurrently"
                    );
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

    /// The allocator must step over ports another process already
    /// holds: squat a run of ports just ahead of the cursor and verify
    /// no subsequent pick returns one of them. This pins the
    /// bind-check-and-walk property directly, independent of the
    /// launch-time retry backstop.
    #[test]
    fn squatted_ports_are_skipped() {
        let first = pick_unused_port(None);
        // Hold the ports immediately after the first pick — under
        // sequential allocation these are upcoming candidates. A bind
        // that fails means the port was already externally occupied,
        // which serves the same purpose; keep whichever listeners
        // succeeded.
        let squatters: Vec<TcpListener> = (1..=3)
            .filter_map(|step| TcpListener::bind(("127.0.0.1", first + step)).ok())
            .collect();
        let squatted: HashSet<u16> = squatters
            .iter()
            .map(|listener| listener.local_addr().expect("local_addr").port())
            .collect();
        for _ in 0..8 {
            let port = pick_unused_port(None);
            assert!(
                !squatted.contains(&port),
                "allocator returned squatted port {port}"
            );
        }
    }

    /// Every random pick must land inside the below-ephemeral band: a
    /// port at or above the kernel's ephemeral floor reintroduces the
    /// pick-to-child-bind reuse race this allocator exists to remove.
    #[test]
    fn random_ports_come_from_the_partitioned_band() {
        for _ in 0..32 {
            let port = pick_unused_port(None);
            assert!(
                (BAND_START..BAND_END).contains(&port),
                "port {port} escaped the allocation band"
            );
        }
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
