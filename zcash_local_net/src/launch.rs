use std::{fs::File, io::Read as _, path::PathBuf, process::Child};

use tempfile::TempDir;

use crate::{error::LaunchError, logs, ProcessId};

/// Read the captured additional-log file (if a path was configured)
/// for a final snapshot at error-emission time. Used to populate the
/// `additional_log` field of `LaunchError` variants so the retry
/// helper's signature scan and the test diagnostics see whatever the
/// child wrote there (lightwalletd writes its bind errors to its own
/// log, not to stderr).
fn snapshot_additional_log(path: Option<&PathBuf>) -> Option<String> {
    path.and_then(|p| std::fs::read_to_string(p).ok())
}

/// Spawn the per-process stdout/stderr drainer threads and wait until
/// the process logs indicate the launch has succeeded or failed. Owns
/// the `write_logs` setup so callers do not need to invoke it
/// separately — calling `logs::write_logs` *and* this function would
/// panic on the second `Child::stdout.take()`.
pub(crate) async fn wait(
    process: ProcessId,
    handle: &mut Child,
    logs_dir: &TempDir,
    additional_log_path: Option<PathBuf>,
    success_indicators: &[&str],
    error_indicators: &[&str],
    excluded_errors: &[&str],
) -> Result<(), LaunchError> {
    logs::write_logs(handle, logs_dir);

    let stdout_log_path = logs_dir.path().join(logs::STDOUT_LOG);
    let mut stdout_log = File::open(stdout_log_path).expect("should be able to open log");
    let mut stdout = String::new();

    let stderr_log_path = logs_dir.path().join(logs::STDERR_LOG);
    let mut stderr_log = File::open(stderr_log_path).expect("should be able to open log");
    let mut stderr = String::new();

    let (mut additional_log_file, mut additional_log) = if let Some(log_path) =
        additional_log_path.as_ref()
    {
        let log_file = File::open(log_path).expect("should be able to open log");
        let log = String::new();

        (Some(log_file), Some(log))
    } else {
        (None, None)
    };

    // wait for stdout log entry that indicates daemon is ready
    let interval = std::time::Duration::from_millis(100);
    loop {
        match handle.try_wait() {
            Ok(Some(exit_status)) => {
                stdout_log.read_to_string(&mut stdout).unwrap();
                stderr_log.read_to_string(&mut stderr).unwrap();

                return Err(LaunchError::ProcessFailed {
                    process_name: process.to_string(),
                    exit_status,
                    stdout,
                    stderr,
                    additional_log: snapshot_additional_log(additional_log_path.as_ref()),
                });
            }
            Ok(None) => (),
            Err(e) => {
                panic!("Unexpected Error: {e}")
            }
        }

        stdout_log.read_to_string(&mut stdout).unwrap();
        stderr_log.read_to_string(&mut stderr).unwrap();

        if contains_any(&stdout, success_indicators) || contains_any(&stderr, success_indicators) {
            // launch successful
            break;
        }

        let trimmed_stdout = exclude_errors(&stdout, excluded_errors);
        let trimmed_stderr = exclude_errors(&stderr, excluded_errors);
        if let Some(matched) = first_match(&trimmed_stdout, error_indicators)
            .or_else(|| first_match(&trimmed_stderr, error_indicators))
        {
            tracing::info!("\nSTDOUT:\n{}", stdout);
            if additional_log_file.is_some() {
                let mut log_file = additional_log_file
                    .take()
                    .expect("additional log exists in this scope");
                let mut log = additional_log
                    .take()
                    .expect("additional log exists in this scope");

                log_file.read_to_string(&mut log).unwrap();
                tracing::info!("\nADDITIONAL LOG:\n{}", log);
            }
            tracing::error!("\nSTDERR:\n{}", stderr);
            return Err(LaunchError::LaunchAborted {
                process_name: process.to_string(),
                matched_indicator: matched.to_string(),
                stdout,
                stderr,
                additional_log: snapshot_additional_log(additional_log_path.as_ref()),
            });
        }

        if additional_log_file.is_some() {
            let mut log_file = additional_log_file
                .take()
                .expect("additional log exists in this scope");
            let mut log = additional_log
                .take()
                .expect("additional log exists in this scope");

            log_file.read_to_string(&mut log).unwrap();

            if contains_any(&log, success_indicators) {
                // launch successful
                break;
            }

            let trimmed_log = exclude_errors(&log, excluded_errors);
            if let Some(matched) = first_match(&trimmed_log, error_indicators) {
                tracing::info!("\nSTDOUT:\n{}", stdout);
                tracing::info!("\nADDITIONAL LOG:\n{}", log);
                tracing::error!("\nSTDERR:\n{}", stderr);
                return Err(LaunchError::LaunchAborted {
                    process_name: process.to_string(),
                    matched_indicator: matched.to_string(),
                    stdout,
                    stderr,
                    additional_log: Some(log),
                });
            } else {
                additional_log_file = Some(log_file);
                additional_log = Some(log);
            }
        }

        tokio::time::sleep(interval).await;
    }

    Ok(())
}

