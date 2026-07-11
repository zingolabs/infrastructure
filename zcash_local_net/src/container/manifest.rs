//! The artifact manifest: a consumer-authored description of where
//! each managed artifact (Validator, Indexer, Wallet) comes from.
//!
//! Consumers of this library check a manifest file into their test
//! setup, point the `ZCASH_LOCAL_NET_MANIFEST` environment variable
//! (or a `--manifest` CLI flag) at it, and get the same harness
//! behavior everywhere: CI runs pinned container images, while a
//! developer iterating on zebrad or zaino flips one artifact to a
//! `local` path — the escape hatch — without touching the tests.
//!
//! The format is JSON (this crate already depends on serde_json;
//! adding a TOML parser was rejected to keep the supply-chain surface
//! flat). `zcash-local-net template` prints a starting point.
//!
//! ```json
//! {
//!   "version": 1,
//!   "runtime": "docker",
//!   "validator": { "image": "zfnd/zebra:v6.0.0" },
//!   "indexer": { "local": "/home/dev/zaino/target/release/zainod" },
//!   "wallet": {}
//! }
//! ```
//!
//! Per artifact (`validator`, `indexer`, `wallet`):
//!
//! - `image`: run from this container image reference. Optional
//!   companions: `entrypoint` (defaults to the artifact's binary
//!   name) and `pull` (`"never"` / `"if-missing"` / `"always"`,
//!   default `"if-missing"`).
//! - `local`: run this host binary directly — the escape hatch for
//!   locally built artifacts. Mutually exclusive with `image`.
//! - `{}` or omitted: resolve the binary via `TEST_BINARIES_DIR` /
//!   `PATH`, the historical default.
//!
//! Top-level `runtime` (`"docker"` / `"podman"`) is optional; when
//! omitted and any artifact names an `image`, the runtime is
//! auto-detected at load time (docker first, then podman).

use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::{ArtifactSource, ContainerImage, ContainerRuntime, PullPolicy};
use crate::{
    LocalNet, error::LaunchError, indexer::zainod::Zainod, indexer::zainod::ZainodConfig,
    validator::zebrad::Zebrad, validator::zebrad::ZebradConfig,
};

/// The manifest file name looked for in the working directory when no
/// explicit path is given (CLI convention).
pub const DEFAULT_MANIFEST_FILENAME: &str = "zcash-local-net.json";

/// Environment variable naming the manifest file test suites should
/// load (see [`ArtifactManifest::from_env`]).
pub const MANIFEST_ENV_VAR: &str = "ZCASH_LOCAL_NET_MANIFEST";

/// The only manifest schema version this build understands.
const SUPPORTED_VERSION: u32 = 1;

/// Errors loading or validating an artifact manifest.
#[derive(thiserror::Error, Debug)]
pub enum ManifestError {
    /// The manifest file could not be read.
    #[error("could not read manifest {path}: {io_error}")]
    Unreadable {
        /// Path that was being read.
        path: PathBuf,
        /// Underlying io::Error description.
        io_error: String,
    },
    /// The manifest is not valid JSON, or does not match the schema
    /// (unknown fields are rejected so typos fail loudly).
    #[error("manifest {path:?} did not parse: {detail}")]
    Parse {
        /// Path of the offending file, or `None` when parsing an
        /// in-memory string.
        path: Option<PathBuf>,
        /// serde error description.
        detail: String,
    },
    /// `version` names a schema this build does not understand.
    #[error("unsupported manifest version {found} (this build supports {SUPPORTED_VERSION})")]
    UnsupportedVersion {
        /// The version the manifest declared.
        found: u32,
    },
    /// An artifact entry is self-contradictory.
    #[error("manifest artifact `{artifact}`: {reason}")]
    InvalidArtifact {
        /// Which artifact table (`validator` / `indexer` / `wallet`).
        artifact: &'static str,
        /// What is wrong with it.
        reason: String,
    },
    /// `runtime` names something other than `docker` or `podman`.
    #[error("unknown container runtime {name:?} (expected \"docker\" or \"podman\")")]
    UnknownRuntime {
        /// The runtime string the manifest declared.
        name: String,
    },
    /// The manifest names container images but neither `docker` nor
    /// `podman` is installed.
    #[error(
        "manifest names container images but no container runtime was found \
         (probed `docker --version`, then `podman --version`)"
    )]
    NoContainerRuntime,
}

