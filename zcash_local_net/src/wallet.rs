//! The Wallet abstraction: the interface through which the harness
//! actuates wallets, generically over their implementations.
//!
//! Wallets are the third kind of process this crate manages, alongside
//! validators ([`crate::validator`]) and indexers ([`crate::indexer`]).
//! This module defines only the contract — the [`Wallet`] and
//! [`WalletConfig`] traits and their supporting types. Implementations
//! live with their binaries: the zcash-devtool wallet is in
//! [`zcash_devtool`] (in-tree for now), and the zingo-cli wallet is
//! implemented in the zingolib repository against this trait. The
//! harness drives whichever implementation the caller names, e.g.
//! through `LocalNet::launch_wallet::<W>`.
//!
//! Wallets speak the lightwalletd protocol (gRPC) to an indexer — they
//! never talk to the validator directly. Launch order is therefore
//! validator → indexer → wallet; [`WalletConfig::setup_indexer_connection`]
//! mirrors [`crate::indexer::IndexerConfig::setup_validator_connection`]
//! for wiring the wallet to a running indexer.
//!
//! Activation heights are the one exception to "never talk to the
//! validator": a regtest wallet needs the chain's schedule, the
//! light-client protocol does not expose it, and ADR 0003 forbids a
//! second source of truth. The harness therefore queries the Validator
//! on the wallet's behalf, and the type system enforces it — see
//! [`WalletNetwork`] and [`ValidatorHeights`].

use crate::{error::WalletError, indexer::Indexer};

/// Regtest activation heights whose provenance is a query of a running
/// Validator. The inner value has no public constructor; the only way
/// to obtain one is [`WalletNetwork::from_validator`]. A wallet
/// configured with these heights is therefore guaranteed, at compile
/// time, to have derived them from the Validator (ADR 0003: the
/// Validator is the single source of truth for activation heights).
/// Crate-internal tests may construct the value directly to pin
/// serialization offline, where no chain exists for the heights to
/// disagree with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidatorHeights(pub(crate) zingo_consensus::ActivationHeights);

impl ValidatorHeights {
    /// The schedule the Validator reported. Reading is public because
    /// every [`Wallet`] implementation must serialize the heights into
    /// its own binary's configuration; only *construction* is
    /// restricted, since provenance — not secrecy — is the invariant.
    pub fn activation_heights(&self) -> zingo_consensus::ActivationHeights {
        self.0
    }
}

/// The network a wallet client is launched against. Unlike
/// [`zingo_consensus::NetworkType`], the regtest variant cannot carry
/// caller-supplied heights: it demands a [`ValidatorHeights`], which
/// only a Validator query produces. Writing a hand-typed height vector
/// into a wallet config is unrepresentable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalletNetwork {
    /// Mainnet. The binaries compile the public network's parameters
    /// in; no heights are carried.
    Mainnet,
    /// Testnet. The binaries compile the public network's parameters
    /// in; no heights are carried.
    Testnet,
    /// Regtest, with activation heights derived from the running
    /// Validator.
    Regtest(ValidatorHeights),
}

impl WalletNetwork {
    /// Build the regtest wallet network by querying the running
    /// `validator` for its activation-height schedule. This is the
    /// only public constructor of [`ValidatorHeights`].
    pub async fn from_validator<V: crate::validator::Validator>(validator: &V) -> Self {
        WalletNetwork::Regtest(ValidatorHeights(validator.get_activation_heights().await))
    }
}

/// Configuration behavior every wallet implementation shares.
pub trait WalletConfig: std::fmt::Debug {
    /// To receive the connection details of the indexer this
    /// wallet will sync from and broadcast through.
    fn setup_indexer_connection<I: Indexer>(&mut self, indexer: &I);
}

/// Which receiver of the wallet's unified address to emit from
/// [`Wallet::address`].
///
/// A dedicated enum rather than [`zingo_consensus::MinerPool`], which
/// has no `Unified` variant — the wrong shape for "give me this
/// receiver of my UA".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddressReceiver {
    /// The full unified address (all available receivers).
    Unified,
    /// The transparent (P2PKH) receiver, as a bare transparent address.
    Transparent,
    /// The Sapling receiver, as a bare Sapling address.
    Sapling,
    /// The Orchard receiver. Orchard receivers have no bare encoding,
    /// so this is a unified address carrying only the Orchard receiver.
    Orchard,
}

/// The interface through which the harness actuates a wallet.
///
/// The operation set mirrors what wallet integration suites (zaino's in
/// particular) drive between asserts: sync to tip, send, shield,
/// per-pool balance, address derivation, and rescan-from-scratch. All
/// operations run to completion before returning — callers can sequence
/// `act → mine → wait → assert` without additional synchronization.
pub trait Wallet: Sized {
    /// The configuration for this wallet implementation.
    type Config: WalletConfig;

