//! Explicitly bump a manifest's image pins to what their tracked
//! references currently denote.
//!
//! Run-time code never consults the registry about what a floating
//! tag means — pins only move through this module (or its CLI face,
//! `zcash-local-net update`). For each container artifact the update:
//!
//! 1. determines the **tracked reference** — the artifact's `track`
//!    field when set, else its `image` with any digest stripped (i.e.
//!    the tag it was pinned from). A digest-only `image` with no
//!    `track` carries no floating preference and is left untouched.
//! 2. resolves that reference against the registry (`pull`, then read
//!    the repo digest);
//! 3. rewrites `image` to `<tracked>@<digest>` — tag preserved for
//!    readability, digest authoritative — and writes the manifest
//!    back in its canonical formatting.
//!
//! The result is a reviewable version-control diff per bump, exactly
//! like a lockfile update.

use std::path::{Path, PathBuf};

use super::ContainerRuntime;
use super::manifest::{ArtifactManifest, ManifestError};

/// The outcome of updating one container artifact.
#[derive(Clone, Debug)]
pub struct ImageBump {
    /// Which artifact (`validator` / `indexer` / `wallet`).
    pub artifact: &'static str,
    /// The floating reference that was resolved.
    pub tracked: String,
    /// The pinned `image` before the update.
    pub previous: String,
    /// The pinned `image` written by the update.
    pub pinned: String,
}

impl ImageBump {
    /// Whether the update actually moved the pin.
    pub fn changed(&self) -> bool {
        self.previous != self.pinned
    }
}

/// Errors from [`update_manifest_file`].
#[derive(thiserror::Error, Debug)]
pub enum UpdateError {
    /// The manifest could not be read, parsed, or — after bumping —
    /// no longer validated.
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    /// An `only` filter named something other than
    /// `validator` / `indexer` / `wallet`.
    #[error("unknown artifact {name:?} (expected \"validator\", \"indexer\" or \"wallet\")")]
    UnknownArtifact {
        /// The unrecognized artifact name.
        name: String,
    },
    /// Resolving a tracked reference against the registry failed.
    #[error("artifact `{artifact}`: could not resolve {reference:?}: {detail}")]
    Resolve {
        /// Which artifact was being updated.
        artifact: &'static str,
        /// The tracked reference that failed to resolve.
        reference: String,
        /// What went wrong (runtime CLI stderr or spawn failure).
        detail: String,
    },
    /// Writing the updated manifest back failed.
    #[error("could not write updated manifest {path}: {io_error}")]
    Write {
        /// The manifest path.
        path: PathBuf,
        /// Underlying io::Error description.
        io_error: String,
    },
}

/// Resolve every (selected) container artifact's tracked reference and
/// rewrite the manifest's `image` pins in place. `only` restricts the
/// update to the named artifacts (`"validator"` / `"indexer"` /
/// `"wallet"`); empty means all. Returns one [`ImageBump`] per
/// artifact that has a floating preference to follow — including
/// unchanged ones, so callers can report "already current".
///
/// The file is rewritten in the manifest's canonical formatting
/// (stable field order, two-space indent). Host-process artifacts and
/// digest-only images without `track` are untouched and produce no
/// bump entry.
pub async fn update_manifest_file(
    path: &Path,
    only: &[&str],
) -> Result<Vec<ImageBump>, UpdateError> {
    update_with_resolver(path, only, resolve_via_registry).await
}

