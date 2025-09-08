//! Utilities module

use std::{env, path::PathBuf, process::Command};

#[derive(Clone)] //
/// Where the executable is launched from
pub enum ExecutableLocation {
    /// a specific path
    Specific(PathBuf),
    /// a program name, e.g. a program in $PATH
    Global(String),
}

impl ExecutableLocation {
    pub fn by_name(name: &str) -> Self {
        Self::Global(name.to_string())
    }
    pub(crate) fn command(&self) -> Command {
        match &self {
            Self::Specific(pathbuf) => Command::new(pathbuf),
            Self::Global(name) => Command::new(name),
        }
    }
}

impl std::fmt::Debug for ExecutableLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let actual_string = match self {
            ExecutableLocation::Specific(path_buf) => format!("Specific binary at {path_buf:?}"),
            ExecutableLocation::Global(name) => format!("Global binary named {name}"),
        };
        write!(f, "{actual_string}");
        Ok(())
    }
}

/// Returns path to cargo manifest directory (project root)
pub(crate) fn cargo_manifest_dir() -> PathBuf {
    PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo manifest to resolve to pathbuf"))
}

/// Returns path to chain cache directory
pub fn chain_cache_dir() -> PathBuf {
    cargo_manifest_dir().join("chain_cache")
}
