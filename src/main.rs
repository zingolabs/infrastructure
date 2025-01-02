use core::panic;
use hex;
use reqwest::{Certificate, Client, Url};
use std::collections::BinaryHeap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
// use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use sha2::{Digest, Sha512};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use std::{env, ffi::OsString};
use tokio::task::JoinSet;

#[tokio::main]
async fn main() {
    // find locally comitted cert for binary-dealer remote
    let cert: Certificate = reqwest::Certificate::from_pem(
        &fs::read("cert/cert.pem").expect("cert file to be readable"),
    )
    .expect("reqwest to ingest cert");
    println!("cert ingested : {:?}", cert);

    // let s_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(199, 167, 151, 146)), 3953);
    // Client deafult is idle sockets being kept-alive 90 seconds
    let req_client = reqwest::ClientBuilder::new()
        .connection_verbose(true)
        .zstd(true)
        .use_rustls_tls()
        .tls_info(true)
        .connect_timeout(Duration::from_secs(5)) // to connect // defaults to None
        .read_timeout(Duration::from_secs(10)) // how long to we wait for a read operation // defaults to no timeout
        .add_root_certificate(cert)
        //.resolve_to_addrs("zingo-1.decentcloud.net", &[s_addr]) // Override DNS resolution for specific domains to a particular IP address.
        .build()
        .expect("client builder to read system configuration and initialize TLS backend");

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
        seek_binaries.spawn(validate_binary(n, req_client.clone()));
    }

    seek_binaries.join_all().await;
    println!("program exiting, declare victory!");
}

