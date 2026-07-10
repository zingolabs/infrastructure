//! The minimal backend abstraction the front-proxy machinery sees.
//!
//! Everything the harness reifies as a process today may become a
//! (podman) container tomorrow. The front-proxy and observer layers in
//! [`crate::front`] therefore depend only on this trait, which states
//! the three capabilities a managed backend must offer: the start/stop
//! lifecycle (inherited from [`Process`]), log access for readiness
//! parsing, and — once ready — the backend's raw listener endpoints as
//! full socket addresses.
//!
//! Endpoints are [`SocketAddr`], never bare `u16` ports: a bare port
//! silently assumes loopback and same-host, and a container backend's
//! published endpoints (`podman port`-style ephemeral host mappings)
//! need not be either. Nothing in this trait or in the front layer may
//! name `std::process` types, PIDs, or same-host assumptions; the
//! review litmus is that a hypothetical `ContainerBackend` could
//! implement this trait without changing one line of [`crate::front`].
//!
//! The trait is deliberately crate-internal. The raw endpoints it
//! reveals exist only so launch plumbing can point a [`crate::front::Front`]
//! at its backend; every published accessor returns the front's
//! address, and exporting the raw endpoints would reopen the hole the
//! fronts close.

use std::net::SocketAddr;

use crate::process::Process;

/// A managed backend as the front-proxy layer is allowed to see it.
///
/// [`Process`] supplies the start/stop lifecycle; this trait adds the
/// two capabilities the front machinery needs beyond it. The process
/// wrappers ([`crate::validator::zebrad::Zebrad`],
/// [`crate::indexer::zainod::Zainod`]) are the only implementors
/// today; container backends are a design constraint, not yet an
/// implementation.
pub(crate) trait Backend: Process {
    /// The backend's captured log text — the surface readiness parsing
    /// reads. For a process this is its piped stdout; a container
    /// backend would return its container logs.
    fn log_text(&self) -> std::io::Result<String>;

    /// The raw socket address of each listener the backend exposes to
    /// clients, in the backend's declared order, available once the
    /// backend is ready. These are the real endpoints the fronts dial;
    /// they are never published to callers.
    fn listener_endpoints(&self) -> Vec<SocketAddr>;
}
