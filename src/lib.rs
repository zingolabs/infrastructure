#![warn(missing_docs)]
//! Zcash Localnet

use std::{fs::File, io::Read, path::PathBuf, process::Child};

use error::LaunchError;
use getset::{CopyGetters, Getters};
use network::ActivationHeights;
use portpicker::Port;
use tempfile::TempDir;

pub(crate) mod config;
pub mod error;
pub mod network;

const STDOUT_LOG: &str = "stdout.log";
const STDERR_LOG: &str = "stderr.log";
pub(crate) const LIGHTWALLETD_LOG: &str = "lwd.log";

#[derive(Clone, Copy)]
enum Process {
    Zcashd,
    Zainod,
    Lightwalletd,
}

impl std::fmt::Display for Process {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let process = match self {
            Self::Zcashd => "zcashd",
            Self::Zainod => "zainod",
            Self::Lightwalletd => "lightwalletd",
        };
        write!(f, "{}", process)
    }
}

fn print_log(log_path: PathBuf) {
    let mut log_file = File::open(log_path).unwrap();
    let mut log = String::new();
    log_file.read_to_string(&mut log).unwrap();
    println!("{}", log);
}

fn write_logs(handle: &mut Child, logs_dir: &TempDir) {
    let stdout_log_path = logs_dir.path().join(STDOUT_LOG);
    let mut stdout_log = File::create(&stdout_log_path).unwrap();
    let mut stdout = handle.stdout.take().unwrap();
    std::thread::spawn(move || std::io::copy(&mut stdout, &mut stdout_log).unwrap());

    let stderr_log_path = logs_dir.path().join(STDERR_LOG);
    let mut stderr_log = File::create(&stderr_log_path).unwrap();
    let mut stderr = handle.stderr.take().unwrap();
    std::thread::spawn(move || std::io::copy(&mut stderr, &mut stderr_log).unwrap());
}

fn wait_for_launch(
    process: Process,
    handle: &mut Child,
    logs_dir: &TempDir,
    additional_log_path: Option<PathBuf>,
    success_indicator: &str,
    error_indicator: &str,
) -> Result<(), LaunchError> {
    let stdout_log_path = logs_dir.path().join(STDOUT_LOG);
    let mut stdout_log = File::open(stdout_log_path).expect("should be able to open log");
    let mut stdout = String::new();

    let stderr_log_path = logs_dir.path().join(STDERR_LOG);
    let mut stderr_log = File::open(stderr_log_path).expect("should be able to open log");
    let mut stderr = String::new();

    let (mut additional_log_file, mut additional_log) = if let Some(log_path) = additional_log_path
    {
        let log_file = File::open(log_path).expect("should be able to open log");
        let log = String::new();

        (Some(log_file), Some(log))
    } else {
        (None, None)
    };

    // wait for stdout log entry that indicates daemon is ready
    let interval = std::time::Duration::from_millis(100);
    loop {
        match handle.try_wait() {
            Ok(Some(exit_status)) => {
                stdout_log.read_to_string(&mut stdout).unwrap();
                stderr_log.read_to_string(&mut stderr).unwrap();

                return Err(LaunchError::ProcessFailed {
                    process_name: process.to_string(),
                    exit_status,
                    stdout,
                    stderr,
                });
            }
            Ok(None) => (),
            Err(e) => {
                panic!("Unexpected Error: {e}")
            }
        };

        stdout_log.read_to_string(&mut stdout).unwrap();
        stderr_log.read_to_string(&mut stderr).unwrap();
        if stdout.contains(error_indicator) || stderr.contains(error_indicator) {
            panic!("{} launch failed without reporting an error code!\nexiting with panic. you may have to shut the daemon down manually.", process);
        } else if stdout.contains(success_indicator) {
            // launch successful
            break;
        }

        if additional_log_file.is_some() {
            let mut log_file = additional_log_file
                .take()
                .expect("additional log exists in this scope");
            let mut log = additional_log
                .take()
                .expect("additional log exists in this scope");

            log_file.read_to_string(&mut log).unwrap();
            if log.contains(success_indicator) {
                // launch successful
                break;
            } else {
                additional_log_file = Some(log_file);
                additional_log = Some(log);
            }
        }

        std::thread::sleep(interval);
    }

    Ok(())
}

/// This struct is used to represent and manage the Zcashd process.
#[derive(Getters, CopyGetters)]
#[getset(get = "pub")]
pub struct Zcashd {
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
    /// Path to zcash cli binary
    zcash_cli_bin: Option<PathBuf>,
}

