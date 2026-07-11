//! Containerized artifacts: run the managed processes from container
//! images instead of host binaries.
//!
//! Every process this crate manages (the zebrad Validator, the zainod
//! Indexer, the zcash-devtool Wallet) is spawned through one choke
//! point: an [`ArtifactSource`] on its launch config. The default,
//! [`ArtifactSource::HostProcess`], is the classic behavior — resolve
//! the binary via `TEST_BINARIES_DIR` / `PATH` (or an explicit path,
//! the escape hatch for locally built binaries). The alternative,
//! [`ArtifactSource::Container`], runs the same binary out of a
//! container image via the `docker` or `podman` CLI.
//!
//! Container mode deliberately changes *nothing else*: the container
//! is launched in the **foreground** (`run --rm`, no `--detach`), so
//! its stdout/stderr stream through the runtime client — the child
//! process the harness already manages — and every existing mechanism
//! (readiness-indicator scanning, launch-log endpoint discovery,
//! port-collision retry, front proxies, Indexer-convergence parsing)
//! operates on containers exactly as it does on host processes.
//! Containers join the **host network namespace** (`--network host`)
//! and the harness's temporary config/data directories are
//! bind-mounted at identical paths, so the generated config files are
//! valid verbatim inside the container and all loopback wiring (the
//! front proxies, the Indexer→Validator connection) is unchanged.
//! Host networking makes this mode Linux-first: on Docker Desktop
//! (macOS/Windows) loopback listeners inside the VM are not reachable
//! from the host.
//!
//! Consumers describe which artifacts come from where with an
//! [`manifest::ArtifactManifest`] — a small JSON file that can be
//! validated ahead of a test run with [`manifest::ArtifactManifest::preflight`]
//! (also exposed by the `zcash-local-net` CLI as `zcash-local-net
//! preflight`).
//!
//! ## Lifecycle and cleanup
//!
//! A foreground `run --rm` container is removed by the runtime when
//! its process exits. Stopping a managed process in container mode
//! additionally force-removes the container by name (`rm --force`),
//! because killing the *client* process alone would leave the
//! container running. Containers are named
//! `zcash-local-net-<binary>-<pid>-<n>`, so stragglers from a killed
//! harness process can be cleaned up with e.g.
//! `docker ps -aq --filter name=zcash-local-net- | xargs docker rm -f`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::utils::executable_finder::pick_command;

pub mod manifest;
pub mod preflight;
pub mod update;

/// The container runtime CLI used to run [`ArtifactSource::Container`]
/// artifacts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ContainerRuntime {
    /// The `docker` CLI.
    Docker,
    /// The `podman` CLI.
    Podman,
}

impl ContainerRuntime {
    /// The CLI executable name for this runtime.
    pub fn cli_name(&self) -> &'static str {
        match self {
            Self::Docker => "docker",
            Self::Podman => "podman",
        }
    }

    /// Detect an available container runtime by probing `docker
    /// --version` then `podman --version`. Returns the first CLI that
    /// runs successfully, or `None` when neither is installed. Client
    /// presence only — daemon reachability is a preflight check
    /// ([`manifest::ArtifactManifest::preflight`]).
    pub fn detect() -> Option<Self> {
        [Self::Docker, Self::Podman]
            .into_iter()
            .find(|runtime| runtime.client_available())
    }

    /// Whether `<cli> --version` runs and exits successfully.
    pub(crate) fn client_available(&self) -> bool {
        Command::new(self.cli_name())
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success())
    }

    /// Parse the manifest's `runtime` string.
    pub(crate) fn from_manifest_name(name: &str) -> Option<Self> {
        match name {
            "docker" => Some(Self::Docker),
            "podman" => Some(Self::Podman),
            _ => None,
        }
    }
}

impl std::fmt::Display for ContainerRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.cli_name())
    }
}

/// When a container image is (re)fetched from its registry.
///
/// Enforced both at preflight (an explicit `pull` step) and at launch
/// (mapped to the runtime's `run --pull=<policy>` flag, so a launch
/// can never silently pull an image the policy forbids).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PullPolicy {
    /// Never pull: the image must already be present locally. The
    /// right policy for hermetic CI caches and for images built
    /// locally from a source checkout.
    Never,
    /// Pull only when the image is not present locally (the default).
    #[default]
    IfMissing,
    /// Always pull, refreshing a floating tag like `latest`.
    Always,
}

impl PullPolicy {
    /// The value for the runtime's `run --pull=<value>` flag.
    fn as_run_flag_value(&self) -> &'static str {
        match self {
            Self::Never => "never",
            Self::IfMissing => "missing",
            Self::Always => "always",
        }
    }

    /// Parse the manifest's `pull` string.
    pub(crate) fn from_manifest_name(name: &str) -> Option<Self> {
        match name {
            "never" => Some(Self::Never),
            "if-missing" => Some(Self::IfMissing),
            "always" => Some(Self::Always),
            _ => None,
        }
    }
}

