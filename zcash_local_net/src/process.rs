//! common behavior to processes

use std::{future::Future, path::PathBuf};

use tempfile::TempDir;

use crate::{error::LaunchError, logs, Process};

/// yaeh
pub trait ItsAProcess: Sized {
    /// Process
    const PROCESS: Process;

    /// Indexer config struct
    type Config: Default;

    /// Launch the process.
    fn launch(config: Self::Config) -> impl Future<Output = Result<Self, LaunchError>> + Send;

    /// Stop the process.
    fn stop(&mut self);

    /// Get temporary logs directory.
    fn logs_dir(&self) -> &TempDir;

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

    /// To print all the things.
    fn print_all(&self) {
        self.print_stdout();
        self.print_stderr();
    }

    /// Returns the indexer process.
    fn process(&self) -> Process {
        Self::PROCESS
    }

    /// To launch with untouched default config.
    fn launch_default() -> impl Future<Output = Result<Self, LaunchError>> + Send {
        Self::launch(Self::Config::default())
    }
}