impl Zcashd {
    /// Launches Zcashd process and returns [`crate::Zcashd`] with the handle and associated directories.
    ///
    /// Use `zcashd_bin` and `zcash_cli_bin` to specify the paths to the binaries.
    /// If these binaries are in $PATH, `None` can be specified to run "zcashd" / "zcash-cli".
    ///
    /// Use `fixed_port` to specify a port for Zcashd. Otherwise, a port is picked at random.
    ///
    /// Use `activation_heights` to specify custom network upgrade activation heights
    ///
    /// Use `miner_address` to specify the target address for the block rewards when blocks are generated.  
    pub fn launch(
        zcashd_bin: Option<PathBuf>,
        zcash_cli_bin: Option<PathBuf>,
        rpc_port: Option<Port>,
        activation_heights: &ActivationHeights,
        miner_address: Option<&str>,
    ) -> Result<Zcashd, LaunchError> {
        let data_dir = tempfile::tempdir().unwrap();
        let logs_dir = tempfile::tempdir().unwrap();

        let port = network::pick_unused_port(rpc_port);
        let config_dir = tempfile::tempdir().unwrap();
        let config_file_path =
            config::zcashd(config_dir.path(), port, activation_heights, miner_address).unwrap();

        let mut command = match zcashd_bin {
            Some(path) => std::process::Command::new(path),
            None => std::process::Command::new("zcashd"),
        };
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

        let mut handle = command.spawn().unwrap();

        write_logs(&mut handle, &logs_dir);
        wait_for_launch(
            Process::Zcashd,
            &mut handle,
            &logs_dir,
            None,
            "init message: Done loading",
            "Error:",
        )?;

        let zcashd = Zcashd {
            handle,
            port,
            _data_dir: data_dir,
            logs_dir,
            config_dir,
            zcash_cli_bin,
        };

        // generate genesis block
        zcashd.generate_blocks(1).unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));

        Ok(zcashd)
    }

    /// Returns path to config file.
    pub fn config_path(&self) -> PathBuf {
        self.config_dir.path().join(config::ZCASHD_FILENAME)
    }

    /// Runs a Zcash-cli command with the given `args`.
    ///
    /// Example usage for generating blocks in Zcashd local net:
    /// ```ignore (incomplete)
    /// self.zcash_cli_command(&["generate", "1"]);
    /// ```
    pub fn zcash_cli_command(&self, args: &[&str]) -> std::io::Result<std::process::Output> {
        let mut command = match &self.zcash_cli_bin {
            Some(path) => std::process::Command::new(path),
            None => std::process::Command::new("zcash-cli"),
        };

        command.arg(format!("-conf={}", self.config_path().to_str().unwrap()));
        command.args(args).output()
    }

    /// Stops the Zcashd process.
    pub fn stop(&mut self) {
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

    /// Generate `num_blocks` blocks.
    pub fn generate_blocks(&self, num_blocks: u32) -> std::io::Result<std::process::Output> {
        self.zcash_cli_command(&["generate", &num_blocks.to_string()])
    }

    /// Prints the stdout log.
    pub fn print_stdout(&self) {
        let stdout_log_path = self.logs_dir.path().join(STDOUT_LOG);
        print_log(stdout_log_path);
    }

    /// Prints the stdout log.
    pub fn print_stderr(&self) {
        let stdout_log_path = self.logs_dir.path().join(STDERR_LOG);
        print_log(stdout_log_path);
    }
}

impl Default for Zcashd {
    /// Default launch for Zcashd.
    /// Panics on failure.
    fn default() -> Self {
        Zcashd::launch(None, None, None, &ActivationHeights::default(), None).unwrap()
    }
}

impl Drop for Zcashd {
    fn drop(&mut self) {
        self.stop();
    }
}

/// This struct is used to represent and manage the Zainod process.
#[derive(Getters, CopyGetters)]
#[getset(get = "pub")]
pub struct Zainod {
    /// Child process handle
    handle: Child,
    /// RPC Port
    #[getset(skip)]
    #[getset(get_copy = "pub")]
    port: Port,
    /// Logs directory
    logs_dir: TempDir,
    /// Config directory
    config_dir: TempDir,
}

