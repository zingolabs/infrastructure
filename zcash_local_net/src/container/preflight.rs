//! Preflight: validate a manifest's artifacts against the current
//! environment *before* any launch.
//!
//! A failed container launch surfaces as a `LaunchError` carrying the
//! runtime client's stderr — accurate, but late and buried inside a
//! test failure. Preflight front-loads the same checks into precise,
//! human-readable diagnostics: run it once in test-suite setup (or via
//! `zcash-local-net preflight` before `cargo nextest run`) and every
//! environment problem — missing runtime, unreachable daemon, absent
//! image, unpullable reference, missing or non-executable binary — is
//! reported per artifact, all at once.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::manifest::ArtifactManifest;
use super::{ArtifactSource, ContainerImage, ContainerRuntime, PullPolicy};

/// One preflight check's outcome.
#[derive(Clone, Debug)]
pub struct PreflightCheck {
    /// What was checked, e.g. `validator: image zfnd/zebra:v6.0.0`.
    pub subject: String,
    /// Whether the check passed.
    pub passed: bool,
    /// Evidence: where the artifact resolved to, or what exactly is
    /// wrong and how to fix it.
    pub detail: String,
}

/// Every check preflight ran, in execution order.
#[derive(Clone, Debug)]
pub struct PreflightReport {
    checks: Vec<PreflightCheck>,
}

impl PreflightReport {
    /// Whether every check passed.
    pub fn passed(&self) -> bool {
        self.checks.iter().all(|check| check.passed)
    }

    /// The individual check outcomes.
    pub fn checks(&self) -> &[PreflightCheck] {
        &self.checks
    }
}

impl std::fmt::Display for PreflightReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for check in &self.checks {
            let status = if check.passed { "ok  " } else { "FAIL" };
            writeln!(f, "[{status}] {}: {}", check.subject, check.detail)?;
        }
        Ok(())
    }
}

