use std::{fs::File, path::PathBuf, process::Child};

use getset::{CopyGetters, Getters};
use portpicker::Port;
use tempfile::TempDir;

use crate::{
    config,
    error::LaunchError,
    indexer::{Indexer, IndexerConfig},
    launch,
    logs::{self, LogsToDir, LogsToStdoutAndStderr as _},
    network::{self},
    process::Process,
    utils::executable_finder::{pick_command, EXPECT_SPAWN},
    ProcessId,
};

/// Lightwalletd configuration
///
/// If `listen_port` is `None`, a port is picked at random between 15000-25000.
///
/// The `zcash_conf` path must be specified and the validator process must be running before launching Lightwalletd.
/// When running a validator that is not Zcashd (i.e. Zebrad), a zcash config file must still be created to specify the
/// validator port. This is automatically handled by [`crate::LocalNet::launch`] when using [`crate::LocalNet`].
#[derive(Debug)]
pub struct LightwalletdConfig {
    /// Listen RPC port
    pub listen_port: Option<Port>,
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

    fn set_listen_port(&mut self, indexer_listen_port: Option<Port>) {
        self.listen_port = indexer_listen_port;
    }
}
/// This struct is used to represent and manage the Lightwalletd process.
#[derive(Debug, Getters, CopyGetters)]
#[getset(get = "pub")]
pub struct Lightwalletd {
    /// Child process handle
    handle: Child,
    /// RPC Port
    #[getset(skip)]
    #[getset(get_copy = "pub")]
    port: Port,
    /// Data directory
    _data_dir: TempDir,
    /// Logs directory
    logs_dir: TempDir,
    /// Config directory
    config_dir: TempDir,
}

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

impl Process for Lightwalletd {
    const PROCESS: ProcessId = ProcessId::Lightwalletd;

    type Config = LightwalletdConfig;

    async fn launch(config: Self::Config) -> Result<Self, LaunchError> {
        let logs_dir = tempfile::tempdir().unwrap();
        let lwd_log_file_path = logs_dir.path().join(logs::LIGHTWALLETD_LOG);
        let _lwd_log_file = File::create(&lwd_log_file_path).unwrap();

        let data_dir = tempfile::tempdir().unwrap();

        let port = network::pick_unused_port(config.listen_port);
        let config_dir = tempfile::tempdir().unwrap();
        let config_file_path = config::lightwalletd(
            config_dir.path(),
            port,
            lwd_log_file_path.clone(),
            config.zcashd_conf.clone(),
        )
        .unwrap();

        let mut command = pick_command("lightwalletd");
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

        command
            .args(args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut handle = command.spawn().expect(EXPECT_SPAWN);
        logs::write_logs(&mut handle, &logs_dir);
        launch::wait(
            ProcessId::Lightwalletd,
            &mut handle,
            &logs_dir,
            Some(lwd_log_file_path),
            &["Starting insecure no-TLS (plaintext) server"],
            &["error"],
            &[],
        )?;

        Ok(Lightwalletd {
            handle,
            port,
            _data_dir: data_dir,
            logs_dir,
            config_dir,
        })
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
    fn listen_port(&self) -> Port {
        self.port
    }
}

impl Drop for Lightwalletd {
    fn drop(&mut self) {
        self.stop();
    }
}