/// [`update_manifest_file`] with the registry interaction injected —
/// the seam the unit tests use. `resolver(runtime, reference)` returns
/// the digest (`sha256:…`) the reference currently denotes.
async fn update_with_resolver<F>(
    path: &Path,
    only: &[&str],
    resolver: F,
) -> Result<Vec<ImageBump>, UpdateError>
where
    F: AsyncFn(ContainerRuntime, &str) -> Result<String, String>,
{
    const ARTIFACTS: [&str; 3] = ["validator", "indexer", "wallet"];
    for name in only {
        if !ARTIFACTS.contains(name) {
            return Err(UpdateError::UnknownArtifact {
                name: (*name).to_string(),
            });
        }
    }
    let selected = |name: &str| only.is_empty() || only.contains(&name);

    let text = std::fs::read_to_string(path).map_err(|io_error| ManifestError::Unreadable {
        path: path.to_path_buf(),
        io_error: io_error.to_string(),
    })?;
    let mut raw = ArtifactManifest::parse_raw(&text, Some(path))?;

    // The update needs a concrete runtime to resolve through, even
    // though the artifacts may not all be launchable yet: resolve the
    // declared runtime the same way `from_raw` does.
    let runtime = match raw.runtime.as_deref() {
        Some(name) => ContainerRuntime::from_manifest_name(name).ok_or_else(|| {
            ManifestError::UnknownRuntime {
                name: name.to_string(),
            }
        })?,
        None => ContainerRuntime::detect().ok_or(ManifestError::NoContainerRuntime)?,
    };

    let mut bumps = Vec::new();
    for (artifact, entry) in [
        ("validator", raw.validator.as_mut()),
        ("indexer", raw.indexer.as_mut()),
        ("wallet", raw.wallet.as_mut()),
    ] {
        let Some(entry) = entry else { continue };
        if !selected(artifact) {
            continue;
        }
        let Some(tracked) = tracked_reference(entry.track.as_deref(), entry.image.as_deref())
        else {
            continue;
        };
        let digest = resolver(runtime, &tracked)
            .await
            .map_err(|detail| UpdateError::Resolve {
                artifact,
                reference: tracked.clone(),
                detail,
            })?;
        let pinned = format!("{tracked}@{digest}");
        bumps.push(ImageBump {
            artifact,
            tracked,
            previous: entry.image.clone().unwrap_or_default(),
            pinned: pinned.clone(),
        });
        entry.image = Some(pinned);
    }

    // Self-check: the bumped manifest must be valid and launchable
    // before it replaces the file.
    ArtifactManifest::from_raw(raw.clone())?;

    let mut rendered =
        serde_json::to_string_pretty(&raw).expect("the manifest schema serializes infallibly");
    rendered.push('\n');
    std::fs::write(path, rendered).map_err(|io_error| UpdateError::Write {
        path: path.to_path_buf(),
        io_error: io_error.to_string(),
    })?;

    Ok(bumps)
}

/// The floating reference an artifact follows, or `None` when it has
/// no container image or no floating preference (digest-only `image`
/// without `track`).
fn tracked_reference(track: Option<&str>, image: Option<&str>) -> Option<String> {
    if let Some(track) = track {
        return Some(track.to_string());
    }
    let image = image?;
    // Strip the digest to recover the tag the image was pinned from.
    let base = image.split('@').next().unwrap_or(image);
    let last_component = base.rsplit('/').next().unwrap_or(base);
    // Digest-only pin (`repo@sha256:…`): no tag, nothing to follow.
    last_component.contains(':').then(|| base.to_string())
}

