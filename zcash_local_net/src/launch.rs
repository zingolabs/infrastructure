use std::{fs::File, io::Read as _, path::PathBuf, process::Child};

use tempfile::TempDir;

use crate::{error::LaunchError, logs, ProcessId};

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

    let (mut additional_log_file, mut additional_log) = if let Some(log_path) = additional_log_path
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

        let collision = err
            .stderr()
            .and_then(|s| first_match(s, collision_signatures));
        match (collision, attempt_n == max_attempts) {
            (Some(sig), false) => {
                tracing::info!(
                    target: "zcash_local_net::launch::retry",
                    process = process_name,
                    attempt = attempt_n,
                    signature = sig,
                    "port collision detected; clearing port pins and retrying"
                );
                clear_port_pins(&mut config);
            }
            (Some(sig), true) => {
                tracing::error!(
                    target: "zcash_local_net::launch::retry",
                    process = process_name,
                    attempts = max_attempts,
                    signature = sig,
                    "port collision retry exhausted; surfacing last error"
                );
                return Err(err);
            }
            (None, _) => return Err(err),
        }
    }
    unreachable!("with_retry_on_collision loop returns on every iteration")
}
