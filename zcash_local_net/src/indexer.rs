//! Module for the structs that represent and manage the indexer processes i.e. Zainod.
//!
//! Processes which are not strictly indexers but have a similar role in serving light-clients/light-wallets
//! (i.e. Lightwalletd) are also included in this category and are referred to as "light-nodes".

use portpicker::Port;

use crate::{process::Process, validator::Validator};

/// Can offer specific functionality shared across configuration for all indexers.
pub trait IndexerConfig: Default + std::fmt::Debug {
    /// To receive a port to instruct an indexer to listen at.
    fn set_listen_port(&mut self, indexer_listen_port: Option<Port>);
    /// To receive a port to instruct an indexer to listen at.
    fn setup_validator_connection<V: Validator>(&mut self, validator: &V);
}

/// Functionality for indexer/light-node processes.
pub trait Indexer: Process<Config: IndexerConfig> + std::fmt::Debug {
    /// Indexer listen port
    fn listen_port(&self) -> Port;
}

/// The Zainod executable support struct.
pub mod zainod;

/// The Lightwalletd executable support struct.
pub mod lightwalletd;

/// Empty
pub mod empty;
