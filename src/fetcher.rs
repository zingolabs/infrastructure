use hex;
use reqwest::{Certificate, Url};
use sha2::{Digest, Sha512};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
// use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use std::{env, ffi::OsString};
use tokio::task::JoinSet;

#[tokio::main]
pub async fn main() {
    // find or fetch zingo-blessed binaries.
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
        seek_binaries.spawn(validate_binary(n));
    }

    // TODO print helpful message if some threads return an error
    seek_binaries.join_all().await;
    println!("program exiting, declare victory!");
}

async fn validate_binary(n: &str) {
    let crate_dir: OsString = env::var("CARGO_MANIFEST_DIR")
        .expect("cargo manifest path to be found")
        .into();
    let bin_dir = Path::new(&crate_dir)
        .join("fetched_resources/test_binaries")
        .join(n);
    let bin_path = bin_dir.join(n);
    let shasum_path = bin_dir.join("shasum");

    loop {
        if !bin_path.is_file() {
            println!("{:?} = file not found!", &bin_path);
            // we have to go get it!
            fetch_binary(&bin_path, n).await
        }
        if bin_path.is_file() {
            //file is found, perform checks
            match confirm_binary(&bin_path, &shasum_path, n).await {
                Ok(()) => {
                    println!("{} binary confirmed.", &n);
                    break;
                }
                _ => println!("binary confirmation failure, deleted found binary. plz fetch again"),
            }
        }
    }
}

async fn confirm_binary(bin_path: &PathBuf, shasum_path: &PathBuf, n: &str) -> Result<(), ()> {
    // see if file is readable and print out the first 64 bytes, which should be unique among them.
    let file_read_sample = File::open(&bin_path).expect("file to be readable");
    let mut reader = BufReader::with_capacity(64, file_read_sample);
    let bytes_read = reader.fill_buf().expect("reader to fill_buf");
    println!("{:?} bytes : {:?}", &bin_path, bytes_read);

    // fast, soft binary check
    const LWD_BYTES: [u8; 64] = [
        127, 69, 76, 70, 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 62, 0, 1, 0, 0, 0, 64, 188, 71,
        0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 56, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 64, 0, 56, 0,
        9, 0, 64, 0, 36, 0, 33, 0,
    ];
    const ZAI_BYTES: [u8; 64] = [
        127, 69, 76, 70, 2, 1, 1, 3, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 62, 0, 1, 0, 0, 0, 0, 87, 19, 0,
        0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 112, 143, 238, 0, 0, 0, 0, 0, 0, 0, 0, 0, 64, 0, 56,
        0, 14, 0, 64, 0, 34, 0, 33, 0,
    ];
    const ZCD_BYTES: [u8; 64] = [
        127, 69, 76, 70, 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 62, 0, 1, 0, 0, 0, 0, 58, 121,
        3, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 8, 39, 154, 10, 0, 0, 0, 0, 0, 0, 0, 0, 64, 0, 56,
        0, 12, 0, 64, 0, 47, 0, 45, 0,
    ];
    const ZCC_BYTES: [u8; 64] = [
        127, 69, 76, 70, 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 62, 0, 1, 0, 0, 0, 208, 254, 85,
        3, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 216, 43, 87, 4, 0, 0, 0, 0, 0, 0, 0, 0, 64, 0, 56,
        0, 12, 0, 64, 0, 47, 0, 45, 0,
    ];
    const ZINGO_BYTES: [u8; 64] = [
        127, 69, 76, 70, 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 62, 0, 1, 0, 0, 0, 48, 151, 16,
        0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 56, 16, 122, 4, 0, 0, 0, 0, 0, 0, 0, 0, 64, 0, 56,
        0, 14, 0, 64, 0, 34, 0, 33, 0,
    ];
    const ZEBRA_BYTES: [u8; 64] = [
        127, 69, 76, 70, 2, 1, 1, 3, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 62, 0, 1, 0, 0, 0, 208, 141, 33,
        0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 152, 215, 66, 5, 0, 0, 0, 0, 0, 0, 0, 0, 64, 0, 56,
        0, 14, 0, 64, 0, 34, 0, 33, 0,
    ];

    // const version strings for soft-confirming binaries when found
    // lwd and zaino don't like --version, they return stderr
    const VS_ZEBRAD: &str = "zebrad 2.1.0";
    const VS_ZCASHD: &str = "Zcash Daemon version v6.1.0";
    const VS_ZCASHCLI: &str = "Zcash RPC client version v6.0.0";
    const VS_LWD: &str =
        "Use \"lightwalletd [command] --help\" for more information about a command.";
    const VS_ZAINOD: &str = "zainod [OPTIONS]";
    const VS_ZINGOCLI: &str = "Zingo CLI 0.1.1";

    let mut vs = Command::new(&bin_path);
    vs.arg("--version")
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .output()
        .expect("command with --version argument and stddout + stderr to be created");

    // we have to collect both becayse LWD and Zaino don't print to stdout with --version
    let mut std_out = String::new();
    let mut std_err = String::new();
    vs.spawn()
        .expect("vs spawn to work")
        .stdout
        .expect("stdout to happen")
        .read_to_string(&mut std_out)
        .expect("writing to buffer to complete");
    vs.spawn()
        .expect("vs spawn to work")
        .stderr
        .expect("stderr to happen")
        .read_to_string(&mut std_err)
        .expect("writing to buffer to complete");

    match n {
        "lightwalletd" => {
            if bytes_read == LWD_BYTES {
                println!("lightwalletd bytes okay!");
            } else {
                fs::remove_file(bin_path).expect("bin to be deleted");
                println!("binary {} removed!", n);
                return Err(());
            }
            if !std_err.contains(VS_LWD) {
                panic!("expected LWD version string incorrect")
            }
            println!("lightwalletd version string okay!");
        }
        "zainod" => {
            if bytes_read == ZAI_BYTES {
                println!("zainod bytes okay!");
            } else {
                fs::remove_file(bin_path).expect("bin to be deleted");
                println!("binary {} removed!", n);
                return Err(());
            }
            if !std_err.contains(VS_ZAINOD) {
                panic!("expected Zainod version string incorrect")
            }
            println!("zainod version string okay!");
        }
        "zcashd" => {
            if bytes_read == ZCD_BYTES {
                println!("zcashd bytes okay!");
            } else {
                fs::remove_file(bin_path).expect("bin to be deleted");
                println!("binary {} removed!", n);
                return Err(());
            }
            if !std_out.contains(VS_ZCASHD) {
                panic!("ZCD version string incorrect")
            }
            println!("zcashd version string okay!");
        }
        "zcash-cli" => {
            if bytes_read == ZCC_BYTES {
                println!("Zcash-cli bytes okay!");
            } else {
                fs::remove_file(bin_path).expect("bin to be deleted");
                println!("binary {} removed!", n);
                return Err(());
            }
            if !std_out.contains(VS_ZCASHCLI) {
                panic!("ZCC version string incorrect")
            }
            println!("Zcash-cli version string okay!");
        }
        "zebrad" => {
            if bytes_read == ZEBRA_BYTES {
                println!("zebrad bytes okay!");
            } else {
                fs::remove_file(bin_path).expect("bin to be deleted");
                println!("binary {} removed!", n);
                return Err(());
            }
            if !std_out.contains(VS_ZEBRAD) {
                panic!("Zebrad version string incorrect")
            }
            println!("zebrad version string okay!");
        }
        "zingo-cli" => {
            if bytes_read == ZINGO_BYTES {
                println!("Zingo-cli bytes okay!");
            } else {
                fs::remove_file(bin_path).expect("bin to be deleted");
                println!("binary {} removed!", n);
                return Err(());
            }
            if !std_out.contains(VS_ZINGOCLI) {
                panic!("Zingo-cli version string incorrect")
            }
            println!("Zingo-cli version string okay!");
        }
        _ => println!("looked for unknown binary"),
    }
    println!("confirming {} hashsum against local record", n);

    // hashes for confirming expected binaries
    let lines: Vec<String> = BufReader::new(File::open(shasum_path).expect("shasum to open"))
        .lines()
        .collect::<Result<_, _>>()
        .expect("collection of lines to unwrap");

    for l in lines {
        if l.contains(n) {
            let hash = l.split_whitespace().next().expect("line to be splitable");

            // run sha512sum against file and see result
            let file_bytes = std::fs::read(&bin_path).expect("to be able to read binary");
            let mut hasher = Sha512::new();
            hasher.update(&file_bytes);
            let res = hex::encode(hasher.finalize());
            println!(
                "found sha512sum of binary. asserting hash equality of local record {}",
                l
            );
            println!("{:?} :: {:?}", res, hash);

            // assert_eq!(res, hash);
            if !(res == hash) {
                fs::remove_file(bin_path).expect("bin to be deleted");
                return Err(());
            }
            println!(
                "binary hash matches local record! Completing validation process for {}",
                n
            );
        }
    }

    return Ok(());
}

