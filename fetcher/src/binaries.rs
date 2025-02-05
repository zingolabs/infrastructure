use std::path::PathBuf;

use crate::get_fetcher_out_dir;

fn get_out_dir() -> PathBuf {
    get_fetcher_out_dir()
}

pub fn get_binaries_dir() -> PathBuf {
    let out_dir = get_out_dir();
    out_dir.join("test_binaries")
}

pub enum SupportedBinaries {
    Lightwalletd,
    Zainod,
    Zcashd,
    ZcashCli,
    ZingoCli,
    Zebrad,
}

pub fn get_path_for_binary(binary_name: SupportedBinaries) -> PathBuf {
    let bin_name = match binary_name {
        SupportedBinaries::Lightwalletd => "lightwalletd",
        SupportedBinaries::Zainod => "zainod",
        SupportedBinaries::Zcashd => "zcashd",
        SupportedBinaries::ZcashCli => "zcash-cli",
        SupportedBinaries::ZingoCli => "zingo-cli",
        SupportedBinaries::Zebrad => "zebrad",
    };
    get_binaries_dir().join(bin_name)
}
