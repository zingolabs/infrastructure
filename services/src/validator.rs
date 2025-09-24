//! Module for the structs that represent and manage the validator/full-node processes i.e. Zebrad.
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    process::Child,
};

use zcash_protocol::consensus::{BlockHeight, Parameters};

use getset::{CopyGetters, Getters};
use portpicker::Port;
use tempfile::TempDir;
use zebra_chain::parameters::NetworkKind;
use zebra_chain::{
    parameters::testnet::ConfiguredActivationHeights, serialization::ZcashSerialize as _,
};
use zebra_node_services::rpc_client::RpcRequestClient;
use zebra_rpc::{
    client::{BlockTemplateResponse, BlockTemplateTimeSource},
    proposal_block_from_template,
};

use crate::{
    config, error::LaunchError, launch, logs, network, utils::ExecutableLocation, Process,
};

/// Returns a LocalNetwork with all upgrades activated at height 1
pub fn default_regtest_heights() -> zcash_protocol::local_consensus::LocalNetwork {
    zcash_protocol::local_consensus::LocalNetwork {
        overwinter: Some(BlockHeight::from(1)),
        sapling: Some(BlockHeight::from(1)),
        blossom: Some(BlockHeight::from(1)),
        heartwood: Some(BlockHeight::from(1)),
        canopy: Some(BlockHeight::from(1)),
        nu5: Some(BlockHeight::from(1)),
        nu6: Some(BlockHeight::from(1)),
        nu6_1: Some(BlockHeight::from(1)),
    }
}

/// Returns a LocalNetwork with sequential activation heights (1, 2, 3, 4, 5, 6, 7, 8)
pub fn sequential_regtest_heights() -> zcash_protocol::local_consensus::LocalNetwork {
    zcash_protocol::local_consensus::LocalNetwork {
        overwinter: Some(BlockHeight::from(1)),
        sapling: Some(BlockHeight::from(2)),
        blossom: Some(BlockHeight::from(3)),
        heartwood: Some(BlockHeight::from(4)),
        canopy: Some(BlockHeight::from(5)),
        nu5: Some(BlockHeight::from(6)),
        nu6: Some(BlockHeight::from(7)),
        nu6_1: Some(BlockHeight::from(8)),
    }
}

/// faucet addresses
/// this should be in a test-vectors crate. However, in order to distangle this knot, a cut and paste in merited here -fv
pub const REG_O_ADDR_FROM_ABANDONART: &str = "uregtest1zkuzfv5m3yhv2j4fmvq5rjurkxenxyq8r7h4daun2zkznrjaa8ra8asgdm8wwgwjvlwwrxx7347r8w0ee6dqyw4rufw4wg9djwcr6frzkezmdw6dud3wsm99eany5r8wgsctlxquu009nzd6hsme2tcsk0v3sgjvxa70er7h27z5epr67p5q767s2z5gt88paru56mxpm6pwz0cu35m";
/// TODO: Add Doc Comment Here!
pub const REG_Z_ADDR_FROM_ABANDONART: &str =
    "zregtestsapling1fmq2ufux3gm0v8qf7x585wj56le4wjfsqsj27zprjghntrerntggg507hxh2ydcdkn7sx8kya7p";
/// TODO: Add Doc Comment Here!
pub const REG_T_ADDR_FROM_ABANDONART: &str = "tmBsTi2xWTjUdEXnuTceL7fecEQKeWaPDJd";

/// Zebrad default miner address. Regtest/Testnet transparent address for [Abandon Abandon .. Art] seed (entropy all zeros)
pub const ZEBRAD_DEFAULT_MINER: &str = "tmBsTi2xWTjUdEXnuTceL7fecEQKeWaPDJd";

