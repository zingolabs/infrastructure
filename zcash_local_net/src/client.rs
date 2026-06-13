//! Module for the structs that represent and manage wallet client processes i.e. zcash-devtool.
//!
//! Clients are the third kind of process this crate manages, alongside
//! validators ([`crate::validator`]) and indexers ([`crate::indexer`]).
//! They differ structurally from both: the managed binary is not a
//! daemon. Each wallet operation (`init`, `sync`, `send`, …) is a
//! separate run-to-completion subprocess invocation against a persistent
//! wallet directory. There is no long-lived child handle to stop or to
//! probe for readiness; the managed state is the wallet directory
//! itself, created in a tempdir owned by the client struct and removed
//! when it drops.
//!
//! Clients speak the lightwalletd protocol (gRPC) to an indexer — they
//! never talk to the validator directly. Launch order is therefore
//! validator → indexer → client; [`ClientConfig::setup_indexer_connection`]
//! mirrors [`crate::indexer::IndexerConfig::setup_validator_connection`]
//! for wiring the client to a running indexer.

use crate::{error::ClientError, indexer::Indexer};

/// Can offer specific functionality shared across configuration for all clients.
pub trait ClientConfig: Default + std::fmt::Debug {
    /// To receive the connection details of the indexer this client's
    /// wallet will sync from and broadcast through.
    fn setup_indexer_connection<I: Indexer>(&mut self, indexer: &I);
}

/// Functionality for wallet client processes.
///
/// The operation set mirrors what wallet integration suites (zaino's in
/// particular) drive between asserts: sync to tip, send, shield,
/// per-pool balance, address derivation, and rescan-from-scratch. All
/// operations run to completion before returning — callers can sequence
/// `act → mine → wait → assert` without additional synchronization.
pub trait Client: Sized {
    /// A config struct for the client.
    type Config: ClientConfig;

    /// Create the wallet (restoring from the configured mnemonic and
    /// birthday) and return the managed client. The configured indexer
    /// must already be serving: wallet initialization fetches the chain
    /// tip and the birthday tree state from it.
    fn launch(config: Self::Config)
    -> impl std::future::Future<Output = Result<Self, ClientError>>;

    /// Scan the chain and sync the wallet to the indexer's tip.
    fn sync(&self) -> impl std::future::Future<Output = Result<(), ClientError>>;

    /// Send `value_zats` zatoshis to `address` (transparent, sapling or
    /// unified). Returns the txid of the broadcast transaction as a hex
    /// string. The transaction is broadcast but NOT mined; mine a block
    /// and [`Client::sync`] to confirm it.
    fn send(
        &self,
        address: &str,
        value_zats: u64,
    ) -> impl std::future::Future<Output = Result<String, ClientError>>;

    /// Shield transparent funds (including mature transparent coinbase)
    /// into the orchard pool. Returns the txid of the broadcast
    /// transaction as a hex string.
    fn shield(&self) -> impl std::future::Future<Output = Result<String, ClientError>>;

    /// The wallet's view of its balance. Run [`Client::sync`] first;
    /// this reads the local wallet database without contacting the
    /// indexer.
    fn balance(&self) -> impl std::future::Future<Output = Result<WalletBalance, ClientError>>;

    /// The wallet's default unified address.
    fn default_address(&self) -> impl std::future::Future<Output = Result<String, ClientError>>;

    /// Wipe the wallet state and re-restore from the stored mnemonic
    /// and birthday, preserving account metadata. Equivalent to a
    /// rescan from scratch; [`Client::sync`] afterwards to rebuild.
    fn rescan(&self) -> impl std::future::Future<Output = Result<(), ClientError>>;
}

/// A wallet balance snapshot, in zatoshis.
///
/// Spendable values are as reported by the client's configured
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
    /// Spendable transparent balance.
    pub transparent_spendable: u64,
    /// The chain tip height the wallet has synced to.
    pub chain_tip_height: u32,
}

/// The zcash-devtool executable support struct.
pub mod zcash_devtool;
