use reqwest::{Certificate, Client, Url};
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

    // Client deafult is idle sockets being kept-alive 90 seconds
    let req_client = reqwest::ClientBuilder::new()
        .connection_verbose(true)
        .zstd(true)
        .use_rustls_tls()
        .tls_info(true)
        //.connect_timeout(Duration) // to connect // defaults to None
        //.read_timeout(Duration) // how long to we wait for a read operation // defaults to no timeout
        // TODO address these:
        .danger_accept_invalid_hostnames(true)
        .danger_accept_invalid_certs(true)
        // TODO if this works it should take care of that stuff...
        // .add_root_certificate(Certificate) // reqwest::Certificate
        // .resolve_to_addrs(domain, addrs) // Override DNS resolution for specific domains to a particular IP address.
        .build()
        .expect("client builder to read system configuration and initialize TLS backend");

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
        let bin_path = binary_dir.join(n);
        // Client uses an Arc internally.
        seek_binaries.spawn(validate_binary(bin_path, req_client.clone()));
    }

    seek_binaries.join_all().await;
}

async fn validate_binary(bin_path: PathBuf, r_client: Client) {
    if bin_path.is_file() {
        //see if file is readable and print out the first 64 bytes, which should be unique.
        let file_read_sample = File::open(&bin_path).expect("file to be readable");
        let mut reader = BufReader::with_capacity(64, file_read_sample);
        let bytes_read = reader.fill_buf().expect("reader to fill_buf");
        println!("{:?} bytes : {:?}", &bin_path, bytes_read);

        // TODO check version strings
        //print out version stdouts - maybe for logging or tracing later
        // lwd and zaino don't like --version
        let mut _mc = Command::new(bin_path);
        _mc.arg("--version");
        //println!("{:?}", mc.spawn().expect("mc spawn to work").stdout);

        return;
    } else {
        println!(
            "{:?} = file not found! temporary problems, no fetch yet!",
            &bin_path
        );
        // we have to go get it!
        // TODO helper function?
        // TODO temp directory?

        // reqwest some stuff
        // suppports native_tls (openssl on linux) by default, but rustls is a feature
        //r_client.get(URL);
        let fetch_url = Url::parse("127.0.0.1:3953").expect("fetch_url to parse");
        let resp = r_client.get(fetch_url).send().await;
        println!("{:?}", resp.unwrap())

        //.basic_auth(username, password);
    }
    // TODO check hash,
    // signatures, metadata?
}
