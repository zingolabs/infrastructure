use getset::{CopyGetters, Getters};
use portpicker::Port;
use tempfile::TempDir;

use crate::{
    error::LaunchError,
    indexer::{Indexer, IndexerConfig},
    logs::LogsToStdoutAndStderr,
    process::Process,
    ProcessId,
};

/// Empty configuration
///
/// For use when not launching an Indexer with [`crate::LocalNet::launch`].
#[derive(Debug, Default)]
pub struct EmptyConfig {}

impl IndexerConfig for EmptyConfig {
    fn setup_validator_connection<V: crate::validator::Validator>(&mut self, _validator: &V) {
        tracing::info!("Empty Validator cannot accept a port!");
    }

    fn set_listen_port(&mut self, indexer_listen_port: Option<Port>) {
        panic!("Empty validator cannot listen on port! {indexer_listen_port:?}");
    }
}

/// This struct is used to represent and manage an empty Indexer process.
///
/// Dirs are created for integration.
#[derive(Debug, Getters, CopyGetters)]
#[getset(get = "pub")]
pub struct Empty {
    /// Logs directory
    logs_dir: TempDir,
    /// Config directory
    config_dir: TempDir,
}

impl LogsToStdoutAndStderr for Empty {
    fn print_stdout(&self) {
        tracing::info!("Empty indexer stdout.");
        todo!()
    }

    fn print_stderr(&self) {
        tracing::info!("Empty indexer stderr.");
    }
}

impl Process for Empty {
    const PROCESS: ProcessId = ProcessId::Empty;

    type Config = EmptyConfig;

    async fn launch(_config: Self::Config) -> Result<Self, LaunchError> {
        let logs_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();

        Ok(Empty {
            logs_dir,
            config_dir,
        })
    }

    fn stop(&mut self) {}

    fn print_all(&self) {
        tracing::info!("Empty indexer.");
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
