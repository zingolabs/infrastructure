//! Module for the structs that represent and manage the validator/full-node processes i.e. Zebrad.
use std::path::PathBuf;

use tempfile::TempDir;
use zingo_consensus::{ActivationHeights, MinerPool, NetworkType};

use crate::process::Process;

pub mod zcashd;
pub mod zebrad;

/// **Single source of truth** for regtest fixture activation heights
/// across this whole repo (`zcash_local_net` validators + indexer
/// `Default` impls, plus `regtest-launcher`'s CLI default — see
/// [`REGTEST_FIXTURE_HEIGHTS_CLI_STRING`] for the matching string form).
///
/// **Why these specific heights**:
///
/// - Pre-NU5 upgrades all activate at height 1: the genesis-mining
///   block is the first chain block, and putting Sapling/Blossom/etc.
///   here matches mainnet's eventual deep-history shape.
/// - NU5/NU6 at height 2: the first post-genesis block. NU5 needs
///   to be active before NU6 since NU6 builds on the NU5 commitment
///   scheme.
/// - **NU6.1 at height 5**: keeps NU6.1 reachable in normal regtest
///   mining (any test that mines ≥ 5 blocks crosses the activation
///   block) while leaving 3 NU6 blocks for [`regtest_test_post_nu6_funding_streams`]
///   to deposit into Zebra's `Deferred` value pool. zebrad's
///   `subsidy_is_valid` rejects the activation block if either the
///   `lockbox_disbursements` list is empty, the address is not
///   P2SH, or the post-block deferred-pool balance goes negative
///   (zingolabs/infrastructure#244 walks through all three checks).
///
/// **Companion config required for any caller that mines past
/// height 5**: pair this with [`regtest_test_lockbox_disbursements`]
/// and [`regtest_test_post_nu6_funding_streams`]. The default
/// `ZebradConfig` impl wires both automatically; ad-hoc callers must
/// set `lockbox_disbursements` and `post_nu6_funding_streams`
/// explicitly or the activation block will be rejected.
///
/// **Cross-repo alignment** — when zainod is launched as a subprocess,
/// it reads only `network = "Regtest"` from its TOML config.
/// Activation heights are *not* propagated through the TOML; zainod
/// falls back to `zaino-common::ZEBRAD_DEFAULT_ACTIVATION_HEIGHTS`. If
/// zebrad's view of activation heights differs from zainod's, the
/// chain-index sync loop fails with
/// `InvalidData("Block commitment could not be computed")`. The same
/// (NU5=2, NU6=2, NU6.1=5) tuple must therefore be set in
/// `zaino-common::ZEBRAD_DEFAULT_ACTIVATION_HEIGHTS`. Tracked in
/// zingolabs/zaino#1076.
pub fn regtest_test_activation_heights() -> ActivationHeights {
    ActivationHeights::builder()
        .set_overwinter(Some(1))
        .set_sapling(Some(1))
        .set_blossom(Some(1))
        .set_heartwood(Some(1))
        .set_canopy(Some(1))
        .set_nu5(Some(2))
        .set_nu6(Some(2))
        .set_nu6_1(Some(5))
        .set_nu6_2(Some(5))
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
    "all=1,nu5=2,nu6=2,nu6_1=5,nu6_2=5,nu6_3=off,nu7=off";

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
    /// One zatoshi to a known-valid testnet/regtest P2SH address.
    ///
    /// zebrad's `subsidy_is_valid` (`zebra-consensus/src/block/check.rs:177`)
    /// asserts `addr.is_script_hash()` for every disbursement entry —
    /// **lockbox disbursement addresses must be P2SH** (`t2…` prefix
    /// on regtest/testnet). The standard regtest miner address is
    /// P2PKH (`tm…`) and is rejected.
    ///
    /// `t2RnBRiqrN1nW4ecZs1Fj3WWjNdnSs4kiX8` is Zebra's reference
    /// testnet NU6.1 disbursement address (`zebra-chain/src/parameters
    /// /network/subsidy/constants/testnet.rs::NU6_1_LOCKBOX_DISBURSEMENTS`)
    /// — guaranteed to parse and decode under any Testnet-class
    /// network kind (which regtest is).
    pub fn dummy() -> Self {
        Self {
            address: "t2RnBRiqrN1nW4ecZs1Fj3WWjNdnSs4kiX8".to_string(),
            amount_zats: 1,
        }
    }
}

