//! The Zainod executable support struct and associated.

use std::{path::PathBuf, process::Child};

use tempfile::TempDir;

use zingo_consensus::NetworkKind;

use crate::logs::LogsToDir;
use crate::logs::LogsToStdoutAndStderr as _;
use crate::utils::executable_finder::trace_version_and_location;
use crate::{
    ProcessId, config,
    error::{IndexerSyncError, LaunchError},
    indexer::{Indexer, IndexerConfig},
    launch,
    network::{self},
    process::Process,
    utils::executable_finder::pick_command,
};

/// The stdout marker zainod prints for every block its chain index
/// adds, captured verbatim from zainod 0.4.3-ironwood.1 running
/// against a live regtest zebrad (shown here after ANSI stripping):
///
/// ```text
/// Syncing block, height: 4, hash: 3057c360..
/// ```
///
/// This is a log-format contract with the zainod binary. The
/// `indexer_convergence` integration test pins it against the real
/// binary; if zainod's format drifts, that test and
/// [`IndexerSyncError::SyncMarkerDrift`] fire with the offending line
/// rather than letting a convergence wait hang.
const SYNC_MARKER: &str = "Syncing block, ";
/// The field prefix carrying the block height inside a marker line.
const SYNC_HEIGHT_FIELD: &str = "height: ";

/// Remove ANSI escape sequences (CSI `ESC [ … <final>` and two-byte
/// `ESC <c>`) from a log line. zainod colors its logs even on a piped
/// stdout, and the escapes sit between a marker line's words, so
/// substring matching on the raw bytes fails.
fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        // A two-byte escape (`ESC <c>`) is fully consumed by the
        // `next()` call itself; only CSI sequences need the walk to
        // their final byte (0x40–0x7e).
        if chars.next() == Some('[') {
            for follower in chars.by_ref() {
                if ('\u{40}'..='\u{7e}').contains(&follower) {
                    break;
                }
            }
        }
    }
    out
}

/// The height of the last [`SYNC_MARKER`] line in `log_text`, or
/// `None` if no line carries the marker.
fn last_sync_height_in(log_text: &str) -> Result<Option<u32>, IndexerSyncError> {
    let mut last = None;
    for raw_line in log_text.lines() {
        let stripped = strip_ansi(raw_line);
        if stripped.contains(SYNC_MARKER) {
            last = Some(parse_sync_marker_height(&stripped)?);
        }
    }
    Ok(last)
}

/// Parse the height out of an ANSI-stripped line that contains
/// [`SYNC_MARKER`]. Fails loudly with the full line on any deviation
/// from the pinned shape.
fn parse_sync_marker_height(stripped: &str) -> Result<u32, IndexerSyncError> {
    let drift = || IndexerSyncError::SyncMarkerDrift {
        marker: SYNC_MARKER,
        line: stripped.to_string(),
    };
    let (_, after_marker) = stripped.split_once(SYNC_MARKER).ok_or_else(drift)?;
    let (_, after_field) = after_marker
        .split_once(SYNC_HEIGHT_FIELD)
        .ok_or_else(drift)?;
    let digits: &str = after_field
        .split(|c: char| !c.is_ascii_digit())
        .next()
        .unwrap_or("");
    if digits.is_empty() {
        return Err(drift());
    }
    digits.parse::<u32>().map_err(|_| drift())
}

/// Zainod configuration
///
/// If `listen_port` is `None`, a port is allocated from the harness's
/// partitioned below-ephemeral band (see `network::pick_unused_port`).
///
/// The `validator_port` must be specified and the validator process must be running before launching Zainod.
///
/// `network` must match the configured network *kind* of the validator.
#[derive(Clone, Debug)]
pub struct ZainodConfig {
    /// Listen RPC port
    pub listen_port: Option<u16>,
    /// Validator RPC port
    pub validator_port: u16,
    /// Chain cache path
    pub chain_cache: Option<PathBuf>,
    /// Network kind — deliberately without activation heights. The
    /// Indexer must learn heights from the Validator, never from
    /// harness config (ADR 0003); only the kind string reaches the
    /// zainod TOML. Until zingolabs/zaino#1076 ships a zainod that
    /// queries the validator, the binary falls back to its compiled-in
    /// regtest heights and mismatched schedules kill its sync loop
    /// with `InvalidData("Block commitment could not be computed")`.
    pub network: NetworkKind,
}

impl Default for ZainodConfig {
    fn default() -> Self {
        ZainodConfig {
            listen_port: None,
            validator_port: 0,
            chain_cache: None,
            network: NetworkKind::Regtest,
        }
    }
}

