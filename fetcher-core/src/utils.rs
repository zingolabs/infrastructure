use std::path::PathBuf;

pub(crate) fn get_out_dir() -> PathBuf {
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR to be defined");
    PathBuf::from(&out_dir)
}

pub(crate) fn get_manifest_dir() -> PathBuf {
    PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR to be set"))
}
