//! The Zebrad executable support struct and associated.

use crate::{
    ProcessId, config,
    error::LaunchError,
    launch,
    logs::{LogsToDir, LogsToStdoutAndStderr as _},
    network,
    process::Process,
    utils::{
        executable_finder::{EXPECT_SPAWN, pick_command, trace_version_and_location},
        type_conversions::zingo_to_zebra_activation_heights,
    },
    validator::{Validator, ValidatorConfig},
};
use zcash_protocol::PoolType;
use zingo_common_components::protocol::{ActivationHeights, NetworkType};
use zingo_test_vectors::ZEBRAD_DEFAULT_MINER;

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    process::Child,
};

use getset::{CopyGetters, Getters};
use tempfile::TempDir;
use zebra_chain::serialization::ZcashSerialize as _;
use zebra_node_services::rpc_client::RpcRequestClient;
use zebra_rpc::{
    client::{BlockTemplateResponse, BlockTemplateTimeSource},
    proposal_block_from_template,
};

/// Zebrad configuration
///
/// Use `zebrad_bin` to specify the binary location.
/// If the binary is in $PATH, `None` can be specified to run "zebrad".
///
/// If `rpc_listen_port` is `None`, a port is picked at random between 15000-25000.
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
        mine_to_pool: PoolType,
        activation_heights: ActivationHeights,
        chain_cache: Option<PathBuf>,
    ) {
        assert_eq!(
            mine_to_pool,
            PoolType::Transparent,
            "Zebra can only mine to transparent using this test infrastructure currently, but tried to set to {mine_to_pool}"
        );
        self.network_type = NetworkType::Regtest(activation_heights);
        self.chain_cache = chain_cache;
    }
}

/// This struct is used to represent and manage the Zebrad process.
#[derive(Debug, Getters, CopyGetters)]
#[getset(get = "pub")]
pub struct Zebrad {
    /// Child process handle
    handle: Child,
    /// network listen port
    #[getset(skip)]
    #[getset(get_copy = "pub")]
    network_listen_port: u16,
    /// json RPC listen port
    #[getset(skip)]
    #[getset(get_copy = "pub")]
    rpc_listen_port: u16,
    /// gRPC listen port
    #[getset(skip)]
    #[getset(get_copy = "pub")]
    indexer_listen_port: u16,
    /// `[health]` HTTP listen port (serves `/healthy` and `/ready`)
    #[getset(skip)]
    #[getset(get_copy = "pub")]
    health_listen_port: u16,
    /// Config directory
    config_dir: TempDir,
    /// Logs directory
    logs_dir: TempDir,
    /// Data directory
    data_dir: TempDir,
    /// RPC request client
    client: RpcRequestClient,
    /// Network type
    network: NetworkType,
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
        let url = format!("http://127.0.0.1:{}/{}", self.health_listen_port, path);
        let response = reqwest::get(&url).await?;
        Ok(response.status() == reqwest::StatusCode::OK)
    }
}

/// Listen ports zebrad needs to bind during launch. Picked atomically
/// as a unit so the planned retry-on-collision helper in `launch::wait`
/// can re-roll all four in a single call rather than open-coding the
/// picks per validator. Re-rolling individual fields would risk one of
/// the surviving picks being a port a sibling test subprocess just
/// claimed (the cross-process TOCTOU these tests document).
#[derive(Debug, Clone, Copy)]
struct ZebradPorts {
    network: u16,
    rpc: u16,
    indexer: u16,
    health: u16,
}

impl ZebradPorts {
    fn pick(config: &ZebradConfig) -> Self {
        Self {
            network: network::pick_unused_port(config.network_listen_port),
            rpc: network::pick_unused_port(config.rpc_listen_port),
            indexer: network::pick_unused_port(config.indexer_listen_port),
            health: network::pick_unused_port(config.health_listen_port),
        }
    }
}

