//! Structs and utility functions associated with local network configuration

use portpicker::Port;
use zcash_primitives::consensus::BlockHeight;
use zcash_protocol::consensus::{NetworkUpgrade, Parameters as _};

pub(crate) const LOCALHOST_IPV4: &str = "http://127.0.0.1";

/// Activation heights for local network upgrades
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActivationHeights {
    inner: zcash_protocol::local_consensus::LocalNetwork,
}

impl Default for ActivationHeights {
    fn default() -> Self {
        Self {
            inner: zcash_protocol::local_consensus::LocalNetwork {
                overwinter: Some(BlockHeight::from(1)),
                sapling: Some(BlockHeight::from(1)),
                blossom: Some(BlockHeight::from(1)),
                heartwood: Some(BlockHeight::from(1)),
                canopy: Some(BlockHeight::from(1)),
                nu5: Some(BlockHeight::from(1)),
                nu6: Some(BlockHeight::from(1)),
                nu6_1: Some(BlockHeight::from(1)),
            },
        }
    }
}

impl ActivationHeights {
    /// Returns activation height for given `network_upgrade`.
    pub fn new(inner: zcash_protocol::local_consensus::LocalNetwork) -> Self {
        Self { inner }
    }

    /// Creates activation heights with sequential block heights (1, 2, 3, 4, 5, 6, 7, 8)
    pub fn sequential_heights() -> Self {
        Self {
            inner: zcash_protocol::local_consensus::LocalNetwork {
                overwinter: Some(BlockHeight::from(1)),
                sapling: Some(BlockHeight::from(2)),
                blossom: Some(BlockHeight::from(3)),
                heartwood: Some(BlockHeight::from(4)),
                canopy: Some(BlockHeight::from(5)),
                nu5: Some(BlockHeight::from(6)),
                nu6: Some(BlockHeight::from(7)),
                nu6_1: Some(BlockHeight::from(8)),
            },
        }
    }
    /// get the activation height of a specific network, as specified by the struct
    pub fn get_height(&self, upgrade: zcash_protocol::consensus::NetworkUpgrade) -> u32 {
        self.activation_height(upgrade)
            .unwrap_or(BlockHeight::from(1))
            .into()
    }
}

impl zcash_protocol::consensus::Parameters for ActivationHeights {
    fn network_type(&self) -> zcash_protocol::consensus::NetworkType {
        self.inner.network_type()
    }

    fn activation_height(&self, nu: NetworkUpgrade) -> Option<BlockHeight> {
        self.inner.activation_height(nu)
    }
}

/// Checks `fixed_port` is not in use.
/// If `fixed_port` is `None`, returns a random free port between 15_000 and 25_000.
pub fn pick_unused_port(fixed_port: Option<Port>) -> Port {
    if let Some(port) = fixed_port {
        if !portpicker::is_free(port) {
            panic!("Fixed port is not free!");
        };
        port
    } else {
        portpicker::pick_unused_port().expect("No ports free!")
    }
}

/// Constructs a URI with the localhost IPv4 address and the specified port.
pub fn localhost_uri(port: Port) -> http::Uri {
    format!("{}:{}", LOCALHOST_IPV4, port).try_into().unwrap()
}
