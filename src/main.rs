use std::path::Path;
use std::path::PathBuf;
use std::{env, ffi::OsString};

use tokio::task::JoinSet;

#[tokio::main]
async fn main() {
    // look for zingo-blessed binaries.
    let mut seek_binaries: JoinSet<()> = JoinSet::new();

    let crate_dir: OsString = env::var("CARGO_MANIFEST_DIR")
        .expect("cargo manifest path to be found")
        .into();
    //println!("{:?}",)
    println!("{:?}", crate_dir);

    let binary_dir = Path::new(&crate_dir).join("test_binaries");
    println!("{:?}", binary_dir);

    let bin_names = vec!["lightwalletd", "zainod", "zcashd", "zebrad", "zingo-cli"];

    for n in bin_names {
        println!("working with : {:?}", n);
        let bin_path = binary_dir.join(n);
        seek_binaries.spawn(validate_binary(bin_path));
    }

    seek_binaries.join_all().await;
}

async fn validate_binary(bin_path: PathBuf) {
    //println!("{:?}",)
    println!("{:?}", bin_path);
    // see if file is there
    // if file is there,
    // (what about symlinks?)
    //check hash
    //check signatures, metadata?
}
