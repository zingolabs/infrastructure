use std::{fs::File, io::Read, path::PathBuf, process::Child};

use tempfile::TempDir;

pub mod launch;
pub mod network;

const ZCASHD_STDOUT_LOG: &str = "stdout.log";

pub struct Zcashd {
    handle: Child,
    _data_dir: TempDir,
    logs_dir: TempDir,
    config_dir: TempDir,
    zcash_cli_bin: Option<PathBuf>,
}

impl Zcashd {
    pub fn config_path(&self) -> PathBuf {
        self.config_dir.path().join(launch::config::ZCASHD_FILENAME)
    }

    pub fn zcash_cli_command(&self, args: &[&str]) -> std::io::Result<std::process::Output> {
        let mut command = match &self.zcash_cli_bin {
            Some(path) => std::process::Command::new(path),
            None => std::process::Command::new("zcash-cli"),
        };

        command.arg(format!("-conf={}", self.config_path().to_str().unwrap()));
        command.args(args).output()
    }

    pub fn stop(mut self) {
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

    pub fn generate_blocks(&self, num_blocks: u32) -> std::io::Result<std::process::Output> {
        self.zcash_cli_command(&["generate", &num_blocks.to_string()])
    }

    pub fn print_stdout(&self) {
        let stdout_log_path = self.logs_dir.path().join(ZCASHD_STDOUT_LOG);
        let mut stdout_log = File::open(stdout_log_path).expect("should be able to open log");
        let mut stdout = String::new();
        stdout_log.read_to_string(&mut stdout).unwrap();
        println!("{}", stdout);
    }
}
