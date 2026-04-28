//! The Zebrad executable support struct and associated.

use crate::{
    config,
    error::LaunchError,
    launch,
    logs::{self, LogsToDir, LogsToStdoutAndStderr as _},
    network,
    process::Process,
    utils::{
        executable_finder::{pick_command, trace_version_and_location, EXPECT_SPAWN},
        type_conversions::zingo_to_zebra_activation_heights,
    },
    validator::{Validator, ValidatorConfig},
    ProcessId,
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
}

impl Default for ZebradConfig {
    fn default() -> Self {
        Self {
            network_listen_port: None,
            rpc_listen_port: None,
            indexer_listen_port: None,
            miner_address: ZEBRAD_DEFAULT_MINER.to_string(),
            chain_cache: None,
            network_type: NetworkType::Regtest(
                crate::validator::regtest_test_activation_heights(),
            ),
            lockbox_disbursements: Vec::new(),
            post_nu6_funding_streams: None,
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
        assert_eq!(mine_to_pool, PoolType::Transparent, "Zebra can only mine to transparent using this test infrastructure currently, but tried to set to {mine_to_pool}");
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

impl Process for Zebrad {
    const PROCESS: ProcessId = ProcessId::Zebrad;

    type Config = ZebradConfig;
    async fn launch(config: Self::Config) -> Result<Self, LaunchError> {
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

        let network_listen_port = network::pick_unused_port(config.network_listen_port);
        let rpc_listen_port = network::pick_unused_port(config.rpc_listen_port);
        let indexer_listen_port = network::pick_unused_port(config.indexer_listen_port);
        let config_dir = tempfile::tempdir().unwrap();
        let config_file_path = config::write_zebrad_config(
            config_dir.path().to_path_buf(),
            working_cache_dir,
            network_listen_port,
            rpc_listen_port,
            indexer_listen_port,
            &config.miner_address,
            config.network_type,
            &config.lockbox_disbursements,
            config.post_nu6_funding_streams.as_ref(),
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

        logs::write_logs(&mut handle, &logs_dir);
        launch::wait(
        ProcessId::Zebrad,
        &mut handle,
        &logs_dir,
        None,
        &[
            "zebra_rpc::server: Opened RPC endpoint at ",
            "zebra_rpc::indexer::server: Opened RPC endpoint at ",
            "spawned initial Zebra tasks",
        ],
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
    )?;

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
                    .expect("response should be success output with a serialized `GetBlockTemplate`");

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

    async fn generate_blocks_with_delay(&self, blocks: u32) -> std::io::Result<()> {
        for _ in 0..blocks {
            self.generate_blocks(1).await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        }
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

    async fn poll_chain_height(&self, target_height: u32) {
        while self.get_chain_height().await < target_height {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
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

impl Drop for Zebrad {
    fn drop(&mut self) {
        self.stop();
    }
}