async fn fetch_binary(bin_path: &PathBuf, n: &str) {
    // find locally comitted cert for binary-dealer remote
    let cert: Certificate = reqwest::Certificate::from_pem(
        &fs::read("cert/cert.pem").expect("cert file to be readable"),
    )
    .expect("reqwest to ingest cert");
    println!("cert ingested : {:?}", cert);

    // let s_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(9, 9, 9, 9)), 9073);
    // Client deafult is idle sockets being kept-alive 90 seconds
    let req_client = reqwest::ClientBuilder::new()
        .connection_verbose(true)
        .zstd(true)
        .use_rustls_tls()
        .tls_info(true)
        .connect_timeout(Duration::from_secs(10)) // to connect // defaults to None
        .read_timeout(Duration::from_secs(15)) // how long to we wait for a read operation // defaults to no timeout
        .add_root_certificate(cert)
        //.resolve_to_addrs("zingoproxy.com", &[s_addr]) // Override DNS resolution for specific domains to a particular IP address.
        .build()
        .expect("client builder to read system configuration and initialize TLS backend");

    // reqwest some stuff
    let asset_url = format!("https://zingoproxy.com:9073/{}", n);
    println!("fetching from {:?}", asset_url);
    let fetch_url = Url::parse(&asset_url).expect("fetch_url to parse");

    let mut res = req_client
        .get(fetch_url)
        //.basic_auth(username, password);
        .send()
        .await
        .expect("Response to be ok");
    // TODO instead of panicking, try again

    // with create_new, no file is allowed to exist at the target location
    // with mode we are able to set permissions as the file is created.
    let mut target_binary: File = File::options()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o100775)
        .open(&bin_path)
        .expect("new binary file to be created");
    println!("new empty file for {} made. write about to start!", n);

    // simple progress bar
    let progress = vec!["/", "-", "\\", "-", "o"];
    let mut counter: usize = 0;

    while let Some(chunk) = res
        .chunk()
        .await
        .expect("result to chunk ok.. *NOT A FAILED TRANSFER!")
    {
        target_binary
            .write_all(&chunk)
            .expect("chunk writes to binary");
        print!(
            "\rplease wait, fetching data chunks : {}",
            progress[counter]
        );
        counter = (counter + 1) % 5;
    }
    println!("\nfile {} write complete!\n", n);
}
