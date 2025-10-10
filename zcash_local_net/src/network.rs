//! Structs and utility functions associated with local network configuration

use portpicker::Port;

pub(crate) const LOCALHOST_IPV4: &str = "http://127.0.0.1";

/// Checks `fixed_port` is not in use.
/// If `fixed_port` is `None`, returns a random free port between `15_000` and `25_000`.
#[must_use]
pub fn pick_unused_port(fixed_port: Option<Port>) -> Port {
    if let Some(port) = fixed_port {
        assert!(portpicker::is_free(port), "Fixed port is not free!");
        port
    } else {
        portpicker::pick_unused_port().expect("No ports free!")
    }
}

/// Constructs a URI with the localhost IPv4 address and the specified port.
#[must_use]
pub fn localhost_uri(port: Port) -> http::Uri {
    format!("{LOCALHOST_IPV4}:{port}").try_into().unwrap()
}
