//! Module for the structs that represent and manage the indexer processes i.e. Zainod.
//!
//! Processes which are not strictly indexers but have a similar role in serving light-clients/light-wallets
//! (i.e. Lightwalletd) are also included in this category and are referred to as "light-nodes".

use portpicker::Port;

use crate::{process::ItsAProcess, validator::Validator};

/// Can offer specific functionality shared across configuration for all indexers.
pub trait IndexerConfig {
    /// To receive a port to instruct an indexer to listen at.
    fn setup_validator_connection<V: Validator>(&mut self, validator: &V);
}

/// Functionality for indexer/light-node processes.
pub trait Indexer: ItsAProcess<Config: IndexerConfig> {
    /// Helps set up its config to listen at a port.
    fn setup_validator_connection<V: Validator>(config: &mut Self::Config, validator: &V) {
        config.setup_validator_connection(validator);
    }
    /// Indexer listen port
    fn listen_port(&self) -> Port;
}

/// The Zainod executable support struct.
pub mod zainod;

/// The Lightwalletd executable support struct.
pub mod lightwalletd;

/// Empty
pub mod empty;