/// **Single source of truth** for the regtest fixture lockbox
/// disbursement list. Mirrors [`regtest_test_activation_heights`]:
/// any caller that needs the canonical regtest disbursement set
/// (the harness's own `ZebradConfig`, downstream test fixtures, etc.)
/// goes through this helper rather than hand-rolling the same
/// `vec![dummy()]` literal.
///
/// Today this returns a single [`LockboxDisbursement::dummy`] —
/// enough to satisfy zebrad's `is_empty()` gate at the NU6.1
/// activation block. If the activation-block validation rule grows
/// stricter (e.g. requires multiple disbursements summing to a
/// specific total, or matching coinbase outputs), this helper is the
/// one place to update.
pub fn regtest_test_lockbox_disbursements() -> Vec<LockboxDisbursement> {
    vec![LockboxDisbursement::dummy()]
}

/// Funding-stream receiver category — mirrors Zebra's
/// `FundingStreamReceiver` (`zebra-chain/src/parameters/network/subsidy.rs`).
///
/// Serialized form matches Zebra's `Serialize` derive: PascalCase for
/// most variants, except [`Self::Ecc`] which is renamed to `"ECC"`
/// upstream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FundingStreamReceiver {
    /// Electric Coin Company. Serialized as `"ECC"`.
    Ecc,
    /// Zcash Foundation.
    ZcashFoundation,
    /// Zcash Community Grants.
    MajorGrants,
    /// Deferred / lockbox pool. Subsidy directed to this receiver
    /// accumulates in zebra's `deferred` value pool, where one-time
    /// disbursements at NU6.1 activation are drawn from. See ZIP-1015
    /// and ZIP-271.
    Deferred,
}

impl FundingStreamReceiver {
    /// Returns the receiver name as it appears in Zebra's
    /// `[[network.testnet_parameters.…funding_streams.recipients]]` TOML
    /// (matching the upstream `serde::Serialize` derive).
    pub(crate) fn as_toml(&self) -> &'static str {
        match self {
            Self::Ecc => "ECC",
            Self::ZcashFoundation => "ZcashFoundation",
            Self::MajorGrants => "MajorGrants",
            Self::Deferred => "Deferred",
        }
    }
}

/// One recipient of a funding stream — mirrors Zebra's
/// `ConfiguredFundingStreamRecipient`.
#[derive(Clone, Debug)]
pub struct FundingStreamRecipient {
    /// Receiver category.
    pub receiver: FundingStreamReceiver,
    /// Numerator of the fraction of block subsidy this recipient
    /// receives. The denominator is `100`
    /// (`FUNDING_STREAM_RECEIVER_DENOMINATOR` in Zebra) per ZIP-1015 —
    /// so `numerator: 1` means 1% of block subsidy.
    pub numerator: u64,
    /// Addresses for non-`Deferred` recipients. Ignored / `None` for
    /// `Deferred` (the lockbox is keyed by the deferred pool, not by
    /// addresses).
    pub addresses: Option<Vec<String>>,
}

/// Funding-stream configuration — mirrors Zebra's
/// `ConfiguredFundingStreams`. Written into Zebra's regtest TOML at
/// `[network.testnet_parameters.<post_nu6_>funding_streams]`.
#[derive(Clone, Debug)]
pub struct FundingStreams {
    /// Inclusive start height for the stream.
    pub start_height: u32,
    /// Exclusive end height for the stream.
    pub end_height: u32,
    /// Per-recipient configuration.
    pub recipients: Vec<FundingStreamRecipient>,
}

/// **Single source of truth** for the regtest fixture's post-NU6
/// funding streams.
///
/// Without an active funding stream depositing into the `Deferred`
/// pool, zebrad's `subsidy_is_valid` rejects the NU6.1 activation
/// block: any non-zero disbursement drives the post-block deferred
/// balance negative (the `Deferred(Constraint { value: -1, range:
/// 0..=2_100_000_000_000_000 })` failure mode). Returning a
/// non-empty stream here lets test fixtures cross NU6.1 once the
/// configured `regtest_test_activation_heights` puts NU6.1 at least
/// one block after NU6.
///
/// Default shape: a single `Deferred` recipient drawing 1% of the
/// block subsidy, active from height 2 (the `regtest_test_activation_heights`
/// NU6 height) through a far-future end. With the post-Blossom
/// regtest subsidy at 6.25 ZEC, this deposits roughly 6.25M zatoshis
/// per block into the lockbox — sufficient to cover any small test
/// disbursement after even a single NU6 block.
pub fn regtest_test_post_nu6_funding_streams() -> FundingStreams {
    FundingStreams {
        start_height: 2,
        end_height: 1_000_000,
        recipients: vec![FundingStreamRecipient {
            receiver: FundingStreamReceiver::Deferred,
            numerator: 1,
            addresses: None,
        }],
    }
}

