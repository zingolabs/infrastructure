//! common behavior to processes

use std::future::Future;

use crate::{error::LaunchError, ProcessId};

/// Processes share some behavior.
pub trait Process: Sized {
    /// Process
    const PROCESS: ProcessId;

    /// A config struct for the process.
    type Config: Default + std::fmt::Debug;

    /// Launch the process.
    fn launch(config: Self::Config) -> impl Future<Output = Result<Self, LaunchError>> + Send;

    /// Stop the process.
    fn stop(&mut self);

    /// To print outputs from the process.
    fn print_all(&self);

    /// Returns the indexer process id.
    fn process(&self) -> ProcessId {
        Self::PROCESS
    }

    /// To launch with untouched default config.
    fn launch_default() -> impl Future<Output = Result<Self, LaunchError>> + Send {
        Self::launch(Self::Config::default())
    }
}
