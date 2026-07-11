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
//! The format is TOML — the configuration language everything else in
//! this ecosystem already speaks (`zebrad.toml`, `zindexer.toml`,
//! `Cargo.toml`), comments included. `zcash-local-net template`
//! prints a starting point.
//!
//! ```toml
//! version = 1
//! runtime = "docker"
//!
//! [validator]
//! image = "zfnd/zebra:v6.0.0"
//! track = "zfnd/zebra:latest"
//!
//! [indexer]
//! local = "/home/dev/zaino/target/release/zainod"
//!
//! [wallet]
//! ```
//!
//! ## Intent vs pin: bumping references explicitly
//!
//! The manifest plays both the role of *intent* and of *lockfile*
//! (think `Cargo.toml` and `Cargo.lock` in one file): `track` records
//! the floating reference a consumer prefers to follow (`…:latest`, a
//! release-channel tag, …), while `image` records the pinned
//! reference every run actually uses. Nothing at run time ever
//! consults the registry for "what does the tag mean now" — moving
//! the pin is a deliberate act:
//!
//! ```sh
//! zcash-local-net update --manifest ci-artifacts.toml
//! ```
//!
//! resolves each artifact's tracked reference against the registry,
//! rewrites `image` to the digest-pinned result
//! (`repo:tag@sha256:…`), and reports every bump — leaving a
//! reviewable diff in version control. Artifacts without `track` are
//! bumped by re-resolving their `image` tag; digest-only images
//! without `track` are left untouched (there is no floating
//! preference to follow). See [`crate::container::update`] for the
//! library entry point.
//!
//! Per artifact (`validator`, `indexer`, `wallet`):
//!
//! - `image`: run from this container image reference — the binary's
//!   *actual published image* (e.g. Zebra's official `zfnd/zebra`),
//!   one image per artifact. The reference **must be pinned**: either
//!   digest-pinned (`repo@sha256:…`, the strongest form — immune to
//!   tag reassignment) or carrying an explicit non-`latest` tag.
//!   Untagged and `:latest` references are rejected at load time,
//!   because a floating reference silently changes what a test run
//!   means when the registry moves the tag. Optional companions:
//!   `entrypoint` (defaults to the artifact's binary name) and `pull`
//!   (`"never"` / `"if-missing"` / `"always"`, default
//!   `"if-missing"`).
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
pub const DEFAULT_MANIFEST_FILENAME: &str = "zcash-local-net.toml";

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
    /// The manifest is not valid TOML, or does not match the schema
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
    /// An artifact has a `track` preference but no pinned `image` yet.
    /// The manifest cannot be launched from until the pin is minted —
    /// deliberately, by running the update command.
    #[error(
        "manifest artifact `{artifact}` tracks {track:?} but has no pinned `image` yet; \
         run `zcash-local-net update` to resolve the tracked reference and write the pin"
    )]
    UnresolvedTrack {
        /// Which artifact table (`validator` / `indexer` / `wallet`).
        artifact: &'static str,
        /// The floating reference the artifact tracks.
        track: String,
    },
    /// An `image` reference is not pinned (no tag/digest, or the
    /// floating `latest` tag). Pinning is mandatory: container
    /// artifacts exist to make test runs reproducible, and a floating
    /// reference silently changes meaning when the registry moves it.
    #[error(
        "manifest artifact `{artifact}`: image {image:?} is not pinned ({reason}); \
         pin it with a digest (`repo@sha256:…`) or an explicit non-`latest` tag"
    )]
    UnpinnedImage {
        /// Which artifact table (`validator` / `indexer` / `wallet`).
        artifact: &'static str,
        /// The offending image reference.
        image: String,
        /// What makes the reference floating.
        reason: &'static str,
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
///
/// Also `Serialize`, because `zcash-local-net update` rewrites the
/// manifest file in place; struct fields serialize in declaration
/// order, so this doubles as the manifest's canonical field order.
#[derive(Clone, Debug, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawManifest {
    pub(crate) version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) runtime: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) validator: Option<RawArtifact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) indexer: Option<RawArtifact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) wallet: Option<RawArtifact>,
}

