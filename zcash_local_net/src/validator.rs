//! Module for the structs that represent and manage the validator/full-node processes i.e. Zebrad.
use std::path::PathBuf;

use portpicker::Port;
use tempfile::TempDir;
use zcash_protocol::PoolType;
use zebra_chain::parameters::testnet::ConfiguredActivationHeights;
use zebra_chain::parameters::NetworkKind;

use crate::process::Process;

pub mod zcashd;
pub mod zebrad;

/// Parse activation heights from the upgrades object returned by getblockchaininfo RPC.
fn parse_activation_heights_from_rpc(
    upgrades: &serde_json::Map<String, serde_json::Value>,
) -> ConfiguredActivationHeights {
    // Helper function to extract activation height for a network upgrade by name
    let get_height = |name: &str| -> Option<u32> {
        upgrades.values().find_map(|upgrade| {
            if upgrade.get("name")?.as_str()?.eq_ignore_ascii_case(name) {
                upgrade
                    .get("activationheight")?
                    .as_u64()
                    .and_then(|h| u32::try_from(h).ok())
            } else {
                None
            }
        })
    };

    let configured_activation_heights = ConfiguredActivationHeights {
        before_overwinter: get_height("BeforeOverwinter"),
        overwinter: get_height("Overwinter"),
        sapling: get_height("Sapling"),
        blossom: get_height("Blossom"),
        heartwood: get_height("Heartwood"),
        canopy: get_height("Canopy"),
        nu5: get_height("NU5"),
        nu6: get_height("NU6"),
        nu6_1: get_height("NU6.1"),
        nu7: get_height("NU7"),
    };
    tracing::debug!("regtest validator reports the following activation heights: {configured_activation_heights:?}");
    configured_activation_heights
}

/// Can offer specific functionality shared across configuration for all validators.
pub trait ValidatorConfig: Default {
    /// To set the config for common Regtest parameters.
    fn set_test_parameters(
        &mut self,
        mine_to_pool: PoolType,
        configured_activation_heights: ConfiguredActivationHeights,
        chain_cache: Option<PathBuf>,
    );
}

/// Functionality for validator/full-node processes.
pub trait Validator: Process<Config: ValidatorConfig> + std::fmt::Debug {
    /// A representation of the Network Upgrade Activation heights applied for this
    /// Validator's test configuration.
    fn get_activation_heights(
        &self,
    ) -> impl std::future::Future<Output = ConfiguredActivationHeights> + Send;

    /// Generate `n` blocks. This implementation should also call [`Self::poll_chain_height`] so the chain is at the
    /// correct height when this function returns.
    fn generate_blocks(
        &self,
        n: u32,
    ) -> impl std::future::Future<Output = std::io::Result<()>> + Send;

    /// Generate `n` blocks. This implementation should also call [`Self::poll_chain_height`] so the chain is at the
    /// correct height when this function returns.
    fn generate_blocks_with_delay(
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

    /// Returns path to zcashd-like config file.
    /// Lightwalletd pulls some information from the config file that zcashd builds. When running zebra-lightwalletd, we create compatibility zcash.conf. This is the path to that.
    fn get_zcashd_conf_path(&self) -> PathBuf;

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