/// Zcashd configuration
///
/// Use `zcashd_bin` and `zcash_cli_bin` to specify the paths to the binaries.
/// If these binaries are in $PATH, `None` can be specified to run "zcashd" / "zcash-cli".
///
/// If `rpc_listen_port` is `None`, a port is picked at random between 15000-25000.
///
/// Use `activation_heights` to specify custom network upgrade activation heights.
///
/// Use `miner_address` to specify the target address for the block rewards when blocks are generated.
///
/// If `chain_cache` path is `None`, a new chain is launched.
pub struct ZcashdConfig {
    /// Zcashd binary location
    pub zcashd_bin: ExecutableLocation,
    /// Zcash-cli binary location
    pub zcash_cli_bin: ExecutableLocation,
    /// Zcashd RPC listen port
    pub rpc_listen_port: Option<Port>,
    /// Local network upgrade activation heights
    pub activation_heights: zcash_protocol::local_consensus::LocalNetwork,
    /// Miner address
    pub miner_address: Option<&'static str>,
    /// Chain cache path
    pub chain_cache: Option<PathBuf>,
}

impl ZcashdConfig {
    /// Default location for `zcashd` resolved via `PATH`.
    pub fn default_location() -> ExecutableLocation {
        ExecutableLocation::by_name("zcashd")
    }

    /// Default location for `zcash-cli` resolved via `PATH`.
    pub fn default_cli_location() -> ExecutableLocation {
        ExecutableLocation::by_name("zcash-cli")
    }

    /// Regtest-friendly defaults for testing.
    pub fn default_test() -> Self {
        Self {
            zcashd_bin: Self::default_location(),
            zcash_cli_bin: Self::default_cli_location(),
            rpc_listen_port: None,
            activation_heights: default_regtest_heights(),
            miner_address: Some(REG_O_ADDR_FROM_ABANDONART),
            chain_cache: None,
        }
    }
}

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
pub struct ZebradConfig {
    /// Zebrad binary location
    pub zebrad_bin: ExecutableLocation,
    /// Zebrad network listen port
    pub network_listen_port: Option<Port>,
    /// Zebrad JSON-RPC listen port
    pub rpc_listen_port: Option<Port>,
    /// Zebrad gRPC listen port
    pub indexer_listen_port: Option<Port>,
    /// Local network upgrade activation heights
    pub activation_heights: zcash_protocol::local_consensus::LocalNetwork,
    /// Miner address
    pub miner_address: &'static str,
    /// Chain cache path
    pub chain_cache: Option<PathBuf>,
    /// Network type
    pub network: NetworkKind,
}

impl ZebradConfig {
    /// Default location for `zebrad` resolved via `PATH`.
    pub fn default_location() -> ExecutableLocation {
        ExecutableLocation::by_name("zebrad")
    }

    /// Zebrad defaults for testing
    pub fn default_test() -> Self {
        Self {
            zebrad_bin: Self::default_location(),
            network_listen_port: None,
            rpc_listen_port: None,
            indexer_listen_port: None,
            activation_heights: default_regtest_heights(),
            miner_address: ZEBRAD_DEFAULT_MINER,
            chain_cache: None,
            network: NetworkKind::Regtest,
        }
    }
}

/// Functionality for validator/full-node processes.
pub trait Validator: Sized {
    /// Config filename
    const CONFIG_FILENAME: &str;

    /// Process
    const PROCESS: Process;

    /// Validator config struct
    type Config;

    /// Return activation heights
    fn activation_heights(&self) -> zcash_protocol::local_consensus::LocalNetwork;

    /// generate a default test config
    fn default_test_config() -> Self::Config;

    /// Launch the process.
    fn launch(
        config: Self::Config,
    ) -> impl std::future::Future<Output = Result<Self, LaunchError>> + Send;

    /// Stop the process.
    fn stop(&mut self);

    /// Generate `n` blocks. This implementation should also call [`Self::poll_chain_height`] so the chain is at the
    /// correct height when this function returns.
    fn generate_blocks(
        &self,
        n: u32,
    ) -> impl std::future::Future<Output = std::io::Result<()>> + Send;

    /// Get chain height
    fn get_chain_height(&self) -> impl std::future::Future<Output = BlockHeight> + Send;

    /// Polls chain until it reaches target height
    fn poll_chain_height(
        &self,
        target_height: BlockHeight,
    ) -> impl std::future::Future<Output = ()> + Send;