/// Serde-facing artifact entry.
#[derive(Clone, Debug, Default, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawArtifact {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) track: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) local: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) entrypoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) pull: Option<String>,
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

    /// Parse and validate a manifest from a TOML string.
    pub fn from_toml_str(toml_text: &str) -> Result<Self, ManifestError> {
        Self::parse(toml_text, None)
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

    fn parse(toml_text: &str, path: Option<&Path>) -> Result<Self, ManifestError> {
        Self::from_raw(Self::parse_raw(toml_text, path)?)
    }

    /// Parse the serde-facing schema and check the version, without
    /// resolving artifacts. `zcash-local-net update` operates on this
    /// representation: a manifest whose `track`-only artifacts are not
    /// yet launchable must still be readable for updating.
    pub(crate) fn parse_raw(
        toml_text: &str,
        path: Option<&Path>,
    ) -> Result<RawManifest, ManifestError> {
        let raw: RawManifest = toml::from_str(toml_text).map_err(|error| ManifestError::Parse {
            path: path.map(Path::to_path_buf),
            detail: error.to_string(),
        })?;
        if raw.version != SUPPORTED_VERSION {
            return Err(ManifestError::UnsupportedVersion { found: raw.version });
        }
        Ok(raw)
    }

    /// Validate the raw schema and resolve every artifact source.
    pub(crate) fn from_raw(raw: RawManifest) -> Result<Self, ManifestError> {
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
    pub fn template_toml() -> &'static str {
        r#"# zcash_local_net artifact manifest.
# Validate with `zcash-local-net preflight`; bump image pins with
# `zcash-local-net update`. Point tests at it via ZCASH_LOCAL_NET_MANIFEST.
version = 1

# "docker" or "podman"; omit to auto-detect (docker first).
runtime = "docker"

[validator]
# The binary's actual published image. Must be pinned: a digest
# (`repo@sha256:...`) or an explicit non-`latest` tag.
image = "zfnd/zebra:v6.0.0"
# Optional floating reference to follow; `zcash-local-net update`
# resolves it and rewrites `image` to the digest-pinned result.
track = "zfnd/zebra:latest"
# Optional; defaults to the artifact's binary name ("zebrad").
entrypoint = "zebrad"
# "never" | "if-missing" (default) | "always"
pull = "if-missing"

[indexer]
# Escape hatch: a locally built binary runs as a host process.
local = "/home/dev/zaino/target/release/zainod"

# An empty (or omitted) table resolves the binary via
# TEST_BINARIES_DIR / PATH, the historical default.
[wallet]
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
    if let Some(track) = &raw.track {
        if raw.local.is_some() {
            return Err(ManifestError::InvalidArtifact {
                artifact,
                reason: "`track` only applies to `image` artifacts, not `local` ones".to_string(),
            });
        }
        if track.contains('@') {
            return Err(ManifestError::InvalidArtifact {
                artifact,
                reason: format!(
                    "`track` must be a floating reference (a tag to follow), \
                     but {track:?} is digest-pinned — there is nothing to follow"
                ),
            });
        }
        if raw.image.is_none() {
            return Err(ManifestError::UnresolvedTrack {
                artifact,
                track: track.clone(),
            });
        }
    }
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
            if let Err(reason) = require_pinned(&image) {
                return Err(ManifestError::UnpinnedImage {
                    artifact,
                    image,
                    reason,
                });
            }
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

/// `Ok(())` when `image` is a pinned reference: digest-pinned
/// (`…@sha256:…`) or carrying an explicit tag other than `latest`.
/// `Err` names what makes it floating.
///
/// The tag is looked for after the last `/`, so a registry port
/// (`registry:5000/zebra`) is not mistaken for a tag.
fn require_pinned(image: &str) -> Result<(), &'static str> {
    if image.contains('@') {
        // Digest-pinned: immutable by construction.
        return Ok(());
    }
    let last_component = image.rsplit('/').next().unwrap_or(image);
    match last_component.split_once(':') {
        None => Err("it has no tag or digest"),
        Some((_, "latest")) => Err("`latest` is a floating tag"),
        Some((_, "")) => Err("its tag is empty"),
        Some((_, _tag)) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_manifest_resolves_everything_to_host_defaults() {
        let manifest = ArtifactManifest::from_toml_str("version = 1\n").unwrap();
        for source in [&manifest.validator, &manifest.indexer, &manifest.wallet] {
            assert!(matches!(
                source,
                ArtifactSource::HostProcess { binary: None }
            ));
        }
    }

    #[test]
    fn local_escape_hatch_resolves_to_explicit_binary() {
        let manifest = ArtifactManifest::from_toml_str(
            r#"
                version = 1

                [indexer]
                local = "/builds/zainod"
            "#,
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
        let manifest = ArtifactManifest::from_toml_str(
            r#"
                version = 1
                runtime = "podman"

                [validator]
                image = "example.com/zebra:v6.0.0"
                entrypoint = "/usr/bin/zebrad"
                pull = "never"
            "#,
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
        let error = ArtifactManifest::from_toml_str(
            r#"
                version = 1
                runtime = "docker"

                [validator]
                image = "zebra"
                local = "/builds/zebrad"
            "#,
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
        let error = ArtifactManifest::from_toml_str(
            r#"
                version = 1

                [wallet]
                local = "/builds/zcash-devtool"
                pull = "always"
            "#,
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
    fn track_is_rejected_on_local_artifacts() {
        let error = ArtifactManifest::from_toml_str(
            r#"
                version = 1

                [indexer]
                local = "/builds/zainod"
                track = "zainod:latest"
            "#,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ManifestError::InvalidArtifact {
                artifact: "indexer",
                ..
            }
        ));
    }

    #[test]
    fn digest_pinned_track_is_rejected() {
        let error = ArtifactManifest::from_toml_str(
            r#"
                version = 1
                runtime = "docker"

                [validator]
                image = "zfnd/zebra:v6.0.0"
                track = "zfnd/zebra@sha256:aaaa"
            "#,
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
    fn track_without_image_is_not_launchable() {
        let error = ArtifactManifest::from_toml_str(
            r#"
                version = 1
                runtime = "docker"

                [validator]
                track = "zfnd/zebra:latest"
            "#,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ManifestError::UnresolvedTrack {
                artifact: "validator",
                ..
            }
        ));
    }

    #[test]
    fn unknown_keys_fail_loudly() {
        let error = ArtifactManifest::from_toml_str(
            r#"
                version = 1

                [validatr]
                image = "zebra:v1"
            "#,
        )
        .unwrap_err();
        assert!(matches!(error, ManifestError::Parse { .. }), "{error:?}");
    }

    #[test]
    fn unsupported_version_is_rejected() {
        let error = ArtifactManifest::from_toml_str("version = 2\n").unwrap_err();
        assert!(matches!(
            error,
            ManifestError::UnsupportedVersion { found: 2 }
        ));
    }

    #[test]
    fn unknown_runtime_is_rejected() {
        let error = ArtifactManifest::from_toml_str(
            r#"
                version = 1
                runtime = "containerd"

                [validator]
                image = "zebra:v1"
            "#,
        )
        .unwrap_err();
        assert!(matches!(error, ManifestError::UnknownRuntime { name } if name == "containerd"));
    }

    #[test]
    fn unknown_pull_policy_is_rejected() {
        let error = ArtifactManifest::from_toml_str(
            r#"
                version = 1
                runtime = "docker"

                [validator]
                image = "zebra:v1"
                pull = "sometimes"
            "#,
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

    /// Unpinned image references must be rejected at load time, with
    /// the offending reference and the reason in the error.
    #[test]
    fn unpinned_image_references_are_rejected() {
        for (image, expected_reason_fragment) in [
            ("zebra", "no tag or digest"),
            ("zfnd/zebra:latest", "floating tag"),
            ("zfnd/zebra:", "tag is empty"),
            // A registry port is not a tag.
            ("registry:5000/zebra", "no tag or digest"),
        ] {
            let error = ArtifactManifest::from_toml_str(&format!(
                "version = 1\nruntime = \"docker\"\n\n[validator]\nimage = \"{image}\"\n",
            ))
            .unwrap_err();
            let ManifestError::UnpinnedImage {
                artifact: "validator",
                image: reported,
                reason,
            } = &error
            else {
                panic!("expected UnpinnedImage for {image:?}, got {error:?}");
            };
            assert_eq!(reported, image);
            assert!(
                reason.contains(expected_reason_fragment),
                "{image:?}: reason {reason:?} should mention {expected_reason_fragment:?}"
            );
        }
    }

    /// Pinned references — explicit tags and digests, with or without
    /// a ported registry — must pass.
    #[test]
    fn pinned_image_references_are_accepted() {
        for image in [
            "zfnd/zebra:v6.0.0",
            "registry:5000/zebra:v6.0.0",
            "zfnd/zebra@sha256:0000000000000000000000000000000000000000000000000000000000000000",
            "zfnd/zebra:v6.0.0@sha256:0000000000000000000000000000000000000000000000000000000000000000",
            "zln-test-daemon:local",
        ] {
            let manifest = ArtifactManifest::from_toml_str(&format!(
                "version = 1\nruntime = \"docker\"\n\n[validator]\nimage = \"{image}\"\n",
            ))
            .unwrap_or_else(|error| panic!("{image:?} should be accepted, got {error:?}"));
            assert!(manifest.validator.is_container());
        }
    }

    /// A manifest with no `image` artifacts must never probe for a
    /// container runtime — the manifest (and `Default`) must work on
    /// hosts with neither docker nor podman installed. Indirectly
    /// pinned here by resolving a local-only manifest successfully
    /// regardless of what's installed.
    #[test]
    fn local_only_manifest_needs_no_runtime() {
        let manifest = ArtifactManifest::from_toml_str(
            r#"
                version = 1

                [validator]
                local = "/builds/zebrad"

                [indexer]
                local = "/builds/zainod"

                [wallet]
                local = "/builds/zcash-devtool"
            "#,
        )
        .unwrap();
        assert!(!manifest.validator.is_container());
    }

    /// The template must always parse — it declares `runtime`
    /// explicitly, so no docker/podman probe runs and the test is
    /// hermetic on hosts without either installed.
    #[test]
    fn template_parses() {
        let manifest = ArtifactManifest::from_toml_str(ArtifactManifest::template_toml()).unwrap();
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
