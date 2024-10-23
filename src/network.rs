//! Structs and utility functions associated with local network configuration

use portpicker::Port;
use zcash_primitives::consensus::BlockHeight;

pub(crate) const LOCALHOST_IPV4: &str = "http://127.0.0.1";

/// Activation heights for local network upgrades
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActivationHeights {
    /// Overwinter network upgrade activation height
    pub overwinter: BlockHeight,
    /// Sapling network upgrade activation height
    pub sapling: BlockHeight,
    /// Blossom network upgrade activation height
    pub blossom: BlockHeight,
    /// Heartwood network upgrade activation height
    pub heartwood: BlockHeight,
    /// Canopy network upgrade activation height
    pub canopy: BlockHeight,
    /// Nu5 (a.k.a. Orchard) network upgrade activation height
    pub nu5: BlockHeight,
}

impl Default for ActivationHeights {
    fn default() -> Self {
        Self {
            overwinter: 1.into(),
            sapling: 1.into(),
            blossom: 1.into(),
            heartwood: 1.into(),
            canopy: 1.into(),
            nu5: 1.into(),
        }
    }
}

/// Checks `fixed_port` is not in use.
/// If `fixed_port` is `None`, returns a random free port between 15_000 and 25_000.
pub(crate) fn pick_unused_port(fixed_port: Option<Port>) -> Port {
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
