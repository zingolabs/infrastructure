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
    ProcessId, config,
    error::LaunchError,
    launch,
    logs::LogsToDir,
    network,
    process::Process,
    utils::executable_finder::{EXPECT_SPAWN, pick_command},
    validator::Validator,
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
#[derive(Clone, Debug)]
pub struct ZcashdConfig {
    /// Zcashd RPC listen port
    pub rpc_listen_port: Option<u16>,
    /// Local network upgrade activation heights
    pub activation_heights: ActivationHeights,
    /// Miner address
    pub miner_address: Option<&'static str>,
    /// Chain cache path
    pub chain_cache: Option<PathBuf>,
    /// When `true`, launch zcashd with `-disableshieldedproving`,
    /// which skips loading the Sapling/Orchard proving keys at
    /// startup (~6 s saved on this hardware) at the cost of
    /// disabling proof-creating RPCs (`z_sendmany`,
    /// `z_shieldcoinbase`, mining to a shielded `mineraddress`).
    /// Block validation, transaction validation, sync and
    /// transparent mining all continue to work because they only
    /// need verifying keys, which load in milliseconds.
    ///
    /// Default `true`: every default-launch test in this crate (and
    /// every downstream consumer that does its proving client-side
    /// via zingolib) sees the fast path. Set to `false` for any
    /// test that drives zcashd to *create* a shielded proof
    /// itself; that test pays the full ~6 s cold start.
    ///
    /// See zingolabs/infrastructure#254 for the diagnosis that led
    /// to this knob.
    pub disable_shielded_proving: bool,
    /// When `true`, launch zcashd with `-disablewallet`, which skips
    /// loading the wallet at startup. The harness uses zcashd for
    /// chain state, not for wallet operations (clients drive their
    /// own wallet via zingolib/zaino), so this is the correct
    /// default for the common case.
    ///
    /// Default `true`. Auto-flipped to `false` by
    /// [`ValidatorConfig::set_test_parameters`] when `mine_to_pool
    /// != Transparent` (zcashd needs wallet code to materialize a
    /// shielded coinbase from `mineraddress`). Tests that drive
    /// wallet RPCs directly (`z_sendmany`, `getnewaddress`,
    /// `z_shieldcoinbase`, etc.) must set this to `false`
    /// themselves.
    ///
    /// Unlike `disable_shielded_proving`, this is a *stock* zcashd
    /// flag — no patched fork or capability probe required.
    pub disable_wallet: bool,
}