/// Post-launch verification: confirm the child both survived its
/// own bind and is the entity accepting TCP connections on the
/// picked port. Closes the failure-mode-#4 hazard where a process
/// logs a "started successfully"-shaped indicator before its bind,
/// then dies on AddrInUse — leaving `wait` already returned-Ok
/// with the picked port stored on a defunct child.
///
/// Two phases:
///
///   - **Phase 1 — fast-crash detection.** Poll `try_wait` at 50ms
///     intervals for 200ms. If the child has exited since `wait`
///     returned, read its captured logs and return `ProcessFailed`.
///     Why this matters: a passive `TcpStream::connect` probe is
///     fooled by *any* listener on the picked port, including the
///     regression test's squatter or a sibling test subprocess that
///     won the kernel-ephemeral race. The squatter would
///     `accept`-queue our connection just like our intended child
///     would — kernel-level handshake completes before any
///     application bytes flow. The only reliable distinction is
///     "did the child we just spawned survive long enough to be the
///     listener?". If the child died on AddrInUse, this loop catches
///     it; the surviving listener (squatter or sibling) is then
///     correctly classified as a failure.
///
///   - **Phase 2 — listener probe.** Once the child has survived the
///     fast-crash budget, do up to 5 × 50ms TCP-connect probes to
///     `127.0.0.1:port`. On success return `Ok(())`. On exhaustion
///     return `ListenerNotResponsive`.
///
/// Total budget: 200ms (phase 1) + up to 250ms (phase 2) = 450ms
/// worst case. Successful launches typically pay only the phase-1
/// budget, since the listener is up by the time phase 2 starts.
///
/// `ListenerNotResponsive` and `ProcessFailed` both feed into
/// `LaunchError::captured_output()`, so the retry helper's
/// signature scan covers this path identically to a normal
/// `ProcessFailed` from `wait`.
pub(crate) async fn probe_listener(
    process: ProcessId,
    handle: &mut Child,
    port: u16,
    logs_dir: &TempDir,
    additional_log_path: Option<&PathBuf>,
) -> Result<(), LaunchError> {
    let read_logs = || {
        let stdout = std::fs::read_to_string(logs_dir.path().join(logs::STDOUT_LOG))
            .unwrap_or_default();
        let stderr = std::fs::read_to_string(logs_dir.path().join(logs::STDERR_LOG))
            .unwrap_or_default();
        let additional_log = snapshot_additional_log(additional_log_path);
        (stdout, stderr, additional_log)
    };

    // Phase 1: fast-crash detection.
    for _ in 0..4 {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        if let Ok(Some(exit_status)) = handle.try_wait() {
            let (stdout, stderr, additional_log) = read_logs();
            return Err(LaunchError::ProcessFailed {
                process_name: process.to_string(),
                exit_status,
                stdout,
                stderr,
                additional_log,
            });
        }
    }

    // Phase 2: HTTP/2 application-layer probe. A bare `TcpStream::connect`
    // is fooled by *any* listener on the picked port — the regression
    // test's squatter `TcpListener`, or a sibling test subprocess that
    // won the kernel-ephemeral race — because TCP handshake completes
    // at the kernel level before any application bytes flow. The squatter
    // accepts our connection into its listen queue but never writes
    // back; a real tonic gRPC server, on the other hand, responds to
    // an HTTP/2 connection preface with its own SETTINGS frame.
    //
    // Concretely required because zaino's `TonicServer::spawn`
    // (`packages/zaino-serve/src/server/grpc.rs`) returns Ok before
    // its inner `server_future.await` does the actual bind, and a
    // bind failure inside that future does not propagate to zaino's
    // main process — the main `serve_task` keeps running with
    // `StatusType::Ready` cached on a defunct gRPC handle. Without
    // an application-layer probe, our launch helper has no way to
    // distinguish "zainod bound the port" from "the squatter is
    // holding the port and zaino's bind silently failed."
    //
    // Up to 3 probes × 200ms timeout = 600ms phase-2 budget. Working
    // tonic responds in milliseconds; only the squatter case pays the
    // full timeout.
    for _ in 0..3 {
        if probe_via_http2("127.0.0.1", port).await {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    let (stdout, stderr, additional_log) = read_logs();
    Err(LaunchError::ListenerNotResponsive {
        process_name: process.to_string(),
        port,
        stdout,
        stderr,
        additional_log,
    })
}

/// Send the HTTP/2 connection preface (RFC 7540 §3.5) plus an empty
/// `SETTINGS` frame to `host:port`, then read 1 byte with a 200ms
/// timeout. Returns `true` iff the peer wrote at least one byte (or
/// closed the connection) within the budget — the canonical signal
/// of a real HTTP/2 server. A bare `TcpListener` with no accept loop
/// accepts the TCP connection but never reads or writes, so the read
/// times out and this returns `false`.
async fn probe_via_http2(host: &str, port: u16) -> bool {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    // Preface (24 bytes) + SETTINGS frame with zero-length payload (9 bytes).
    const PREFACE_AND_SETTINGS: &[u8] =
        b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n\x00\x00\x00\x04\x00\x00\x00\x00\x00";
    let connect = tokio::net::TcpStream::connect((host, port));
    let Ok(Ok(mut stream)) =
        tokio::time::timeout(std::time::Duration::from_millis(200), connect).await
    else {
        return false;
    };
    if stream.write_all(PREFACE_AND_SETTINGS).await.is_err() {
        return false;
    }
    let mut buf = [0u8; 1];
    matches!(
        tokio::time::timeout(std::time::Duration::from_millis(200), stream.read(&mut buf)).await,
        Ok(Ok(_))
    )
}

fn contains_any(log: &str, indicators: &[&str]) -> bool {
    indicators.iter().any(|indicator| log.contains(indicator))
}

/// Returns the first indicator from `indicators` that occurs in `log`,
/// or `None` if none match. Used by `wait` so the resulting
/// `LaunchAborted` error can carry the *exact* indicator that tripped
/// the scan — useful both for human triage and for the retry helper to
/// distinguish a port-collision indicator from any other error mode.
fn first_match<'a>(log: &str, indicators: &'a [&'a str]) -> Option<&'a str> {
    indicators
        .iter()
        .copied()
        .find(|indicator| log.contains(indicator))
}

fn exclude_errors(log: &str, excluded_errors: &[&str]) -> String {
    log.lines()
        .filter(|line| !contains_any(line, excluded_errors))
        .collect::<Vec<&str>>()
        .join("\n")
}

/// Bounded retry-on-port-collision wrapper for validator launches.
///
/// Closes the cross-test-subprocess port-pick race documented in
/// `mod launch_recovers_from_rpc_port_collision` (see
/// `zcash_local_net/tests/integration.rs`). When a launch attempt
/// fails with stderr matching one of the validator's
/// `collision_signatures`, the helper:
///
///   1. Calls `clear_port_pins(&mut config)` so the next attempt picks
///      a fresh ephemeral port instead of the one that just collided.
///      Validators with multiple ports clear all of them — partial
///      clearing risks one of the surviving picks being a port a
///      sibling test subprocess just claimed (the same race we are
///      recovering from).
///   2. Re-runs `attempt(config.clone()).await`.
///   3. Repeats up to `max_attempts`. On exhaustion returns the last
///      error from `attempt`, with the per-attempt count preserved in
///      tracing events so CI logs surface the rate.
///
/// Non-collision errors are returned immediately on the first attempt
/// — retry only fires when the failure is recognizably a port
/// collision, never as a blanket "launches sometimes fail, try again"
/// hack.
///
/// Instrumentation: events are emitted under
/// `target = "zcash_local_net::launch::retry"` at:
///
///   - `info!` per detected collision (one event per retry trigger)
///   - `info!` once on successful recovery (when an attempt > 1
///     succeeds), so a log grep tells you both how often the race
///     fires and how often retry actually rescues
///   - `error!` once on retry exhaustion
///
/// Counts can be derived from the event stream; if/when the rate
/// climbs to where atomic counters are warranted, this is the place
/// to add them.
pub(crate) async fn with_retry_on_collision<C, F, Fut, T, M>(
    process_name: &'static str,
    mut config: C,
    collision_signatures: &[&'static str],
    max_attempts: u32,
    mut clear_port_pins: M,
    mut attempt: F,
) -> Result<T, LaunchError>
where
    C: Clone,
    M: FnMut(&mut C),
    F: FnMut(C) -> Fut,
    Fut: std::future::Future<Output = Result<T, LaunchError>>,
{
    assert!(
        max_attempts >= 1,
        "with_retry_on_collision requires at least one attempt"
    );
    for attempt_n in 1..=max_attempts {
        let err = match attempt(config.clone()).await {
            Ok(t) => {
                if attempt_n > 1 {
                    tracing::info!(
                        target: "zcash_local_net::launch::retry",
                        process = process_name,
                        attempts_used = attempt_n,
                        "validator launched on retry after port-collision recovery"
                    );
                }
                return Ok(t);
            }
            Err(e) => e,
        };

        // A `ListenerNotResponsive` outcome is a collision trigger by
        // construction — `probe_listener` only emits it when, after
        // launch::wait succeeded, the picked port either had a child
        // that crashed (caught earlier as `ProcessFailed`) or has no
        // real HTTP/2 server responding. The most plausible cause is
        // exactly the cross-subprocess port-pick race we are
        // recovering from, so we retry without requiring a stderr
        // signature match (the bind error often lives inside an
        // async task whose Err never reaches stderr — see zaino's
        // `TonicServer::spawn`).
        let captured = err.captured_output();
        let collision_signature = first_match(&captured, collision_signatures);
        let listener_unresponsive = matches!(err, LaunchError::ListenerNotResponsive { .. });
        let trigger = collision_signature
            .map(|s| s.to_string())
            .or_else(|| listener_unresponsive.then(|| "ListenerNotResponsive".to_string()));
        match (trigger, attempt_n == max_attempts) {
            (Some(reason), false) => {
                tracing::info!(
                    target: "zcash_local_net::launch::retry",
                    process = process_name,
                    attempt = attempt_n,
                    reason = %reason,
                    "port collision detected; clearing port pins and retrying"
                );
                clear_port_pins(&mut config);
            }
            (Some(reason), true) => {
                tracing::error!(
                    target: "zcash_local_net::launch::retry",
                    process = process_name,
                    attempts = max_attempts,
                    reason = %reason,
                    "port collision retry exhausted; surfacing last error"
                );
                return Err(err);
            }
            (None, _) => return Err(err),
        }
    }
    unreachable!("with_retry_on_collision loop returns on every iteration")
}
