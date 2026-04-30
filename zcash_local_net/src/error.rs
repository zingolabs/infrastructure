//! Crate level error module

/// Errors associated with launching processes
#[derive(thiserror::Error, Debug, Clone)]
pub enum LaunchError {
    /// Process failed during launch
    #[error(
        "{process_name} failed during launch.\nExit status: {exit_status}\nStdout: {stdout}\nStderr: {stderr}\nAdditional log: {additional_log:?}"
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
        /// Additional log (e.g., lightwalletd's own log file) when the
        /// process writes its bind errors there instead of stderr.
        /// `None` for processes that do not configure an additional log.
        additional_log: Option<String>,
    },
    /// `launch::wait`'s indicator-scan matched an error string in the
    /// child's stdout/stderr before the child exited. Replaces the
    /// previous unconditional `panic!` at `launch.rs:88` / `:111` so
    /// callers (in particular, the bounded retry-on-collision wrapper)
    /// can react to the failure as a typed error rather than catching
    /// a panic.
    #[error(
        "{process_name} launch aborted: child stderr/stdout matched an error indicator before exit.\nMatched indicator: {matched_indicator:?}\nStdout: {stdout}\nStderr: {stderr}\nAdditional log: {additional_log:?}"
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
        /// Additional log content if `launch::wait` was reading one.
        additional_log: Option<String>,
    },
    /// `launch::wait` saw a success indicator, but a follow-up
    /// TCP-connect probe to the picked listen port failed within the
    /// post-launch verification budget. Closes the failure-mode-#4
    /// hazard where a process logs "started successfully" *before* its
    /// bind, then dies on AddrInUse — leaving `launch::wait` already
    /// returned-Ok with the picked port stored on a defunct child.
    /// Retry treats this the same as `ProcessFailed` for collision
    /// detection.
    #[error(
        "{process_name} listener at 127.0.0.1:{port} not responsive after launch::wait succeeded.\nStdout: {stdout}\nStderr: {stderr}\nAdditional log: {additional_log:?}"
    )]
    ListenerNotResponsive {
        /// Process name
        process_name: String,
        /// Port that the harness picked but is not actually accepting connections
        port: u16,
        /// Captured stdout up to the probe failure
        stdout: String,
        /// Captured stderr up to the probe failure
        stderr: String,
        /// Additional log content if applicable
        additional_log: Option<String>,
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
    /// All captured child output for this error — stdout + stderr +
    /// the additional log when present, concatenated. Used by the
    /// retry-on-collision helper to scan for AddrInUse signatures
    /// across whichever channel the binary writes them to (zebrad's
    /// tracing goes to stdout; lightwalletd's bind errors go to its
    /// additional log file; zcashd writes to stderr). Empty string for
    /// variants that don't carry captured output.
    pub fn captured_output(&self) -> String {
        match self {
            Self::ProcessFailed {
                stdout,
                stderr,
                additional_log,
                ..
            }
            | Self::LaunchAborted {
                stdout,
                stderr,
                additional_log,
                ..
            }
            | Self::ListenerNotResponsive {
                stdout,
                stderr,
                additional_log,
                ..
            } => {
                let extra_len = additional_log.as_deref().map_or(0, str::len);
                let mut combined =
                    String::with_capacity(stdout.len() + stderr.len() + extra_len + 2);
                combined.push_str(stdout);
                combined.push('\n');
                combined.push_str(stderr);
                if let Some(log) = additional_log {
                    combined.push('\n');
                    combined.push_str(log);
                }
                combined
            }
            Self::RpcReadinessTimeout { .. } => String::new(),
        }
    }
}
