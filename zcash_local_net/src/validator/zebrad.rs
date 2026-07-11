//! The Zebrad executable support struct and associated.

use crate::{
    ProcessId, config,
    error::LaunchError,
    launch,
    logs::{LogsToDir, LogsToStdoutAndStderr as _},
    process::Process,
    validator::{Validator, ValidatorConfig},
};
use zingo_consensus::{ActivationHeights, MinerPool, NetworkType};
use zingo_test_vectors::{
    REG_O_ADDR_FROM_ABANDONART, REG_T_ADDR_FROM_ABANDONART, ZEBRAD_DEFAULT_MINER,
};

use std::{net::SocketAddr, path::PathBuf, process::Child};

use crate::rpc_client::RpcRequestClient;
use tempfile::TempDir;

/// Zebrad configuration
///
/// Use `source` to say where the zebrad binary comes from: the default
/// resolves via `TEST_BINARIES_DIR` / `PATH`, and the alternatives are
/// an explicit local-build path or a container image (see
/// [`crate::container::ArtifactSource`]).
///
/// Each `*_listen_port` field pins the corresponding **raw** zebrad
/// listener when `Some(N)`; `None` (the default) lets zebrad bind port
/// 0 so the kernel assigns the port atomically, and the harness reads
/// the assigned address back out of zebrad's launch log. Note that the
/// raw JSON-RPC listener is never published: every accessor returns
/// the address of the front proxy instead (see
/// [`Zebrad::rpc_listen_port`]).
///
/// Use `activation_heights` to specify custom network upgrade activation heights.
///
/// Use `miner_address` to specify the target address for the block rewards when blocks are generated.
///
/// If `chain_cache` path is `None`, a new chain is launched.
///
/// `network` can be used for testing against cached testnet / mainnet chains where large chains are needed.
/// `activation_heights` and `miner_address` will be ignored while not using regtest network.
#[derive(Clone, Debug)]
pub struct ZebradConfig {
    /// Zebrad network listen port
    pub network_listen_port: Option<u16>,
    /// Zebrad JSON-RPC listen port
    pub rpc_listen_port: Option<u16>,
    /// Zebrad gRPC listen port
    pub indexer_listen_port: Option<u16>,
    /// Zebrad `[health]` HTTP listen port. `None` lets the harness
    /// pick an unused port at launch (the regtest-friendly default,
    /// matching the other listen ports). Some(N) pins to N. The
    /// listener exposes `GET /healthy` and `GET /ready`.
    pub health_listen_port: Option<u16>,
    /// Miner address
    pub miner_address: String,
    /// Chain cache path
    pub chain_cache: Option<PathBuf>,
    /// Network type
    pub network_type: NetworkType,
    /// Lockbox disbursements written into Zebra's regtest
    /// `[network.testnet_parameters]` block. Empty by default —
    /// preserves today's behavior where the NU6.1 activation block
    /// is unreachable. Any test whose chain crosses NU6.1 must
    /// populate this with at least one entry, otherwise zebrad's
    /// `subsidy_is_valid` rejects the activation block.
    pub lockbox_disbursements: Vec<crate::validator::LockboxDisbursement>,
    /// Post-NU6 funding streams written into Zebra's regtest
    /// `[network.testnet_parameters.post_nu6_funding_streams]` block.
    /// `None` by default. To make the chain mineable past NU6.1,
    /// populate this *and* `lockbox_disbursements` together — the
    /// stream deposits into Zebra's `Deferred` value pool, which
    /// the disbursements draw from.
    pub post_nu6_funding_streams: Option<crate::validator::FundingStreams>,
    /// Minimum live peers required for `/healthy` to return 200,
    /// emitted into Zebra's `[health]` block. Default `0` is the
    /// right answer for single-node regtest (no peer network exists
    /// to be on); mainnet/testnet harnesses should override to
    /// match the upstream Zebra default of `1` or higher. The value
    /// only takes effect when `[health].listen_addr` is set —
    /// today the harness leaves the listener disabled, so this
    /// field is effectively a forward-compatible placeholder until
    /// the harness wires in `wait_for_rpc_ready` against `/healthy`.
    pub min_connected_peers: usize,
    /// Observer registered on the JSON-RPC front proxy before the
    /// backend starts, so it sees every byte of the backend's
    /// networked lifetime — including the launch-time readiness
    /// probes and the regtest launch-mine. `None` (the default) is a
    /// passthrough front with no observer.
    pub rpc_front_observer: Option<std::sync::Arc<dyn crate::front::FrontObserver>>,
    /// Where the zebrad binary comes from: host-process resolution
    /// (the default), an explicit local build, or a container image.
    /// See [`crate::container::ArtifactSource`]; typically seeded from
    /// an [`crate::container::manifest::ArtifactManifest`].
    pub source: crate::container::ArtifactSource,
}

