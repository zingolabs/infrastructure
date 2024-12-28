use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
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

    let bin_names = vec![
        "lightwalletd",
        "zainod",
        "zcashd",
        "zcash-cli",
        "zebrad",
        "zingo-cli",
    ];

    for n in bin_names {
        println!("working with : {:?}", n);
        let bin_path = binary_dir.join(n);
        seek_binaries.spawn(validate_binary(bin_path));
    }

    seek_binaries.join_all().await;
}

async fn validate_binary(bin_path: PathBuf) {
    println!("{:?}", bin_path);
    if bin_path.is_file() {
        //see if file is readable and print out the first 64 bytes, which should be unique.
        let file_read_sample = File::open(&bin_path).expect("file to be readable");
        let mut reader = BufReader::with_capacity(64, file_read_sample);
        let bytes_read = reader.fill_buf().expect("reader to fill_buf");
        println!("{:?} bytes : {:?}", &bin_path, bytes_read);
        let mut mc = Command::new(bin_path);
        mc.arg("--version");
        println!("{:?}", mc.spawn().expect("mc spawn to work").stdout);
        return;
    } else {
        println!("{:?} = temporary problems, no fetch yet!", &bin_path);
        //we have to go get it!
    }
    // TODO check hash,
    // signatures, metadata?
}
