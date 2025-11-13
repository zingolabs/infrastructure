use std::{path::PathBuf, process::Command};

/// -Looks for an executable in `TEST_BINARIES_DIR` environment variable-
/// or launches directly, hoping it is in path.
pub fn pick_command(executable_name: &str) -> Command {
    pick_path(executable_name)
        .map(Command::new)
        .unwrap_or(Command::new(executable_name))
}

/// -Checks to see if an executable is in a directory determined by the `TEST_BINARIES_DIR` environment variable.
fn pick_path(executable_name: &str) -> Option<PathBuf> {
    let environment_variable_path: &str = "TEST_BINARIES_DIR";

    match std::env::var(environment_variable_path) {
        Ok(directory) => {
            let path = PathBuf::from(directory).join(executable_name);
            if path.exists() {
                tracing::info!("Running {executable_name} at {path:?}.");
                Some(path)
            } else {
                tracing::warn!("Could not find {executable_name} at {path:?} set by {environment_variable_path}.");
                None
            }
        }
        Err(_err) => {
            tracing::warn!("{environment_variable_path} environment variable is not set. Will attempt to use {executable_name} from PATH.");
            None
        }
    }
}

// be aware these helpers are not dry because of compiler whatever

pub(crate) const EXPECT_SPAWN: &str = "Failed to spawn command! Test executable must be set in TEST_BINARIES_DIR environment variable or be in PATH.";

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