    /// Create the wallet (restoring from the configured mnemonic and
    /// birthday) and return the managed wallet. The configured indexer
    /// must already be serving: wallet initialization fetches the chain
    /// tip and the birthday tree state from it.
    fn launch(config: Self::Config)
    -> impl std::future::Future<Output = Result<Self, WalletError>>;

    /// Scan the chain and sync the wallet to the indexer's tip.
    fn sync(&self) -> impl std::future::Future<Output = Result<(), WalletError>>;

    /// Send `value_zats` zatoshis to `address` (transparent, sapling or
    /// unified). Returns the txid of the broadcast transaction as a hex
    /// string. The transaction is broadcast but NOT mined; mine a block
    /// and [`Wallet::sync`] to confirm it.
    fn send(
        &self,
        address: &str,
        value_zats: u64,
    ) -> impl std::future::Future<Output = Result<String, WalletError>>;

    /// Shield transparent funds (including mature transparent coinbase)
    /// into the orchard pool. Returns the txid of the broadcast
    /// transaction as a hex string.
    fn shield(&self) -> impl std::future::Future<Output = Result<String, WalletError>>;

    /// The wallet's view of its balance. Run [`Wallet::sync`] first;
    /// this reads the local wallet database without contacting the
    /// indexer.
    fn balance(&self) -> impl std::future::Future<Output = Result<WalletBalance, WalletError>>;

    /// The requested `receiver` of the wallet's unified address, as an
    /// encoded address string. Reads the local wallet database without
    /// contacting the indexer.
    fn address(
        &self,
        receiver: AddressReceiver,
    ) -> impl std::future::Future<Output = Result<String, WalletError>>;

    /// The wallet's default unified address. Convenience for
    /// [`Wallet::address`] with [`AddressReceiver::Unified`].
    fn default_address(&self) -> impl std::future::Future<Output = Result<String, WalletError>> {
        self.address(AddressReceiver::Unified)
    }

    /// Node/indexer information reported by the configured server.
    /// Contacts the indexer (the analogue of zingolib's `do_info`); a
    /// smoke check that the wallet can reach and talk to its server.
    fn get_info(&self) -> impl std::future::Future<Output = Result<GetInfo, WalletError>>;

    /// Wipe the wallet state and re-restore from the stored mnemonic
    /// and birthday, preserving account metadata. Equivalent to a
    /// rescan from scratch; [`Wallet::sync`] afterwards to rebuild.
    fn rescan(&self) -> impl std::future::Future<Output = Result<(), WalletError>>;
}

/// Node/indexer information from [`Wallet::get_info`].
///
/// The field set is a frozen contract with the wallet binary: see
/// `wallet::zcash_devtool`'s get-info parser. `chain_tip_height` is the
/// **server/node tip** the indexer reports, never the wallet's
/// locally-synced height (which, if ever surfaced, gets its own
/// explicitly-named field).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GetInfo {
    /// The lightwalletd-protocol server URI the wallet connected to.
    pub server_uri: String,
    /// The chain name the server reports (e.g. `"main"`, `"test"`,
    /// `"regtest"`).
    pub chain_name: String,
    /// The current chain tip height as the server reports it (the
    /// node/indexer tip). `u64` to match the wire `LightdInfo.block_height`.
    pub chain_tip_height: u64,
}

/// A wallet balance snapshot, in zatoshis.
///
/// Spendable values are as reported by the wallet's configured
/// confirmations policy; immature or unconfirmed funds are included in
/// `total` but not in the per-pool spendable fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WalletBalance {
    /// Total wallet balance across all pools, including funds that are
    /// not yet spendable.
    pub total: u64,
    /// Spendable sapling balance.
    pub sapling_spendable: u64,
    /// Spendable orchard balance.
    pub orchard_spendable: u64,
    /// Spendable ironwood balance (the NU6.3 shielded pool; zero until
    /// the chain passes NU6.3 activation and the wallet holds ironwood
    /// notes). Requires a devtool at or past zingolabs/zcash-devtool
    /// `8eccaceb`, which added the field to `balance --json`.
    pub ironwood_spendable: u64,
    /// Spendable transparent balance.
    pub transparent_spendable: u64,
    /// The height of the current chain tip as the wallet sees it (the
    /// node/indexer tip, mirroring `WalletSummary::chain_tip_height` and
    /// the `chain_tip_height` field of the get-info contract). This is
    /// *not* the wallet's locally-synced height — that value, if ever
    /// surfaced, gets its own explicitly-named field (e.g.
    /// `wallet_synced_height`).
    pub chain_tip_height: u32,
}

/// The zcash-devtool executable support struct.
pub mod zcash_devtool;