/// Serde-facing manifest schema. Unknown fields are rejected so a
/// typo'd key fails at load time instead of silently reverting an
/// artifact to its default source.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    version: u32,
    runtime: Option<String>,
    validator: Option<RawArtifact>,
    indexer: Option<RawArtifact>,
    wallet: Option<RawArtifact>,
}

/// Serde-facing artifact entry.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawArtifact {
    image: Option<String>,
    local: Option<PathBuf>,
    entrypoint: Option<String>,
    pull: Option<String>,
}

/// A validated artifact manifest: one resolved [`ArtifactSource`] per
/// managed artifact.
///
/// The `Default` manifest resolves everything as a host process via
/// `TEST_BINARIES_DIR` / `PATH` — byte-for-byte the crate's historical
/// behavior — so `ArtifactManifest::from_env()?.unwrap_or_default()`
/// is a safe drop-in for any test suite.
#[derive(Clone, Debug, Default)]
pub struct ArtifactManifest {
    /// Where the Validator (zebrad) comes from.
    pub validator: ArtifactSource,
    /// Where the Indexer (zainod) comes from.
    pub indexer: ArtifactSource,
    /// Where the Wallet binary (zcash-devtool) comes from.
    pub wallet: ArtifactSource,
}

impl ArtifactManifest {
    /// Load and validate the manifest at `path`.
    pub fn load(path: &Path) -> Result<Self, ManifestError> {
        let text = std::fs::read_to_string(path).map_err(|io_error| ManifestError::Unreadable {
            path: path.to_path_buf(),
            io_error: io_error.to_string(),
        })?;
        Self::parse(&text, Some(path))
    }

    /// Parse and validate a manifest from a JSON string.
    pub fn from_json_str(json: &str) -> Result<Self, ManifestError> {
        Self::parse(json, None)
    }

    /// Load the manifest named by the `ZCASH_LOCAL_NET_MANIFEST`
    /// environment variable, or `Ok(None)` when the variable is not
    /// set. The intended test-suite entry point:
    ///
    /// ```no_run
    /// # async fn f() -> Result<(), Box<dyn std::error::Error>> {
    /// use zcash_local_net::container::manifest::ArtifactManifest;
    ///
    /// let manifest = ArtifactManifest::from_env()?.unwrap_or_default();
    /// let report = manifest.preflight().await;
    /// assert!(report.passed(), "artifact preflight failed:\n{report}");
    /// let net = manifest.launch_local_net().await?;
    /// # Ok(()) }
    /// ```
    pub fn from_env() -> Result<Option<Self>, ManifestError> {
        match std::env::var_os(MANIFEST_ENV_VAR) {
            Some(path) => Self::load(Path::new(&path)).map(Some),
            None => Ok(None),
        }
    }

    fn parse(json: &str, path: Option<&Path>) -> Result<Self, ManifestError> {
        let raw: RawManifest =
            serde_json::from_str(json).map_err(|error| ManifestError::Parse {
                path: path.map(Path::to_path_buf),
                detail: error.to_string(),
            })?;
        if raw.version != SUPPORTED_VERSION {
            return Err(ManifestError::UnsupportedVersion { found: raw.version });
        }

        let declared_runtime = raw
            .runtime
            .as_deref()
            .map(|name| {
                ContainerRuntime::from_manifest_name(name).ok_or_else(|| {
                    ManifestError::UnknownRuntime {
                        name: name.to_string(),
                    }
                })
            })
            .transpose()?;

        // Resolve the runtime once, lazily: probing for docker/podman
        // only happens when some artifact actually names an image.
        let needs_runtime = [&raw.validator, &raw.indexer, &raw.wallet]
            .into_iter()
            .flatten()
            .any(|artifact| artifact.image.is_some());
        let runtime = if needs_runtime {
            Some(match declared_runtime {
                Some(runtime) => runtime,
                None => ContainerRuntime::detect().ok_or(ManifestError::NoContainerRuntime)?,
            })
        } else {
            None
        };

        Ok(Self {
            validator: resolve_artifact("validator", raw.validator, runtime)?,
            indexer: resolve_artifact("indexer", raw.indexer, runtime)?,
            wallet: resolve_artifact("wallet", raw.wallet, runtime)?,
        })
    }

    /// A `ZebradConfig` whose artifact source is this manifest's
    /// `validator`; every other field keeps its default.
    pub fn zebrad_config(&self) -> ZebradConfig {
        ZebradConfig {
            source: self.validator.clone(),
            ..ZebradConfig::default()
        }
    }