/// A container image an artifact runs from.
#[derive(Clone, Debug)]
pub struct ContainerImage {
    /// The runtime CLI that runs this image.
    pub runtime: ContainerRuntime,
    /// Image reference (`repository[:tag][@digest]`), passed verbatim
    /// to the runtime — the binary's *actual published image* (e.g.
    /// Zebra's official `zfnd/zebra`). Pin it: prefer a digest
    /// (`repo@sha256:…`), or at least an explicit non-`latest` tag.
    /// Manifest loading enforces this
    /// ([`manifest::ManifestError::UnpinnedImage`]); constructing this
    /// struct directly bypasses that check, so programmatic callers
    /// carry the reproducibility responsibility themselves.
    pub image: String,
    /// Entrypoint override. `None` uses the artifact's executable name
    /// (`zebrad`, `zainod`, `zcash-devtool`), which deliberately
    /// bypasses image entrypoint scripts — the harness writes complete
    /// config files itself and wants the bare binary.
    pub entrypoint: Option<String>,
    /// When the image is (re)fetched from its registry.
    pub pull: PullPolicy,
}

/// Where an artifact (a managed binary) comes from.
///
/// Every launch config carries one of these; the default is the
/// classic host-process resolution, so existing callers are
/// unaffected.
#[derive(Clone, Debug)]
pub enum ArtifactSource {
    /// Run the binary as a host process.
    ///
    /// `binary: None` resolves via the `TEST_BINARIES_DIR` environment
    /// variable, falling back to `PATH` — exactly the historical
    /// behavior. `binary: Some(path)` runs that path directly: the
    /// **escape hatch** for a Validator or Indexer built locally from
    /// source (e.g. `Some("…/zebra/target/release/zebrad".into())`).
    HostProcess {
        /// Explicit path to the binary, or `None` to resolve via
        /// `TEST_BINARIES_DIR` / `PATH`.
        binary: Option<PathBuf>,
    },
    /// Run the binary from a container image (foreground, host
    /// network, harness dirs bind-mounted — see the module docs).
    Container(ContainerImage),
}

impl Default for ArtifactSource {
    /// The historical behavior: resolve the binary via
    /// `TEST_BINARIES_DIR` / `PATH`.
    fn default() -> Self {
        Self::HostProcess { binary: None }
    }
}

impl ArtifactSource {
    /// Convenience constructor for the local-build escape hatch.
    pub fn local_binary(path: impl Into<PathBuf>) -> Self {
        Self::HostProcess {
            binary: Some(path.into()),
        }
    }

    /// Whether this source runs from a container image.
    pub fn is_container(&self) -> bool {
        matches!(self, Self::Container(_))
    }

    /// Mint the uniquely-named container instance a daemon launch in
    /// container mode will run as (and be force-removed by name
    /// through). `None` in host-process mode, and for the one-shot
    /// wallet operations, which need no name.
    pub(crate) fn new_instance(&self, executable_name: &str) -> Option<ContainerInstance> {
        match self {
            Self::HostProcess { .. } => None,
            Self::Container(image) => Some(ContainerInstance {
                runtime: image.runtime,
                name: unique_container_name(executable_name),
            }),
        }
    }

    /// Build the command that spawns this artifact — the single choke
    /// point through which every managed process is launched. The
    /// caller appends the binary's own arguments afterwards, identical
    /// in both modes.
    pub(crate) fn command(&self, spec: &LaunchSpec<'_>) -> Command {
        match self {
            Self::HostProcess { binary: None } => pick_command(spec.executable_name, false),
            Self::HostProcess { binary: Some(path) } => Command::new(path),
            Self::Container(image) => container_run_command(image, spec),
        }
    }
}

/// How an artifact spawn should be shaped, beyond the binary's own
/// arguments.
pub(crate) struct LaunchSpec<'a> {
    /// The binary's executable name — the host-mode lookup key and the
    /// container-mode default entrypoint.
    pub executable_name: &'static str,
    /// Container name for daemon launches (minted by
    /// [`ArtifactSource::new_instance`]); `None` for one-shot
    /// invocations, which rely on `--rm` alone.
    pub container_name: Option<&'a str>,
    /// Host directories bind-mounted into the container at identical
    /// paths, so harness-written config files are valid verbatim
    /// inside. Ignored in host mode.
    pub mounts: &'a [&'a Path],
    /// Keep stdin open (`--interactive`) so the caller can pipe to it
    /// (the wallet's `init` receives its mnemonic on stdin). Ignored
    /// in host mode — a host `Command` pipes stdin without ceremony.
    pub interactive: bool,
}

