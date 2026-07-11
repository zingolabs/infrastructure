//! The Zainod executable support struct and associated.

use std::{path::PathBuf, process::Child};

use tempfile::TempDir;

use zingo_consensus::NetworkKind;

use crate::logs::LogsToDir;
use crate::logs::LogsToStdoutAndStderr as _;
use crate::{
    ProcessId, config,
    error::{IndexerSyncError, LaunchError},
    indexer::{Indexer, IndexerConfig},
    launch,
    network::{self},
    process::Process,
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
/// `zainod_converges_to_validator_tip_after_generate_blocks` integration test pins it against the real
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
/// If `listen_port` is `None`, the **raw** gRPC listener's port is
/// allocated from the harness's partitioned below-ephemeral band (see
/// `network::pick_unused_port`); `Some(N)` pins it. Either way the raw
/// listener is never published: every accessor returns the address of
/// the gRPC front proxy (see [`Zainod::port`]). The raw listener keeps
/// a picked port instead of binding port 0 because zainod binds port 0
/// happily but never logs the kernel-assigned address (verified
/// against zainod 0.4.3-ironwood.1), so the harness could not discover
/// where to point the front.
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
    /// Observer registered on the gRPC front proxy before the backend
    /// starts, so it sees every byte of the backend's networked
    /// lifetime — the launch-time listener probe included. `None`
    /// (the default) is a passthrough front with no observer.
    pub grpc_front_observer: Option<std::sync::Arc<dyn crate::front::FrontObserver>>,
    /// Where the zainod binary comes from: host-process resolution
    /// (the default), an explicit local build, or a container image.
    /// See [`crate::container::ArtifactSource`]; typically seeded from
    /// an [`crate::container::manifest::ArtifactManifest`].
    pub source: crate::container::ArtifactSource,
}