impl ArtifactManifest {
    /// Check every artifact of this manifest against the current
    /// environment. Never fails early: the report carries one entry
    /// per check so a broken environment is diagnosed in a single
    /// pass. See the module docs for the check list.
    pub async fn preflight(&self) -> PreflightReport {
        let artifacts: [(&str, &'static str, &ArtifactSource); 3] = [
            ("validator", "zebrad", &self.validator),
            ("indexer", "zainod", &self.indexer),
            ("wallet", "zcash-devtool", &self.wallet),
        ];

        let mut checks = Vec::new();

        // Runtime checks, once per distinct runtime in use.
        let runtimes: BTreeSet<&'static str> = artifacts
            .iter()
            .filter_map(|(_, _, source)| match source {
                ArtifactSource::Container(image) => Some(image.runtime.cli_name()),
                ArtifactSource::HostProcess { .. } => None,
            })
            .collect();
        for cli_name in runtimes {
            let runtime = ContainerRuntime::from_manifest_name(cli_name)
                .expect("cli_name round-trips through from_manifest_name");
            checks.push(check_runtime(runtime).await);
        }

        for (artifact, executable_name, source) in artifacts {
            match source {
                ArtifactSource::HostProcess { binary } => {
                    checks.push(check_host_binary(
                        artifact,
                        executable_name,
                        binary.as_deref(),
                    ));
                }
                ArtifactSource::Container(image) => {
                    checks.push(check_image(artifact, image).await);
                }
            }
        }

        PreflightReport { checks }
    }
}

/// Runtime client present (`--version`) and daemon reachable (`info`).
async fn check_runtime(runtime: ContainerRuntime) -> PreflightCheck {
    let subject = format!("runtime: {runtime}");
    let version = tokio::process::Command::new(runtime.cli_name())
        .arg("--version")
        .output()
        .await;
    let version = match version {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        Ok(output) => {
            return PreflightCheck {
                subject,
                passed: false,
                detail: format!(
                    "`{runtime} --version` exited {}: {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            };
        }
        Err(io_error) => {
            return PreflightCheck {
                subject,
                passed: false,
                detail: format!(
                    "`{runtime}` could not be spawned ({io_error}); install it or set \
                     the manifest's `runtime` to an installed one"
                ),
            };
        }
    };

    // `info` exercises the daemon/socket, which `--version` does not.
    match tokio::process::Command::new(runtime.cli_name())
        .arg("info")
        .output()
        .await
    {
        Ok(output) if output.status.success() => PreflightCheck {
            subject,
            passed: true,
            detail: format!("{version}; daemon reachable"),
        },
        Ok(output) => PreflightCheck {
            subject,
            passed: false,
            detail: format!(
                "client found ({version}) but `{runtime} info` exited {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        },
        Err(io_error) => PreflightCheck {
            subject,
            passed: false,
            detail: format!("client found ({version}) but `{runtime} info` failed: {io_error}"),
        },
    }
}

/// Image present locally, pulled per policy when not.
async fn check_image(artifact: &str, image: &ContainerImage) -> PreflightCheck {
    let subject = format!("{artifact}: image {}", image.image);
    let cli = image.runtime.cli_name();

    let present = tokio::process::Command::new(cli)
        .args(["image", "inspect", "--format", "{{.Id}}", &image.image])
        .output()
        .await
        .is_ok_and(|output| output.status.success());

    let must_pull = match (present, image.pull) {
        (true, PullPolicy::Always) => true,
        (true, _) => {
            return PreflightCheck {
                subject,
                passed: true,
                detail: format!(
                    "image present locally ({})",
                    image_identity(cli, &image.image).await
                ),
            };
        }
        (false, PullPolicy::Never) => {
            return PreflightCheck {
                subject,
                passed: false,
                detail: format!(
                    "image not present locally and pull policy is \"never\"; \
                     build or load it first (`{cli} pull {}` / `{cli} load`)",
                    image.image
                ),
            };
        }
        (false, _) => true,
    };
    debug_assert!(must_pull);

    match tokio::process::Command::new(cli)
        .args(["pull", &image.image])
        .output()
        .await
    {
        Ok(output) if output.status.success() => {
            let identity = image_identity(cli, &image.image).await;
            PreflightCheck {
                subject,
                passed: true,
                detail: if present {
                    format!("image refreshed (pull policy \"always\"; {identity})")
                } else {
                    format!("image pulled ({identity})")
                },
            }
        }
        Ok(output) => PreflightCheck {
            subject,
            passed: false,
            detail: format!(
                "`{cli} pull {}` exited {}: {}",
                image.image,
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        },
        Err(io_error) => PreflightCheck {
            subject,
            passed: false,
            detail: format!("`{cli} pull {}` failed to spawn: {io_error}", image.image),
        },
    }
}

/// The image's immutable identity, as reproducibility evidence in the
/// report: the first registry digest (`repo@sha256:…`) when the image
/// came from a registry, else the local image ID (locally built
/// images have no repo digest). Manifests must already *pin* their
/// references; this states what the pin resolved to on this machine —
/// for a tag-pinned reference, the line to compare across machines.
async fn image_identity(cli: &str, image: &str) -> String {
    // `{{json .RepoDigests}}` rather than `{{index …}}`: the json
    // helper exists in both docker's and podman's template engines and
    // does not error on an empty list.
    let repo_digest = tokio::process::Command::new(cli)
        .args([
            "image",
            "inspect",
            "--format",
            "{{json .RepoDigests}}",
            image,
        ])
        .output()
        .await
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| serde_json::from_slice::<Vec<String>>(&output.stdout).ok())
        .and_then(|digests| digests.into_iter().next());
    if let Some(digest) = repo_digest {
        return digest;
    }
    tokio::process::Command::new(cli)
        .args(["image", "inspect", "--format", "{{.Id}}", image])
        .output()
        .await
        .ok()
        .filter(|output| output.status.success())
        .map(|output| {
            format!(
                "local image id {}",
                String::from_utf8_lossy(&output.stdout).trim()
            )
        })
        .unwrap_or_else(|| "identity unavailable".to_string())
}

/// Host binary resolvable and executable — explicit path, or the
/// `TEST_BINARIES_DIR` / `PATH` resolution the launch will perform.
fn check_host_binary(
    artifact: &str,
    executable_name: &'static str,
    binary: Option<&Path>,
) -> PreflightCheck {
    let subject = format!("{artifact}: host binary {executable_name}");
    match binary {
        Some(path) => match executable_file_status(path) {
            Ok(()) => PreflightCheck {
                subject,
                passed: true,
                detail: format!("local build at {}", path.display()),
            },
            Err(reason) => PreflightCheck {
                subject,
                passed: false,
                detail: format!("{}: {reason}", path.display()),
            },
        },
        None => match resolve_via_env(executable_name) {
            Some((provenance, path)) => match executable_file_status(&path) {
                Ok(()) => PreflightCheck {
                    subject,
                    passed: true,
                    detail: format!("resolved via {provenance} to {}", path.display()),
                },
                Err(reason) => PreflightCheck {
                    subject,
                    passed: false,
                    detail: format!(
                        "resolved via {provenance} to {} but {reason}",
                        path.display()
                    ),
                },
            },
            None => PreflightCheck {
                subject,
                passed: false,
                detail: format!(
                    "not found in TEST_BINARIES_DIR or PATH; set TEST_BINARIES_DIR to a \
                     directory containing {executable_name}, add it to PATH, or give this \
                     artifact an explicit `local` path or an `image` in the manifest"
                ),
            },
        },
    }
}

/// Mirror the launch-time lookup: `TEST_BINARIES_DIR` first (see
/// `crate::utils::executable_finder`), then a `PATH` walk.
fn resolve_via_env(executable_name: &str) -> Option<(&'static str, PathBuf)> {
    if let Ok(directory) = std::env::var("TEST_BINARIES_DIR") {
        let candidate = PathBuf::from(directory).join(executable_name);
        if candidate.exists() {
            return Some(("TEST_BINARIES_DIR", candidate));
        }
    }
    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var)
        .map(|dir| dir.join(executable_name))
        .find(|candidate| candidate.is_file())
        .map(|found| ("PATH", found))
}

/// `Ok(())` when `path` is an existing, executable regular file;
/// otherwise the human-readable reason it is not.
fn executable_file_status(path: &Path) -> Result<(), String> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(io_error) => return Err(format!("cannot stat ({io_error})")),
    };
    if !metadata.is_file() {
        return Err("exists but is not a regular file".to_string());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err("exists but is not executable (chmod +x?)".to_string());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn manifest_with_wallet(source: ArtifactSource) -> ArtifactManifest {
        ArtifactManifest {
            // Point the other two artifacts at concrete files that
            // pass, so each test observes exactly one interesting
            // check. `std::env::current_exe` is a real executable
            // regular file by construction.
            validator: ArtifactSource::local_binary(std::env::current_exe().unwrap()),
            indexer: ArtifactSource::local_binary(std::env::current_exe().unwrap()),
            wallet: source,
        }
    }

    fn wallet_check(report: &PreflightReport) -> &PreflightCheck {
        report
            .checks()
            .iter()
            .find(|check| check.subject.starts_with("wallet:"))
            .expect("wallet check present")
    }

    #[tokio::test]
    async fn missing_explicit_binary_fails_with_the_path() {
        let manifest = manifest_with_wallet(ArtifactSource::local_binary(
            "/nonexistent/zcash-devtool-build",
        ));
        let report = manifest.preflight().await;
        let check = wallet_check(&report);
        assert!(!check.passed);
        assert!(check.detail.contains("/nonexistent/zcash-devtool-build"));
        assert!(!report.passed());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn non_executable_binary_fails_with_remediation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("zcash-devtool");
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(b"#!/bin/sh\n").unwrap();
        // Default create mode is 0o644: not executable.
        let manifest = manifest_with_wallet(ArtifactSource::local_binary(&path));
        let report = manifest.preflight().await;
        let check = wallet_check(&report);
        assert!(!check.passed);
        assert!(check.detail.contains("not executable"), "{}", check.detail);
    }

    #[tokio::test]
    async fn existing_executable_passes() {
        let manifest = manifest_with_wallet(ArtifactSource::local_binary(
            std::env::current_exe().unwrap(),
        ));
        let report = manifest.preflight().await;
        assert!(report.passed(), "{report}");
    }

    /// The report renders one line per check with a stable pass/fail
    /// tag — the CLI's output contract.
    #[tokio::test]
    async fn report_renders_one_line_per_check() {
        let manifest = manifest_with_wallet(ArtifactSource::local_binary("/nonexistent"));
        let report = manifest.preflight().await;
        let rendered = report.to_string();
        assert_eq!(rendered.lines().count(), report.checks().len());
        assert!(rendered.contains("[ok  ]"));
        assert!(rendered.contains("[FAIL]"));
    }
}