impl<'a> LaunchSpec<'a> {
    /// A spec with no mounts, no name, and no stdin — version traces
    /// and other one-shot probes.
    pub(crate) fn bare(executable_name: &'static str) -> Self {
        Self {
            executable_name,
            container_name: None,
            mounts: &[],
            interactive: false,
        }
    }
}

/// Assemble `docker|podman run --rm [--name N] --pull=P --network host
/// [--user uid:gid] [-i] --entrypoint E -v p:p… IMAGE`.
///
/// `--network host` keeps every loopback assumption of the process
/// harness true inside the container (see the module docs); `--user`
/// runs the containerized binary as the harness's own uid/gid so it
/// can write the bind-mounted temp dirs and the files it creates there
/// are owned by the harness.
fn container_run_command(image: &ContainerImage, spec: &LaunchSpec<'_>) -> Command {
    let mut command = Command::new(image.runtime.cli_name());
    command.args(["run", "--rm"]);
    if let Some(name) = spec.container_name {
        command.args(["--name", name]);
    }
    command.arg(format!("--pull={}", image.pull.as_run_flag_value()));
    command.args(["--network", "host"]);
    if let Some((uid, gid)) = current_uid_gid() {
        command.arg("--user").arg(format!("{uid}:{gid}"));
    }
    if spec.interactive {
        command.arg("--interactive");
    }
    command
        .arg("--entrypoint")
        .arg(image.entrypoint.as_deref().unwrap_or(spec.executable_name));
    for mount in spec.mounts {
        command
            .arg("--volume")
            .arg(format!("{}:{}", mount.display(), mount.display()));
    }
    command.arg(&image.image);
    command
}

/// The harness process's `(uid, gid)`, learned from the ownership of a
/// freshly created temp dir (no libc dependency). Cached for the
/// process lifetime. `None` on non-Unix targets or if the probe fails,
/// in which case `--user` is omitted and the image's default user
/// applies.
fn current_uid_gid() -> Option<(u32, u32)> {
    static CACHED: OnceLock<Option<(u32, u32)>> = OnceLock::new();
    *CACHED.get_or_init(|| {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            let probe = tempfile::tempdir().ok()?;
            let metadata = std::fs::metadata(probe.path()).ok()?;
            Some((metadata.uid(), metadata.gid()))
        }
        #[cfg(not(unix))]
        {
            None
        }
    })
}

/// Monotonic counter distinguishing containers launched by one harness
/// process.
static CONTAINER_COUNTER: AtomicU64 = AtomicU64::new(0);

/// `zcash-local-net-<binary>-<pid>-<n>`: unique per launch within a
/// host, greppable for cleanup of stragglers.
fn unique_container_name(executable_name: &str) -> String {
    format!(
        "zcash-local-net-{executable_name}-{}-{}",
        std::process::id(),
        CONTAINER_COUNTER.fetch_add(1, Ordering::Relaxed),
    )
}

/// A named container a daemon launch is running as. Held by the
/// process wrapper so `stop` can force-remove the container — killing
/// the foreground runtime *client* alone would leave the container
/// running.
#[derive(Debug)]
pub(crate) struct ContainerInstance {
    runtime: ContainerRuntime,
    name: String,
}

impl ContainerInstance {
    /// The container's `--name`.
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// `docker|podman rm --force <name>`, blocking and best-effort.
    /// A "no such container" failure is the goal state (the container
    /// already exited and `--rm` reaped it) and is only traced at
    /// debug level; any other failure is surfaced as a warning — the
    /// caller is tearing down and has nothing better to do with the
    /// error than report it.
    pub(crate) fn force_remove(&self) {
        match Command::new(self.runtime.cli_name())
            .args(["rm", "--force", &self.name])
            .output()
        {
            Ok(output) if output.status.success() => {}
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if stderr.to_ascii_lowercase().contains("no such container") {
                    tracing::debug!(container = %self.name, "container already removed");
                } else {
                    tracing::warn!(
                        container = %self.name,
                        %stderr,
                        "failed to force-remove container"
                    );
                }
            }
            Err(io_error) => {
                tracing::warn!(
                    container = %self.name,
                    %io_error,
                    "failed to run the container runtime CLI for force-remove"
                );
            }
        }
    }
}