impl Default for ZainodConfig {
    fn default() -> Self {
        ZainodConfig {
            listen_port: None,
            validator_port: 0,
            chain_cache: None,
            network: NetworkKind::Regtest,
            grpc_front_observer: None,
            source: crate::container::ArtifactSource::default(),
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
    /// Child process handle. In container mode this is the foreground
    /// runtime client (`docker|podman run`), through which the
    /// container's stdio streams.
    handle: Child,
    /// The named container zainod runs as when the artifact source is
    /// a container image; `None` in host-process mode. Held so `stop`
    /// can force-remove the container — killing the client alone would
    /// leave it running.
    container: Option<crate::container::ContainerInstance>,
    /// gRPC front proxy — the canonical public endpoint of the gRPC
    /// listener, bound before the process started.
    grpc_front: crate::front::Front,
    /// Raw gRPC listener address. What the front dials; never
    /// published.
    raw_grpc_listen_addr: std::net::SocketAddr,
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

impl Zainod {
    /// The public gRPC port. **This is the port of the front proxy**,
    /// not of zainod's own listener: the raw endpoint is a private
    /// detail of launch plumbing and is never published.
    pub fn port(&self) -> u16 {
        self.grpc_front.public_port()
    }
}

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

    /// Read the whole stdout log through the backend abstraction's
    /// log-access surface, converting a read failure into the loud
    /// convergence error.
    fn read_stdout_log(&self) -> Result<String, IndexerSyncError> {
        crate::backend::Backend::log_text(self).map_err(|io_error| {
            IndexerSyncError::LogUnreadable {
                path: self.logs_dir.path().join(crate::logs::STDOUT_LOG),
                io_error: io_error.to_string(),
            }
        })
    }

    /// Single launch attempt: bind the gRPC front, pick a raw port,
    /// write the config, spawn zainod, wait for the readiness
    /// indicator, point the front at the raw gRPC endpoint, then probe
    /// the listener *through the front*. Wrapped by `Process::launch`
    /// in a bounded retry-on-port-collision loop (see
    /// `launch::with_retry_on_collision`); each retry calls this
    /// fresh with a config whose port pin has been cleared so the pick
    /// re-rolls via `network::pick_unused_port`.
    async fn launch_once(config: ZainodConfig) -> Result<Self, LaunchError> {
        let logs_dir = tempfile::tempdir().unwrap();
        let data_dir = tempfile::tempdir().unwrap();

        // The front binds before the backend starts: its public
        // address exists for the backend's entire networked lifetime,
        // and the OS assigns it atomically on 127.0.0.1:0 — no
        // check-then-bind race exists on the public surface.
        let grpc_front = crate::front::Front::bind(config.grpc_front_observer.clone())
            .expect("the gRPC front should bind on 127.0.0.1:0");

        // The raw listener keeps a picked port: zainod binds port 0
        // happily but never logs the kernel-assigned address, so a
        // kernel-assigned raw port would be undiscoverable (see the
        // `ZainodConfig` docs).
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
        crate::container::trace_version(&config.source, executable_name, "--version");
        // In container mode, mount the config dir (and the caller's
        // chain cache, when one is set) at identical paths so the
        // config file just written is valid verbatim inside the
        // container. The ephemeral cache dir is deliberately *not*
        // mounted: its `TempDir` is deleted when this function
        // returns even in host mode, and zainod (re)creates the
        // configured path itself — inside the container's own
        // filesystem, reaped with the container by `--rm`.
        let container = config.source.new_instance(executable_name);
        let mut mounts: Vec<&std::path::Path> = vec![config_dir.path()];
        if let Some(cache) = config.chain_cache.as_ref() {
            mounts.push(cache.as_path());
        }
        let mut command = config.source.command(&crate::container::LaunchSpec {
            executable_name,
            container_name: container.as_ref().map(|c| c.name()),
            mounts: &mounts,
            interactive: false,
        });
        command.args([
            "start",
            "--config",
            config_file_path.to_str().expect("should be valid UTF-8"),
        ]);

        let spawned = launch::spawn_and_wait(
            ProcessId::Zainod,
            &mut command,
            &logs_dir,
            None,
            &["Zaino Indexer started successfully."],
            &["Error:"],
            &[],
        )
        .await;
        let handle = match spawned {
            Ok(handle) => handle,
            Err(error) => {
                // A failed container launch may leave the container
                // running even though `launch::wait` gave up on it;
                // don't leak it past the failed launch.
                if let Some(container) = &container {
                    container.force_remove();
                }
                return Err(error);
            }
        };

        let mut zainod = Zainod {
            handle,
            container,
            grpc_front,
            raw_grpc_listen_addr: std::net::SocketAddr::from(([127, 0, 0, 1], port)),
            logs_dir,
            config_dir,
        };

        // Point the front at the raw gRPC endpoint, discovered through
        // the backend abstraction: connections the front has been
        // holding proceed from here.
        zainod
            .grpc_front
            .point_at(&zainod, ZAINOD_GRPC_LISTENER_INDEX);

        // Verify the gRPC listener is actually accepting connections —
        // probed through the front, the same public address every
        // client uses. Closes failure mode #4: if Zaino logs "started
        // successfully" before completing the gRPC bind, AddrInUse on
        // a squatted port would otherwise let `launch::wait` return Ok
        // with the picked port stored on a defunct child. The probe's
        // phase-1 `try_wait` polling catches the child crashing on
        // AddrInUse before its phase-2 TCP probe is fooled by the
        // squatter's accept queue.
        let front_port = zainod.port();
        launch::probe_listener(
            ProcessId::Zainod,
            &mut zainod.handle,
            front_port,
            &zainod.logs_dir,
            None,
        )
        .await?;

        Ok(zainod)
    }
}

/// Index of the gRPC endpoint in [`Zainod`]'s declared listener order
/// (`crate::backend::Backend::listener_endpoints`). The gRPC listener
/// is the only one zainod exposes, so it is the only entry.
const ZAINOD_GRPC_LISTENER_INDEX: usize = 0;

impl crate::backend::Backend for Zainod {
    fn log_text(&self) -> std::io::Result<String> {
        // Lossy UTF-8 conversion is safe here: the text is scanned for
        // ASCII markers and ASCII digits, and a replacement character
        // inside a marker line trips the callers' drift tripwires
        // loudly instead of corrupting a parse.
        let path = self.logs_dir.path().join(crate::logs::STDOUT_LOG);
        Ok(String::from_utf8_lossy(&std::fs::read(path)?).into_owned())
    }

    fn listener_endpoints(&self) -> Vec<std::net::SocketAddr> {
        vec![self.raw_grpc_listen_addr]
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
        // Container mode: the handle is only the runtime client;
        // killing it would orphan the container, so remove the
        // container first (which also ends the client).
        if let Some(container) = &self.container {
            container.force_remove();
        }
        match self.handle.kill() {
            Ok(()) => {}
            // `kill` returns `InvalidInput` when the child has already
            // exited and been reaped — e.g. by `probe_listener`'s
            // `try_wait` on a failed launch, after which the assembled
            // `Zainod` is dropped. An already-stopped process is what
            // stop() wants, not a panic.
            Err(e) if e.kind() == std::io::ErrorKind::InvalidInput => {}
            Err(e) => panic!("zainod couldn't be killed: {e}"),
        }
    }

    fn print_all(&self) {
        self.print_stdout();
        self.print_stderr();
    }
}

impl Indexer for Zainod {
    /// The public gRPC port — the front proxy's port, never the raw
    /// listener's (see [`Zainod::port`]).
    fn listen_port(&self) -> u16 {
        self.port()
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
