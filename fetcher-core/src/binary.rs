use core::panic;
use std::fs::{self, read, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use hex::encode;
use reqwest::{Certificate, Url};
use sha2::{Digest, Sha512};

use crate::error::Error;
use crate::{
    cache::Cache,
    error::{self},
    utils::get_manifest_dir,
};

use super::Binaries;

impl Binaries {
    pub fn get_resource_type_id(&self) -> String {
        "binaries".to_string()
    }

    // TODO: make this truly unique
    fn get_key(&self) -> String {
        format!("{}_{}", self.get_resource_type_id(), self.get_name())
    }

    fn get_version_command(&self) -> &str {
        match self {
            Binaries::Zainod => "--help",
            Binaries::Lightwalletd => "version",
            Binaries::Zcashd => "--version",
        }
    }

    fn _get_version_string(&self) -> &str {
        match self {
            Binaries::Zainod => "zainod [OPTIONS]",
            Binaries::Lightwalletd => "v0.4.17-18-g1e63bee",
            Binaries::Zcashd => "Zcash Daemon version v6.1.0",
        }
    }

    fn get_bytes(&self) -> [u8; 64] {
        match self {
            Binaries::Zainod => [
                127, 69, 76, 70, 2, 1, 1, 3, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 62, 0, 1, 0, 0, 0, 0,
                87, 19, 0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 112, 143, 238, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 64, 0, 56, 0, 14, 0, 64, 0, 34, 0, 33, 0,
            ],
            Binaries::Lightwalletd => [
                127, 69, 76, 70, 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 62, 0, 1, 0, 0, 0, 64,
                188, 71, 0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 56, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 64, 0, 56, 0, 9, 0, 64, 0, 36, 0, 33, 0,
            ],
            Binaries::Zcashd => [
                127, 69, 76, 70, 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 62, 0, 1, 0, 0, 0, 0,
                58, 121, 3, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 8, 39, 154, 10, 0, 0, 0, 0, 0, 0,
                0, 0, 64, 0, 56, 0, 12, 0, 64, 0, 47, 0, 45, 0,
            ],
        }
    }

    fn _get_fetch_url(&self) -> String {
        format!("some_base_url/{}", self.get_name())
    }

    fn _get_path(&self, cache: &Cache) -> Result<PathBuf, Error> {
        let key = self.get_key();
        if cache.exists(&key) {
            Ok(cache.get_path(&key))
        } else {
            Err(Error::ResourceNotFound)
        }
    }

    fn _get_shasum(&self) -> Result<String, Error> {
        // get path to the shasum file
        let shasum_path = get_manifest_dir()
            .join("shasums")
            .join(self.get_resource_type_id())
            .join(format!("{}_shasum", self.get_name()));
        dbg!(&shasum_path);
        // hashes for confirming expected binaries
        let mut buf: BufReader<File> =
            BufReader::new(File::open(shasum_path).expect("shasum to open"));
        let mut shasum_record = String::new();
        buf.read_to_string(&mut shasum_record)
            .expect("buffer to write into String");

        if !shasum_record.contains(self.get_name()) {
            return Err(Error::InvalidShasumFile);
        }

        let record = shasum_record.split_whitespace().next();

        match record {
            Some(s) => return Ok(s.to_string()),
            None => return Err(Error::InvalidShasumFile),
        }
    }

    fn confirm(&self, cache: &Cache) -> Result<bool, Error> {
        println!("Im confirming...");
        Ok(cache.exists(&self.get_key()))
    }

    fn verify(&self, cache: &Cache) -> Result<bool, Error> {
        println!("I'm verifying...");
        let hash = self._get_shasum()?;
        let bin_path = self._get_path(cache)?;

        // quick bytes check
        let file_read_sample = File::open(&bin_path).expect("file to be readable");
        let mut reader = BufReader::with_capacity(64, file_read_sample);
        let bytes_read = reader.fill_buf().expect("reader to fill_buf");
        println!("{:?} bytes : {:?}", &bin_path, bytes_read);

        if bytes_read == self.get_bytes() {
            println!("{} bytes okay!", self.get_name());
        } else {
            println!("binary {} removed!", self.get_name());
            fs::remove_file(bin_path).expect("bin to be deleted");
            return Err(Error::InvalidResource);
        }

        // verify version
        let mut version = Command::new(&bin_path);
        version
            .arg(self.get_version_command())
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .output()
            .expect("command with --version argument and stddout + stderr to be created");

        let mut std_out = String::new();
        // let mut std_err = String::new();
        version
            .spawn()
            .expect("vs spawn to work")
            .stdout
            .expect("stdout to happen")
            .read_to_string(&mut std_out)
            .expect("writing to buffer to complete");
        // version
        //     .spawn()
        //     .expect("vs spawn to work")
        //     .stderr
        //     .expect("stderr to happen")
        //     .read_to_string(&mut std_err)
        //     .expect("writing to buffer to complete");

        if !std_out.contains(self._get_version_string()) {
            panic!("{} version string incorrect!", self.get_name())
        }

        // verify whole hash
        let bin = sha512sum_file(&bin_path);

        println!(
            "found sha512sum of binary. asserting hash equality of local record {}",
            hash
        );

        println!("{:?} :: {:?}", bin, hash);

        if hash != bin {
            fs::remove_file(bin_path).expect("bin to be deleted");
            Ok(false)
        } else {
            println!(
                "binary hash matches local record! Completing validation process for {}",
                self.get_name()
            );
            Ok(true)
        }
    }

    pub async fn fetch(&self, cache: &Cache) -> Result<(), Error> {
        println!("I'm fetching...");
        // find locally committed cert for binary-dealer remote
        let cert: Certificate = reqwest::Certificate::from_pem(
            &read(get_manifest_dir().join("cert/cert.pem")).expect("cert path to be readable"),
        )
        .expect("reqwest to ingest cert");

        println!("cert ingested : {:?}", cert);

        // let s_addr = socketaddr::new(ipaddr::v4(ipv4addr::new(9, 9, 9, 9)), 9073);
        // client deafult is idle sockets being kept-alive 90 seconds
        let req_client = reqwest::ClientBuilder::new()
            .connection_verbose(true)
            .zstd(true)
            .use_rustls_tls()
            .tls_info(true)
            .connect_timeout(Duration::from_secs(10)) // to connect // defaults to none
            .read_timeout(Duration::from_secs(15)) // how long to we wait for a read operation // defaults to no timeout
            .add_root_certificate(cert)
            //.resolve_to_addrs("zingolabs.nexus", &[s_addr]) // override dns resolution for specific domains to a particular ip address.
            .build()
            .expect("client builder to read system configuration and initialize tls backend");

        // reqwest some stuff
        let asset_url = self._get_fetch_url();
        println!("fetching from {:?}", asset_url);
        let fetch_url = Url::parse(&asset_url).expect("fetch_url to parse");

        let mut res = req_client
            .get(fetch_url)
            //.basic_auth(username, password);
            .send()
            .await
            .expect("response to be ok");
        // todo instead of panicking, try again

        // with create_new, no file is allowed to exist at the target location
        // with .mode() we are able to set permissions as the file is created.
        let mut target_binary: File = File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o100775)
            .open(self._get_path(cache).expect("path to be loaded"))
            .expect("new binary file to be created");
        println!(
            "new empty file for {} made. write about to start!",
            self.get_name()
        );

        // simple progress bar
        let progress = ["/", "-", "\\", "-", "o"];
        let mut counter: usize = 0;

        while let Some(chunk) = res
            .chunk()
            .await
            .expect("result to chunk ok.. *not a failed transfer!")
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
        println!("\nfile {} write complete!\n", self.get_name());

        return Ok(());
    }

    fn get_name(&self) -> &str {
        match self {
            Binaries::Zainod => "zainod",
            Binaries::Lightwalletd => "lightwalletd",
            Binaries::Zcashd => "zcashd",
        }
        // .to_string()
    }

    fn get_result(&self, _cache: &Cache) -> Result<(), Error> {
        // self.get_path(cache)
        Ok(())
    }
    pub async fn get(&self, cache: &Cache) -> Result<(), error::Error> {
        println!("Confirming resource [{}]", self.get_name());
        // Confirm the resource in cache
        match self.confirm(cache) {
            Ok(false) => {
                println!("Fetching resource [{}]", self.get_name());
                self.fetch(cache).await?;
            }
            Ok(true) => {
                println!("Resource [{}] found locally.", self.get_name());
            }
            Err(e) => {
                println!(
                    "Confirmation failed for resource [{}]: {:?}",
                    self.get_name(),
                    e
                );
                return Err(e);
            }
        }
        // Verify the resource after fetching if needed
        match self.verify(cache) {
            Ok(true) => {
                println!("Resource [{}] verified correctly!", self.get_name());
                return self.get_result(cache);
            }
            Ok(false) => {
                println!("Resource [{}] invalid!", self.get_name());
                return Err(Error::InvalidResource);
            }
            Err(e) => {
                println!(
                    "Verification failed for resource [{}]: {:?}",
                    self.get_name(),
                    e
                );
                return Err(e);
            }
        }
    }
}

/*
impl Resource for Binaries {
}
*/

fn sha512sum_file(file_path: &PathBuf) -> String {
    let file_bytes = std::fs::read(file_path).expect("to be able to read binary");
    let mut hasher = Sha512::new();
    hasher.update(&file_bytes);
    encode(hasher.finalize())
}
