//! The Zainod executable support struct and associated.

use std::{path::PathBuf, process::Child};

use tempfile::TempDir;

use zingo_consensus::NetworkKind;

use crate::logs::LogsToDir;
use crate::logs::LogsToStdoutAndStderr as _;
use crate::utils::executable_finder::trace_version_and_location;
use crate::{
    ProcessId, config,
    error::LaunchError,
    indexer::{Indexer, IndexerConfig},
    launch,
    network::{self},
    process::Process,
    utils::executable_finder::pick_command,
};

/// Zainod configuration
///
/// If `listen_port` is `None`, a port is picked at random between 15000-25000.
///
/// The `validator_port` must be specified and the validator process must be running before launching Zainod.
///
/// `network` must match the configured network *kind* of the validator.
#[derive(Clone, Debug)]
pub struct ZainodConfig {
    /// Listen RPC port
    pub listen_port: Option<u16>,
    /// Validator RPC port
    pub validator_port: u16,
    /// Chain cache path
    pub chain_cache: Option<PathBuf>,
    /// Network kind — deliberately without activation heights. The
    /// Indexer must learn heights from the Validator, never from
    /// harness config (ADR 0003); only the kind string reaches the
    /// zainod TOML. Until zingolabs/zaino#1076 ships a zainod that
    /// queries the validator, the binary falls back to its compiled-in
    /// regtest heights and mismatched schedules kill its sync loop
    /// with `InvalidData("Block commitment could not be computed")`.
    pub network: NetworkKind,
}

impl Default for ZainodConfig {
    fn default() -> Self {
        ZainodConfig {
            listen_port: None,
            validator_port: 0,
            chain_cache: None,
            network: NetworkKind::Regtest,
        }
    }
}

impl IndexerConfig for ZainodConfig {
    fn setup_validator_connection<V: crate::validator::Validator>(&mut self, validator: &V) {
        self.validator_port = validator.get_port();
    }

    fn set_listen_port(&mut self, indexer_listen_port: Option<u16>) {
        self.listen_port = indexer_listen_port;
    }
}

/// This struct is used to represent and manage the Zainod process.
#[derive(Debug)]
pub struct Zainod {
    /// Child process handle
    handle: Child,
    /// RPC port
    port: u16,
    /// Logs directory
    logs_dir: TempDir,
    /// Config directory
    config_dir: TempDir,
}

crate::macros::ref_getters!(Zainod {
    /// Child process handle.
    handle: Child,
    /// Config directory.
    config_dir: TempDir,
});

crate::macros::copy_getters!(Zainod {
    /// RPC port.
    port: u16,
});

impl LogsToDir for Zainod {
    fn logs_dir(&self) -> &TempDir {
        &self.logs_dir
    }
}

impl launch::PortPins for ZainodConfig {
    fn pinned_ports(&self) -> Vec<u16> {
        self.listen_port.into_iter().collect()
    }

    fn clear_port_pins(&mut self) {
        // Single-port indexer — clear the only pin so the next
        // attempt's pick calls `network::pick_unused_port(None)` and
        // the kernel hands back a fresh ephemeral.
        self.listen_port = None;
    }
}

impl Zainod {
    /// Single launch attempt: pick a port, write the config, spawn
    /// zainod, wait for the readiness indicator. Wrapped by
    /// `Process::launch` in a bounded retry-on-port-collision loop
    /// (see `launch::with_retry_on_collision`); each retry calls this
    /// fresh with a config whose port pin has been cleared so the pick
    /// re-rolls via `network::pick_unused_port`.
    async fn launch_once(config: ZainodConfig) -> Result<Self, LaunchError> {
        let logs_dir = tempfile::tempdir().unwrap();
        let data_dir = tempfile::tempdir().unwrap();

        let port = network::pick_unused_port(config.listen_port);
        let config_dir = tempfile::tempdir().unwrap();

        let cache_dir = if let Some(cache) = config.chain_cache.clone() {
            cache
        } else {
            data_dir.path().to_path_buf()
        };

        let config_file_path = config::write_zainod_config(
            config_dir.path(),
            cache_dir,
            port,
            config.validator_port,
            config.network,
        )
        .unwrap();

        let executable_name = "zainod";
        trace_version_and_location(executable_name, "--version");
        let mut command = pick_command(executable_name, false);
        command.args([
            "start",
            "--config",
            config_file_path.to_str().expect("should be valid UTF-8"),
        ]);

        let mut handle = launch::spawn_and_wait(
            ProcessId::Zainod,
            &mut command,
            &logs_dir,
            None,
            &["Zaino Indexer started successfully."],
            &["Error:"],
            &[],
        )
        .await?;

        // Verify the gRPC listener is actually accepting connections.
        // Closes failure mode #4: if Zaino logs "started successfully"
        // before completing the gRPC bind, AddrInUse on a squatted
        // port would otherwise let `launch::wait` return Ok with the
        // picked port stored on a defunct child. The probe's phase-1
        // `try_wait` polling catches the child crashing on AddrInUse
        // before its phase-2 TCP probe is fooled by the squatter's
        // accept queue.
        launch::probe_listener(ProcessId::Zainod, &mut handle, port, &logs_dir, None).await?;

        Ok(Zainod {
            handle,
            port,
            logs_dir,
            config_dir,
        })
    }
}

impl Process for Zainod {
    const PROCESS: ProcessId = ProcessId::Zainod;

    type Config = ZainodConfig;

    async fn launch(config: Self::Config) -> Result<Self, LaunchError> {
        // Zaino's gRPC server (tonic/tower) surfaces AddrInUse via
        // libc-shaped strings. The indexer regression test in
        // `tests/integration.rs` is the source of truth for which
        // strings the build emits today; if a future Zaino change
        // produces something else, the test trips UNEXPECTED FAILURE
        // MODE before silently classifying the failure here.
        const COLLISION_SIGNATURES: &[&str] = &[
            "address already in use",
            "Address already in use",
            "AddrInUse",
        ];

        launch::with_retry_on_collision(
            "zainod",
            config,
            COLLISION_SIGNATURES,
            launch::MAX_LAUNCH_ATTEMPTS,
            Self::launch_once,
        )
        .await
    }

    fn stop(&mut self) {
        self.handle.kill().expect("zainod couldn't be killed");
    }

    fn print_all(&self) {
        self.print_stdout();
        self.print_stderr();
    }
}

impl Indexer for Zainod {
    fn listen_port(&self) -> u16 {
        self.port
    }
}

crate::macros::impl_stop_on_drop!(Zainod);