impl IndexerConfig for ZainodConfig {
    fn setup_validator_connection<V: crate::validator::Validator>(&mut self, validator: &V) {
        self.validator_port = validator.get_port();
    }

    fn set_listen_port(&mut self, indexer_listen_port: Option<u16>) {
        self.listen_port = indexer_listen_port;
    }
}

/// This struct is used to represent and manage the Zainod process.
#[derive(Debug)]
pub struct Zainod {
    /// Child process handle
    handle: Child,
    /// RPC port
    port: u16,
    /// Logs directory
    logs_dir: TempDir,
    /// Config directory
    config_dir: TempDir,
}

crate::macros::ref_getters!(Zainod {
    /// Child process handle.
    handle: Child,
    /// Config directory.
    config_dir: TempDir,
});

crate::macros::copy_getters!(Zainod {
    /// RPC port.
    port: u16,
});

impl LogsToDir for Zainod {
    fn logs_dir(&self) -> &TempDir {
        &self.logs_dir
    }
}

impl launch::PortPins for ZainodConfig {
    fn pinned_ports(&self) -> Vec<u16> {
        self.listen_port.into_iter().collect()
    }

    fn clear_port_pins(&mut self) {
        // Single-port indexer — clear the only pin so the next
        // attempt's pick calls `network::pick_unused_port(None)` and
        // the allocator walks to a fresh candidate.
        self.listen_port = None;
    }
}

impl Zainod {
    /// The latest chain-index height this zainod has reported in its
    /// stdout log, or `None` if it has not reported one yet (its sync
    /// loop runs on an interval and may not have indexed anything).
    ///
    /// This is the observation channel for Indexer convergence: the
    /// light-client protocol's own height answers on the `fetch`
    /// backend are proxied straight to the validator, so the log is
    /// the only place the binary states how far *it* has synced.
    /// Errors are loud and precise (see [`IndexerSyncError`]); a
    /// drifted log format cannot silently hang a caller.
    pub fn logged_sync_height(&self) -> Result<Option<u32>, IndexerSyncError> {
        last_sync_height_in(&self.read_stdout_log()?)
    }

    /// The final `max_lines` lines of this zainod's stdout log,
    /// ANSI-stripped — diagnostic payload for convergence timeouts.
    pub fn stripped_log_tail(&self, max_lines: usize) -> Result<String, IndexerSyncError> {
        let text = self.read_stdout_log()?;
        let lines: Vec<&str> = text.lines().collect();
        let start = lines.len().saturating_sub(max_lines);
        Ok(lines[start..]
            .iter()
            .map(|line| strip_ansi(line))
            .collect::<Vec<_>>()
            .join("\n"))
    }

    /// Read the whole stdout log. Lossy UTF-8 conversion is safe here:
    /// the file is scanned for an ASCII marker and ASCII digits, and a
    /// replacement character inside a marker line trips
    /// [`IndexerSyncError::SyncMarkerDrift`] loudly instead of
    /// corrupting a height.
    fn read_stdout_log(&self) -> Result<String, IndexerSyncError> {
        let path = self.logs_dir.path().join(crate::logs::STDOUT_LOG);
        match std::fs::read(&path) {
            Ok(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
            Err(io_error) => Err(IndexerSyncError::LogUnreadable {
                path,
                io_error: io_error.to_string(),
            }),
        }
    }

    /// Single launch attempt: pick a port, write the config, spawn
    /// zainod, wait for the readiness indicator. Wrapped by
    /// `Process::launch` in a bounded retry-on-port-collision loop
    /// (see `launch::with_retry_on_collision`); each retry calls this
    /// fresh with a config whose port pin has been cleared so the pick
    /// re-rolls via `network::pick_unused_port`.
    async fn launch_once(config: ZainodConfig) -> Result<Self, LaunchError> {
        let logs_dir = tempfile::tempdir().unwrap();
        let data_dir = tempfile::tempdir().unwrap();

        let port = network::pick_unused_port(config.listen_port);
        let config_dir = tempfile::tempdir().unwrap();

        let cache_dir = if let Some(cache) = config.chain_cache.clone() {
            cache
        } else {
            data_dir.path().to_path_buf()
        };

        let config_file_path = config::write_zainod_config(
            config_dir.path(),
            cache_dir,
            port,
            config.validator_port,
            config.network,
        )
        .unwrap();

        let executable_name = "zainod";
        trace_version_and_location(executable_name, "--version");
        let mut command = pick_command(executable_name, false);
        command.args([
            "start",
            "--config",
            config_file_path.to_str().expect("should be valid UTF-8"),
        ]);

        let mut handle = launch::spawn_and_wait(
            ProcessId::Zainod,
            &mut command,
            &logs_dir,
            None,
            &["Zaino Indexer started successfully."],
            &["Error:"],
            &[],
        )
        .await?;

        // Verify the gRPC listener is actually accepting connections.
        // Closes failure mode #4: if Zaino logs "started successfully"
        // before completing the gRPC bind, AddrInUse on a squatted
        // port would otherwise let `launch::wait` return Ok with the
        // picked port stored on a defunct child. The probe's phase-1
        // `try_wait` polling catches the child crashing on AddrInUse
        // before its phase-2 TCP probe is fooled by the squatter's
        // accept queue.
        launch::probe_listener(ProcessId::Zainod, &mut handle, port, &logs_dir, None).await?;

        Ok(Zainod {
            handle,
            port,
            logs_dir,
            config_dir,
        })
    }
}

impl Process for Zainod {
    const PROCESS: ProcessId = ProcessId::Zainod;

