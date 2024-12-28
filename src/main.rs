use std::path::Path;
use std::{env, ffi::OsString};

use tokio::task::JoinSet;

#[tokio::main]
async fn main() {
    // look for zingo-blessed binaries.
    let mut seek_binaries: JoinSet<()> = JoinSet::new();

    let rootdir: OsString = env::var("CARGO_MANIFEST_DIR")
        .expect("cargo manifest path to be found")
        .into();
    //println!("{:?}",)
    println!("{:?}", rootdir);

    let path_to_binaries = Path::new(&rootdir).join("test_binaries");
    println!("{:?}", path_to_binaries);

    // we're looking for howevermany binaries
    let bin_names = vec!["lightwalletd", "zainod", "zcashd", "zebrad", "zingo-cli"];

    for n in bin_names {
        println!("working with : {:?}", n);
        seek_binaries.spawn(validate_binaries(n));
    }
    //
    //check hash
    //check signatures, metadata?

    seek_binaries.join_all().await;
}

async fn validate_binaries(name: &str) {
    //println!("{:?}",)
    println!("{:?}", name);
}
