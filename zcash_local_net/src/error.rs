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
