//! Utilities module

use std::{env, path::PathBuf};

/// Functions for picking executables from env variables in the global runtime.
pub mod executable_finder;
/// FD-anchored, no-symlink-following recursive directory copy.
pub mod safe_copy;
pub(crate) mod type_conversions;

/// Returns path to cargo manifest directory (project root)
pub(crate) fn cargo_manifest_dir() -> PathBuf {
    PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo manifest to resolve to pathbuf"))
}

/// Returns a path to the chain cache directory
#[must_use]
pub fn chain_cache_dir() -> PathBuf {
    cargo_manifest_dir().join("chain_cache")
}
