use std::{path::PathBuf, process::Command};

/// -Checks to see if an executable is in a directory determined by the `TEST_BINARIES_DIR` environment variable.
/// or launches directly, hoping it is in path.
pub(crate) fn pick_command(executable_name: &str) -> Command {
    pick_path(executable_name).map_or_else(
        || {
            tracing::info!("Trying to launch {executable_name} from PATH environment variable.");
            Command::new(executable_name)
        },
        Command::new,
    )
}

/// The part of `pick_command` that is unit-testable.
fn pick_path(executable_name: &str) -> Option<PathBuf> {
    let environment_variable_path: &str = "TEST_BINARIES_DIR";

    match std::env::var(environment_variable_path) {
        Ok(directory) => {
            let path = PathBuf::from(directory).join(executable_name);
            if path.exists() {
                tracing::warn!("Found {executable_name} at {path:?}.");
                tracing::info!("Ready to launch to launch {executable_name}.");
                Some(path)
            } else {
                tracing::warn!("Could not find {executable_name} at {path:?} set by {environment_variable_path} environment variable.");
                None
            }
        }
        Err(_err) => {
            tracing::warn!("{environment_variable_path} environment variable is not set.");
            None
        }
    }
}

// be aware these helpers are not dry because of compiler whatever

/// Used to `expect` `pick_command`.
pub(crate) const EXPECT_SPAWN: &str = "Failed to spawn command! Test executable must be set in TEST_BINARIES_DIR environment variable or be in PATH.";

#[cfg(test)]
mod tests {
    use super::pick_path;

    #[test]
    fn cargo() {
        let pick_path = pick_path("cargo");
        assert_eq!(pick_path, None);
    }
    #[test]
    #[ignore = "Needs TEST_BINARIES_DIR to be set and contain zcashd."]
    fn zcashd() {
        let pick_path = pick_path("zcashd");
        assert!(pick_path.is_some());
    }
}
