//! common behavior to processes

use std::path::PathBuf;

use tempfile::TempDir;

use crate::{error::LaunchError, logs, Process};

/// yaeh
pub trait ItsAProcess: Sized {
    /// Config filename
    const CONFIG_FILENAME: &str;

    /// Process
    const PROCESS: Process;

    /// Indexer config struct
    type Config: Default;

    /// Launch the process.
    fn launch(config: Self::Config) -> Result<Self, LaunchError>;

    /// Stop the process.
    fn stop(&mut self);

    /// Get temporary config directory.
    fn config_dir(&self) -> &TempDir;

    /// Get temporary logs directory.
    fn logs_dir(&self) -> &TempDir;

    /// Returns path to config file.
    fn config_path(&self) -> PathBuf {
        self.config_dir().path().join(Self::CONFIG_FILENAME)
    }

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

    /// Returns the indexer process.
    fn process(&self) -> Process {
        Self::PROCESS
    }
}
