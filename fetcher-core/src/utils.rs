use std::path::PathBuf;

use crate::ResourcesEnum;

pub(crate) fn get_out_dir() -> PathBuf {
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR to be defined");
    PathBuf::from(&out_dir)
}

pub(crate) fn get_manifest_dir() -> PathBuf {
    PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR to be set"))
}

fn get_shasum_for_resource(resource: ResourcesEnum) -> PathBuf {
    let checksums_dir = get_manifest_dir().join("shasums");
    match resource {
        ResourcesEnum::Binaries(bin) => checksums_dir.join(bin.get_resource_type_id()),
    }
}