    type Config = ZainodConfig;

    async fn launch(config: Self::Config) -> Result<Self, LaunchError> {
        // Zaino's gRPC server (tonic/tower) surfaces AddrInUse via
        // libc-shaped strings. The indexer regression test in
        // `tests/integration.rs` is the source of truth for which
        // strings the build emits today; if a future Zaino change
        // produces something else, the test trips UNEXPECTED FAILURE
        // MODE before silently classifying the failure here.
        const COLLISION_SIGNATURES: &[&str] = &[
            "address already in use",
            "Address already in use",
            "AddrInUse",
        ];

        launch::with_retry_on_collision(
            "zainod",
            config,
            COLLISION_SIGNATURES,
            launch::MAX_LAUNCH_ATTEMPTS,
            Self::launch_once,
        )
        .await
    }

    fn stop(&mut self) {
        self.handle.kill().expect("zainod couldn't be killed");
    }

    fn print_all(&self) {
        self.print_stdout();
        self.print_stderr();
    }
}

impl Indexer for Zainod {
    fn listen_port(&self) -> u16 {
        self.port
    }
}

crate::macros::impl_stop_on_drop!(Zainod);

#[cfg(test)]
mod tests {
    use super::*;

    /// A `Syncing block` line captured verbatim (ANSI escapes included)
    /// from zainod 0.4.3-ironwood.1 against a live regtest zebrad. The
    /// escapes sit between the marker's words, which is why the parser
    /// strips before matching.
    const CAPTURED_SYNC_LINE: &str = "  \u{1b}[2m07:06:34.491\u{1b}[0m \u{1b}[32m INFO\u{1b}[0m \
\u{1b}[1;32mzaino_state::chain_index::non_finalised_state\u{1b}[0m\u{1b}[32m: \
\u{1b}[32mSyncing block, \u{1b}[1;32mheight\u{1b}[0m\u{1b}[32m: 1, \
\u{1b}[1;32mhash\u{1b}[0m\u{1b}[32m: 0658f4ff..\u{1b}[0m";

    #[test]
    fn captured_sync_line_parses() {
        let stripped = strip_ansi(CAPTURED_SYNC_LINE);
        assert!(
            stripped.contains("Syncing block, height: 1, hash: 0658f4ff.."),
            "ANSI stripping did not recover the plain line: {stripped:?}"
        );
        assert_eq!(parse_sync_marker_height(&stripped).unwrap(), 1);
    }

    #[test]
    fn last_marker_line_wins() {
        let log = format!(
            "{CAPTURED_SYNC_LINE}\n    at packages/zaino-state/src/x.rs:515\n\
             plain noise line\n\
             Syncing block, height: 42, hash: da2b9284..\n"
        );
        assert_eq!(last_sync_height_in(&log).unwrap(), Some(42));
    }

    #[test]
    fn no_marker_means_no_height() {
        assert_eq!(
            last_sync_height_in("Zaino Indexer started successfully.\n").unwrap(),
            None
        );
    }

    #[test]
    fn drifted_marker_line_fails_loud() {
        let drifted = "Syncing block, tallness: 7";
        let error = last_sync_height_in(drifted).unwrap_err();
        assert!(
            matches!(&error, IndexerSyncError::SyncMarkerDrift { line, .. } if line == drifted),
            "expected SyncMarkerDrift carrying the offending line, got {error:?}"
        );
    }
}
