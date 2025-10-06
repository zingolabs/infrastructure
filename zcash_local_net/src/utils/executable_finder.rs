use std::{path::PathBuf, process::Command};

/// -Looks for an executable in TEST_BINARIES_DIR environment variable-
/// or launches directly, hoping it is in path.
pub fn pick_command(executable_name: &str) -> Command {
    match pick_path_from_envvar(executable_name) {
        Some(path) => Command::new(path),
        None => Command::new(executable_name),
    }
}

/// -Checks to see if an executable is in a directory determined by the TEST_BINARIES_DIR environment variable.
pub fn pick_path_from_envvar(executable_name: &str) -> Option<PathBuf> {
    std::env::var("TEST_BINARIES_DIR")
        .ok()
        .map(|dir| PathBuf::from(dir).join(executable_name))
        .filter(|pathbuf| pathbuf.exists())
}

// be aware these helpers are not dry because of compiler whatever

pub(crate) const EXPECT_SPAWN: &str = "Failed to spawn command! Test executable must be set in TEST_BINARIES_DIR environment variable or be in PATH.";

#[test]
fn cargo() {
    let pick_path = pick_path_from_envvar("cargo");
    // cargo is not in the TEST_BINARIES
    assert_eq!(pick_path, None);
}
#[test]
#[ignore = "Needs TEST_BINARIES_DIR to be set and contain zcashd."]
fn zcashd() {
    let pick_path = pick_path_from_envvar("zcashd");
    // cargo is not in the TEST_BINARIES
    assert_eq!(pick_path, None);
}
