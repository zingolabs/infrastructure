//! The Zebrad executable support struct and associated.

use std::{path::PathBuf, process::Child};

use getset::{CopyGetters, Getters};
use tempfile::TempDir;

use zcash_protocol::PoolType;

use zingo_common_components::protocol::{ActivationHeights, NetworkType};
use zingo_test_vectors::{
    REG_O_ADDR_FROM_ABANDONART, REG_T_ADDR_FROM_ABANDONART, REG_Z_ADDR_FROM_ABANDONART,
};

use crate::logs::LogsToStdoutAndStderr;
use crate::utils::executable_finder::trace_version_and_location;
use crate::validator::ValidatorConfig;
use crate::{
    config,
    error::LaunchError,
    launch,
    logs::LogsToDir,
    network,
    process::Process,
    utils::executable_finder::{pick_command, EXPECT_SPAWN},
    validator::Validator,
    ProcessId,
};

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
    pub rpc_listen_port: Option<u16>,
    /// Local network upgrade activation heights
    pub activation_heights: ActivationHeights,
    /// Miner address
    pub miner_address: Option<&'static str>,
    /// Chain cache path
    pub chain_cache: Option<PathBuf>,
}

impl Default for ZcashdConfig {
    fn default() -> Self {
        Self {
            rpc_listen_port: None,
            activation_heights: crate::validator::regtest_test_activation_heights(),
            miner_address: Some(REG_O_ADDR_FROM_ABANDONART),
            chain_cache: None,
        }
    }
}

impl ValidatorConfig for ZcashdConfig {
    fn set_test_parameters(
        &mut self,
        mine_to_pool: PoolType,
        activation_heights: ActivationHeights,
        chain_cache: Option<PathBuf>,
    ) {
        self.miner_address = Some(match mine_to_pool {
            PoolType::ORCHARD => REG_O_ADDR_FROM_ABANDONART,
            PoolType::SAPLING => REG_Z_ADDR_FROM_ABANDONART,
            PoolType::Transparent => REG_T_ADDR_FROM_ABANDONART,
        });
        self.activation_heights = activation_heights;
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
    port: u16,
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
        let mut command = pick_command("zcash-cli", false);

        command.arg(format!("-conf={}", self.config_path().to_str().unwrap()));
        command.args(args).output()
    }
}

impl LogsToDir for Zcashd {
    fn logs_dir(&self) -> &TempDir {
        &self.logs_dir
    }
}

/// Listen ports zcashd needs to bind during launch. A single-field
/// counterpart to `ZebradPorts` — kept symmetric so the planned
/// retry-on-collision helper in `launch::wait` can treat all
/// validators uniformly and re-roll an entire validator's port set
/// in one call rather than open-coding the picks per validator.
#[derive(Debug, Clone, Copy)]
struct ZcashdPorts {
    rpc: u16,
}

impl ZcashdPorts {
    fn pick(config: &ZcashdConfig) -> Self {
        Self {
            rpc: network::pick_unused_port(config.rpc_listen_port),
        }
    }
}

impl Process for Zcashd {
    const PROCESS: ProcessId = ProcessId::Zcashd;

    type Config = ZcashdConfig;

    async fn launch(config: Self::Config) -> Result<Self, LaunchError> {
        let logs_dir = tempfile::tempdir().unwrap();
        let data_dir = tempfile::tempdir().unwrap();

        if let Some(cache) = config.chain_cache.clone() {
            Self::load_chain(
                cache,
                data_dir.path().to_path_buf(),
                NetworkType::Regtest(ActivationHeights::default()),
            );
        }

        let activation_heights = config.activation_heights;
        tracing::info!(
            "Configuring zcashd to regtest with these activation heights: {activation_heights:?}"
        );

        let ZcashdPorts { rpc: port } = ZcashdPorts::pick(&config);
        let config_dir = tempfile::tempdir().unwrap();
        let config_file_path = config::write_zcashd_config(
            config_dir.path(),
            port,
            activation_heights,
            config.miner_address,
        )
        .unwrap();

        let executable_name = "zcashd";
        trace_version_and_location(executable_name, "--version");
        trace_version_and_location("zcash-cli", "--version");

        let mut command = pick_command(executable_name, false);
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

        launch::wait(
            ProcessId::Zcashd,
            &mut handle,
            &logs_dir,
            None,
            &["init message: Done loading"],
            &["Error:"],
            &[],
        )
        .await?;

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
    async fn get_activation_heights(&self) -> ActivationHeights {
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
    async fn get_chain_height(&self) -> u32 {
        let output = self
            .zcash_cli_command(&["getchaintips"])
            .expect(EXPECT_SPAWN);
        let stdout_json = json::parse(&String::from_utf8_lossy(&output.stdout)).unwrap();
        stdout_json[0]["height"].as_u32().unwrap()
    }

    fn data_dir(&self) -> &TempDir {
        &self.data_dir
    }

    fn get_zcashd_conf_path(&self) -> PathBuf {
        self.config_dir.path().join(config::ZCASHD_FILENAME)
    }

    fn network(&self) -> NetworkType {
        unimplemented!();
    }

    fn load_chain(
        chain_cache: PathBuf,
        validator_data_dir: PathBuf,
        _validator_network: NetworkType,
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

    fn get_port(&self) -> u16 {
        self.port()
    }
}

impl Drop for Zcashd {
    fn drop(&mut self) {
        self.stop();
    }
}
