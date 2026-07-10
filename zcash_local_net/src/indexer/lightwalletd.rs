use std::{fs::File, path::PathBuf, process::Child};

use tempfile::TempDir;

use crate::{
    ProcessId, config,
    error::LaunchError,
    indexer::{Indexer, IndexerConfig},
    launch,
    logs::{self, LogsToDir, LogsToStdoutAndStderr as _},
    network::{self},
    process::Process,
    utils::executable_finder::{pick_command, trace_version_and_location},
};

/// Lightwalletd configuration
///
/// If `listen_port` is `None`, a port is picked at random between 15000-25000.
///
/// The `zcash_conf` path must be specified and the validator process must be running before launching Lightwalletd.
/// When running a validator that is not Zcashd (i.e. Zebrad), a zcash config file must still be created to specify the
/// validator port. This is automatically handled by [`crate::LocalNet::launch`] when using [`crate::LocalNet`].
#[derive(Clone, Debug)]
pub struct LightwalletdConfig {
    /// Listen RPC port
    pub listen_port: Option<u16>,
    /// Zcashd configuration file location. Required even when running non-Zcashd validators.
    pub zcashd_conf: PathBuf,
    /// Enables darkside
    pub darkside: bool,
}

impl Default for LightwalletdConfig {
    fn default() -> Self {
        LightwalletdConfig {
            listen_port: None,
            zcashd_conf: PathBuf::new(),
            darkside: false,
        }
    }
}

impl IndexerConfig for LightwalletdConfig {
    fn setup_validator_connection<V: crate::validator::Validator>(&mut self, validator: &V) {
        self.zcashd_conf = validator.get_zcashd_conf_path();
    }

    fn set_listen_port(&mut self, indexer_listen_port: Option<u16>) {
        self.listen_port = indexer_listen_port;
    }
}
/// This struct is used to represent and manage the Lightwalletd process.
#[derive(Debug)]
pub struct Lightwalletd {
    /// Child process handle
    handle: Child,
    /// RPC Port
    port: u16,
    /// Data directory
    _data_dir: TempDir,
    /// Logs directory
    logs_dir: TempDir,
    /// Config directory
    config_dir: TempDir,
}

crate::macros::ref_getters!(Lightwalletd {
    /// Child process handle.
    handle: Child,
    /// Config directory.
    config_dir: TempDir,
});

crate::macros::copy_getters!(Lightwalletd {
    /// RPC port.
    port: u16,
});

impl Lightwalletd {
    /// Prints the stdout log.
    pub fn print_lwd_log(&self) {
        let stdout_log_path = self.logs_dir.path().join(logs::LIGHTWALLETD_LOG);
        logs::print_log(stdout_log_path);
    }
}

impl LogsToDir for Lightwalletd {
    fn logs_dir(&self) -> &TempDir {
        &self.logs_dir
    }
}

impl launch::PortPins for LightwalletdConfig {
    fn pinned_ports(&self) -> Vec<u16> {
        self.listen_port.into_iter().collect()
    }

    fn clear_port_pins(&mut self) {
        // Single-port indexer — clear the only pin so the next
        // attempt's pick calls `network::pick_unused_port(None)` and
        // the allocator walks to a fresh candidate.
        self.listen_port = None;
    }
}

impl Lightwalletd {
    /// Single launch attempt: pick a port, write the config, spawn
    /// lightwalletd, wait for the readiness indicator. Wrapped by
    /// `Process::launch` in a bounded retry-on-port-collision loop
    /// (see `launch::with_retry_on_collision`); each retry calls this
    /// fresh with a config whose port pin has been cleared so the pick
    /// re-rolls via `network::pick_unused_port`.
    async fn launch_once(config: LightwalletdConfig) -> Result<Self, LaunchError> {
        let logs_dir = tempfile::tempdir().unwrap();
        let lwd_log_file_path = logs_dir.path().join(logs::LIGHTWALLETD_LOG);
        let _lwd_log_file = File::create(&lwd_log_file_path).unwrap();

        let data_dir = tempfile::tempdir().unwrap();

        let port = network::pick_unused_port(config.listen_port);
        let config_dir = tempfile::tempdir().unwrap();
        let config_file_path = config::write_lightwalletd_config(
            config_dir.path(),
            port,
            lwd_log_file_path.clone(),
            config.zcashd_conf.clone(),
        )
        .unwrap();

        let lightwalletd_executable_name = "lightwalletd";
        trace_version_and_location(lightwalletd_executable_name, "version");
        let mut command = pick_command(lightwalletd_executable_name, false);
        let mut args = vec![
            "--no-tls-very-insecure",
            "--data-dir",
            data_dir.path().to_str().unwrap(),
            "--log-file",
            lwd_log_file_path.to_str().unwrap(),
            "--zcash-conf-path",
            config.zcashd_conf.to_str().unwrap(),
            "--config",
            config_file_path.to_str().unwrap(),
        ];
        if config.darkside {
            args.push("--darkside-very-insecure");
        }

        command.args(args);

        let mut handle = launch::spawn_and_wait(
            ProcessId::Lightwalletd,
            &mut command,
            &logs_dir,
            Some(lwd_log_file_path.clone()),
            &["Starting insecure no-TLS (plaintext) server"],
            &["error"],
            &[],
        )
        .await?;

        // Verify the gRPC listener is actually accepting connections.
        // Closes failure mode #4 for lightwalletd: the
        // "Starting ... server" indicator may fire before the bind is
        // complete; the probe's phase-1 `try_wait` polling catches the
        // child crashing on AddrInUse before its phase-2 TCP probe is
        // fooled by another listener (squatter or sibling test
        // subprocess) on the same port.
        launch::probe_listener(
            ProcessId::Lightwalletd,
            &mut handle,
            port,
            &logs_dir,
            Some(&lwd_log_file_path),
        )
        .await?;

        Ok(Lightwalletd {
            handle,
            port,
            _data_dir: data_dir,
            logs_dir,
            config_dir,
        })
    }
}

impl Process for Lightwalletd {
    const PROCESS: ProcessId = ProcessId::Lightwalletd;

    type Config = LightwalletdConfig;

    async fn launch(config: Self::Config) -> Result<Self, LaunchError> {
        // lightwalletd is Go; `net.Listen` surfaces `EADDRINUSE` as
        // `"bind: address already in use"`. The substring
        // `"address already in use"` matches that and any libc-shaped
        // variant a future build might emit.
        const COLLISION_SIGNATURES: &[&str] = &["address already in use", "bind:"];

        launch::with_retry_on_collision(
            "lightwalletd",
            config,
            COLLISION_SIGNATURES,
            launch::MAX_LAUNCH_ATTEMPTS,
            Self::launch_once,
        )
        .await
    }

    fn stop(&mut self) {
        self.handle.kill().expect("lightwalletd couldn't be killed");
    }

    /// To print ALLL the things.
    fn print_all(&self) {
        self.print_stdout();
        self.print_lwd_log();
        self.print_stderr();
    }
}

impl Indexer for Lightwalletd {
    fn listen_port(&self) -> u16 {
        self.port
    }
}

crate::macros::impl_stop_on_drop!(Lightwalletd);
