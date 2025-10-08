//! Module for the structs that represent and manage the indexer processes i.e. Zainod.
//!
//! Processes which are not strictly indexers but have a similar role in serving light-clients/light-wallets
//! (i.e. Lightwalletd) are also included in this category and are referred to as "light-nodes".

use std::{fs::File, path::PathBuf, process::Child};

use getset::{CopyGetters, Getters};
use portpicker::Port;
use tempfile::TempDir;

use zebra_chain::parameters::NetworkKind;

use crate::{
    config,
    error::LaunchError,
    launch, logs,
    network::{self},
    process::ItsAProcess,
    utils::executable_finder::{pick_command, EXPECT_SPAWN},
    Process,
};

/// Zainod configuration
///
/// If `listen_port` is `None`, a port is picked at random between 15000-25000.
///
/// The `validator_port` must be specified and the validator process must be running before launching Zainod.
///
/// `network` must match the configured network of the validator.
pub struct ZainodConfig {
    /// Listen RPC port
    pub listen_port: Option<Port>,
    /// Validator RPC port
    pub validator_port: Port,
    /// Chain cache path
    pub chain_cache: Option<PathBuf>,
    /// Network type.
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
/// Lightwalletd configuration
///
/// If `listen_port` is `None`, a port is picked at random between 15000-25000.
///
/// The `zcash_conf` path must be specified and the validator process must be running before launching Lightwalletd.
/// When running a validator that is not Zcashd (i.e. Zebrad), a zcash config file must still be created to specify the
/// validator port. This is automatically handled by [`crate::LocalNet::launch`] when using [`crate::LocalNet`].
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
/// Empty configuration
///
/// For use when not launching an Indexer with [`crate::LocalNet::launch`].
pub struct EmptyConfig {}

/// Functionality for indexer/light-node processes.
pub trait Indexer: ItsAProcess {
    /// Indexer listen port
    fn listen_port(&self) -> Port;
}

/// This struct is used to represent and manage the Zainod process.
#[derive(Getters, CopyGetters)]
#[getset(get = "pub")]
pub struct Zainod {
    /// Child process handle
    handle: Child,
    /// RPC port
    #[getset(skip)]
    #[getset(get_copy = "pub")]
    port: Port,
    /// Logs directory
    logs_dir: TempDir,
    /// Config directory
    config_dir: TempDir,
}

impl ItsAProcess for Zainod {
    const CONFIG_FILENAME: &str = config::ZAINOD_FILENAME;
    const PROCESS: Process = Process::Zainod;

    type Config = ZainodConfig;

    fn launch(config: Self::Config) -> Result<Self, LaunchError> {
        let logs_dir = tempfile::tempdir().unwrap();
        let data_dir = tempfile::tempdir().unwrap();

        let port = network::pick_unused_port(config.listen_port);
        let config_dir = tempfile::tempdir().unwrap();

        let cache_dir = if let Some(cache) = config.chain_cache.clone() {
            cache
        } else {
            data_dir.path().to_path_buf()
        };

        let config_file_path = config::zainod(
            config_dir.path(),
            cache_dir,
            port,
            config.validator_port,
            config.network,
        )
        .unwrap();

        let mut command = pick_command("zainod");
        command
            .args([
                "--config",
                config_file_path.to_str().expect("should be valid UTF-8"),
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut handle = command.spawn().expect(EXPECT_SPAWN);
        logs::write_logs(&mut handle, &logs_dir);
        launch::wait(
            Process::Zainod,
            &mut handle,
            &logs_dir,
            None,
            &["Zaino Indexer started successfully."],
            &["Error:"],
            &[],
        )?;

        Ok(Zainod {
            handle,
            port,
            logs_dir,
            config_dir,
        })
    }

    fn stop(&mut self) {
        self.handle.kill().expect("zainod couldn't be killed")
    }

    fn config_dir(&self) -> &TempDir {
        &self.config_dir
    }

    fn logs_dir(&self) -> &TempDir {
        &self.logs_dir
    }
}

impl Indexer for Zainod {
    fn listen_port(&self) -> Port {
        self.port
    }
}

impl Drop for Zainod {
    fn drop(&mut self) {
        self.stop();
    }
}

/// This struct is used to represent and manage the Lightwalletd process.
#[derive(Getters, CopyGetters)]
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

impl ItsAProcess for Lightwalletd {
    const CONFIG_FILENAME: &str = config::LIGHTWALLETD_FILENAME;
    const PROCESS: Process = Process::Lightwalletd;

    type Config = LightwalletdConfig;

    fn launch(config: Self::Config) -> Result<Self, LaunchError> {
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
            Process::Lightwalletd,
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
        self.handle.kill().expect("lightwalletd couldn't be killed")
    }

    fn config_dir(&self) -> &TempDir {
        &self.config_dir
    }

    fn logs_dir(&self) -> &TempDir {
        &self.logs_dir
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

/// This struct is used to represent and manage an empty Indexer process.
///
/// Dirs are created for integration.
#[derive(Getters, CopyGetters)]
#[getset(get = "pub")]
pub struct Empty {
    /// Logs directory
    logs_dir: TempDir,
    /// Config directory
    config_dir: TempDir,
}

impl ItsAProcess for Empty {
    const CONFIG_FILENAME: &str = "";
    const PROCESS: Process = Process::Empty;

    type Config = EmptyConfig;

    fn listen_port(&self) -> Port {
        0
    }

    /// Generate a default test config
    fn default_test_config() -> Self::Config {
        EmptyConfig {}
    }

    fn launch(_config: Self::Config) -> Result<Self, LaunchError> {
        let logs_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();

        Ok(Empty {
            logs_dir,
            config_dir,
        })
    }

    fn stop(&mut self) {}

    fn config_dir(&self) -> &TempDir {
        &self.config_dir
    }

    fn logs_dir(&self) -> &TempDir {
        &self.logs_dir
    }
}

impl Drop for Empty {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Indexer for Empty {
    fn listen_port(&self) -> Port {
        0
    }
}
