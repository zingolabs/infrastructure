//! Module for the structs that represent and manage the validator/full-node processes i.e. Zebrad.
use std::path::PathBuf;

use portpicker::Port;
use tempfile::TempDir;
use zebra_chain::parameters::testnet;
use zebra_chain::parameters::NetworkKind;

use crate::process::IsAProcess;

pub mod zcashd;
pub mod zebrad;

/// Functionality for validator/full-node processes.
pub trait Validator: IsAProcess {
    /// A representation of the Network Upgrade Activation heights applied for this
    /// Validator's test configuration.
    fn get_activation_heights(&self) -> testnet::ConfiguredActivationHeights;

    /// Generate `n` blocks. This implementation should also call [`Self::poll_chain_height`] so the chain is at the
    /// correct height when this function returns.
    fn generate_blocks(
        &self,
        n: u32,
    ) -> impl std::future::Future<Output = std::io::Result<()>> + Send;

    /// Get chain height
    fn get_chain_height(&self) -> impl std::future::Future<Output = u32> + Send;

    /// Polls chain until it reaches target height
    fn poll_chain_height(&self, target_height: u32)
        -> impl std::future::Future<Output = ()> + Send;

    /// Get temporary data directory.
    fn data_dir(&self) -> &TempDir;

    /// Returns path to config file.
    fn get_zcashd_like_config_path(&self) -> PathBuf;

    /// Network type
    fn network(&self) -> NetworkKind;

    /// Caches chain. This stops the zcashd process.
    fn cache_chain(&mut self, chain_cache: PathBuf) -> std::process::Output {
        assert!(!chain_cache.exists(), "chain cache already exists!");

        self.stop();
        std::thread::sleep(std::time::Duration::from_secs(3));

        std::process::Command::new("cp")
            .arg("-r")
            .arg(self.data_dir().path())
            .arg(chain_cache)
            .output()
            .unwrap()
    }

    /// Checks `chain cache` is valid and loads into `validator_data_dir`.
    /// Returns the path to the loaded chain cache.
    ///
    /// If network is not `Regtest` variant, the chain cache will not be copied and the original cache path will be
    /// returned instead
    fn load_chain(
        chain_cache: PathBuf,
        validator_data_dir: PathBuf,
        validator_network: NetworkKind,
    ) -> PathBuf;

    /// To reveal a port.
    fn get_port(&self) -> Port;
}
