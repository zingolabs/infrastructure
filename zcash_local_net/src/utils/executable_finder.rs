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
    dbg!(std::env::var("TEST_BINARIES_DIR").ok())
        .map(|dir| PathBuf::from(dir).join(executable_name))
        .filter(|pathbuf| pathbuf.exists())
}

// be aware these helpers are not dry because of compiler whatever

pub(crate) const EXPECT_SPAWN: &str = "Failed to spawn command! Test executable must be set in TEST_BINARIES_DIR environment variable or be in PATH.";
