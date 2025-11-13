//! The Zebrad executable support struct and associated.

use crate::{
    config,
    error::LaunchError,
    launch,
    logs::{self, LogsToDir, LogsToStdoutAndStderr as _},
    network,
    process::Process,
    utils::executable_finder::{pick_command, EXPECT_SPAWN},
    validator::{Validator, ValidatorConfig},
    ProcessId,
};
use zcash_protocol::PoolType;
use zingo_common_components::protocol::activation_heights::for_test;
use zingo_test_vectors::ZEBRAD_DEFAULT_MINER;

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    process::Child,
};

use getset::{CopyGetters, Getters};
use portpicker::Port;
use tempfile::TempDir;
use zebra_chain::parameters::{self, testnet::ConfiguredActivationHeights, NetworkKind};
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
    pub network_listen_port: Option<Port>,
    /// Zebrad JSON-RPC listen port
    pub rpc_listen_port: Option<Port>,
    /// Zebrad gRPC listen port
    pub indexer_listen_port: Option<Port>,
    /// Local network upgrade activation heights
    pub configured_activation_heights: ConfiguredActivationHeights,
    /// Miner address
    pub miner_address: &'static str,
    /// Chain cache path
    pub chain_cache: Option<PathBuf>,
    /// Network type
    pub network: NetworkKind,
}

impl Default for ZebradConfig {
    fn default() -> Self {
        Self {
            network_listen_port: None,
            rpc_listen_port: None,
            indexer_listen_port: None,
            configured_activation_heights: for_test::all_height_one_nus(),
            miner_address: ZEBRAD_DEFAULT_MINER,
            chain_cache: None,
            network: NetworkKind::Regtest,
        }
    }
}

impl ValidatorConfig for ZebradConfig {
    fn set_test_parameters(
        &mut self,
        mine_to_pool: PoolType,
        configured_activation_heights: ConfiguredActivationHeights,
        chain_cache: Option<PathBuf>,
    ) {
        assert_eq!(mine_to_pool, PoolType::Transparent, "Zebra can only mine to transparent using this test infrastructure currently, but tried to set to {mine_to_pool}");
        self.configured_activation_heights = configured_activation_heights;
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
    network_listen_port: Port,
    /// RPC listen port
    #[getset(skip)]
    #[getset(get_copy = "pub")]
    rpc_listen_port: Port,
    /// Config directory
    config_dir: TempDir,
    /// Logs directory
    logs_dir: TempDir,
    /// Data directory
    data_dir: TempDir,
    /// Network upgrade activation heights
    #[getset(skip)]
    configured_activation_heights: ConfiguredActivationHeights,
    /// RPC request client
    client: RpcRequestClient,
    /// Network type
    network: NetworkKind,
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
            matches!(config.network, NetworkKind::Regtest) || config.chain_cache.is_some(),
            "chain cache must be specified when not using a regtest network!"
        );

        let working_cache_dir = data_dir.path().to_path_buf();

        if let Some(src) = config.chain_cache.as_ref() {
            Self::load_chain(src.clone(), working_cache_dir.clone(), config.network);
        }

        let network_listen_port = network::pick_unused_port(config.network_listen_port);
        let rpc_listen_port = network::pick_unused_port(config.rpc_listen_port);
        let indexer_listen_port = network::pick_unused_port(config.indexer_listen_port);
        let config_dir = tempfile::tempdir().unwrap();
        let config_file_path = config::zebrad(
            config_dir.path().to_path_buf(),
            working_cache_dir,
            network_listen_port,
            rpc_listen_port,
            indexer_listen_port,
            &config.configured_activation_heights,
            config.miner_address,
            config.network,
        )
        .unwrap();
        // create zcashd conf necessary for lightwalletd
        config::zcashd(
            config_dir.path(),
            rpc_listen_port,
            &config.configured_activation_heights,
            None,
        )
        .unwrap();

        let mut command = pick_command("zebrad");
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
        std::thread::sleep(std::time::Duration::from_secs(5));

        let rpc_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), rpc_listen_port);
        let client = zebra_node_services::rpc_client::RpcRequestClient::new(rpc_address);

        let zebrad = Zebrad {
            handle,
            network_listen_port,
            rpc_listen_port,
            config_dir,
            logs_dir,
            data_dir,
            configured_activation_heights: config.configured_activation_heights,
            client,
            network: config.network,
        };

        if config.chain_cache.is_none() && matches!(config.network, NetworkKind::Regtest) {
            // generate genesis block
            zebrad.generate_blocks(1).await.unwrap();
        }
        std::thread::sleep(std::time::Duration::from_secs(5));

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

impl Validator for Zebrad {
    async fn get_activation_heights(&self) -> ConfiguredActivationHeights {
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

        for _ in 0..n {
            let block_template: BlockTemplateResponse = self
                .client
                .json_result_from_call("getblocktemplate", "[]".to_string())
                .await
                .expect("response should be success output with a serialized `GetBlockTemplate`");

            let network = parameters::Network::new_regtest(
                ConfiguredActivationHeights {
                    before_overwinter: self.configured_activation_heights.before_overwinter,
                    overwinter: self.configured_activation_heights.overwinter,
                    sapling: self.configured_activation_heights.sapling,
                    blossom: self.configured_activation_heights.blossom,
                    heartwood: self.configured_activation_heights.heartwood,
                    canopy: self.configured_activation_heights.canopy,
                    nu5: self.configured_activation_heights.nu5,
                    nu6: self.configured_activation_heights.nu6,
                    nu6_1: self.configured_activation_heights.nu6_1,
                    nu7: self.configured_activation_heights.nu7,
                }
                .into(),
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

            let submit_block_response = self
                .client
                .text_from_call("submitblock", format!(r#"["{block_data}"]"#))
                .await
                .unwrap();

            if !submit_block_response.contains(r#""result":null"#) {
                tracing::error!("Failed to submit block: {submit_block_response}");
                panic!("Failed to submit block!");
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

    fn network(&self) -> NetworkKind {
        self.network
    }

    fn load_chain(
        chain_cache: PathBuf,
        validator_data_dir: PathBuf,
        validator_network: NetworkKind,
    ) -> PathBuf {
        let state_dir = chain_cache.clone().join("state");
        assert!(state_dir.exists(), "state directory not found!");

        if matches!(validator_network, NetworkKind::Regtest) {
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

    fn get_port(&self) -> Port {
        self.rpc_listen_port()
    }
}

impl Drop for Zebrad {
    fn drop(&mut self) {
        self.stop();
    }
}
