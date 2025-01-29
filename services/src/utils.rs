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
    /// [Lightwalletd](https://github.com/zcash/lightwalletd) is a backend service that provides a bandwidth-efficient interface to the Zcash blockchain.
    Lightwalletd,
    /// [Zaino](https://github.com/zingolabs/zaino) is an indexer for the Zcash blockchain implemented in Rust.
    Zainod,
    /// [Zcashd & Zcash-cli](https://zcash.readthedocs.io/en/latest/rtd_pages/zcashd.html#zcash-full-node-and-cli) allow you to run a full node and interact with it via a command-line interface.The zcash-cli allows interactions with the node (e.g. to tell it to send a transaction).
    ZcashCli,
    /// [Zcashd & Zcash-cli](https://zcash.readthedocs.io/en/latest/rtd_pages/zcashd.html#zcash-full-node-and-cli) allow you to run a full node and interact with it via a command-line interface. The zcashd full node downloads a copy of the Zcash blockchain, enforces rules of the Zcash network, and can execute all functionalities.
    Zcashd,
    /// [Zebra](https://github.com/ZcashFoundation/zebra) is a Zcash full-node written in Rust.
    Zebrad,
    /// [Zingo CLI](https://github.com/zingolabs/zingolib?tab=readme-ov-file#zingo-cli) is a command line lightwalletd-proxy client.
    ZingoCli,
}

/// Gets the right binary path for a certain Testing Binary
pub fn get_testing_bin_path(binary: TestingBinary) -> PathBuf {
    let crate_dir: String =
        env::var("CARGO_MANIFEST_DIR").expect("cargo manifest path to be found");
    let bins_dir = Path::new(&crate_dir).join("../services/fetched_resources/test_binaries");

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