/// Parse activation heights from a `getblockchaininfo` RPC response.
/// Shared by every validator's `get_activation_heights`; only the RPC
/// transport that fetches the response differs per validator.
fn activation_heights_from_getblockchaininfo(response: &serde_json::Value) -> ActivationHeights {
    let upgrades = response
        .get("upgrades")
        .expect("upgrades field should exist")
        .as_object()
        .expect("upgrades should be an object");

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
        .set_nu6_2(get_height("NU6.2"))
        .set_nu7(get_height("NU7"))
        .build();
    tracing::debug!(
        "regtest validator reports the following activation heights: {configured_activation_heights:?}"
    );

    configured_activation_heights
}

/// Can offer specific functionality shared across configuration for all validators.
pub trait ValidatorConfig: Default {
    /// To set the config for common Regtest parameters.
    fn set_test_parameters(
        &mut self,
        mine_to_pool: MinerPool,
        activation_heights: ActivationHeights,
        chain_cache: Option<PathBuf>,
    );
}

/// Functionality for validator/full-node processes.
pub trait Validator: Process<Config: ValidatorConfig> + Send + Sync + std::fmt::Debug {
    /// Interval between successive `get_chain_height` checks in the
    /// default [`Self::poll_chain_height`]. Override on a concrete impl
    /// only if the validator's chain-tip RPC has cadence constraints
    /// that 100ms violates.
    const CHAIN_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);

    /// Maximum total time the default [`Self::poll_chain_height`] will
    /// wait for the chain to reach the target height before panicking.
    /// Finite by design — wedges should surface, not hang regtest CI.
    const CHAIN_POLL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

    /// Delay between successive single-block mines in the default
    /// [`Self::generate_blocks_with_delay`]. Provenance of the 1500ms
    /// value is unaudited at time of writing — see lifecycle audit
    /// follow-up notes.
    const BLOCK_GENERATION_DELAY: std::time::Duration = std::time::Duration::from_millis(1500);

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

    /// Generate `n` blocks one at a time, sleeping
    /// [`Self::BLOCK_GENERATION_DELAY`] between each. Each inner mine
    /// goes through [`Self::generate_blocks`], which calls
    /// [`Self::poll_chain_height`], so the chain is at the correct
    /// height when this function returns. Concrete validators should
    /// not override this method — only the constant.
    fn generate_blocks_with_delay(
        &self,
        n: u32,
    ) -> impl std::future::Future<Output = std::io::Result<()>> + Send {
        async move {
            for _ in 0..n {
                self.generate_blocks(1).await?;
                tokio::time::sleep(Self::BLOCK_GENERATION_DELAY).await;
            }
            Ok(())
        }
    }

    /// Get chain height
    fn get_chain_height(&self) -> impl std::future::Future<Output = u32> + Send;

    /// Polls the chain until it reaches `target_height`. Default impl
    /// polls [`Self::get_chain_height`] every
    /// [`Self::CHAIN_POLL_INTERVAL`] via the shared
    /// `poll_until` primitive, panicking after
    /// [`Self::CHAIN_POLL_TIMEOUT`] elapses. Concrete validators should
    /// not override this method — only the constants.
    fn poll_chain_height(
        &self,
        target_height: u32,
    ) -> impl std::future::Future<Output = ()> + Send {
        async move {
            crate::poll::poll_until(
                Self::CHAIN_POLL_INTERVAL,
                Self::CHAIN_POLL_TIMEOUT,
                || async move { self.get_chain_height().await >= target_height },
            )
            .await
            .expect("chain failed to reach target height before CHAIN_POLL_TIMEOUT");
        }
    }

    /// Get temporary data directory.
    fn data_dir(&self) -> &TempDir;

    /// Returns path to zcashd-like config file.
    /// Lightwalletd pulls some information from the config file that zcashd builds. When running zebra-lightwalletd, we create compatibility zcash.conf. This is the path to that.
    fn get_zcashd_conf_path(&self) -> PathBuf;

    /// Network type
    fn network(&self) -> NetworkType;

    /// Caches chain. This stops the zcashd process.
    fn cache_chain(
        &mut self,
        chain_cache: PathBuf,
    ) -> impl std::future::Future<Output = std::process::Output> + Send {
        async move {
            assert!(!chain_cache.exists(), "chain cache already exists!");

            self.stop();
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;

            std::process::Command::new("cp")
                .arg("-r")
                .arg(self.data_dir().path())
                .arg(chain_cache)
                .output()
                .unwrap()
        }
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
