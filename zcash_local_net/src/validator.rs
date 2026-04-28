//! Module for the structs that represent and manage the validator/full-node processes i.e. Zebrad.
use std::path::PathBuf;

use tempfile::TempDir;
use zcash_protocol::PoolType;
use zingo_common_components::protocol::{ActivationHeights, NetworkType};

use crate::process::Process;

pub mod zcashd;
pub mod zebrad;

/// **Single source of truth** for regtest fixture activation heights
/// across this whole repo (`zcash_local_net` validators + indexer
/// `Default` impls, plus `regtest-launcher`'s CLI default — see
/// [`REGTEST_FIXTURE_HEIGHTS_CLI_STRING`] for the matching string form).
///
/// **Why this exists, and why these specific heights**:
///
/// - `ActivationHeights::default()` from `zingo_common_components` puts
///   every upgrade including NU6.1 at height 1, which makes the
///   genesis-mining block the NU6.1 activation block. zebrad rejects
///   the proposal because it lacks the NU6.1 lockbox disbursements
///   that `proposal_block_from_template` does not generate (consensus
///   error: "missing lockbox disbursements for NU6.1 activation
///   block"). See zingolabs/infrastructure#241.
///
/// - When zainod is launched as a subprocess, it reads only
///   `network = "Regtest"` from its TOML config. Activation heights
///   are *not* propagated through the TOML — zainod falls back to
///   `zaino-common::ZEBRAD_DEFAULT_ACTIVATION_HEIGHTS`, which sets
///   nu5/nu6 at height 2 and nu6_1 at height 1000.
///
/// If zebrad's view of activation heights differs from zainod's, the
/// chain-index sync loop fails with
/// `InvalidData("Block commitment could not be computed")` because
/// `block.commitment(network)` evaluates the wrong commitment scheme
/// for that block height. We must therefore align this helper exactly
/// with `zaino-common::ZEBRAD_DEFAULT_ACTIVATION_HEIGHTS` so that all
/// fixture configs (validator + indexer) agree on what regtest looks
/// like.
pub fn regtest_test_activation_heights() -> ActivationHeights {
    ActivationHeights::builder()
        .set_overwinter(Some(1))
        .set_sapling(Some(1))
        .set_blossom(Some(1))
        .set_heartwood(Some(1))
        .set_canopy(Some(1))
        .set_nu5(Some(2))
        .set_nu6(Some(2))
        .set_nu6_1(Some(1000))
        .set_nu7(None)
        .build()
}

/// CLI string form of [`regtest_test_activation_heights`] for use as
/// `clap`'s `default_value` (which requires a `&'static str`).
///
/// **Drift between this string and the helper above is enforced by a
/// unit test in `regtest-launcher::cli::tests`** — the test parses
/// this string and verifies the result, after the same conversion
/// that `regtest-launcher::main` applies, equals the helper output.
pub const REGTEST_FIXTURE_HEIGHTS_CLI_STRING: &str =
    "all=1,nu5=2,nu6=2,nu6_1=1000,nu7=off";

/// One lockbox disbursement output to inject into Zebra's regtest
/// `[network.testnet_parameters]` configuration.
///
/// Pairs with Zebra's upstream `ConfiguredLockboxDisbursement`
/// (`zebra-chain/src/parameters/network/testnet.rs`). On Mainnet and
/// the default Testnet, Zebra ships a hardcoded ZIP-271 disbursement
/// list. On regtest the list defaults to empty, which makes
/// `subsidy_is_valid` (`zebra-consensus/src/block/check.rs`) reject
/// the NU6.1 activation block with
/// `"missing lockbox disbursements for NU6.1 activation block"`.
/// Any regtest test whose chain reaches NU6.1 needs a non-empty
/// list here.
#[derive(Clone, Debug)]
pub struct LockboxDisbursement {
    /// Recipient address, as a valid regtest transparent address
    /// string. Zebra parses this string the same way it parses any
    /// other configured testnet-parameters address.
    pub address: String,
    /// Disbursement amount, in zatoshis.
    pub amount_zats: u64,
}

impl LockboxDisbursement {
    /// One zatoshi to the standard regtest miner address. Sufficient
    /// to satisfy zebrad's `lockbox_disbursements.is_empty()` check
    /// without needing to allocate a separate funded address.
    pub fn dummy() -> Self {
        Self {
            address: zingo_test_vectors::ZEBRAD_DEFAULT_MINER.to_string(),
            amount_zats: 1,
        }
    }
}

/// Parse activation heights from the upgrades object returned by getblockchaininfo RPC.
fn parse_activation_heights_from_rpc(
    upgrades: &serde_json::Map<String, serde_json::Value>,
) -> ActivationHeights {
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

    let configured_activation_heights = ActivationHeights::builder()
        .set_overwinter(get_height("Overwinter"))
        .set_sapling(get_height("Sapling"))
        .set_blossom(get_height("Blossom"))
        .set_heartwood(get_height("Heartwood"))
        .set_canopy(get_height("Canopy"))
        .set_nu5(get_height("NU5"))
        .set_nu6(get_height("NU6"))
        .set_nu6_1(get_height("NU6.1"))
        .set_nu7(get_height("NU7"))
        .build();
    tracing::debug!("regtest validator reports the following activation heights: {configured_activation_heights:?}");

    configured_activation_heights
}

/// Can offer specific functionality shared across configuration for all validators.
pub trait ValidatorConfig: Default {
    /// To set the config for common Regtest parameters.
    fn set_test_parameters(
        &mut self,
        mine_to_pool: PoolType,
        activation_heights: ActivationHeights,
        chain_cache: Option<PathBuf>,
    );
}

/// Functionality for validator/full-node processes.
pub trait Validator: Process<Config: ValidatorConfig> + std::fmt::Debug {
    /// A representation of the Network Upgrade Activation heights applied for this
    /// Validator's test configuration.
    fn get_activation_heights(&self)
        -> impl std::future::Future<Output = ActivationHeights> + Send;

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
    fn network(&self) -> NetworkType;

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
        validator_network: NetworkType,
    ) -> PathBuf;

    /// To reveal a port.
    fn get_port(&self) -> u16;
}
