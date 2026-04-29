//! Crate level error module

/// Errors associated with launching processes
#[derive(thiserror::Error, Debug, Clone)]
pub enum LaunchError {
    /// Process failed during launch
    #[error(
        "{process_name} failed during launch.\nExit status: {exit_status}\nStdout: {stdout}\nStderr: {stderr}"
    )]
    ProcessFailed {
        /// Process name
        process_name: String,
        /// Exit status
        exit_status: std::process::ExitStatus,
        /// Stdout log
        stdout: String,
        /// Stderr log
        stderr: String,
    },
    /// `launch::wait`'s indicator-scan matched an error string in the
    /// child's stdout/stderr before the child exited. Replaces the
    /// previous unconditional `panic!` at `launch.rs:88` / `:111` so
    /// callers (in particular, the bounded retry-on-collision wrapper)
    /// can react to the failure as a typed error rather than catching
    /// a panic.
    #[error(
        "{process_name} launch aborted: child stderr/stdout matched an error indicator before exit.\nMatched indicator: {matched_indicator:?}\nStdout: {stdout}\nStderr: {stderr}"
    )]
    LaunchAborted {
        /// Process name
        process_name: String,
        /// The error indicator string that tripped the scan
        matched_indicator: String,
        /// Captured stdout up to the abort
        stdout: String,
        /// Captured stderr up to the abort
        stderr: String,
    },
    /// RPC endpoint did not respond within the readiness budget
    #[error(
        "{process_name} RPC endpoint at {address} did not respond within {timeout:?}: {last_error}"
    )]
    RpcReadinessTimeout {
        /// Process name
        process_name: String,
        /// RPC address polled
        address: std::net::SocketAddr,
        /// Timeout that elapsed
        timeout: std::time::Duration,
        /// Last error returned by the RPC client
        last_error: String,
    },
}

impl LaunchError {
    /// Returns the captured child stderr if this error variant carries
    /// one. Used by the retry-on-collision helper to scan for AddrInUse
    /// signatures across both `ProcessFailed` (child exited) and
    /// `LaunchAborted` (indicator-scan tripped) without duplicating the
    /// pattern-match.
    pub fn stderr(&self) -> Option<&str> {
        match self {
            Self::ProcessFailed { stderr, .. } => Some(stderr),
            Self::LaunchAborted { stderr, .. } => Some(stderr),
            Self::RpcReadinessTimeout { .. } => None,
        }
    }
}
