#![warn(missing_docs)]
//! # Zcash Local Net
//!
//! ## Overview
//!
//! A Rust test utility crate designed to facilitate the launching and management of Zcash processes
//! on a local network (regtest/localnet mode). This crate is ideal for integration testing in the
//! development of light-clients/light-wallets, indexers/light-nodes and validators/full-nodes as it
//! provides a simple and configurable interface for launching and managing other proccesses in the
//! local network to simulate a Zcash environment.
//!
//! ## List of Processes
//! - Zcashd
//! - Zainod
//! - Lightwalletd
//!
//! ## Prerequisites
//!
//! Ensure that any processes used in this crate are installed on your system. The binaries can be in
//! $PATH or the path to the binaries can be specified when launching a process.
//!
//! ## Testing
//!
//! Pre-requisities for running integration tests successfully:
//! - Build the Zcashd, Zebrad, Zainod and Lightwalletd binaries and add to $PATH.
//! - Run `cargo test generate_zebrad_large_chain_cache --features test_fixtures -- --ignored` or `cargo nextest run generate_zebrad_large_chain_cache --run-ignored ignored-only --features test_fixtures`

use indexer::{Indexer, Lightwalletd, LightwalletdConfig, Zainod, ZainodConfig};
use validator::{Validator, Zcashd, ZcashdConfig, Zebrad, ZebradConfig};

pub(crate) mod config;
pub mod error;
pub mod indexer;
pub(crate) mod launch;
pub(crate) mod logs;
pub mod network;
pub mod utils;
pub mod validator;

#[cfg(feature = "test_fixtures")]
pub mod test_fixtures;

#[cfg(feature = "client")]
pub mod client;

#[derive(Clone, Copy)]
enum Process {
    Zcashd,
    Zebrad,
    Zainod,
    Lightwalletd,
}

impl std::fmt::Display for Process {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let process = match self {
            Self::Zcashd => "zcashd",
            Self::Zebrad => "zebrad",
            Self::Zainod => "zainod",
            Self::Lightwalletd => "lightwalletd",
        };
        write!(f, "{}", process)
    }
}

/// This stuct is used to represent and manage the local network.
pub struct LocalNet<I, V>
where
    I: Indexer,
    V: Validator,
{
    indexer: I,
    validator: V,
}

impl<I, V> LocalNet<I, V>
where
    I: Indexer,
    V: Validator,
{
    /// Gets indexer.
    pub fn indexer(&self) -> &I {
        &self.indexer
    }

    /// Gets indexer as mut.
    pub fn indexer_mut(&mut self) -> &mut I {
        &mut self.indexer
    }

    /// Gets validator.
    pub fn validator(&self) -> &V {
        &self.validator
    }

    /// Gets validator as mut.
    pub fn validator_mut(&mut self) -> &mut V {
        &mut self.validator
    }
}

impl LocalNet<Zainod, Zcashd> {
    /// Launch LocalNet.
    ///
    /// The `validator_port` field of [`crate::indexer::ZainodConfig`] will be overwritten to match the validator's RPC port.
    pub async fn launch(mut indexer_config: ZainodConfig, validator_config: ZcashdConfig) -> Self {
        let validator = Zcashd::launch(validator_config).await.unwrap();
        indexer_config.validator_port = validator.port();
        let indexer = Zainod::launch(indexer_config).unwrap();

        LocalNet { indexer, validator }
    }
}

impl LocalNet<Zainod, Zebrad> {
    /// Launch LocalNet.
    ///
    /// The `validator_port` field of [`crate::indexer::ZainodConfig`] will be overwritten to match the validator's RPC port.
    pub async fn launch(mut indexer_config: ZainodConfig, validator_config: ZebradConfig) -> Self {
        let validator = Zebrad::launch(validator_config).await.unwrap();
        indexer_config.validator_port = validator.rpc_listen_port();
        let indexer = Zainod::launch(indexer_config).unwrap();

        LocalNet { indexer, validator }
    }
}

impl LocalNet<Lightwalletd, Zcashd> {
    /// Launch LocalNet.
    ///
    /// The `validator_conf` field of [`crate::indexer::LightwalletdConfig`] will be overwritten to match the validator's config path.
    pub async fn launch(
        mut indexer_config: LightwalletdConfig,
        validator_config: ZcashdConfig,
    ) -> Self {
        let validator = Zcashd::launch(validator_config).await.unwrap();
        indexer_config.zcashd_conf = validator.config_path();
        let indexer = Lightwalletd::launch(indexer_config).unwrap();

        LocalNet { indexer, validator }
    }
}

impl LocalNet<Lightwalletd, Zebrad> {
    /// Launch LocalNet.
    ///
    /// The `validator_conf` field of [`crate::indexer::LightwalletdConfig`] will be overwritten to match the validator's config path.
    pub async fn launch(
        mut indexer_config: LightwalletdConfig,
        validator_config: ZebradConfig,
    ) -> Self {
        let validator = Zebrad::launch(validator_config).await.unwrap();
        indexer_config.zcashd_conf = validator.config_dir().path().join(config::ZCASHD_FILENAME);
        let indexer = Lightwalletd::launch(indexer_config).unwrap();

        LocalNet { indexer, validator }
    }
}