impl Default for ZcashdConfig {
    fn default() -> Self {
        Self {
            rpc_listen_port: None,
            activation_heights: crate::validator::regtest_test_activation_heights(),
            // Mine to a transparent address by default. `Zcashd::launch`
            // always mines a genesis block, and an Orchard or Sapling
            // coinbase forces zcashd to generate a Halo2 / Groth16 proof
            // (~1 s pre-NU6.1, ~4 s post-NU6.1 for Orchard) for every
            // mined block. The harness's lifecycle/launch tests don't
            // use the funds — the proving cost was pure overhead.
            // Tests that need shielded-mined funds opt in via
            // `ValidatorConfig::set_test_parameters` with
            // `PoolType::ORCHARD` or `PoolType::SAPLING`.
            miner_address: Some(REG_T_ADDR_FROM_ABANDONART),
            chain_cache: None,
            disable_shielded_proving: true,
            disable_wallet: true,
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
        // Shielded coinbase needs the wallet to materialize the
        // mineraddress output. Re-enable for non-Transparent pools
        // so callers don't have to know about the disable_wallet
        // default.
        if !matches!(mine_to_pool, PoolType::Transparent) {
            self.disable_wallet = false;
        }
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

/// Pre-launch capability probe: confirm the resolved zcashd binary
/// accepts `-disableshieldedproving`. The Zingolabs patched fork
/// accepts the flag and exits 0 from `-version`; stock zcashd
/// rejects the unknown option and exits non-zero. Run from
/// `Zcashd::launch_once` before any tempdirs are created when
/// `ZcashdConfig::disable_shielded_proving = true` (the default), so
/// an unpatched binary fails fast with a clear, actionable error
/// instead of producing a confusing failure deep in the launch
/// pipeline.
fn ensure_disableshieldedproving_supported() -> Result<(), LaunchError> {
    const FLAG: &str = "-disableshieldedproving";
    let mut command = pick_command("zcashd", false);
    command.arg(FLAG).arg("-version");
    let output = command
        .output()
        .map_err(|e| LaunchError::CapabilityProbeFailed {
            process_name: ProcessId::Zcashd.to_string(),
            capability: FLAG,
            io_error: e.to_string(),
        })?;
    if !output.status.success() {
        return Err(LaunchError::UnsupportedZcashdCapability {
            process_name: ProcessId::Zcashd.to_string(),
            capability: FLAG,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            hint: concat!(
                "Hint: this looks like stock zcashd, which does not accept ",
                "`-disableshieldedproving`. The Zingolabs harness defaults to ",
                "passing this flag for the proving-key-load fast path. Either ",
                "install the Zingolabs patched fork ",
                "(https://github.com/zingolabs/zcash) and point TEST_BINARIES_DIR ",
                "at it, or set ZcashdConfig::disable_shielded_proving = false ",
                "(slower; loads Sapling/Orchard proving keys at startup)."
            )
            .to_string(),
        });
    }
    Ok(())
}

impl Zcashd {
    /// Single launch attempt: pick a port, write the config, spawn
    /// zcashd, wait for the readiness indicator, generate genesis if
    /// not loading from a cache. Wrapped by `Process::launch` in a
    /// bounded retry-on-port-collision loop (see
    /// `launch::with_retry_on_collision`); each retry calls this fresh
    /// with a config whose port pins have been cleared so
    /// `ZcashdPorts::pick` re-rolls them via `network::pick_unused_port`.
    async fn launch_once(config: ZcashdConfig) -> Result<Self, LaunchError> {
        if config.disable_shielded_proving {
            ensure_disableshieldedproving_supported()?;
        }

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

        // Skip Sapling/Orchard proving-key load when no test on this
        // launch needs zcashd to build a shielded proof. Default-true
        // — see `ZcashdConfig::disable_shielded_proving` and
        // zingolabs/infrastructure#254.
        if config.disable_shielded_proving {
            command.arg("-disableshieldedproving");
        }

        // Skip wallet load when the launch isn't going to use wallet
        // RPCs. Default-true; auto-flipped to false by
        // `set_test_parameters` for non-Transparent mining pools.
        // See `ZcashdConfig::disable_wallet`.
        if config.disable_wallet {
            command.arg("-disablewallet");
        }

        let spawn_start = std::time::Instant::now();
        let mut handle = command.spawn().expect(EXPECT_SPAWN);
        tracing::info!(
            elapsed_ms = spawn_start.elapsed().as_millis() as u64,
            "zcashd: process spawned"
        );

        let wait_start = std::time::Instant::now();
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
        tracing::info!(
            elapsed_ms = wait_start.elapsed().as_millis() as u64,
            "zcashd: launch::wait returned (Done loading observed)"
        );

        let zcashd = Zcashd {
            handle,
            port,
            config_dir,
            logs_dir,
            data_dir,
        };

        if config.chain_cache.is_none() {
            // generate genesis block
            let genesis_start = std::time::Instant::now();
            zcashd.generate_blocks(1).await.unwrap();
            tracing::info!(
                elapsed_ms = genesis_start.elapsed().as_millis() as u64,
                "zcashd: genesis block mined (post-launch generate_blocks(1))"
            );
        }

        Ok(zcashd)
    }
}

impl Process for Zcashd {
    const PROCESS: ProcessId = ProcessId::Zcashd;

    type Config = ZcashdConfig;

    async fn launch(config: Self::Config) -> Result<Self, LaunchError> {
        // Stderr signatures that zcashd emits when its RPC bind hits
        // an `EADDRINUSE`. The user-facing `Error: Unable to start
        // HTTP server` is the canonical line; the preceding `Unable to
        // bind any endpoint for RPC server` is also reliable.
        // `AddrInUse` / `Address already in use` are libc-level
        // strings included as belt-and-braces — zcashd does not emit
        // them today, but a future build that surfaces the raw OS
        // error string would still be classified correctly.
        const COLLISION_SIGNATURES: &[&str] = &[
            "Unable to start HTTP server",
            "Unable to bind any endpoint",
            "AddrInUse",
            "Address already in use",
        ];
        const MAX_ATTEMPTS: u32 = 3;

        launch::with_retry_on_collision(
            "zcashd",
            config,
            COLLISION_SIGNATURES,
            MAX_ATTEMPTS,
            |c: &ZcashdConfig| c.rpc_listen_port.into_iter().collect(),
            |c: &mut ZcashdConfig| {
                // Single-port validator — clear the only pin so the
                // next attempt's `ZcashdPorts::pick` calls
                // `network::pick_unused_port(None)` and the kernel
                // hands back a fresh ephemeral.
                c.rpc_listen_port = None;
            },
            Self::launch_once,
        )
        .await
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
    /// Tighter than the trait default (100 ms) because every
    /// `get_chain_height` call here spawns `zcash-cli` as a subprocess
    /// — process exec + RPC round-trip + JSON parse, ~50-100 ms by
    /// itself — so the per-poll cycle is `spawn + interval`. Idle wait
    /// of 100 ms between spawns wastes time the chain might already be
    /// at target. 25 ms keeps us responsive without back-to-back
    /// spawning faster than zcashd can answer.
    const CHAIN_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(25);

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

        let cli_start = std::time::Instant::now();
        self.zcash_cli_command(&["generate", &n.to_string()])?;
        let cli_ms = cli_start.elapsed().as_millis() as u64;

        let poll_start = std::time::Instant::now();
        self.poll_chain_height(chain_height + n).await;
        let poll_ms = poll_start.elapsed().as_millis() as u64;

        tracing::info!(
            n,
            target_height = chain_height + n,
            cli_ms,
            poll_ms,
            total_ms = cli_ms + poll_ms,
            "zcashd: generate_blocks"
        );

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

#[cfg(test)]
mod unit_tests {
    /// Regression tests for `ZcashdConfig`'s overrideable defaults.
    /// Each test pins a single specced default; flipping that
    /// default in code without updating the spec breaks exactly one
    /// of these tests.
    mod overrideable_defaults {
        use super::super::*;
        use crate::validator::ValidatorConfig;

        #[test]
        fn default_disables_shielded_proving() {
            assert!(
                ZcashdConfig::default().disable_shielded_proving,
                "ZcashdConfig::default().disable_shielded_proving must be \
                 true: the harness's narrow zcashd scope and the \
                 patched-fork capability probe at launch both depend on \
                 it. Flipping this default requires a deliberate spec \
                 change (see CHANGELOG)."
            );
        }

        #[test]
        fn default_disables_wallet() {
            assert!(
                ZcashdConfig::default().disable_wallet,
                "ZcashdConfig::default().disable_wallet must be true: \
                 the harness uses zcashd for chain state, not wallet \
                 operations (clients drive their own wallet via \
                 zingolib/zaino). Flipping this default requires a \
                 deliberate spec change (see CHANGELOG)."
            );
        }

        #[test]
        fn set_test_parameters_transparent_pool_keeps_wallet_disabled() {
            let mut config = ZcashdConfig::default();
            config.set_test_parameters(PoolType::Transparent, ActivationHeights::default(), None);
            assert!(
                config.disable_wallet,
                "Transparent mining does not need zcashd's wallet to \
                 materialize coinbase outputs; set_test_parameters must \
                 preserve the default-true `disable_wallet` for \
                 PoolType::Transparent."
            );
        }

        #[test]
        fn set_test_parameters_orchard_pool_enables_wallet() {
            let mut config = ZcashdConfig::default();
            config.set_test_parameters(PoolType::ORCHARD, ActivationHeights::default(), None);
            assert!(
                !config.disable_wallet,
                "Orchard mining needs zcashd's wallet to materialize the \
                 shielded coinbase output from `mineraddress`; \
                 set_test_parameters must auto-flip `disable_wallet` to \
                 false for PoolType::ORCHARD so callers don't have to \
                 know about the default."
            );
        }

        #[test]
        fn set_test_parameters_sapling_pool_enables_wallet() {
            let mut config = ZcashdConfig::default();
            config.set_test_parameters(PoolType::SAPLING, ActivationHeights::default(), None);
            assert!(
                !config.disable_wallet,
                "Sapling mining needs zcashd's wallet to materialize \
                 the shielded coinbase output from `mineraddress`; \
                 set_test_parameters must auto-flip `disable_wallet` to \
                 false for PoolType::SAPLING so callers don't have to \
                 know about the default."
            );
        }
    }
}
