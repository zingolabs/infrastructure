use portpicker::Port;

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
