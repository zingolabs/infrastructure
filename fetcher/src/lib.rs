use std::path::PathBuf;

// this file is generated by build.rs
include!(concat!(env!("OUT_DIR"), "/config.rs"));

pub mod binaries;

pub fn get_fetcher_out_dir() -> PathBuf {
    PathBuf::from(FETCHER_OUT_DIR)
    // PathBuf::new()
}
