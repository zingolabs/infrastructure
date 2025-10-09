use getset::{CopyGetters, Getters};
use portpicker::Port;
use tempfile::TempDir;

use crate::{
    error::LaunchError,
    indexer::{Indexer, IndexerConfig},
    process::ItsAProcess,
    Process,
};

/// Empty configuration
///
/// For use when not launching an Indexer with [`crate::LocalNet::launch`].
#[derive(Default)]
pub struct EmptyConfig {}

impl IndexerConfig for EmptyConfig {
    fn set_validator_port(&mut self, _listen_port: Port) {
        tracing::info!("Empty Validator cannot accept a port!");
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
    const PROCESS: Process = Process::Empty;

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
        println!("Empty indexer.");
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