impl Zainod {
    /// Launches Zainod process and returns [`crate::Zainod`] with the handle and associated directories.
    ///
    /// Use `fixed_port` to specify a port for Zainod. Otherwise, a port is picked at random.
    ///
    /// The `validator_port` must be specified and the validator process must be running before launching Zainod.
    pub fn launch(
        zainod_bin: Option<PathBuf>,
        listen_port: Option<Port>,
        validator_port: Port,
    ) -> Result<Zainod, LaunchError> {
        let logs_dir = tempfile::tempdir().unwrap();

        let port = network::pick_unused_port(listen_port);
        let config_dir = tempfile::tempdir().unwrap();
        let config_file_path = config::zainod(config_dir.path(), port, validator_port).unwrap();

        let mut command = match zainod_bin {
            Some(path) => std::process::Command::new(path),
            None => std::process::Command::new("zainod"),
        };
        command
            .args([
                "--config",
                config_file_path.to_str().expect("should be valid UTF-8"),
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut handle = command.spawn().unwrap();

        write_logs(&mut handle, &logs_dir);
        wait_for_launch(
            Process::Zainod,
            &mut handle,
            &logs_dir,
            None,
            "Server Ready.",
            "Error:",
        )?;

        Ok(Zainod {
            handle,
            port,
            logs_dir,
            config_dir,
        })
    }

    /// Returns path to config file.
    pub fn config_path(&self) -> PathBuf {
        self.config_dir.path().join(config::ZAINOD_FILENAME)
    }

    /// Stops the Zcashd process.
    pub fn stop(&mut self) {
        self.handle.kill().expect("zainod couldn't be killed")
    }

    /// Prints the stdout log.
    pub fn print_stdout(&self) {
        let stdout_log_path = self.logs_dir.path().join(STDOUT_LOG);
        print_log(stdout_log_path);
    }

    /// Prints the stdout log.
    pub fn print_stderr(&self) {
        let stdout_log_path = self.logs_dir.path().join(STDERR_LOG);
        print_log(stdout_log_path);
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
    /// Launches Lightwalletd process and returns [`crate::Lightwalletd`] with the handle and associated directories.
    ///
    /// Use `fixed_port` to specify a port for Lightwalletd. Otherwise, a port is picked at random.
    ///
    /// The `validator_port` must be specified and the validator process must be running before launching Lightwalletd.
    pub fn launch(
        lightwalletd_bin: Option<PathBuf>,
        listen_port: Option<Port>,
        validator_conf: PathBuf,
    ) -> Result<Lightwalletd, LaunchError> {
        let logs_dir = tempfile::tempdir().unwrap();
        let lwd_log_file_path = logs_dir.path().join(LIGHTWALLETD_LOG);
        let _lwd_log_file = File::create(&lwd_log_file_path).unwrap();

        let data_dir = tempfile::tempdir().unwrap();

        let port = network::pick_unused_port(listen_port);
        let config_dir = tempfile::tempdir().unwrap();
        let config_file_path = config::lightwalletd(
            config_dir.path(),
            port,
            lwd_log_file_path.clone(),
            validator_conf.clone(),
        )
        .unwrap();

        let mut command = match lightwalletd_bin {
            Some(path) => std::process::Command::new(path),
            None => std::process::Command::new("lightwalletd"),
        };
        command
            .args([
                "--no-tls-very-insecure",
                "--data-dir",
                data_dir.path().to_str().unwrap(),
                "--log-file",
                lwd_log_file_path.to_str().unwrap(),
                "--zcash-conf-path",
                validator_conf.to_str().unwrap(),
                "--config",
                config_file_path.to_str().unwrap(),
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut handle = command.spawn().unwrap();

        write_logs(&mut handle, &logs_dir);
        wait_for_launch(
            Process::Lightwalletd,
            &mut handle,
            &logs_dir,
            Some(lwd_log_file_path),
            "Starting insecure no-TLS (plaintext) server",
            "Error:",
        )?;

        Ok(Lightwalletd {
            handle,
            port,
            _data_dir: data_dir,
            logs_dir,
            config_dir,
        })
    }

    /// Returns path to config file.
    pub fn config_path(&self) -> PathBuf {
        self.config_dir.path().join(config::LIGHTWALLETD_FILENAME)
    }

    /// Stops the Zcashd process.
    pub fn stop(&mut self) {
        self.handle.kill().expect("lightwalletd couldn't be killed")
    }

    /// Prints the stdout log.
    pub fn print_stdout(&self) {
        let stdout_log_path = self.logs_dir.path().join(STDOUT_LOG);
        print_log(stdout_log_path);
    }

    /// Prints the stdout log.
    pub fn print_stderr(&self) {
        let stdout_log_path = self.logs_dir.path().join(STDERR_LOG);
        print_log(stdout_log_path);
    }

    /// Prints the stdout log.
    pub fn print_lwd_log(&self) {
        let stdout_log_path = self.logs_dir.path().join(LIGHTWALLETD_LOG);
        print_log(stdout_log_path);
    }
}

impl Drop for Lightwalletd {
    fn drop(&mut self) {
        self.stop();
    }
}
