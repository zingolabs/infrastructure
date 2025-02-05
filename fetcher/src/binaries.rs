use std::path::PathBuf;

use crate::get_fetcher_out_dir;

fn get_out_dir() -> PathBuf {
    // PathBuf::from(var("OUT_DIR").unwrap())
    get_fetcher_out_dir()
}

pub fn get_binaries_dir() -> PathBuf {
    let out_dir = get_out_dir();
    out_dir.join("test_binaries")
}