    /// Get temporary config directory.
    fn config_dir(&self) -> &TempDir;

    /// Get temporary logs directory.
    fn logs_dir(&self) -> &TempDir;

    /// Get temporary data directory.
    fn data_dir(&self) -> &TempDir;

    /// Returns path to config file.
    fn config_path(&self) -> PathBuf {
        self.config_dir().path().join(Self::CONFIG_FILENAME)
    }

    /// Network type
    fn network(&self) -> NetworkKind;

    /// Caches chain. This stops the zcashd process.
    fn cache_chain(&mut self, chain_cache: PathBuf) -> std::process::Output {
        if chain_cache.exists() {
            panic!("chain cache already exists!");
        }

        self.stop();
        std::thread::sleep(std::time::Duration::from_secs(3));

        std::process::Command::new("cp")
            .arg("-r")
            .arg(self.data_dir().path())
            .arg(chain_cache)
            .output()
            .unwrap()
    }

    /// Checks `chain cache` is valid and loads into `validator_data_dir`.
    /// Returns the path to the loaded chain cache.
    ///
    /// If network is not `Regtest` variant, the chain cache will not be copied and the original cache path will be
    /// returned instead
    fn load_chain(
        chain_cache: PathBuf,
        validator_data_dir: PathBuf,
        validator_network: NetworkKind,
    ) -> PathBuf;

    /// Prints the stdout log.
    fn print_stdout(&self) {
        let stdout_log_path = self.logs_dir().path().join(logs::STDOUT_LOG);
        logs::print_log(stdout_log_path);
    }

    /// Prints the stdout log.
    fn print_stderr(&self) {
        let stdout_log_path = self.logs_dir().path().join(logs::STDERR_LOG);
        logs::print_log(stdout_log_path);
    }

    /// Returns the validator process.
    fn process(&self) -> Process {
        Self::PROCESS
    }
}

/// This struct is used to represent and manage the Zcashd process.
#[derive(Getters, CopyGetters)]
#[getset(get = "pub")]
pub struct Zcashd {
    /// Child process handle
    handle: Child,
    /// RPC port
    #[getset(skip)]
    #[getset(get_copy = "pub")]
    port: Port,
    /// Config directory
    config_dir: TempDir,
    /// Logs directory
    logs_dir: TempDir,
    /// Data directory
    data_dir: TempDir,
    /// Zcash cli binary location
    zcash_cli_bin: ExecutableLocation,
    /// Network upgrade activation heights
    #[getset(skip)]
    activation_heights: zcash_protocol::local_consensus::LocalNetwork,
}

impl Zcashd {
    /// Runs a Zcash-cli command with the given `args`.
    ///
    /// Example usage for generating blocks in Zcashd local net:
    /// ```ignore (incomplete)
    /// self.zcash_cli_command(&["generate", "1"]);
    /// ```
    pub fn zcash_cli_command(&self, args: &[&str]) -> std::io::Result<std::process::Output> {
        let mut command = self.zcash_cli_bin.command();

        command.arg(format!("-conf={}", self.config_path().to_str().unwrap()));
        command.args(args).output()
    }
}

impl Validator for Zcashd {
    const CONFIG_FILENAME: &str = config::ZCASHD_FILENAME;
    const PROCESS: Process = Process::Zcashd;

    type Config = ZcashdConfig;

    fn activation_heights(&self) -> zcash_protocol::local_consensus::LocalNetwork {
        self.activation_heights
    }

    /// generate a default test config
    fn default_test_config() -> Self::Config {
        ZcashdConfig::default_test()
    }

