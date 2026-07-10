//! Per-listener front proxies: the canonical public endpoints of the
//! managed backends.
//!
//! Traffic issued *inside* a launch — the regtest launch-mine, the
//! readiness probes — used to dial a backend's real port before any
//! external tap could exist, and no userspace relay can interpose a
//! connection between endpoints that already know each other's real
//! address. The inversion implemented here dissolves that blind spot:
//! a front (the crate-internal `Front` relay) binds `127.0.0.1:0`
//! *before* its backend starts, every
//! port and address accessor publishes the front, and the backend's
//! real endpoint stays a private detail of launch plumbing. All
//! clients — the harness's own launch-time clients included — cross
//! the front for the backend's entire lifespan, by construction.
//!
//! A front is a transparent TCP relay with a registration point for a
//! [`FrontObserver`], which receives one [`ChunkEvent`] per relayed
//! chunk. The default is passthrough: no observer, no behavioral
//! difference. The relay and the observer hook are built from std and
//! tokio primitives alone, and they depend only on the crate-internal
//! backend abstraction (`crate::backend::Backend`) — never on
//! `std::process` types or same-host assumptions.
//!
//! Each front drives its relay on a **dedicated thread** with its own
//! single-threaded tokio runtime. Relaying must not depend on the
//! launching runtime staying responsive: a consumer that blocks its
//! runtime thread — this crate's own wallet layer synchronously waits
//! on `zcash-devtool` subprocesses whose traffic crosses the fronts —
//! would otherwise starve the relay and deadlock against it.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream, tcp};

use crate::backend::Backend;

/// Direction of one relayed chunk, relative to the backend behind the
/// front.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// From the connecting client toward the backend.
    ToBackend,
    /// From the backend toward the connecting client.
    ToClient,
}

/// One relayed chunk, as delivered to a [`FrontObserver`].
#[derive(Clone, Debug)]
pub struct ChunkEvent {
    /// When the chunk crossed the front.
    pub at: std::time::SystemTime,
    /// Which connection through the front carried the chunk. Ids count
    /// up from zero per front, in accept order.
    pub connection: u64,
    /// Which way the chunk was traveling.
    pub direction: Direction,
    /// The chunk's bytes, copied out of the relay buffer.
    pub payload: Vec<u8>,
}

impl ChunkEvent {
    /// Byte count of the relayed chunk.
    pub fn byte_count(&self) -> usize {
        self.payload.len()
    }
}

/// Observer of the traffic crossing a front.
///
/// Register one through the launch config
/// ([`crate::validator::zebrad::ZebradConfig::rpc_front_observer`],
/// [`crate::indexer::zainod::ZainodConfig::grpc_front_observer`]) so it
/// is in place before the backend starts — that is what makes the
/// launch window observable. `on_chunk` runs on the front's relay
/// thread, so it should return promptly; recording consumers typically
/// push the event somewhere and return.
pub trait FrontObserver: Send + Sync {
    /// Called once per relayed chunk, after the chunk has been
    /// forwarded.
    fn on_chunk(&self, event: &ChunkEvent);
}

impl std::fmt::Debug for dyn FrontObserver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("dyn FrontObserver")
    }
}

/// A front proxy for one backend listener: the listener's canonical
/// public endpoint.
///
/// Bound on `127.0.0.1:0` before the backend starts, so the OS assigns
/// the public port atomically — no check-then-bind race exists on the
/// public surface. Connections accepted before the upstream is known
/// are held until launch plumbing calls [`Front::point_at`]; the first
/// clients through are the harness's own readiness poll loops, which
/// tolerate the wait by design.
///
/// A front is owned by its backend's handle and stops with it: its
/// `Drop` signals the relay thread, which closes the public listener
/// and cancels in-flight relay connections by dropping its runtime.
pub(crate) struct Front {
    public_addr: SocketAddr,
    upstream: Arc<OnceLock<SocketAddr>>,
    shutdown: Arc<AtomicBool>,
    relay_thread: Option<std::thread::JoinHandle<()>>,
}

impl Front {
    /// Bind a front on `127.0.0.1:0` — call this *before* starting the
    /// backend. `observer` is the registration point for the traffic
    /// tap; `None` is the passthrough default.
    pub(crate) fn bind(observer: Option<Arc<dyn FrontObserver>>) -> std::io::Result<Front> {
        // The relay gets its own single-threaded runtime, driven by a
        // dedicated thread below, so the front stays live even while
        // the launching runtime is blocked (see the module docs).
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()?;

        let std_listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        std_listener.set_nonblocking(true)?;
        let public_addr = std_listener.local_addr()?;
        let listener = {
            // `from_std` registers with the runtime's reactor, so it
            // must run inside the relay runtime's context.
            let _guard = runtime.enter();
            TcpListener::from_std(std_listener)?
        };

        let upstream: Arc<OnceLock<SocketAddr>> = Arc::new(OnceLock::new());
        let shutdown = Arc::new(AtomicBool::new(false));

        let accept_upstream = Arc::clone(&upstream);
        let accept_shutdown = Arc::clone(&shutdown);
        let relay_thread = std::thread::Builder::new()
            .name(format!("front-{}", public_addr.port()))
            .spawn(move || {
                runtime.block_on(async move {
                    let mut next_connection: u64 = 0;
                    loop {
                        let Ok((inbound, _)) = listener.accept().await else {
                            break;
                        };
                        if accept_shutdown.load(Ordering::SeqCst) {
                            break;
                        }
                        let connection = next_connection;
                        next_connection += 1;
                        tokio::spawn(relay_connection(
                            inbound,
                            Arc::clone(&accept_upstream),
                            connection,
                            observer.clone(),
                        ));
                    }
                });
                // block_on returned: the relay runtime drops here,
                // closing the listener and cancelling any in-flight
                // relay tasks with it.
            })?;

        Ok(Front {
            public_addr,
            upstream,
            shutdown,
            relay_thread: Some(relay_thread),
        })
    }

