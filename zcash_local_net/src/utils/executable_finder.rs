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
fn pick_path(executable_name: &str, trace_location: bool) -> Option<PathBuf> {
    let environment_variable_path: &str = "TEST_BINARIES_DIR";

    match std::env::var(environment_variable_path) {
        Ok(directory) => {
            let path = PathBuf::from(directory).join(executable_name);
            if path.exists() {
                if trace_location {
                    tracing::info!("Found {executable_name} at {path:?}.");
                    tracing::info!("Ready to launch to launch {executable_name}.");
                }
                Some(path)
            } else {
                if trace_location {
                    tracing::info!("Could not find {executable_name} at {path:?} set by {environment_variable_path} environment variable.");
                }
                None
            }
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
