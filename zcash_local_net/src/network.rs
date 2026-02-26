//! Structs and utility functions associated with local network configuration

/// Checks `fixed_port` is not in use.
/// If `fixed_port` is `None`, returns a random free port between `15_000` and `25_000`.
#[must_use]
pub fn pick_unused_port(fixed_port: Option<u16>) -> u16 {
    if let Some(port) = fixed_port {
        assert!(portpicker::is_free(port), "Fixed port is not free!");
        port
    } else {
        portpicker::pick_unused_port().expect("No ports free!")
    }
}