    /// The front's public address — what every accessor publishes.
    pub(crate) fn public_addr(&self) -> SocketAddr {
        self.public_addr
    }

    /// The front's public port.
    pub(crate) fn public_port(&self) -> u16 {
        self.public_addr.port()
    }

    /// Discover the backend's raw endpoint through the backend
    /// abstraction and point the relay at it. `listener` indexes the
    /// backend's declared listener order
    /// (`Backend::listener_endpoints`). Call once, after the backend
    /// is ready; held connections proceed from here.
    pub(crate) fn point_at<B: Backend>(&self, backend: &B, listener: usize) {
        let endpoints = backend.listener_endpoints();
        let upstream = *endpoints.get(listener).unwrap_or_else(|| {
            panic!(
                "backend declares {} listener endpoint(s), front asked for index {listener}",
                endpoints.len()
            )
        });
        self.upstream
            .set(upstream)
            .expect("a front's upstream is pointed exactly once");
    }
}

impl Drop for Front {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        // Wake the accept loop with a throwaway connection so it
        // observes the flag. A connect error means the relay thread is
        // already gone, which is the goal state.
        let _ = std::net::TcpStream::connect(self.public_addr);
        if let Some(relay_thread) = self.relay_thread.take() {
            // The join is prompt — the accept loop breaks on the wake
            // connection — and its result carries nothing actionable:
            // an Err only repeats a relay-thread panic at teardown.
            let _ = relay_thread.join();
        }
    }
}

impl std::fmt::Debug for Front {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Front")
            .field("public_addr", &self.public_addr)
            .field("upstream", &self.upstream.get())
            .finish_non_exhaustive()
    }
}

/// Relay one accepted connection: hold it until the upstream is known,
/// dial the backend, then pump both directions until either side
/// closes.
async fn relay_connection(
    inbound: TcpStream,
    upstream: Arc<OnceLock<SocketAddr>>,
    connection: u64,
    observer: Option<Arc<dyn FrontObserver>>,
) {
    // Accept-and-hold: the front binds before the backend starts, so a
    // connection may arrive before launch plumbing has pointed the
    // front at the backend's real endpoint.
    let upstream_addr = loop {
        if let Some(addr) = upstream.get() {
            break *addr;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    };
    let Ok(outbound) = TcpStream::connect(upstream_addr).await else {
        // The backend is not accepting (not bound yet, or already
        // gone). Dropping the inbound connection tells the client to
        // retry — the first clients through a fresh front are the
        // harness's own readiness poll loops.
        return;
    };
    let (inbound_read, inbound_write) = inbound.into_split();
    let (outbound_read, outbound_write) = outbound.into_split();
    let to_backend = tokio::spawn(pump(
        inbound_read,
        outbound_write,
        Direction::ToBackend,
        connection,
        observer.clone(),
    ));
    let to_client = tokio::spawn(pump(
        outbound_read,
        inbound_write,
        Direction::ToClient,
        connection,
        observer,
    ));
    // A pump task only errs if it panicked or was cancelled at relay
    // shutdown; either way the connection is over and there is nothing
    // to report, so the join results carry no information.
    let _ = to_backend.await;
    let _ = to_client.await;
}

/// Copy bytes one way between the relay's stream halves, delivering
/// each chunk to the observer after it has been forwarded.
async fn pump(
    mut reader: tcp::OwnedReadHalf,
    mut writer: tcp::OwnedWriteHalf,
    direction: Direction,
    connection: u64,
    observer: Option<Arc<dyn FrontObserver>>,
) {
    let mut buf = [0u8; 8192];
    loop {
        match reader.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if writer.write_all(&buf[..n]).await.is_err() {
                    break;
                }
                if let Some(observer) = &observer {
                    observer.on_chunk(&ChunkEvent {
                        at: std::time::SystemTime::now(),
                        connection,
                        direction,
                        payload: buf[..n].to_vec(),
                    });
                }
            }
        }
    }
    // Propagate this direction's EOF to the peer; the connection is
    // over either way, so a shutdown error carries no information.
    let _ = writer.shutdown().await;
}
