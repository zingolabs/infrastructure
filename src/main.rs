use reqwest::{Certificate, Client, Url};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
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

    let bin_names = vec![
        "lightwalletd",
        "zainod",
        "zcashd",
        "zcash-cli",
        "zebrad",
        "zingo-cli",
    ];

    for n in bin_names {
        // Client uses an Arc internally.
        seek_binaries.spawn(validate_binary(n, req_client.clone()));
    }

    seek_binaries.join_all().await;
}

async fn validate_binary(n: &str, r_client: Client) {
    let crate_dir: OsString = env::var("CARGO_MANIFEST_DIR")
        .expect("cargo manifest path to be found")
        .into();
    // INFO println!("{:?}", crate_dir);

    let binary_dir = Path::new(&crate_dir).join("test_binaries");
    let bin_path = binary_dir.join(n);
    if bin_path.is_file() {
        //see if file is readable and print out the first 64 bytes, which should be unique.
        let file_read_sample = File::open(&bin_path).expect("file to be readable");
        let mut reader = BufReader::with_capacity(64, file_read_sample);
        let bytes_read = reader.fill_buf().expect("reader to fill_buf");
        println!("{:?} bytes : {:?}", &bin_path, bytes_read);

        // TODO check version strings
        // lwd and zaino don't like --version
        if !bin_path.ends_with("zainod") && !bin_path.ends_with("lightwalletd") {
            let mut _vc = Command::new(bin_path);
            _vc.arg("--version");
            // print out version stdouts - maybe for logging or tracing later
            // println!("{:?}", vc.spawn().expect("mc spawn to work").stdout);
        }
        return;
    } else {
        println!("{:?} = file not found!", &bin_path);
        // we have to go get it!
        // TODO helper function?
        // TODO temp directory?

        // reqwest some stuff
        //r_client.get(URL);
        let asset_url = format!("https://127.0.0.1:3953/{}", n);
        let fetch_url = Url::parse(&asset_url).expect("fetch_url to parse");
        let mut res = r_client
            .get(fetch_url)
            //.basic_auth(username, password);
            .send()
            .await
            .expect("Response to be ok");
        //println!("R : {:?} {:?}", res.status(), res.text().await); // simple diagnostic for GETME text file
        let mut target_binary: File = File::create(bin_path).expect("file to be created");
        println!("new empty file for {} made. write about to start!", n);

        // simple progress bar
        let progress = vec!["/", "-", "\\", "-", "o"];
        let mut counter: usize = 0;
        while let Some(chunk) = res.chunk().await.expect("result to chunk ok") {
            target_binary
                .write_all(&chunk)
                .expect("chunk writes to binary");
            print!(
                "\rplease wait, fetching data chunks : {}",
                progress[counter]
            );
            //print!("\r");
            counter = counter + 1;
            if counter == 5 {
                counter = 0;
            }
        }
        println!("\nfile {} write complete!\n", n);
        //println!("{:?}",)
    }

    // TODO check hash,
    // signatures, metadata?
}