impl Default for ZebradConfig {
    fn default() -> Self {
        // The default fixture activates NU6.1 at height 5 (see
        // `regtest_test_activation_heights`), so the matching
        // `lockbox_disbursements` and post-NU6 funding stream are
        // both required for any test that mines past block 4. We
        // populate them here so callers don't have to remember the
        // pairing.
        Self {
            network_listen_port: None,
            rpc_listen_port: None,
            indexer_listen_port: None,
            health_listen_port: None,
            miner_address: ZEBRAD_DEFAULT_MINER.to_string(),
            chain_cache: None,
            network_type: NetworkType::Regtest(crate::validator::regtest_test_activation_heights()),
            lockbox_disbursements: crate::validator::regtest_test_lockbox_disbursements(),
            post_nu6_funding_streams: Some(
                crate::validator::regtest_test_post_nu6_funding_streams(),
            ),
            min_connected_peers: 0,
            rpc_front_observer: None,
            source: crate::container::ArtifactSource::default(),
        }
    }
}

impl ZebradConfig {
    /// Sets the miner address.
    pub fn with_miner_address(mut self, miner_address: String) -> Self {
        self.miner_address = miner_address;
        self
    }

    /// Sets the validator to run in regtest mode, with the specified activation heights.
    pub fn with_regtest_enabled(mut self, activation_heights: ActivationHeights) -> Self {
        self.network_type = NetworkType::Regtest(activation_heights);
        self
    }
}

impl ValidatorConfig for ZebradConfig {
    fn set_test_parameters(
        &mut self,
        mine_to_pool: MinerPool,
        activation_heights: ActivationHeights,
        chain_cache: Option<PathBuf>,
    ) {
        self.miner_address = match mine_to_pool {
            MinerPool::Orchard => REG_O_ADDR_FROM_ABANDONART,
            MinerPool::Transparent => REG_T_ADDR_FROM_ABANDONART,
            MinerPool::Sapling => {
                panic!("zebrad does not support mining to a Sapling address; use Orchard or Transparent")
            }
        }
        .to_string();
        self.network_type = NetworkType::Regtest(activation_heights);
        self.chain_cache = chain_cache;
    }
}

/// This struct is used to represent and manage the Zebrad process.
#[derive(Debug)]
pub struct Zebrad {
    /// Child process handle. In container mode this is the foreground
    /// runtime client (`docker|podman run`), through which the
    /// container's stdio streams.
    handle: Child,
    /// The named container zebrad runs as when the artifact source is
    /// a container image; `None` in host-process mode. Held so `stop`
    /// can force-remove the container — killing the client alone would
    /// leave it running.
    container: Option<crate::container::ContainerInstance>,
    /// JSON-RPC front proxy — the canonical public endpoint of the
    /// JSON-RPC listener, bound before the process started.
    rpc_front: crate::front::Front,
    /// Raw listener addresses, discovered from the launch log. The
    /// JSON-RPC one is what the front dials; none of them are
    /// published.
    raw_listen_addrs: RawListenAddrs,
    /// Config directory
    config_dir: TempDir,
    /// Logs directory
    logs_dir: TempDir,
    /// Data directory
    data_dir: TempDir,
    /// RPC request client, targeting the JSON-RPC front — so even the
    /// harness's own launch-time traffic crosses the front.
    client: RpcRequestClient,
    /// Network type
    network: NetworkType,
}

crate::macros::ref_getters!(Zebrad {
    /// Child process handle.
    handle: Child,
    /// Config directory.
    config_dir: TempDir,
    /// RPC request client for the launched node.
    client: RpcRequestClient,
    /// Network type the node was launched with.
    network: NetworkType,
});

impl Zebrad {
    /// The public JSON-RPC address. **This is the address of the
    /// front proxy**, not of zebrad's own listener: the raw endpoint
    /// is a private detail of launch plumbing and is never published.
    pub fn rpc_listen_addr(&self) -> SocketAddr {
        self.rpc_front.public_addr()
    }

