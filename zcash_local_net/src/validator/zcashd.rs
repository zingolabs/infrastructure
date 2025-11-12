//! The Zebrad executable support struct and associated.

use std::{path::PathBuf, process::Child};

use getset::{CopyGetters, Getters};
use portpicker::Port;
use tempfile::TempDir;
use zcash_protocol::PoolType;
use zebra_chain::parameters::testnet::ConfiguredActivationHeights;
use zebra_chain::parameters::NetworkKind;
use zingo_test_vectors::{
    REG_O_ADDR_FROM_ABANDONART, REG_T_ADDR_FROM_ABANDONART, REG_Z_ADDR_FROM_ABANDONART,
};

use crate::logs::LogsToStdoutAndStderr;
use crate::validator::ValidatorConfig;
use crate::{
    config,
    error::LaunchError,
    launch,
    logs::{self, LogsToDir},
    network,
    process::Process,
    utils::executable_finder::{pick_command, EXPECT_SPAWN},
    validator::Validator,
    ProcessId,
};
use zingo_common_components::protocol::activation_heights::for_test;

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
#[derive(Debug)]
pub struct ZcashdConfig {
    /// Zcashd RPC listen port
    pub rpc_listen_port: Option<Port>,
    /// Local network upgrade activation heights
    pub configured_activation_heights: ConfiguredActivationHeights,
    /// Miner address
    pub miner_address: Option<&'static str>,
    /// Chain cache path
    pub chain_cache: Option<PathBuf>,
}

impl Default for ZcashdConfig {
    fn default() -> Self {
        Self {
            rpc_listen_port: None,
            configured_activation_heights: for_test::all_height_one_nus(),
            miner_address: Some(REG_O_ADDR_FROM_ABANDONART),
            chain_cache: None,
        }
    }
}

impl ValidatorConfig for ZcashdConfig {
    fn set_test_parameters(
        &mut self,
        mine_to_pool: PoolType,
        configured_activation_heights: ConfiguredActivationHeights,
        chain_cache: Option<PathBuf>,
    ) {
        self.miner_address = Some(match mine_to_pool {
            PoolType::ORCHARD => REG_O_ADDR_FROM_ABANDONART,
            PoolType::SAPLING => REG_Z_ADDR_FROM_ABANDONART,
            PoolType::Transparent => REG_T_ADDR_FROM_ABANDONART,
        });
        self.configured_activation_heights = configured_activation_heights;
        self.chain_cache = chain_cache;
    }
}

/// This struct is used to represent and manage the Zcashd process.
#[derive(Debug, Getters, CopyGetters)]
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
}

impl Zcashd {
    /// Returns path to config file.
    fn config_path(&self) -> PathBuf {
        self.config_dir().path().join(config::ZCASHD_FILENAME)
    }

    /// Runs a Zcash-cli command with the given `args`.
    ///
    /// Example usage for generating blocks in Zcashd local net:
    /// ```ignore (incomplete)
    /// self.zcash_cli_command(&["generate", "1"]);
    /// ```
    pub fn zcash_cli_command(&self, args: &[&str]) -> std::io::Result<std::process::Output> {
        let mut command = pick_command("zcash-cli");

        command.arg(format!("-conf={}", self.config_path().to_str().unwrap()));
        command.args(args).output()
    }
}

impl LogsToDir for Zcashd {
    fn logs_dir(&self) -> &TempDir {
        &self.logs_dir
    }
}

impl Process for Zcashd {
    const PROCESS: ProcessId = ProcessId::Zcashd;

    type Config = ZcashdConfig;

    async fn launch(config: Self::Config) -> Result<Self, LaunchError> {
        let logs_dir = tempfile::tempdir().unwrap();
        let data_dir = tempfile::tempdir().unwrap();

        if let Some(cache) = config.chain_cache.clone() {
            Self::load_chain(cache, data_dir.path().to_path_buf(), NetworkKind::Regtest);
        }

        let configured_activation_heights = &config.configured_activation_heights;
        tracing::info!("Configuring zcashd to regtest with these activation heights: {configured_activation_heights:?}");

        let port = network::pick_unused_port(config.rpc_listen_port);
        let config_dir = tempfile::tempdir().unwrap();
        let config_file_path = config::zcashd(
            config_dir.path(),
            port,
            configured_activation_heights,
            config.miner_address,
        )
        .unwrap();

        let mut command = pick_command("zcashd");
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

        let mut handle = command.spawn().expect(EXPECT_SPAWN);

        logs::write_logs(&mut handle, &logs_dir);
        launch::wait(
            ProcessId::Zcashd,
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
                    tracing::error!("zcashd cannot be awaited: {e}");
                } else {
                    tracing::info!("zcashd successfully shut down");
                }
            }
            Err(e) => {
                tracing::error!(
                    "Can't stop zcashd from zcash-cli: {e}\n\
                    Sending SIGKILL to zcashd process."
                );
                if let Err(e) = self.handle.kill() {
                    tracing::warn!("zcashd has already terminated: {e}");
                }
            }
        }
    }

    fn print_all(&self) {
        <Zcashd as LogsToStdoutAndStderr>::print_stdout(self);
        self.print_stderr();
    }
}

impl Validator for Zcashd {
    async fn get_activation_heights(&self) -> ConfiguredActivationHeights {
        let output = self
            .zcash_cli_command(&["getblockchaininfo"])
            .expect("getblockchaininfo should succeed");

        let response: serde_json::Value =
            serde_json::from_str(&String::from_utf8_lossy(&output.stdout))
                .expect("should parse JSON response");

        let upgrades = response
            .get("upgrades")
            .expect("upgrades field should exist")
            .as_object()
            .expect("upgrades should be an object");

        crate::validator::parse_activation_heights_from_rpc(upgrades)
    }
    async fn generate_blocks(&self, n: u32) -> std::io::Result<()> {
        let chain_height = self.get_chain_height().await;
        self.zcash_cli_command(&["generate", &n.to_string()])?;
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
        let output = self
            .zcash_cli_command(&["getchaintips"])
            .expect(EXPECT_SPAWN);
        let stdout_json = json::parse(&String::from_utf8_lossy(&output.stdout)).unwrap();
        stdout_json[0]["height"].as_u32().unwrap()
    }

    async fn poll_chain_height(&self, target_height: u32) {
        while self.get_chain_height().await < target_height {
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    }

    fn data_dir(&self) -> &TempDir {
        &self.data_dir
    }

    fn get_zcashd_conf_path(&self) -> PathBuf {
        self.config_dir.path().join(config::ZCASHD_FILENAME)
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
        assert!(regtest_dir.exists(), "regtest directory not found!");

        std::process::Command::new("cp")
            .arg("-r")
            .arg(regtest_dir)
            .arg(validator_data_dir)
            .output()
            .unwrap();
        chain_cache
    }

    fn get_port(&self) -> Port {
        self.port()
    }
}

impl Drop for Zcashd {
    fn drop(&mut self) {
        self.stop();
    }
}
