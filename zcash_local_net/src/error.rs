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
    /// The pre-launch capability probe ran the binary and observed
    /// that the requested CLI flag is rejected (binary exits
    /// non-zero from `<binary> <flag> -version`). Surfaces the
    /// missing-capability before any state is created so callers
    /// see a clear, descriptive failure instead of a deep failure
    /// downstream of the actual launch.
    #[cfg(feature = "legacy-stack")]
    #[error(
        "{process_name} binary does not accept `{capability}`.\n{hint}\nProbe stderr: {stderr}"
    )]
    UnsupportedZcashdCapability {
        /// Process name (always `zcashd` today; field for forward
        /// compatibility with future per-binary probes).
        process_name: String,
        /// The CLI flag that was probed
        capability: &'static str,
        /// Captured stderr from the probe invocation
        stderr: String,
        /// Human-readable remediation hint (fork URL, opt-out flag,
        /// etc.)
        hint: String,
    },
    /// The pre-launch capability probe failed to spawn the binary
    /// at all (PATH/permission/etc.) — distinct from
    /// `UnsupportedZcashdCapability` where the binary ran but
    /// rejected the flag.
    #[cfg(feature = "legacy-stack")]
    #[error("{process_name} capability probe for `{capability}` failed to spawn: {io_error}")]
    CapabilityProbeFailed {
        /// Process name
        process_name: String,
        /// The CLI flag that was being probed
        capability: &'static str,
        /// Underlying io::Error / spawn-failure description
        io_error: String,
    },
}

/// Errors associated with driving wallet client operations
/// (see [`crate::wallet`]). Shared by every [`crate::wallet::Wallet`]
/// implementation, so messages name the operation, not one binary.
#[derive(thiserror::Error, Debug, Clone)]
pub enum WalletError {
    /// The client binary could not be spawned at all
    /// (PATH/`TEST_BINARIES_DIR`/permission problems).
    #[error("wallet {operation} failed to spawn: {io_error}")]
    SpawnFailed {
        /// The wallet operation being attempted
        operation: &'static str,
        /// Underlying io::Error description
        io_error: String,
    },
    /// Writing to the child's stdin failed (used by `init`, which
    /// receives the mnemonic on stdin).
    #[error("wallet {operation}: writing to child stdin failed: {io_error}")]
    StdinWriteFailed {
        /// The wallet operation being attempted
        operation: &'static str,
        /// Underlying io::Error description
        io_error: String,
    },
    /// The operation subprocess exited non-zero.
    #[error(
        "wallet {operation} failed.\nExit status: {exit_status}\nStdout: {stdout}\nStderr: {stderr}"
    )]
    OperationFailed {
        /// The wallet operation being attempted
        operation: &'static str,
        /// Exit status of the subprocess
        exit_status: std::process::ExitStatus,
        /// Captured stdout
        stdout: String,
        /// Captured stderr
        stderr: String,
    },
    /// The operation subprocess exited zero but its stdout did not
    /// match the expected shape (txid line, balance lines, …). This is
    /// the contract-drift tripwire: it fires when the client binary's
    /// output format changes out from under the harness's parsers.
    #[error(
        "wallet {operation} succeeded but its output could not be parsed: {reason}\nStdout: {stdout}"
    )]
    UnexpectedOutput {
        /// The wallet operation being attempted
        operation: &'static str,
        /// What the parser was looking for and didn't find
        reason: String,
        /// Captured stdout that failed to parse
        stdout: String,
    },
}

/// Errors from observing Indexer convergence — the harness reading the
/// Indexer's log to learn how far its chain index has synced (see
/// `LocalNet::await_indexer_convergence`). Every variant is loud and
/// precise by design: the observation channel is a log-format contract
/// with the zainod binary, and a drifted contract must fail with the
/// offending evidence, never hang or silently pass.
#[derive(thiserror::Error, Debug, Clone)]
pub enum IndexerSyncError {
    /// Mining failed before the convergence wait began.
    #[error("mining failed before the convergence wait: {io_error}")]
    Mining {
        /// Underlying io::Error description from the validator's
        /// block-generation call.
        io_error: String,
    },
    /// The Indexer's stdout log could not be read at all.
    #[error("could not read the indexer log at {path}: {io_error}")]
    LogUnreadable {
        /// Path of the log file the harness tried to read.
        path: std::path::PathBuf,
        /// Underlying io::Error description.
        io_error: String,
    },
    /// A log line matched the sync marker but its height field did not
    /// parse. This is the contract-drift tripwire: it fires when the
    /// zainod binary's log format changes out from under the harness's
    /// parser (contract pinned against zainod 0.4.3-ironwood.1 by the
    /// `zainod_converges_to_validator_tip_after_generate_blocks` integration test).
    #[error(
        "an indexer log line matched the sync marker {marker:?} but its height did not parse: \
         expected \"{marker}height: <digits>\" after ANSI stripping, got {line:?} — \
         the zainod log contract has drifted"
    )]
    SyncMarkerDrift {
        /// The marker the line matched.
        marker: &'static str,
        /// The full line, after ANSI stripping, that failed to parse.
        line: String,
    },
    /// The Indexer never reported the target height within the timeout.
    #[error(
        "indexer did not converge to height {target} within {waited_secs}s; \
         last height it logged: {last_observed:?}.\nIndexer log tail:\n{log_tail}"
    )]
    ConvergenceTimeout {
        /// The validator tip height the wait was for.
        target: u32,
        /// The last height the indexer had logged when the wait gave
        /// up, or `None` if it never logged one.
        last_observed: Option<u32>,
        /// How long the wait ran before giving up.
        waited_secs: u64,
        /// The final lines of the indexer's log (ANSI-stripped), for
        /// diagnosing why it stalled.
        log_tail: String,
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
            #[cfg(feature = "legacy-stack")]
            Self::UnsupportedZcashdCapability { .. } | Self::CapabilityProbeFailed { .. } => {
                String::new()
            }
        }
    }
}