    /// The public JSON-RPC port. **This is the port of the front
    /// proxy**, not of zebrad's own listener — see
    /// [`Self::rpc_listen_addr`].
    pub fn rpc_listen_port(&self) -> u16 {
        self.rpc_front.public_port()
    }

    /// Raw network (Zcash peer protocol) listen port. Launch plumbing
    /// only; no front exists for this listener.
    pub fn network_listen_port(&self) -> u16 {
        self.raw_listen_addrs.network.port()
    }

    /// Raw indexer-gRPC listen port. Launch plumbing only; no front
    /// exists for this listener.
    pub fn indexer_listen_port(&self) -> u16 {
        self.raw_listen_addrs.indexer.port()
    }

    /// Raw `[health]` HTTP listen port (serves `/healthy` and
    /// `/ready`). Launch plumbing only; no front exists for this
    /// listener.
    pub fn health_listen_port(&self) -> u16 {
        self.raw_listen_addrs.health.port()
    }
}

impl LogsToDir for Zebrad {
    fn logs_dir(&self) -> &TempDir {
        &self.logs_dir
    }
}

impl Zebrad {
    /// `GET http://127.0.0.1:<health_listen_port>/healthy`. Returns
    /// `Ok(true)` when zebrad's health server replies `200 OK`,
    /// `Ok(false)` for a `503 Service Unavailable`, and `Err` if the
    /// HTTP request itself fails (port unreachable, malformed
    /// response, etc.).
    pub async fn healthy(&self) -> Result<bool, reqwest::Error> {
        self.fetch_health_status("healthy").await
    }

    /// `GET http://127.0.0.1:<health_listen_port>/ready`. Same
    /// return convention as [`Self::healthy`].
    pub async fn ready(&self) -> Result<bool, reqwest::Error> {
        self.fetch_health_status("ready").await
    }

    async fn fetch_health_status(&self, path: &str) -> Result<bool, reqwest::Error> {
        let url = format!("http://{}/{}", self.raw_listen_addrs.health, path);
        let response = reqwest::get(&url).await?;
        Ok(response.status() == reqwest::StatusCode::OK)
    }
}

/// The raw listener addresses zebrad reports in its launch log, one
/// per configured listener. Unpinned listeners are configured on port
/// 0, so these are the kernel-assigned endpoints — the only place the
/// harness learns them.
#[derive(Debug, Clone, Copy)]
struct RawListenAddrs {
    network: SocketAddr,
    rpc: SocketAddr,
    indexer: SocketAddr,
    health: SocketAddr,
}

/// The launch-log markers zebrad prints after each listener bind, each
/// followed by the bound socket address. A log-format contract with
/// the zebrad binary (captured verbatim from zebrad 6.0.0-rc.0);
/// every real launch exercises it, and the unit tests below pin the
/// captured lines. Order in this table is bind order.
const ZEBRAD_BOUND_MARKERS: [(&str, &str); 4] = [
    ("network", "Opened Zcash protocol endpoint at "),
    ("rpc", "zebra_rpc::server: Opened RPC endpoint at "),
    (
        "indexer",
        "zebra_rpc::indexer::server: Opened RPC endpoint at ",
    ),
    ("health", "opened health endpoint at "),
];

/// Parse the socket address following `marker` on the first log line
/// that carries it. `Ok(None)` means the marker has not appeared yet;
/// `Err` carries the offending line when the marker is present but the
/// address does not parse — the log contract has drifted, and that
/// must fail loudly rather than time a launch out.
fn bound_addr_after(log: &str, marker: &str) -> Result<Option<SocketAddr>, String> {
    let Some(line) = log.lines().find(|line| line.contains(marker)) else {
        return Ok(None);
    };
    let start = line.find(marker).expect("line was found by the marker") + marker.len();
    let token = line[start..]
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_end_matches(['.', ',']);
    token
        .parse::<SocketAddr>()
        .map(Some)
        .map_err(|parse_error| format!("{line} ({parse_error})"))
}