    /// A `ZainodConfig` whose artifact source is this manifest's
    /// `indexer`; every other field keeps its default.
    pub fn zainod_config(&self) -> ZainodConfig {
        ZainodConfig {
            source: self.indexer.clone(),
            ..ZainodConfig::default()
        }
    }

    /// The Wallet binary's artifact source, for seeding a wallet
    /// config (e.g. `ZcashDevtoolConfig`'s `source` field) inside a
    /// `LocalNet::launch_wallet` closure.
    pub fn wallet_source(&self) -> ArtifactSource {
        self.wallet.clone()
    }

    /// Launch the core stack (zebrad Validator + zainod Indexer) with
    /// each artifact taken from this manifest and default settings
    /// otherwise. For non-default settings, seed configs via
    /// [`Self::zebrad_config`] / [`Self::zainod_config`], adjust them,
    /// and call [`LocalNet::launch_from_two_configs`] yourself.
    ///
    /// # Errors
    /// Returns `LaunchError` if a process (or its container) fails to
    /// launch. Running [`Self::preflight`] first turns most
    /// environment problems into precise, pre-launch diagnostics.
    pub async fn launch_local_net(&self) -> Result<LocalNet<Zebrad, Zainod>, LaunchError> {
        LocalNet::launch_from_two_configs(self.zebrad_config(), self.zainod_config()).await
    }

    /// An example manifest, printed by `zcash-local-net template`.
    /// Kept parseable by a unit test.
    pub fn template_json() -> &'static str {
        r#"{
  "version": 1,
  "runtime": "docker",
  "validator": {
    "image": "zfnd/zebra:v6.0.0",
    "entrypoint": "zebrad",
    "pull": "if-missing"
  },
  "indexer": {
    "local": "/home/dev/zaino/target/release/zainod"
  },
  "wallet": {}
}
"#
    }
}

