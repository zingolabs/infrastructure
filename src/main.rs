use core::panic;
use reqwest::{Certificate, Client, Url};
use std::fmt::Debug;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
// use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;
use std::{env, ffi::OsString};
use tokio::task::JoinSet;

#[tokio::main]
async fn main() {
    // look for zingo-blessed binaries.

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
        .read_timeout(Duration::from_secs(8)) // how long to we wait for a read operation // defaults to no timeout
        .add_root_certificate(cert)
        //.resolve_to_addrs("zingo-1.decentcloud.net", &[s_addr]) // Override DNS resolution for specific domains to a particular IP address.
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
        //"shasum.txt",
        //"shasums.txt",
        //"cert.pem",
    ];

    for n in bin_names {
        // Client uses an Arc internally.
        seek_binaries.spawn(validate_binary(n, req_client.clone()));
    }

    seek_binaries.join_all().await;
    println!("program exiting");
}

async fn validate_binary(n: &str, r_client: Client) {
    // const version strings for soft-confirming binaries when found
    // lwd and zaino don't like --version, they return stderr
    const VS_ZEBRAD: &str = "zebrad 2.1.0";
    const VS_ZCASHD: &str = "Zcash Daemon version v6.0.0";
    const VS_ZCASHCLI: &str = "Zcash RPC client version v6.0.0";
    const VS_LWD: &str =
        "Use \"lightwalletd [command] --help\" for more information about a command.";
    const VS_ZAINOD: &str = "zainod [OPTIONS]";
    const VS_ZINGOCLI: &str = "Zingo CLI 0.1.1";

    let crate_dir: OsString = env::var("CARGO_MANIFEST_DIR")
        .expect("cargo manifest path to be found")
        .into();

    let binary_dir = Path::new(&crate_dir).join("test_binaries");
    let bin_path = binary_dir.join(n);
    if bin_path.is_file() {
        // see if file is readable and print out the first 64 bytes, which should be unique among them.
        let file_read_sample = File::open(&bin_path).expect("file to be readable");
        let mut reader = BufReader::with_capacity(64, file_read_sample);
        let _bytes_read = reader.fill_buf().expect("reader to fill_buf");
        println!("{:?} bytes : {:?}", &bin_path, _bytes_read);

        // TODO check version strings
        let mut vs = Command::new(bin_path);
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
                if !std_err.contains(VS_LWD) {
                    panic!("expected LWD version string incorrect")
                }
                println!("lightwalletd okay!");
            }
            "zainod" => {
                if !std_err.contains(VS_ZAINOD) {
                    panic!("expected Zainod version string incorrect")
                }
                println!("zainod okay!");
            }
            "zcashd" => {
                if !std_out.contains(VS_ZCASHD) {
                    panic!("ZCD version string incorrect")
                }
                println!("zcashd okay!");
            }
            "zcash-cli" => {
                if !std_out.contains(VS_ZCASHCLI) {
                    panic!("ZCC version string incorrect")
                }
                println!("Zcash-cli okay!");
            }
            "zebrad" => {
                if !std_out.contains(VS_ZEBRAD) {
                    panic!("Zebrad version string incorrect")
                }
                println!("zebrad okay!");
            }
            "zingo-cli" => {
                if !std_out.contains(VS_ZINGOCLI) {
                    panic!("Zingo-cli version string incorrect")
                }
                println!("Zingo-cli okay!");
            }
            _ => println!("looked for --version of unknown binary"),
        }
        return;
    } else {
        println!("{:?} = file not found!", &bin_path);
        // we have to go get it!

        // reqwest some stuff
        let asset_url = format!("https://zingo-1.decentcloud.net:3953/{}", n);
        // let asset_url = format!("https://199.167.151.146:3953/{}", n);
        println!("{:?}", asset_url);
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
            counter = counter + 1;
            if counter == 5 {
                counter = 0;
            }
        }
        println!("\nfile {} write complete!\n", n);
    }
    // TODO check hash,
    // signatures, metadata?
    // TODO set file permissions
}
