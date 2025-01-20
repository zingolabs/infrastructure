//! Utilities module

use std::{
    env,
    path::{Path, PathBuf},
};

/// Returns path to cargo manifest directory (project root)
pub(crate) fn cargo_manifest_dir() -> PathBuf {
    PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo manifest to resolve to pathbuf"))
}

/// Returns path to chain cache directory
pub fn chain_cache_dir() -> PathBuf {
    cargo_manifest_dir().join("chain_cache")
}

/// Testing binaries: these are managed and fetched automagically by fetcher.rs
pub enum TestingBinary {
    Lightwalletd,
    Zainod,
    ZcashCli,
    Zcashd,
    Zebrad,
    ZingoCli,
}

/// Gets the right binary path for a certain Testing Binary
pub fn get_testing_bin_path(binary: TestingBinary) -> PathBuf {
    let crate_dir: String =
        env::var("CARGO_MANIFEST_DIR").expect("cargo manifest path to be found");
    let bins_dir = Path::new(&crate_dir).join("fetched_resources/test_binaries");

    let name = match binary {
        TestingBinary::Lightwalletd => "lightwalletd",
        TestingBinary::Zainod => "zainod",
        TestingBinary::ZcashCli => "zcash-cli",
        TestingBinary::Zcashd => "zcashd",
        TestingBinary::Zebrad => "zebrad",
        TestingBinary::ZingoCli => "zingo-cli",
    };

    bins_dir.join(name).join(name)
}