async fn validate_binary(n: &str, r_client: Client) {
    // TODO currently this function checks validity of existing file (though reprecussions only for missing shasum)
    // should fetch a missing one first - helper funtion would allow for test->delete->fetch
    let crate_dir: OsString = env::var("CARGO_MANIFEST_DIR")
        .expect("cargo manifest path to be found")
        .into();

    let binary_dir = Path::new(&crate_dir).join("test_binaries");

    let bin_path = binary_dir.join(n);

    // TODO make fn file check -> Pass / Fail (rm file, get file)
    if bin_path.is_file() {
        //file is found, perform soft checks
        confirm_binary(&binary_dir, &bin_path, n).await
    }
    if !bin_path.is_file() {
        println!("{:?} = file not found!", &bin_path);
        // we have to go get it!

        // reqwest some stuff
        let asset_url = format!("https://zingo-1.decentcloud.net:3953/{}", n);
        // let asset_url = format!("https://199.167.151.146:3953/{}", n);
        println!("fetching from {:?}", asset_url);
        let fetch_url = Url::parse(&asset_url).expect("fetch_url to parse");
        let mut res = r_client
            .get(fetch_url)
            //.basic_auth(username, password);
            .send()
            .await
            .expect("Response to be ok");
        let mut target_binary: File = File::create(&bin_path).expect("file to be created");
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
}
// TODO actively set  or check file permissions

async fn confirm_binary(binary_dir: &PathBuf, bin_path: &PathBuf, n: &str) {
    // see if file is readable and print out the first 64 bytes, which should be unique among them.
    let file_read_sample = File::open(&bin_path).expect("file to be readable");
    let mut reader = BufReader::with_capacity(64, file_read_sample);
    let bytes_read = reader.fill_buf().expect("reader to fill_buf");
    println!("{:?} bytes : {:?}", &bin_path, bytes_read);
    // fast, soft binary check
    const LWD_BYTES: [u8; 64] = [
        127, 69, 76, 70, 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 62, 0, 1, 0, 0, 0, 0, 186, 71,
        0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 56, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 64, 0, 56, 0,
        9, 0, 64, 0, 36, 0, 33, 0,
    ];
    const ZAI_BYTES: [u8; 64] = [
        127, 69, 76, 70, 2, 1, 1, 3, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 62, 0, 1, 0, 0, 0, 16, 87, 19,
        0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 232, 138, 239, 0, 0, 0, 0, 0, 0, 0, 0, 0, 64, 0,
        56, 0, 14, 0, 64, 0, 34, 0, 33, 0,
    ];
    const ZCD_BYTES: [u8; 64] = [
        127, 69, 76, 70, 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 62, 0, 1, 0, 0, 0, 224, 132,
        121, 3, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 128, 198, 154, 10, 0, 0, 0, 0, 0, 0, 0, 0, 64,
        0, 56, 0, 12, 0, 64, 0, 47, 0, 45, 0,
    ];
    const ZCC_BYTES: [u8; 64] = [
        127, 69, 76, 70, 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 62, 0, 1, 0, 0, 0, 128, 73, 86,
        3, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 16, 199, 87, 4, 0, 0, 0, 0, 0, 0, 0, 0, 64, 0, 56,
        0, 12, 0, 64, 0, 47, 0, 45, 0,
    ];
    const ZINGO_BYTES: [u8; 64] = [
        127, 69, 76, 70, 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 62, 0, 1, 0, 0, 0, 48, 151, 16,
        0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 56, 21, 122, 4, 0, 0, 0, 0, 0, 0, 0, 0, 64, 0, 56,
        0, 14, 0, 64, 0, 34, 0, 33, 0,
    ];

    const ZEBRA_BYTES: [u8; 64] = [
        127, 69, 76, 70, 2, 1, 1, 3, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 62, 0, 1, 0, 0, 0, 224, 58, 32,
        0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 48, 101, 1, 5, 0, 0, 0, 0, 0, 0, 0, 0, 64, 0, 56,
        0, 14, 0, 64, 0, 34, 0, 33, 0,
    ];

    /*
    // const version strings for soft-confirming binaries when found
    // lwd and zaino don't like --version, they return stderr
    const VS_ZEBRAD: &str = "zebrad 2.1.0";
    const VS_ZCASHD: &str = "Zcash Daemon version v6.0.0";
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

        */
    match n {
        "lightwalletd" => {
            if bytes_read == LWD_BYTES {
                println!("lightwalletd bytes okay!");
            }
            /*
            if !std_err.contains(VS_LWD) {
                panic!("expected LWD version string incorrect")
            }
            println!("lightwalletd version string okay!");
            */
        }
        "zainod" => {
            if bytes_read == ZAI_BYTES {
                println!("zainod bytes okay!");
            }
            /*
            if !std_err.contains(VS_ZAINOD) {
                panic!("expected Zainod version string incorrect")
            }
            println!("zainod version string okay!");
             */
        }
        "zcashd" => {
            if bytes_read == ZCD_BYTES {
                println!("zcashd bytes okay!");
            }
            /*
            if !std_out.contains(VS_ZCASHD) {
                panic!("ZCD version string incorrect")
            }
            println!("zcashd version string okay!");
             */
        }
        "zcash-cli" => {
            if bytes_read == ZCC_BYTES {
                println!("Zcash-cli bytes okay!");
            }
            /*
            if !std_out.contains(VS_ZCASHCLI) {
                panic!("ZCC version string incorrect")
            }
            println!("Zcash-cli version string okay!");
             */
        }
        "zebrad" => {
            if bytes_read == ZEBRA_BYTES {
                println!("zebrad bytes okay!");
            }
            /*
            if !std_out.contains(VS_ZEBRAD) {
                panic!("Zebrad version string incorrect")
            }
            println!("zebrad version string okay!");
             */
        }
        "zingo-cli" => {
            if bytes_read == ZINGO_BYTES {
                println!("Zingo-cli bytes okay!");
            }
            /*
            if !std_out.contains(VS_ZINGOCLI) {
                panic!("Zingo-cli version string incorrect")
            }
            println!("Zingo-cli version string okay!");
             */
        }
        _ => println!("looked for unknown binary"),
    }
    println!("confirming {} hashsum against local record", n);

    // hashes for confirming expected binaries
    // TODO signatures, metadata?
    let lines: Vec<String> =
        BufReader::new(File::open(binary_dir.join("shasum.txt")).expect("shasum.txt to open"))
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
            assert_eq!(res, hash);
            // TODO case where comparison fails but attempt to purge and fetch binary again
            println!(
                "binary hash matches local record! Completing validation process for {}",
                n
            );
        }
    }
}
