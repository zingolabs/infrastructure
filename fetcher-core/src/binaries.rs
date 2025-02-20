use std::path::PathBuf;

use crate::get_fetcher_out_dir;

fn get_out_dir() -> PathBuf {
    get_fetcher_out_dir()
}

pub fn get_binaries_dir() -> PathBuf {
    let out_dir = get_out_dir();
    out_dir.join("test_binaries")
}

/// Testing binaries: these are managed and fetched automagically by fetcher.rs
pub enum Binaries {
    /// [Lightwalletd](https://github.com/zcash/lightwalletd) is a backend service that provides a bandwidth-efficient interface to the Zcash blockchain.
    Lightwalletd,
    /// [Zaino](https://github.com/zingolabs/zaino) is an indexer for the Zcash blockchain implemented in Rust.
    Zainod,
    /// [Zcashd & Zcash-cli](https://zcash.readthedocs.io/en/latest/rtd_pages/zcashd.html#zcash-full-node-and-cli) allow you to run a full node and interact with it via a command-line interface. The zcashd full node downloads a copy of the Zcash blockchain, enforces rules of the Zcash network, and can execute all functionalities.
    Zcashd,
    /// [Zcashd & Zcash-cli](https://zcash.readthedocs.io/en/latest/rtd_pages/zcashd.html#zcash-full-node-and-cli) allow you to run a full node and interact with it via a command-line interface.The zcash-cli allows interactions with the node (e.g. to tell it to send a transaction).
    ZcashCli,
    /// [Zingo CLI](https://github.com/zingolabs/zingolib?tab=readme-ov-file#zingo-cli) is a command line lightwalletd-proxy client.
    ZingoCli,
    /// [Zebra](https://github.com/ZcashFoundation/zebra) is a Zcash full-node written in Rust.
    Zebrad,
}

pub fn get_path_for_binary(binary_name: Binaries) -> PathBuf {
    let bin_name = match binary_name {
        Binaries::Lightwalletd => "lightwalletd",
        Binaries::Zainod => "zainod",
        Binaries::Zcashd => "zcashd",
        Binaries::ZcashCli => "zcash-cli",
        Binaries::ZingoCli => "zingo-cli",
        Binaries::Zebrad => "zebrad",
    };
    get_binaries_dir().join(bin_name)
}