impl Zebrad {
    /// Single launch attempt: pick all four ports, write configs,
    /// spawn zebrad, wait for the readiness indicator, then probe RPC
    /// readiness. Wrapped by `Process::launch` in a bounded
    /// retry-on-port-collision loop (see
    /// `launch::with_retry_on_collision`); each retry calls this fresh
    /// with a config whose port pins have been cleared so
    /// `ZebradPorts::pick` re-rolls all four atomically via
    /// `network::pick_unused_port`.
    async fn launch_once(config: ZebradConfig) -> Result<Self, LaunchError> {
        let logs_dir = tempfile::tempdir().unwrap();
        let data_dir = tempfile::tempdir().unwrap();

        assert!(
            matches!(config.network_type, NetworkType::Regtest(_)) || config.chain_cache.is_some(),
            "chain cache must be specified when not using a regtest network!"
        );

        let working_cache_dir = data_dir.path().to_path_buf();

        if let Some(src) = config.chain_cache.as_ref() {
            Self::load_chain(src.clone(), working_cache_dir.clone(), config.network_type)
                .expect("load_chain failed");
        }

        let ZebradPorts {
            network: network_listen_port,
            rpc: rpc_listen_port,
            indexer: indexer_listen_port,
            health: health_listen_port,
        } = ZebradPorts::pick(&config);
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
        // create zcashd conf necessary for lightwalletd
        config::write_zcashd_config(
            config_dir.path(),
            rpc_listen_port,
            if let NetworkType::Regtest(activation_heights) = config.network_type {
                activation_heights
            } else {
                ActivationHeights::default()
            },
            None,
        )
        .unwrap();

        let executable_name = "zebrad";
        trace_version_and_location(executable_name, "--version");
        let mut command = pick_command(executable_name, false);
        command
            .args([
                "--config",
                config_file_path
                    .to_str()
                    .expect("should be valid UTF-8")
                    .to_string()
                    .as_str(),
                "start",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut handle = command.spawn().expect(EXPECT_SPAWN);

        launch::wait(
        ProcessId::Zebrad,
        &mut handle,
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
    .await?;

        let rpc_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), rpc_listen_port);
        let client = zebra_node_services::rpc_client::RpcRequestClient::new(rpc_address);

        // Replaces a fixed `std::thread::sleep(5s)`. `launch::wait` already
        // confirmed via stdout that the RPC listener bound; this confirms it
        // actually answers, which is the readiness signal every caller needs.
        // Cost in the happy path is one RPC round-trip (~ms), not 5s.
        wait_for_rpc_ready(&client, rpc_address, std::time::Duration::from_secs(30)).await?;

        let zebrad = Zebrad {
            handle,
            network_listen_port,
            indexer_listen_port,
            rpc_listen_port,
            health_listen_port,
            config_dir,
            logs_dir,
            data_dir,
            client,
            network: config.network_type,
        };

        if config.chain_cache.is_none() && matches!(config.network_type, NetworkType::Regtest(_)) {
            // Generate genesis block. `generate_blocks` calls `poll_chain_height`
            // to the new tip, so by the time it returns the RPC has answered
            // multiple times AND the genesis block is observable. The previously
            // unconditional `sleep(5s)` after this point had no documented
            // rationale and no successor predicate — deleted.
            zebrad.generate_blocks(1).await.unwrap();
        }

        Ok(zebrad)
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
        const MAX_ATTEMPTS: u32 = 3;

        launch::with_retry_on_collision(
            "zebrad",
            config,
            COLLISION_SIGNATURES,
            MAX_ATTEMPTS,
            |c: &ZebradConfig| {
                [
                    c.network_listen_port,
                    c.rpc_listen_port,
                    c.indexer_listen_port,
                    c.health_listen_port,
                ]
                .into_iter()
                .flatten()
                .collect()
            },
            |c: &mut ZebradConfig| {
                // Four-port validator — clear all four pins. Re-rolling
                // only the conflicted port would leave the surviving
                // three exposed to a sibling test subprocess that may
                // have just claimed one of them; cheaper to re-pick the
                // whole set than to detect-which-one and partial-clear.
                c.network_listen_port = None;
                c.rpc_listen_port = None;
                c.indexer_listen_port = None;
                c.health_listen_port = None;
            },
            Self::launch_once,
        )
        .await
    }

    fn stop(&mut self) {
        self.handle.kill().expect("zebrad couldn't be killed");
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

        let upgrades = response
            .get("upgrades")
            .expect("upgrades field should exist")
            .as_object()
            .expect("upgrades should be an object");

        crate::validator::parse_activation_heights_from_rpc(upgrades)
    }

    async fn generate_blocks(&self, n: u32) -> std::io::Result<()> {
        let chain_height = self.get_chain_height().await;
        let NetworkType::Regtest(activation_heights) = self.network() else {
            panic!("Can only generate blocks on regtest networks!");
        };
        let network = zebra_chain::parameters::Network::new_regtest(
            zingo_to_zebra_activation_heights(*activation_heights).into(),
        );

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
                let block_template: BlockTemplateResponse = self
                    .client
                    .json_result_from_call("getblocktemplate", "[]".to_string())
                    .await
                    .expect(
                        "response should be success output with a serialized `GetBlockTemplate`",
                    );

                let block_data = hex::encode(
                    proposal_block_from_template(
                        &block_template,
                        BlockTemplateTimeSource::default(),
                        &network,
                    )
                    .unwrap()
                    .zcash_serialize_to_vec()
                    .unwrap(),
                );

                last_response = self
                    .client
                    .text_from_call("submitblock", format!(r#"["{block_data}"]"#))
                    .await
                    .unwrap();

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
    ) -> std::io::Result<PathBuf> {
        let state_dir = chain_cache.join("state");

        if matches!(validator_network, NetworkType::Regtest(_)) {
            // `safe_copy_into_existing` walks state_dir's parent
            // no-symlinks and opens state_dir's basename with
            // O_NOFOLLOW -- so a symlink at chain_cache/state can no
            // longer redirect the read into an attacker-controlled
            // directory the way the prior `cp -r` did (issue #256, A3).
            crate::utils::safe_copy::safe_copy_into_existing(&state_dir, &validator_data_dir)?;
            Ok(validator_data_dir)
        } else {
            Ok(chain_cache)
        }
    }

    fn get_port(&self) -> u16 {
        self.rpc_listen_port()
    }
}

impl Drop for Zebrad {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod unit_tests {
    /// Audit tests for `CLAUDE.md` checklist item #1 — TOCTOU on
    /// filesystem paths. See issue #256.
    mod corrosion_mitigation {
        mod fs_path_toctou {
            //! Site A3: `Zebrad::load_chain` (line 599) does
            //!   `assert!(state_dir.exists())` then `cp -r state_dir
            //!   validator_data_dir`.
            //! `Path::exists()` follows symlinks, so a *valid* symlink
            //! at `chain_cache/state` pointing at an attacker-controlled
            //! directory passes the assertion, and `cp -r SRC` (which
            //! also follows the symlink) reads the attacker's files
            //! into `validator_data_dir`. Deterministic — no race
            //! window required.
            //!
            //! Mitigation: replace `.exists()` with
            //! `fs::symlink_metadata(p)` and reject if `is_symlink()`
            //! (or open the directory once as an FD and use `*at`
            //! syscalls); drop the `cp -r` subprocess for an
            //! FD-anchored Rust-level recursive copy so the path is
            //! not re-resolved by an external tool.

            use crate::validator::Validator;
            use crate::validator::regtest_test_activation_heights;
            use crate::validator::zebrad::Zebrad;
            use zingo_common_components::protocol::NetworkType;

            /// FAILS while `Zebrad::load_chain` trusts a symlink at
            /// `chain_cache/state`. After the fix, the function rejects
            /// the symlink (or refuses to follow it) and the attacker's
            /// payload never reaches `validator_data_dir`.
            #[test]
            fn load_chain_refuses_symlinked_state_dir() {
                let attacker_dir = tempfile::tempdir().unwrap();
                std::fs::write(
                    attacker_dir.path().join("PWNED"),
                    b"attacker payload -- must not reach validator_data_dir",
                )
                .unwrap();

                let chain_cache = tempfile::tempdir().unwrap();
                let state_path = chain_cache.path().join("state");
                std::os::unix::fs::symlink(attacker_dir.path(), &state_path).unwrap();

                let validator_data_holder = tempfile::tempdir().unwrap();
                let validator_data_dir = validator_data_holder.path().to_path_buf();

                let _ = <Zebrad as Validator>::load_chain(
                    chain_cache.path().to_path_buf(),
                    validator_data_dir.clone(),
                    NetworkType::Regtest(regtest_test_activation_heights()),
                );

                // `cp -r state_dir validator_data_dir` produces
                // `validator_data_dir/state/<contents>` (the basename
                // of the source is appended when copying a directory
                // into an existing directory).
                let attacker_marker = validator_data_dir.join("state").join("PWNED");
                assert!(
                    !attacker_marker.exists(),
                    "audit (issue #256, site A3): Zebrad::load_chain followed \
                     a symlink at chain_cache/state and copied attacker \
                     payload into validator_data_dir at {attacker_marker:?}"
                );
            }
        }
    }
}