/// The live resolver: `pull` the reference (that is the whole point of
/// an update), then read the repo digest it now denotes. Prefers the
/// `RepoDigests` entry whose repository matches the reference; a
/// locally built image with no repo digest cannot be resolved and
/// errs, since a pin that no registry serves is not reproducible
/// elsewhere.
async fn resolve_via_registry(
    runtime: ContainerRuntime,
    reference: &str,
) -> Result<String, String> {
    let cli = runtime.cli_name();
    let pull = tokio::process::Command::new(cli)
        .args(["pull", reference])
        .output()
        .await
        .map_err(|io_error| format!("`{cli} pull` failed to spawn: {io_error}"))?;
    if !pull.status.success() {
        return Err(format!(
            "`{cli} pull {reference}` exited {}: {}",
            pull.status,
            String::from_utf8_lossy(&pull.stderr).trim()
        ));
    }

    let inspect = tokio::process::Command::new(cli)
        .args([
            "image",
            "inspect",
            "--format",
            "{{json .RepoDigests}}",
            reference,
        ])
        .output()
        .await
        .map_err(|io_error| format!("`{cli} image inspect` failed to spawn: {io_error}"))?;
    if !inspect.status.success() {
        return Err(format!(
            "`{cli} image inspect {reference}` exited {}: {}",
            inspect.status,
            String::from_utf8_lossy(&inspect.stderr).trim()
        ));
    }
    let digests: Vec<String> = serde_json::from_slice(&inspect.stdout)
        .map_err(|error| format!("unparseable RepoDigests output: {error}"))?;
    let repository = reference
        .split('@')
        .next()
        .map(|base| {
            let last = base.rsplit('/').next().unwrap_or(base);
            match last.split_once(':') {
                Some((name, _tag)) => &base[..base.len() - last.len() + name.len()],
                None => base,
            }
        })
        .unwrap_or(reference);
    digests
        .iter()
        .find(|entry| entry.split('@').next() == Some(repository))
        .or_else(|| digests.first())
        .and_then(|entry| entry.split('@').nth(1))
        .map(str::to_string)
        .ok_or_else(|| {
            format!(
                "{reference} has no repo digest (locally built image?); \
                 a pin no registry serves is not reproducible elsewhere"
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracked_reference_prefers_track_over_image() {
        assert_eq!(
            tracked_reference(Some("zfnd/zebra:latest"), Some("zfnd/zebra:v6.0.0")),
            Some("zfnd/zebra:latest".to_string())
        );
    }

    #[test]
    fn tracked_reference_recovers_the_tag_from_a_pinned_image() {
        assert_eq!(
            tracked_reference(None, Some("zfnd/zebra:v6.0.0@sha256:aaaa")),
            Some("zfnd/zebra:v6.0.0".to_string())
        );
        assert_eq!(
            tracked_reference(None, Some("registry:5000/zebra:v1")),
            Some("registry:5000/zebra:v1".to_string())
        );
    }

    #[test]
    fn digest_only_image_without_track_has_no_floating_preference() {
        assert_eq!(
            tracked_reference(None, Some("zfnd/zebra@sha256:aaaa")),
            None
        );
        // A registry port is not a tag.
        assert_eq!(
            tracked_reference(None, Some("registry:5000/zebra@sha256:aaaa")),
            None
        );
        assert_eq!(tracked_reference(None, None), None);
    }

    const DIGEST: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

    fn write_manifest(json: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("zcash-local-net.json");
        std::fs::write(&path, json).unwrap();
        (dir, path)
    }

    /// Stub resolver: every reference resolves to [`DIGEST`], and the
    /// references asked about are recorded through the returned log.
    fn stub_resolver() -> impl AsyncFn(ContainerRuntime, &str) -> Result<String, String> + use<> {
        async |_runtime, _reference| Ok(DIGEST.to_string())
    }

    #[tokio::test]
    async fn update_pins_tracked_and_tagged_artifacts_and_rewrites_the_file() {
        let (_dir, path) = write_manifest(
            r#"{
                "version": 1,
                "runtime": "docker",
                "validator": { "image": "zfnd/zebra:v6.0.0", "track": "zfnd/zebra:latest" },
                "indexer": { "image": "example.com/zainod:0.4.3" },
                "wallet": { "local": "/builds/zcash-devtool" }
            }"#,
        );
        let bumps = update_with_resolver(&path, &[], stub_resolver())
            .await
            .unwrap();

        // Two bumps: the tracked validator and the tag-pinned indexer;
        // the host-process wallet produces none.
        assert_eq!(bumps.len(), 2);
        assert_eq!(bumps[0].artifact, "validator");
        assert_eq!(bumps[0].tracked, "zfnd/zebra:latest");
        assert_eq!(bumps[0].pinned, format!("zfnd/zebra:latest@{DIGEST}"));
        assert!(bumps[0].changed());
        assert_eq!(bumps[1].artifact, "indexer");
        assert_eq!(
            bumps[1].pinned,
            format!("example.com/zainod:0.4.3@{DIGEST}")
        );

        // The rewritten file is valid, keeps `track`, and carries the
        // new pins.
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(&format!(r#""image": "zfnd/zebra:latest@{DIGEST}""#)));
        assert!(text.contains(r#""track": "zfnd/zebra:latest""#));
        assert!(text.ends_with('\n'));
        let reloaded = ArtifactManifest::load(&path).unwrap();
        assert!(reloaded.validator.is_container());

        // A second update resolving to the same digest is a no-op bump.
        let again = update_with_resolver(&path, &[], stub_resolver())
            .await
            .unwrap();
        assert!(again.iter().all(|bump| !bump.changed()));
    }

    #[tokio::test]
    async fn update_mints_the_first_pin_for_a_track_only_artifact() {
        let (_dir, path) = write_manifest(
            r#"{
                "version": 1,
                "runtime": "docker",
                "validator": { "track": "zfnd/zebra:latest" }
            }"#,
        );
        // Not launchable before the update…
        assert!(matches!(
            ArtifactManifest::load(&path),
            Err(ManifestError::UnresolvedTrack { .. })
        ));
        // …the update mints the pin…
        let bumps = update_with_resolver(&path, &[], stub_resolver())
            .await
            .unwrap();
        assert_eq!(bumps.len(), 1);
        assert_eq!(bumps[0].previous, "");
        // …and the manifest is launchable afterwards.
        assert!(
            ArtifactManifest::load(&path)
                .unwrap()
                .validator
                .is_container()
        );
    }

    #[tokio::test]
    async fn update_respects_the_artifact_filter() {
        let (_dir, path) = write_manifest(
            r#"{
                "version": 1,
                "runtime": "docker",
                "validator": { "image": "zfnd/zebra:v6.0.0" },
                "indexer": { "image": "example.com/zainod:0.4.3" }
            }"#,
        );
        let bumps = update_with_resolver(&path, &["indexer"], stub_resolver())
            .await
            .unwrap();
        assert_eq!(bumps.len(), 1);
        assert_eq!(bumps[0].artifact, "indexer");
        let text = std::fs::read_to_string(&path).unwrap();
        // The unselected validator pin is untouched.
        assert!(text.contains(r#""image": "zfnd/zebra:v6.0.0""#));
    }

    #[tokio::test]
    async fn update_rejects_unknown_artifact_filters() {
        let (_dir, path) = write_manifest(r#"{ "version": 1 }"#);
        let error = update_with_resolver(&path, &["validatr"], stub_resolver())
            .await
            .unwrap_err();
        assert!(matches!(error, UpdateError::UnknownArtifact { name } if name == "validatr"));
    }

    #[tokio::test]
    async fn digest_only_pin_without_track_is_left_untouched() {
        let manifest = format!(
            r#"{{
                "version": 1,
                "runtime": "docker",
                "validator": {{ "image": "zfnd/zebra@{DIGEST}" }}
            }}"#,
        );
        let (_dir, path) = write_manifest(&manifest);
        let bumps = update_with_resolver(&path, &[], stub_resolver())
            .await
            .unwrap();
        assert!(bumps.is_empty());
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(&format!(r#""image": "zfnd/zebra@{DIGEST}""#)));
    }

    #[tokio::test]
    async fn resolver_failure_surfaces_without_touching_the_file() {
        let original = r#"{
                "version": 1,
                "runtime": "docker",
                "validator": { "image": "zfnd/zebra:v6.0.0" }
            }"#;
        let (_dir, path) = write_manifest(original);
        let error = update_with_resolver(&path, &[], async |_runtime, _reference| {
            Err("registry unreachable".to_string())
        })
        .await
        .unwrap_err();
        assert!(matches!(
            error,
            UpdateError::Resolve {
                artifact: "validator",
                ..
            }
        ));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    }
}