    async fn launch(config: Self::Config) -> Result<Self, LaunchError> {
        let logs_dir = tempfile::tempdir().unwrap();
        let data_dir = tempfile::tempdir().unwrap();

        if let Some(cache) = config.chain_cache.clone() {
            Self::load_chain(cache, data_dir.path().to_path_buf(), NetworkKind::Regtest);
        }

        let port = network::pick_unused_port(config.rpc_listen_port);
        let config_dir = tempfile::tempdir().unwrap();
        let config_file_path = config::zcashd(
            config_dir.path(),
            port,
            &config.activation_heights,
            config.miner_address,
        )
        .unwrap();

        let mut command = config.zcashd_bin.command();
        command
            .args([
                "--printtoconsole",
                format!(
                    "--conf={}",
                    config_file_path.to_str().expect("should be valid UTF-8")
                )
                .as_str(),
                format!(
                    "--datadir={}",
                    data_dir.path().to_str().expect("should be valid UTF-8")
                )
                .as_str(),
                "-debug=1",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut handle = command.spawn().unwrap_or_else(|err| {
            let executable_location = config.zcashd_bin;
            panic!(
                "Running {executable_location:?}
{} {}
Error: {err}",
                command.get_program().to_string_lossy(),
                command
                    .get_args()
                    .map(|arg| arg.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        });

        logs::write_logs(&mut handle, &logs_dir);
        launch::wait(
            Process::Zcashd,
            &mut handle,
            &logs_dir,
            None,
            &["init message: Done loading"],
            &["Error:"],
            &[],
        )?;

        let zcashd = Zcashd {
            handle,
            port,
            config_dir,
            logs_dir,
            data_dir,
            zcash_cli_bin: config.zcash_cli_bin,
            activation_heights: config.activation_heights,
        };

        if config.chain_cache.is_none() {
            // generate genesis block
            zcashd.generate_blocks(1).await.unwrap();
        }

        Ok(zcashd)
    }

    fn stop(&mut self) {
        match self.zcash_cli_command(&["stop"]) {
            Ok(_) => {
                if let Err(e) = self.handle.wait() {
                    tracing::error!("zcashd cannot be awaited: {e}")
                } else {
                    tracing::info!("zcashd successfully shut down")
                };
            }
            Err(e) => {
                tracing::error!(
                    "Can't stop zcashd from zcash-cli: {e}\n\
                    Sending SIGKILL to zcashd process."
                );
                if let Err(e) = self.handle.kill() {
                    tracing::warn!("zcashd has already terminated: {e}")
                };
            }
        }
    }

    async fn generate_blocks(&self, n: u32) -> std::io::Result<()> {
        let chain_height = self.get_chain_height().await;
        self.zcash_cli_command(&["generate", &n.to_string()])?;
        self.poll_chain_height(chain_height + n).await;

        Ok(())
    }

    async fn get_chain_height(&self) -> BlockHeight {
        let output = self
            .zcash_cli_command(&["getchaintips"])
            .unwrap_or_else(|err| {
                let executable_location = &self.zcash_cli_bin;
                panic!(
                    "Running {executable_location:?}
getchaintips
Error: {err}",
                )
            });
        let stdout_json = json::parse(&String::from_utf8_lossy(&output.stdout)).unwrap();
        BlockHeight::from_u32(stdout_json[0]["height"].as_u32().unwrap())
    }

    async fn poll_chain_height(&self, target_height: BlockHeight) {
        while self.get_chain_height().await < target_height {
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    }

    fn config_dir(&self) -> &TempDir {
        &self.config_dir
    }

    fn logs_dir(&self) -> &TempDir {
        &self.logs_dir
    }

    fn data_dir(&self) -> &TempDir {
        &self.data_dir
    }

    fn network(&self) -> NetworkKind {
        unimplemented!();
    }

    fn load_chain(
        chain_cache: PathBuf,
        validator_data_dir: PathBuf,
        _validator_network: NetworkKind,
    ) -> PathBuf {
        let regtest_dir = chain_cache.clone().join("regtest");
        if !regtest_dir.exists() {
            panic!("regtest directory not found!");
        }

        std::process::Command::new("cp")
            .arg("-r")
            .arg(regtest_dir)
            .arg(validator_data_dir)
            .output()
            .unwrap();
        chain_cache
    }
}

impl Drop for Zcashd {
    fn drop(&mut self) {
        self.stop();
    }
}

/// This struct is used to represent and manage the Zebrad process.
#[derive(Getters, CopyGetters)]
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
    activation_heights: zcash_protocol::local_consensus::LocalNetwork,
    /// RPC request client
    client: RpcRequestClient,
    /// Network type
    network: NetworkKind,
}

impl Zebrad {
    // TODO: don't rely on `cp`
    /// Launches a Zebrad instance with an exclusive chain cache.
    pub async fn launch_with_cache(
        config: <Zebrad as Validator>::Config,
        cache_path: PathBuf,
    ) -> Result<Self, LaunchError> {
        let logs_dir = tempfile::tempdir().unwrap();
        let data_dir = tempfile::tempdir().unwrap();

        if !matches!(config.network, NetworkKind::Regtest) && config.chain_cache.is_none() {
            panic!("chain cache must be specified when not using a regtest network!")
        }

        Self::load_chain(
            cache_path.clone(),
            data_dir.path().to_path_buf(),
            config.network,
        );

        let network_listen_port = network::pick_unused_port(config.network_listen_port);
        let rpc_listen_port = network::pick_unused_port(config.rpc_listen_port);
        let indexer_listen_port = network::pick_unused_port(config.indexer_listen_port);
        let config_dir = tempfile::tempdir().unwrap();
        let config_file_path = config::zebrad(
            config_dir.path().to_path_buf(),
            data_dir.path().to_path_buf(),
            network_listen_port,
            rpc_listen_port,
            indexer_listen_port,
            &config.activation_heights,
            config.miner_address,
            config.network,
        )
        .unwrap();
        // create zcashd conf necessary for lightwalletd
        config::zcashd(
            config_dir.path(),
            rpc_listen_port,
            &config.activation_heights,
            None,
        )
        .unwrap();

        let mut command = config.zebrad_bin.command();
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

        let mut handle = command.spawn().unwrap_or_else(|err| {
            let executable_location = config.zebrad_bin;
            panic!(
                "Running {executable_location:?}
{} {}
Error: {err}",
                command.get_program().to_string_lossy(),
                command
                    .get_args()
                    .map(|arg| arg.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        });

        logs::write_logs(&mut handle, &logs_dir);
        launch::wait(
            Process::Zebrad,
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

        let rpc_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), rpc_listen_port);
        let client = zebra_node_services::rpc_client::RpcRequestClient::new(rpc_address);

        let zebrad = Zebrad {
            handle,
            network_listen_port,
            rpc_listen_port,
            config_dir,
            logs_dir,
            data_dir,
            activation_heights: config.activation_heights,
            client,
            network: config.network,
        };
        std::thread::sleep(std::time::Duration::from_secs(5));

        Ok(zebrad)
    }
}

impl Validator for Zebrad {
    const CONFIG_FILENAME: &str = config::ZEBRAD_FILENAME;
    const PROCESS: Process = Process::Zebrad;

    type Config = ZebradConfig;

    fn activation_heights(&self) -> zcash_protocol::local_consensus::LocalNetwork {
        self.activation_heights
    }

    /// generate a default test config
    fn default_test_config() -> Self::Config {
        ZebradConfig::default_test()
    }

    async fn launch(config: Self::Config) -> Result<Self, LaunchError> {
        let logs_dir = tempfile::tempdir().unwrap();
        let data_dir = tempfile::tempdir().unwrap();

        if !matches!(config.network, NetworkKind::Regtest) && config.chain_cache.is_none() {
            panic!("chain cache must be specified when not using a regtest network!")
        }

        let cache_dir = if let Some(cache) = config.chain_cache.clone() {
            Self::load_chain(cache.clone(), data_dir.path().to_path_buf(), config.network);
            cache
        } else {
            data_dir.path().to_path_buf()
        };

        let network_listen_port = network::pick_unused_port(config.network_listen_port);
        let rpc_listen_port = network::pick_unused_port(config.rpc_listen_port);
        let indexer_listen_port = network::pick_unused_port(config.indexer_listen_port);
        let config_dir = tempfile::tempdir().unwrap();
        let config_file_path = config::zebrad(
            config_dir.path().to_path_buf(),
            cache_dir,
            network_listen_port,
            rpc_listen_port,
            indexer_listen_port,
            &config.activation_heights,
            config.miner_address,
            config.network,
        )
        .unwrap();
        // create zcashd conf necessary for lightwalletd
        config::zcashd(
            config_dir.path(),
            rpc_listen_port,
            &config.activation_heights,
            None,
        )
        .unwrap();

        let mut command = config.zebrad_bin.command();
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

        let mut handle = command.spawn().unwrap_or_else(|err| {
            let executable_location = config.zebrad_bin;
            panic!(
                "Running {executable_location:?}
{} {}
Error: {err}",
                command.get_program().to_string_lossy(),
                command
                    .get_args()
                    .map(|arg| arg.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        });

        logs::write_logs(&mut handle, &logs_dir);
        launch::wait(
            Process::Zebrad,
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

        let rpc_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), rpc_listen_port);
        let client = zebra_node_services::rpc_client::RpcRequestClient::new(rpc_address);

        let zebrad = Zebrad {
            handle,
            network_listen_port,
            rpc_listen_port,
            config_dir,
            logs_dir,
            data_dir,
            activation_heights: config.activation_heights,
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
        self.handle.kill().expect("zebrad couldn't be killed")
    }

    async fn generate_blocks(&self, n: u32) -> std::io::Result<()> {
        let chain_height = self.get_chain_height().await;

        for _ in 0..n {
            let block_template: BlockTemplateResponse = self
                .client
                .json_result_from_call("getblocktemplate", "[]".to_string())
                .await
                .expect("response should be success output with a serialized `GetBlockTemplate`");
            use zcash_protocol::consensus::NetworkUpgrade;

            let network =
                zebra_chain::parameters::Network::new_regtest(ConfiguredActivationHeights {
                    before_overwinter: Some(1),
                    overwinter: self
                        .activation_heights
                        .activation_height(NetworkUpgrade::Overwinter)
                        .map(u32::from),
                    sapling: self
                        .activation_heights
                        .activation_height(NetworkUpgrade::Sapling)
                        .map(u32::from),
                    blossom: self
                        .activation_heights
                        .activation_height(NetworkUpgrade::Blossom)
                        .map(u32::from),
                    heartwood: self
                        .activation_heights
                        .activation_height(NetworkUpgrade::Heartwood)
                        .map(u32::from),
                    canopy: self
                        .activation_heights
                        .activation_height(NetworkUpgrade::Canopy)
                        .map(u32::from),
                    nu5: self
                        .activation_heights
                        .activation_height(NetworkUpgrade::Nu5)
                        .map(u32::from),
                    nu6: self
                        .activation_heights
                        .activation_height(NetworkUpgrade::Nu6)
                        .map(u32::from),
                    nu6_1: self
                        .activation_heights
                        .activation_height(NetworkUpgrade::Nu6_1)
                        .map(u32::from),
                    nu7: None,
                });

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
                dbg!(&submit_block_response);
                panic!("failed to submit block!");
            }
        }
        self.poll_chain_height(chain_height + n).await;

        Ok(())
    }

    async fn get_chain_height(&self) -> BlockHeight {
        let response: serde_json::Value = self
            .client
            .json_result_from_call("getblockchaininfo", "[]".to_string())
            .await
            .unwrap();

        let chain_height: u32 = response
            .get("blocks")
            .and_then(|h| h.as_u64())
            .and_then(|h| u32::try_from(h).ok())
            .unwrap();

        BlockHeight::from_u32(chain_height)
    }

    async fn poll_chain_height(&self, target_height: BlockHeight) {
        while self.get_chain_height().await < target_height {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    fn config_dir(&self) -> &TempDir {
        &self.config_dir
    }

    fn logs_dir(&self) -> &TempDir {
        &self.logs_dir
    }

    fn data_dir(&self) -> &TempDir {
        &self.data_dir
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
        if !state_dir.exists() {
            panic!("state directory not found!");
        }

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
}

impl Drop for Zebrad {
    fn drop(&mut self) {
        self.stop();
    }
}
