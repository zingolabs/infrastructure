//! Module for the structs that represent and manage the indexer processes i.e. Zainod.
//!
//! Processes which are not strictly indexers but have a similar role in serving light-clients/light-wallets
//! (i.e. Lightwalletd) are also included in this category and are referred to as "light-nodes".

use portpicker::Port;

use crate::process::ItsAProcess;

/// Can offer specific functionality shared across configuration for all indexers.
pub trait IndexerConfig {
    /// To receive a port to instruct an indexer to listen at.
    fn set_validator_port(&mut self, listen_port: Port);
}

/// Functionality for indexer/light-node processes.
pub trait Indexer: ItsAProcess<Config: IndexerConfig> {
    /// Helps set up its config to listen at a port.
    fn set_config_port(config: &mut Self::Config, port: Port) {
        config.set_validator_port(port);
    }
    /// Indexer listen port
    fn listen_port(&self) -> Port;
}

pub mod zainod;

pub mod lightwalletd;

pub mod empty;
