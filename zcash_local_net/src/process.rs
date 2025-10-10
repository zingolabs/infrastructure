//! common behavior to processes

use std::future::Future;

use crate::{error::LaunchError, Process};

/// Processes share some behavior.
pub trait IsAProcess: Sized {
    /// Process
    const PROCESS: Process;

    /// Indexer config struct
    type Config: Default;

    /// Launch the process.
    fn launch(config: Self::Config) -> impl Future<Output = Result<Self, LaunchError>> + Send;

    /// Stop the process.
    fn stop(&mut self);

    /// To print all the things.
    fn print_all(&self);

    /// Returns the indexer process.
    fn process(&self) -> Process {
        Self::PROCESS
    }

    /// To launch with untouched default config.
    #[must_use]
    fn launch_default() -> impl Future<Output = Result<Self, LaunchError>> + Send {
        Self::launch(Self::Config::default())
    }
}