/// Trace the artifact's version and provenance, in whichever mode the
/// source runs it. The container branch is the source-aware analogue
/// of [`crate::utils::executable_finder::trace_version_and_location`];
/// failures are traced, not fatal — the subsequent launch produces the
/// authoritative error with full context.
pub(crate) fn trace_version(
    source: &ArtifactSource,
    executable_name: &'static str,
    version_flag: &str,
) {
    match source {
        ArtifactSource::HostProcess { binary: None } => {
            crate::utils::executable_finder::trace_version_and_location(
                executable_name,
                version_flag,
            );
        }
        _ => {
            let mut command = source.command(&LaunchSpec::bare(executable_name));
            command.arg(version_flag);
            match command.output() {
                Ok(version) => {
                    tracing::info!("$ {executable_name} {version_flag} ({source:?})\n {version:?}");
                }
                Err(io_error) => tracing::warn!(
                    %io_error,
                    "could not probe {executable_name} {version_flag} via {source:?}"
                ),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_image(entrypoint: Option<&str>, pull: PullPolicy) -> ContainerImage {
        ContainerImage {
            runtime: ContainerRuntime::Docker,
            image: "example.com/zebra:v6.0.0".to_string(),
            entrypoint: entrypoint.map(str::to_string),
            pull,
        }
    }

    fn rendered_args(command: &Command) -> Vec<String> {
        command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    /// Pins the container-run CLI contract this module emits: flag
    /// set, flag order, identical-path volume syntax, entrypoint
    /// default. Consumers script cleanup against the `--name` prefix
    /// and the harness relies on `--rm` + foreground semantics, so
    /// drift here is behavior drift.
    #[test]
    fn daemon_run_command_shape() {
        let mounts: &[&Path] = &[Path::new("/tmp/cfg"), Path::new("/tmp/data")];
        let source = ArtifactSource::Container(test_image(None, PullPolicy::IfMissing));
        let command = source.command(&LaunchSpec {
            executable_name: "zebrad",
            container_name: Some("zcash-local-net-zebrad-1-0"),
            mounts,
            interactive: false,
        });

        assert_eq!(command.get_program(), "docker");
        let args = rendered_args(&command);
        let mut expected = vec![
            "run".to_string(),
            "--rm".to_string(),
            "--name".to_string(),
            "zcash-local-net-zebrad-1-0".to_string(),
            "--pull=missing".to_string(),
            "--network".to_string(),
            "host".to_string(),
        ];
        if let Some((uid, gid)) = current_uid_gid() {
            expected.push("--user".to_string());
            expected.push(format!("{uid}:{gid}"));
        }
        expected.extend(
            [
                "--entrypoint",
                "zebrad",
                "--volume",
                "/tmp/cfg:/tmp/cfg",
                "--volume",
                "/tmp/data:/tmp/data",
                "example.com/zebra:v6.0.0",
            ]
            .map(str::to_string),
        );
        assert_eq!(args, expected);
    }

    /// One-shot (wallet-op) shape: no `--name`, `--interactive` when
    /// stdin will be piped, explicit entrypoint override respected,
    /// and `--pull=never` enforced at run time.
    #[test]
    fn one_shot_interactive_run_command_shape() {
        let mounts: &[&Path] = &[Path::new("/tmp/wallet")];
        let source = ArtifactSource::Container(test_image(
            Some("/usr/local/bin/zcash-devtool"),
            PullPolicy::Never,
        ));
        let command = source.command(&LaunchSpec {
            executable_name: "zcash-devtool",
            container_name: None,
            mounts,
            interactive: true,
        });

        let args = rendered_args(&command);
        assert!(!args.contains(&"--name".to_string()));
        assert!(args.contains(&"--interactive".to_string()));
        assert!(args.contains(&"--pull=never".to_string()));
        let entrypoint_index = args
            .iter()
            .position(|arg| arg == "--entrypoint")
            .expect("entrypoint flag present");
        assert_eq!(args[entrypoint_index + 1], "/usr/local/bin/zcash-devtool");
        assert_eq!(args.last().unwrap(), "example.com/zebra:v6.0.0");
    }

    /// The escape hatch: an explicit binary path is run directly,
    /// bypassing `TEST_BINARIES_DIR` / `PATH` resolution entirely.
    #[test]
    fn explicit_local_binary_is_run_directly() {
        let source = ArtifactSource::local_binary("/home/dev/zebra/target/release/zebrad");
        let command = source.command(&LaunchSpec::bare("zebrad"));
        assert_eq!(
            command.get_program(),
            "/home/dev/zebra/target/release/zebrad"
        );
        assert_eq!(command.get_args().count(), 0);
    }

    /// The default source is the historical host-process resolution.
    #[test]
    fn default_source_is_host_process_resolution() {
        assert!(matches!(
            ArtifactSource::default(),
            ArtifactSource::HostProcess { binary: None }
        ));
    }

    #[test]
    fn container_names_are_unique_and_greppable() {
        let a = unique_container_name("zebrad");
        let b = unique_container_name("zebrad");
        assert_ne!(a, b);
        assert!(a.starts_with("zcash-local-net-zebrad-"));
    }
}