/// Poll zebrad's captured stdout until every listener in
/// [`ZEBRAD_BOUND_MARKERS`] has reported its bound address, the child
/// exits, or the budget elapses. The bind reports land within
/// microseconds of each other right before the readiness indicator
/// `launch::wait` already saw, so the happy path costs one file read.
async fn discover_raw_listen_addrs(
    handle: &mut Child,
    logs_dir: &TempDir,
) -> Result<RawListenAddrs, LaunchError> {
    const BUDGET: std::time::Duration = std::time::Duration::from_secs(10);
    const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(50);

    let stdout_path = logs_dir.path().join(crate::logs::STDOUT_LOG);
    let read_logs = || {
        let stdout = std::fs::read_to_string(&stdout_path).unwrap_or_default();
        let stderr = std::fs::read_to_string(logs_dir.path().join(crate::logs::STDERR_LOG))
            .unwrap_or_default();
        (stdout, stderr)
    };

    let deadline = tokio::time::Instant::now() + BUDGET;
    loop {
        let stdout = std::fs::read_to_string(&stdout_path).unwrap_or_default();
        let mut addrs = Vec::with_capacity(ZEBRAD_BOUND_MARKERS.len());
        let mut missing = Vec::new();
        for (listener, marker) in ZEBRAD_BOUND_MARKERS {
            match bound_addr_after(&stdout, marker) {
                Ok(Some(addr)) => addrs.push(addr),
                Ok(None) => missing.push(listener),
                Err(offending_line) => {
                    return Err(LaunchError::ListenerEndpointsUndiscovered {
                        process_name: ProcessId::Zebrad.to_string(),
                        detail: format!(
                            "the {listener} bind report matched marker {marker:?} but its \
                             address did not parse — the zebrad log contract has drifted: \
                             {offending_line}"
                        ),
                        stdout,
                    });
                }
            }
        }
        if let [network, rpc, indexer, health] = addrs[..] {
            return Ok(RawListenAddrs {
                network,
                rpc,
                indexer,
                health,
            });
        }

        // A pinned port can still collide on a bind that happens after
        // `launch::wait`'s readiness indicator; the child then exits
        // and its captured output carries the AddrInUse signature the
        // retry helper scans for.
        if let Ok(Some(exit_status)) = handle.try_wait() {
            let (stdout, stderr) = read_logs();
            return Err(LaunchError::ProcessFailed {
                process_name: ProcessId::Zebrad.to_string(),
                exit_status,
                stdout,
                stderr,
                additional_log: None,
            });
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(LaunchError::ListenerEndpointsUndiscovered {
                process_name: ProcessId::Zebrad.to_string(),
                detail: format!(
                    "no bind report for listener(s) {} within {BUDGET:?}",
                    missing.join(", ")
                ),
                stdout,
            });
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

impl launch::PortPins for ZebradConfig {
    fn pinned_ports(&self) -> Vec<u16> {
        [
            self.network_listen_port,
            self.rpc_listen_port,
            self.indexer_listen_port,
            self.health_listen_port,
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    fn clear_port_pins(&mut self) {
        // Four-port validator — clear all four pins. A cleared pin
        // makes the listener bind port 0, where the kernel assigns
        // the port atomically and no collision is possible, so the
        // retry cannot re-collide.
        self.network_listen_port = None;
        self.rpc_listen_port = None;
        self.indexer_listen_port = None;
        self.health_listen_port = None;
    }
}

impl Zebrad {
    /// Single launch attempt: bind the JSON-RPC front, write configs
    /// (unpinned listeners on port 0), spawn zebrad, wait for the
    /// readiness indicator, discover the raw listener addresses from
    /// the launch log, point the front at the raw JSON-RPC endpoint,
    /// then probe RPC readiness *through the front*. Wrapped by
    /// `Process::launch` in a bounded retry-on-port-collision loop
    /// (see `launch::with_retry_on_collision`) that only matters for
    /// pinned ports; each retry calls this fresh with a config whose
    /// port pins have been cleared, i.e. with kernel-assigned ports
    /// that cannot collide.
    async fn launch_once(config: ZebradConfig) -> Result<Self, LaunchError> {
        let logs_dir = tempfile::tempdir().unwrap();
        let data_dir = tempfile::tempdir().unwrap();

        assert!(
            matches!(config.network_type, NetworkType::Regtest(_)) || config.chain_cache.is_some(),
            "chain cache must be specified when not using a regtest network!"
        );

        let working_cache_dir = data_dir.path().to_path_buf();

        if let Some(src) = config.chain_cache.as_ref() {
            Self::load_chain(src.clone(), working_cache_dir.clone(), config.network_type);
        }

        // The front binds before the backend starts: its public
        // address exists for the backend's entire networked lifetime,
        // and the OS assigns it atomically on 127.0.0.1:0 — the public
        // surface has no check-then-bind race, ever.
        let rpc_front = crate::front::Front::bind(config.rpc_front_observer.clone())
            .expect("the JSON-RPC front should bind on 127.0.0.1:0");

        // Raw listener ports: `Some(N)` pins N (the collision-retry
        // machinery remains the backstop for pinned ports); `None`
        // becomes port 0, kernel-assigned at bind time and read back
        // out of the launch log by `discover_raw_listen_addrs`.
        let network_listen_port = config.network_listen_port.unwrap_or(0);
        let rpc_listen_port = config.rpc_listen_port.unwrap_or(0);
        let indexer_listen_port = config.indexer_listen_port.unwrap_or(0);
        let health_listen_port = config.health_listen_port.unwrap_or(0);
        let config_dir = tempfile::tempdir().unwrap();
        let config_file_path = config::write_zebrad_config(
            config_dir.path().to_path_buf(),
            working_cache_dir,
            network_listen_port,
            rpc_listen_port,
            indexer_listen_port,
            health_listen_port,
            &config.miner_address,
            config.network_type,
            &config.lockbox_disbursements,
            config.post_nu6_funding_streams.as_ref(),
            config.min_connected_peers,
        )
        .unwrap();

        let executable_name = "zebrad";
        crate::container::trace_version(&config.source, executable_name, "--version");
        // In container mode, mount the harness dirs at identical paths
        // so the config file just written is valid verbatim inside the
        // container (see `crate::container`). The chain cache is
        // mounted too so cached-chain launches can read it.
        let container = config.source.new_instance(executable_name);
        let mut mounts: Vec<&std::path::Path> = vec![config_dir.path(), data_dir.path()];
        if let Some(cache) = config.chain_cache.as_ref() {
            mounts.push(cache.as_path());
        }
        let mut command = config.source.command(&crate::container::LaunchSpec {
            executable_name,
            container_name: container.as_ref().map(|c| c.name()),
            mounts: &mounts,
            interactive: false,
        });
        command.args([
            "--config",
            config_file_path.to_str().expect("should be valid UTF-8"),
            "start",
        ]);

        let spawned = launch::spawn_and_wait(
        ProcessId::Zebrad,
        &mut command,
        &logs_dir,
        None,
        // Only the indexer-RPC indicator is reliably *post-bind* for
        // every listener zebrad opens. The previous list also included
        // `"zebra_rpc::server: Opened RPC endpoint at "` (fires after
        // the main RPC bind but BEFORE the indexer-RPC bind) and
        // `"spawned initial Zebra tasks"` (firing point unclear); both
        // let `launch::wait` return Ok before zebrad's full set of
        // binds had completed, which produced failure mode #4: when
        // the indexer-RPC bind subsequently hit AddrInUse, zebrad shut
        // down all listeners (including the main RPC), and downstream
        // `wait_for_rpc_ready` saw `ConnectionRefused` for 30 s with
        // no chance for the retry helper to fire. Waiting only for
        // the indexer indicator means: bind succeeds → we proceed; or
        // bind fails → child exits → `launch::wait` returns
        // `ProcessFailed` whose captured stdout contains
        // `"Address already in use"` for the retry helper's signature
        // scan to match.
        &["zebra_rpc::indexer::server: Opened RPC endpoint at "],
        &[
            " panicked at",
            "ERROR ",
            "fatal",
            "failed to ",
            "unable to ",
            "Aborting",
            " backtrace:",
        ],
        &[
            // exclude benign noise that often shows up during bootstrap:
            "DNS error resolving peer IP addresses",
            "Seed peer DNS resolution failed",
            "warning: some trace filter directives would enable traces that are disabled statically",
        ],
    )
    .await;
        let mut handle = match spawned {
            Ok(handle) => handle,
            Err(error) => {
                // A failed container launch may leave the container
                // running even though `launch::wait` gave up on it
                // (e.g. an error indicator matched while the process
                // survives); don't leak it past the failed launch.
                if let Some(container) = &container {
                    container.force_remove();
                }
                return Err(error);
            }
        };

        // Discover where zebrad actually bound each listener. With
        // unpinned (port 0) listeners the launch log is the only place
        // the kernel-assigned addresses appear.
        let raw_listen_addrs = match discover_raw_listen_addrs(&mut handle, &logs_dir).await {
            Ok(addrs) => addrs,
            Err(error) => {
                // The child may still be running (an undiscoverable
                // endpoint is not an exited process); don't leak it
                // past the failed launch. A kill error only means the
                // child already exited, which is the state kill wants.
                if let Some(container) = &container {
                    container.force_remove();
                }
                let _ = handle.kill();
                return Err(error);
            }
        };

        // create zcashd conf necessary for lightwalletd, which reads
        // the validator's RPC port out of it — written post-discovery
        // because the raw port is kernel-assigned.
        #[cfg(feature = "legacy-stack")]
        config::write_zcashd_config(
            config_dir.path(),
            raw_listen_addrs.rpc.port(),
            if let NetworkType::Regtest(activation_heights) = config.network_type {
                activation_heights
            } else {
                ActivationHeights::default()
            },
            None,
        )
        .unwrap();

        // The client targets the front, so every internal client —
        // the readiness probe below and the launch-mine — crosses the
        // front like any external caller would.
        let client = RpcRequestClient::new(rpc_front.public_addr());

        let zebrad = Zebrad {
            handle,
            container,
            rpc_front,
            raw_listen_addrs,
            config_dir,
            logs_dir,
            data_dir,
            client,
            network: config.network_type,
        };

        // Point the front at the raw JSON-RPC endpoint, discovered
        // through the backend abstraction: connections the front has
        // been holding proceed from here.
        zebrad
            .rpc_front
            .point_at(&zebrad, ZEBRAD_RPC_LISTENER_INDEX);

        // Replaces a fixed `std::thread::sleep(5s)`. `launch::wait` already
        // confirmed via stdout that the RPC listener bound; this confirms it
        // actually answers, which is the readiness signal every caller needs.
        // Cost in the happy path is one RPC round-trip (~ms), not 5s.
        wait_for_rpc_ready(
            &zebrad.client,
            zebrad.rpc_front.public_addr(),
            std::time::Duration::from_secs(30),
        )
        .await?;

        if config.chain_cache.is_none() && matches!(config.network_type, NetworkType::Regtest(_)) {
            // Generate genesis block. `generate_blocks` calls `poll_chain_height`
            // to the new tip, so by the time it returns the RPC has answered
            // multiple times AND the genesis block is observable. The previously
            // unconditional `sleep(5s)` after this point had no documented
            // rationale and no successor predicate — deleted. This is the
            // launch-mine: it speaks through `client`, i.e. through the
            // front, so a registered observer sees it.
            zebrad.generate_blocks(1).await.unwrap();
        }

        Ok(zebrad)
    }
}

/// Index of the JSON-RPC endpoint in [`Zebrad`]'s declared listener
/// order (`crate::backend::Backend::listener_endpoints`). The JSON-RPC
/// listener is the only one zebrad exposes to clients, so it is the
/// only entry.
const ZEBRAD_RPC_LISTENER_INDEX: usize = 0;

impl crate::backend::Backend for Zebrad {
    fn log_text(&self) -> std::io::Result<String> {
        std::fs::read_to_string(self.logs_dir.path().join(crate::logs::STDOUT_LOG))
    }

    fn listener_endpoints(&self) -> Vec<SocketAddr> {
        vec![self.raw_listen_addrs.rpc]
    }
}

impl Process for Zebrad {
    const PROCESS: ProcessId = ProcessId::Zebrad;

    type Config = ZebradConfig;

    async fn launch(config: Self::Config) -> Result<Self, LaunchError> {
        // Stderr signatures that zebrad emits when one of its four
        // listen-port binds hits an `EADDRINUSE`. The RPC bind path
        // panics through Rust's panic format ("kind: AddrInUse,
        // message: 'Address already in use'", "code: 98"); the
        // peer-protocol bind path raises a typed eyre error that
        // includes "AddrInUse" in its `{:?}` rendering. All four
        // bind paths funnel through one of these strings.
        const COLLISION_SIGNATURES: &[&str] = &["AddrInUse", "code: 98", "Address already in use"];

        launch::with_retry_on_collision(
            "zebrad",
            config,
            COLLISION_SIGNATURES,
            launch::MAX_LAUNCH_ATTEMPTS,
            Self::launch_once,
        )
        .await
    }

    fn stop(&mut self) {
        // Container mode: the handle is only the runtime client;
        // killing it would orphan the container, so remove the
        // container first (which also ends the client).
        if let Some(container) = &self.container {
            container.force_remove();
        }
        match self.handle.kill() {
            Ok(()) => {}
            // `kill` returns `InvalidInput` when the child has already
            // exited and been reaped — e.g. the runtime client after
            // its container was just force-removed.
            Err(e) if e.kind() == std::io::ErrorKind::InvalidInput => {}
            Err(e) => panic!("zebrad couldn't be killed: {e}"),
        }
    }

    fn print_all(&self) {
        self.print_stdout();
        self.print_stderr();
    }
}

/// Polls Zebrad's RPC endpoint until `getblocktemplate` returns success, or
/// the timeout elapses. Replaces a fixed `std::thread::sleep` previously used
/// in `Zebrad::launch` as a paper-over for RPC bind→mining-service-ready
/// latency. `getblocktemplate` is used (not `getblockchaininfo`) because the
/// first thing every caller does after launch is generate a genesis block via
/// `generate_blocks`, which needs the mining service. `getblockchaininfo`
/// answers as soon as the listener binds, well before the mining service is
/// up — exactly the gap the old sleep was masking.
async fn wait_for_rpc_ready(
    client: &RpcRequestClient,
    address: SocketAddr,
    timeout: std::time::Duration,
) -> Result<(), LaunchError> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        match client
            .json_result_from_call::<serde_json::Value>("getblocktemplate", "[]".to_string())
            .await
        {
            Ok(_) => return Ok(()),
            Err(e) => {
                let last_error = format!("{e:?}");
                if tokio::time::Instant::now() >= deadline {
                    return Err(LaunchError::RpcReadinessTimeout {
                        process_name: ProcessId::Zebrad.to_string(),
                        address,
                        timeout,
                        last_error,
                    });
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }
    }
}

impl Validator for Zebrad {
    async fn get_activation_heights(&self) -> ActivationHeights {
        let response: serde_json::Value = self
            .client
            .json_result_from_call("getblockchaininfo", "[]".to_string())
            .await
            .expect("getblockchaininfo should succeed");

        crate::validator::activation_heights_from_getblockchaininfo(&response)
    }

    async fn generate_blocks(&self, n: u32) -> std::io::Result<()> {
        let chain_height = self.get_chain_height().await;
        let NetworkType::Regtest(activation_heights) = self.network() else {
            panic!("Can only generate blocks on regtest networks!");
        };

        // Drive the chain forward one block per outer iteration. Success
        // criterion is *chain advance*, not the RPC response: zebra returns
        // "duplicate" / "duplicate-inconclusive" when validation outruns the
        // 100 ms retry interval (notably the NU6.1 activation block), and
        // those responses don't tell us whether the new submission committed
        // — only that something with the same hash was already submitted.
        // Polling chain height between submits is the unambiguous answer.
        const MAX_ATTEMPTS: u32 = 30;
        const ATTEMPT_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);
        for i in 0..n {
            let target_height = chain_height + i + 1;
            let mut last_response = String::new();
            let mut advanced = false;
            for _ in 0..MAX_ATTEMPTS {
                let submission =
                    crate::zebra_rpc::submit_template_block(&self.client, activation_heights)
                        .await
                        .expect("template block submission should succeed");
                last_response = submission.response;

                if self.get_chain_height().await >= target_height {
                    advanced = true;
                    break;
                }
                tokio::time::sleep(ATTEMPT_INTERVAL).await;
            }

            if !advanced {
                tracing::error!(
                    "chain failed to reach height {target_height} after \
                     {MAX_ATTEMPTS} attempts; last submitblock response: \
                     {last_response}"
                );
                panic!("Failed to advance chain to height {target_height}!");
            }
        }
        self.poll_chain_height(chain_height + n).await;

        Ok(())
    }

    async fn get_chain_height(&self) -> u32 {
        let response: serde_json::Value = self
            .client
            .json_result_from_call("getblockchaininfo", "[]".to_string())
            .await
            .unwrap();

        response
            .get("blocks")
            .and_then(serde_json::Value::as_u64)
            .and_then(|h| u32::try_from(h).ok())
            .unwrap()
    }

    fn data_dir(&self) -> &TempDir {
        &self.data_dir
    }

    #[cfg(feature = "legacy-stack")]
    fn get_zcashd_conf_path(&self) -> PathBuf {
        self.config_dir.path().join(config::ZCASHD_FILENAME)
    }

    fn network(&self) -> NetworkType {
        self.network
    }

    fn load_chain(
        chain_cache: PathBuf,
        validator_data_dir: PathBuf,
        validator_network: NetworkType,
    ) -> PathBuf {
        let state_dir = chain_cache.clone().join("state");
        assert!(state_dir.exists(), "state directory not found!");

        if matches!(validator_network, NetworkType::Regtest(_)) {
            std::process::Command::new("cp")
                .arg("-r")
                .arg(state_dir)
                .arg(validator_data_dir.clone())
                .output()
                .unwrap();
            validator_data_dir
        } else {
            chain_cache
        }
    }

    fn get_port(&self) -> u16 {
        self.rpc_listen_port()
    }
}

crate::macros::impl_stop_on_drop!(Zebrad);

#[cfg(test)]
mod tests {
    use super::*;

    /// The four bind-report lines captured verbatim from a zebrad
    /// 6.0.0-rc.0 launch whose config put every listener on port 0.
    /// This pins the log-format contract `discover_raw_listen_addrs`
    /// parses; every real launch exercises it live.
    const CAPTURED_BIND_REPORT: &str = "\
2026-07-09T01:02:35.994662Z  INFO open_listener{addr=127.0.0.1:0}: zebra_network::peer_set::initialize: Trying to open Zcash protocol endpoint at 127.0.0.1:0...
2026-07-09T01:02:35.994684Z  INFO open_listener{addr=127.0.0.1:0}: zebra_network::peer_set::initialize: Opened Zcash protocol endpoint at 127.0.0.1:36971
2026-07-09T01:02:35.995300Z  INFO zebra_rpc::server: Opened RPC endpoint at 127.0.0.1:46389
2026-07-09T01:02:35.995400Z  INFO init: zebra_rpc::indexer::server: Trying to open indexer RPC endpoint at 127.0.0.1:0...
2026-07-09T01:02:35.995408Z  INFO init: zebra_rpc::indexer::server: Opened RPC endpoint at 127.0.0.1:45685
2026-07-09T01:02:35.995431Z  INFO zebrad::commands::start: initializing health endpoints
2026-07-09T01:02:35.995432Z  INFO zebrad::components::health: opening health endpoint at 127.0.0.1:0...
2026-07-09T01:02:35.995438Z  INFO zebrad::components::health: opened health endpoint at 127.0.0.1:34961";

    #[test]
    fn captured_bind_report_yields_all_four_raw_addresses() {
        let expected: [(&str, u16); 4] = [
            ("network", 36971),
            ("rpc", 46389),
            ("indexer", 45685),
            ("health", 34961),
        ];
        for ((listener, marker), (expected_listener, expected_port)) in
            ZEBRAD_BOUND_MARKERS.into_iter().zip(expected)
        {
            assert_eq!(listener, expected_listener);
            let addr = bound_addr_after(CAPTURED_BIND_REPORT, marker)
                .unwrap_or_else(|line| panic!("{listener} report did not parse: {line}"))
                .unwrap_or_else(|| panic!("{listener} report not found"));
            assert_eq!(addr, SocketAddr::from(([127, 0, 0, 1], expected_port)));
        }
    }

    /// The `Trying to open … at 127.0.0.1:0...` announcement lines must
    /// not satisfy the markers — they carry the configured port-0
    /// address, not the bound one.
    #[test]
    fn announcement_lines_do_not_match_the_bound_markers() {
        let announcements = "\
Trying to open Zcash protocol endpoint at 127.0.0.1:0...
zebra_rpc::indexer::server: Trying to open indexer RPC endpoint at 127.0.0.1:0...
zebrad::components::health: opening health endpoint at 127.0.0.1:0...";
        for (listener, marker) in ZEBRAD_BOUND_MARKERS {
            assert_eq!(
                bound_addr_after(announcements, marker),
                Ok(None),
                "{listener} marker matched an announcement line"
            );
        }
    }

    /// A marker line whose address does not parse must fail loudly
    /// with the offending line — the drift tripwire.
    #[test]
    fn drifted_bind_report_fails_loud() {
        let drifted = "zebra_rpc::server: Opened RPC endpoint at <dynamic>";
        let error = bound_addr_after(drifted, "zebra_rpc::server: Opened RPC endpoint at ")
            .expect_err("a non-address token should not parse");
        assert!(
            error.contains("<dynamic>"),
            "drift error should carry the offending line, got {error:?}"
        );
    }
}
