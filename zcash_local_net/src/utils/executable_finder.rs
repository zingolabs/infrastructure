use std::{path::PathBuf, process::Command};

/// -Checks to see if an executable is in a directory determined by the `TEST_BINARIES_DIR` environment variable.
/// or launches directly, hoping it is in path.
pub(crate) fn pick_command(executable_name: &str, trace_location: bool) -> Command {
    pick_path(executable_name, trace_location).map_or_else(
        || {
            if trace_location {
                tracing::info!(
                    "Trying to launch {executable_name} from PATH environment variable."
                );
            }
            Command::new(executable_name)
        },
        Command::new,
    )
}

/// The part of `pick_command` that is unit-testable.
///
/// Resolution is decoupled from existence: when `TEST_BINARIES_DIR`
/// is set we return `Some(<dir>/<name>)` without stat-ing the path.
/// The caller's `Command::new(path).spawn()` is the single syscall
/// that binds the path, closing the TOCTOU window between a
/// redundant `.exists()` and the eventual `execve` (issue #256, A1).
/// Same change also removes the silent `PATH` fallback when
/// `TEST_BINARIES_DIR` is set but the binary is missing -- a missing
/// binary now surfaces as `ENOENT` from the spawn at the resolved
/// path instead of disappearing into the PATH lookup.
fn pick_path(executable_name: &str, trace_location: bool) -> Option<PathBuf> {
    let environment_variable_path: &str = "TEST_BINARIES_DIR";

    match std::env::var(environment_variable_path) {
        Ok(directory) => {
            let path = PathBuf::from(directory).join(executable_name);
            if trace_location {
                tracing::info!(
                    "Resolved {executable_name} to {path:?} via {environment_variable_path}; \
                     existence is verified at spawn time."
                );
            }
            Some(path)
        }
        Err(_err) => {
            if trace_location {
                tracing::info!("{environment_variable_path} environment variable is not set.");
            }
            None
        }
    }
}

// be aware these helpers are not dry because of compiler whatever

/// Used to `expect` `pick_command`.
pub(crate) const EXPECT_SPAWN: &str = "Failed to spawn command! Test executable must be set in TEST_BINARIES_DIR environment variable or be in PATH.";

/// Helper to trace the executable version.
pub fn trace_version_and_location(executable_name: &str, version_command: &str) {
    let mut command = pick_command(executable_name, true);

    let args = vec![version_command];

    command.args(args);

    let version = command
        .output()
        .expect(crate::utils::executable_finder::EXPECT_SPAWN);
    tracing::info!(
        "$ {executable_name} {version_command}
 {version:?}"
    );
}

#[cfg(test)]
mod tests {
    use super::pick_path;

    #[test]
    fn cargo() {
        // Pin the env-unset branch deterministically; otherwise this
        // test depends on whatever `TEST_BINARIES_DIR` happens to be
        // in the parent shell.
        std::env::remove_var("TEST_BINARIES_DIR");
        let pick_path = pick_path("cargo", true);
        assert_eq!(pick_path, None);
    }
    #[test]
    #[ignore = "Needs TEST_BINARIES_DIR to be set and contain zcashd."]
    fn zcashd() {
        let pick_path = pick_path("zcashd", true);
        assert!(pick_path.is_some());
    }
}

#[cfg(test)]
mod unit_tests {
    /// Audit tests for `CLAUDE.md` checklist item #1 — TOCTOU on
    /// filesystem paths. See issue #256.
    mod corrosion_mitigation {
        mod fs_path_toctou {
            //! Site A1: `pick_path` pre-validates the resolved path with
            //! `.exists()` (line 26) before returning it; the caller
            //! then execs via `Command::new(path).spawn()`. Two syscalls
            //! on the same `&Path` → TOCTOU window between lookup and
            //! exec.
            //!
            //! Mitigation: drop the `.exists()` check entirely. Let
            //! `Command::new(...).spawn()` fail with `ENOENT`. The same
            //! change also fixes the silent-fallback-to-`PATH` bug
            //! observed when zainod is missing from
            //! `TEST_BINARIES_DIR` (the wrapper currently swallows the
            //! lookup miss and re-spawns from PATH, producing a
            //! confusing "Failed to spawn command" error far from the
            //! real cause).

            use crate::utils::executable_finder::pick_path;

            /// FAILS while `pick_path` calls `.exists()` on the
            /// resolved path before returning. After the fix (remove
            /// the check), `pick_path` returns `Some(<dir>/<name>)`
            /// regardless of whether the file is present, and lookup
            /// is decoupled from existence — the path is only bound by
            /// a syscall at exec time.
            #[test]
            fn pick_path_does_not_pre_validate_with_exists() {
                let dir = tempfile::tempdir().unwrap();
                std::env::set_var("TEST_BINARIES_DIR", dir.path());

                let absent_name = "audit_a1_nonexistent_target";
                let resolved = pick_path(absent_name, false);

                assert_eq!(
                    resolved,
                    Some(dir.path().join(absent_name)),
                    "audit (issue #256, site A1): pick_path pre-validates \
                     the resolved path with .exists() before returning. \
                     Drop the check; let `Command::new(path).spawn()` fail \
                     with ENOENT instead of pre-checking and triggering a \
                     silent PATH fallback."
                );
            }
        }
    }
}