/// Validate one artifact entry and resolve it to an [`ArtifactSource`].
fn resolve_artifact(
    artifact: &'static str,
    raw: Option<RawArtifact>,
    runtime: Option<ContainerRuntime>,
) -> Result<ArtifactSource, ManifestError> {
    let raw = raw.unwrap_or_default();
    match (raw.image, raw.local) {
        (Some(_), Some(_)) => Err(ManifestError::InvalidArtifact {
            artifact,
            reason: "`image` and `local` are mutually exclusive — pick one".to_string(),
        }),
        (None, local) => {
            if raw.entrypoint.is_some() || raw.pull.is_some() {
                return Err(ManifestError::InvalidArtifact {
                    artifact,
                    reason: "`entrypoint` and `pull` only apply to `image` artifacts".to_string(),
                });
            }
            Ok(ArtifactSource::HostProcess { binary: local })
        }
        (Some(image), None) => {
            let pull = raw
                .pull
                .as_deref()
                .map(|name| {
                    PullPolicy::from_manifest_name(name).ok_or_else(|| {
                        ManifestError::InvalidArtifact {
                            artifact,
                            reason: format!(
                                "unknown pull policy {name:?} \
                                 (expected \"never\", \"if-missing\" or \"always\")"
                            ),
                        }
                    })
                })
                .transpose()?
                .unwrap_or_default();
            Ok(ArtifactSource::Container(ContainerImage {
                // `needs_runtime` in the caller guarantees Some here.
                runtime: runtime.expect("runtime resolved when any artifact names an image"),
                image,
                entrypoint: raw.entrypoint,
                pull,
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_manifest_resolves_everything_to_host_defaults() {
        let manifest = ArtifactManifest::from_json_str(r#"{ "version": 1 }"#).unwrap();
        for source in [&manifest.validator, &manifest.indexer, &manifest.wallet] {
            assert!(matches!(
                source,
                ArtifactSource::HostProcess { binary: None }
            ));
        }
    }

    #[test]
    fn local_escape_hatch_resolves_to_explicit_binary() {
        let manifest = ArtifactManifest::from_json_str(
            r#"{
                "version": 1,
                "indexer": { "local": "/builds/zainod" }
            }"#,
        )
        .unwrap();
        let ArtifactSource::HostProcess {
            binary: Some(binary),
        } = &manifest.indexer
        else {
            panic!("expected explicit host binary, got {:?}", manifest.indexer);
        };
        assert_eq!(binary, Path::new("/builds/zainod"));
        // The untouched artifacts stay on the historical default.
        assert!(matches!(
            manifest.validator,
            ArtifactSource::HostProcess { binary: None }
        ));
    }

    #[test]
    fn image_artifact_carries_entrypoint_and_pull_policy() {
        let manifest = ArtifactManifest::from_json_str(
            r#"{
                "version": 1,
                "runtime": "podman",
                "validator": {
                    "image": "example.com/zebra:v6.0.0",
                    "entrypoint": "/usr/bin/zebrad",
                    "pull": "never"
                }
            }"#,
        )
        .unwrap();
        let ArtifactSource::Container(image) = &manifest.validator else {
            panic!("expected container source, got {:?}", manifest.validator);
        };
        assert_eq!(image.runtime, ContainerRuntime::Podman);
        assert_eq!(image.image, "example.com/zebra:v6.0.0");
        assert_eq!(image.entrypoint.as_deref(), Some("/usr/bin/zebrad"));
        assert_eq!(image.pull, PullPolicy::Never);
    }

    #[test]
    fn image_and_local_are_mutually_exclusive() {
        let error = ArtifactManifest::from_json_str(
            r#"{
                "version": 1,
                "runtime": "docker",
                "validator": { "image": "zebra", "local": "/builds/zebrad" }
            }"#,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ManifestError::InvalidArtifact {
                artifact: "validator",
                ..
            }
        ));
    }

    #[test]
    fn image_only_fields_are_rejected_on_host_artifacts() {
        let error = ArtifactManifest::from_json_str(
            r#"{
                "version": 1,
                "wallet": { "local": "/builds/zcash-devtool", "pull": "always" }
            }"#,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ManifestError::InvalidArtifact {
                artifact: "wallet",
                ..
            }
        ));
    }

    #[test]
    fn unknown_keys_fail_loudly() {
        let error = ArtifactManifest::from_json_str(
            r#"{ "version": 1, "validatr": { "image": "zebra" } }"#,
        )
        .unwrap_err();
        assert!(matches!(error, ManifestError::Parse { .. }), "{error:?}");
    }

    #[test]
    fn unsupported_version_is_rejected() {
        let error = ArtifactManifest::from_json_str(r#"{ "version": 2 }"#).unwrap_err();
        assert!(matches!(
            error,
            ManifestError::UnsupportedVersion { found: 2 }
        ));
    }

    #[test]
    fn unknown_runtime_is_rejected() {
        let error = ArtifactManifest::from_json_str(
            r#"{ "version": 1, "runtime": "containerd", "validator": { "image": "zebra" } }"#,
        )
        .unwrap_err();
        assert!(matches!(error, ManifestError::UnknownRuntime { name } if name == "containerd"));
    }

    #[test]
    fn unknown_pull_policy_is_rejected() {
        let error = ArtifactManifest::from_json_str(
            r#"{
                "version": 1,
                "runtime": "docker",
                "validator": { "image": "zebra", "pull": "sometimes" }
            }"#,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ManifestError::InvalidArtifact {
                artifact: "validator",
                ..
            }
        ));
    }

    /// A manifest with no `image` artifacts must never probe for a
    /// container runtime — the manifest (and `Default`) must work on
    /// hosts with neither docker nor podman installed. Indirectly
    /// pinned here by resolving a local-only manifest successfully
    /// regardless of what's installed.
    #[test]
    fn local_only_manifest_needs_no_runtime() {
        let manifest = ArtifactManifest::from_json_str(
            r#"{
                "version": 1,
                "validator": { "local": "/builds/zebrad" },
                "indexer": { "local": "/builds/zainod" },
                "wallet": { "local": "/builds/zcash-devtool" }
            }"#,
        )
        .unwrap();
        assert!(!manifest.validator.is_container());
    }

    /// The template must always parse — it declares `runtime`
    /// explicitly, so no docker/podman probe runs and the test is
    /// hermetic on hosts without either installed.
    #[test]
    fn template_parses() {
        let manifest = ArtifactManifest::from_json_str(ArtifactManifest::template_json()).unwrap();
        assert!(manifest.validator.is_container());
        assert!(matches!(
            &manifest.indexer,
            ArtifactSource::HostProcess { binary: Some(_) }
        ));
    }

    #[test]
    fn default_manifest_is_all_host_defaults() {
        let manifest = ArtifactManifest::default();
        for source in [&manifest.validator, &manifest.indexer, &manifest.wallet] {
            assert!(matches!(
                source,
                ArtifactSource::HostProcess { binary: None }
            ));
        }
    }
}
