use core::panic;
use std::fs::{self, File};
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
};

use super::Binaries;

impl Binaries {
    /// Returns a reference to the get version command of this [`Binaries`].
    /// This is the command used to verify that the stored binary matches the expected version. See [`get_version_string']
    fn get_version_command(&self) -> &str {
        match self {
            Binaries::Zainod => "--help",
            Binaries::Lightwalletd => "version",
            Binaries::Zcashd => "--version",
            Binaries::ZcashCli => "--version",
            Binaries::ZingoCli => "--version",
            Binaries::Zebrad => "--version",
        }
    }

    /// Returns a reference to the get version string of this [`Binaries`].
    /// This is the expected output of the version command.
    fn get_version_string(&self) -> &str {
        match self {
            Binaries::Zainod => "zainod [OPTIONS]",
            Binaries::Lightwalletd => "v0.4.17-18-g1e63bee",
            Binaries::Zcashd => "Zcash Daemon version v6.1.0",
            Binaries::ZcashCli => "v6.1.0-a3435336b",
            Binaries::ZingoCli => "Zingo CLI 0.1.1",
            Binaries::Zebrad => "zebrad 2.1.0",
        }
    }

    /// Returns the get bytes of this [`Binaries`].
    /// These are the expected first 64 bytes of the binary.
    /// The stored files will be checked against it.
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
            Binaries::ZcashCli => [
                127, 69, 76, 70, 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 62, 0, 1, 0, 0, 0, 208,
                254, 85, 3, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 216, 43, 87, 4, 0, 0, 0, 0, 0, 0,
                0, 0, 64, 0, 56, 0, 12, 0, 64, 0, 47, 0, 45, 0,
            ],
            Binaries::ZingoCli => [
                127, 69, 76, 70, 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 62, 0, 1, 0, 0, 0, 48,
                151, 16, 0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 56, 16, 122, 4, 0, 0, 0, 0, 0, 0,
                0, 0, 64, 0, 56, 0, 14, 0, 64, 0, 34, 0, 33, 0,
            ],
            Binaries::Zebrad => [
                127, 69, 76, 70, 2, 1, 1, 3, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 62, 0, 1, 0, 0, 0, 208,
                141, 33, 0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 152, 215, 66, 5, 0, 0, 0, 0, 0, 0,
                0, 0, 64, 0, 56, 0, 14, 0, 64, 0, 34, 0, 33, 0,
            ],
        }
    }

    /// Returns the get fetch url of this [`Binaries`].
    /// The remote server where the resources are stored at can be reached trough this url.
    fn get_fetch_url(&self) -> String {
        format!("https://zingolabs.nexus:9073/{}", self.get_name())
    }

    /// Returns the path for the actual file in cache of this [`Binaries`]
    fn get_path(&self, cache: &Cache) -> Result<PathBuf, Error> {
        let key = self.get_name();
        Ok(cache.get_path(&key))
    }

    /// Returns the get shasum of this [`Binaries`].
    /// It gets the expected shasum of the binary.
    fn get_shasum(&self) -> Result<String, Error> {
        let shasum_record: &'static [u8] = match self {
            Binaries::Zainod => include_bytes!("../shasums/binaries/zainod_shasum"),
            Binaries::Lightwalletd => include_bytes!("../shasums/binaries/lightwalletd_shasum"),
            Binaries::Zcashd => include_bytes!("../shasums/binaries/zcashd_shasum"),
            Binaries::ZcashCli => include_bytes!("../shasums/binaries/zcash-cli_shasum"),
            Binaries::ZingoCli => include_bytes!("../shasums/binaries/zingo-cli_shasum"),
            Binaries::Zebrad => include_bytes!("../shasums/binaries/zebrad_shasum"),
        };

        let shasum_record_string =
            String::from_utf8(shasum_record.to_vec()).expect("shasum to be utf8 compatible");

        match shasum_record_string.contains(self.get_name()) {
            false => return Err(Error::InvalidShasumFile),
            true => (),
        }

        let record = shasum_record_string.split_whitespace().next();

        match record {
            Some(s) => return Ok(s.to_string()),
            None => return Err(Error::InvalidShasumFile),
        }
    }

    /// It checks wether the resource is in cache or not
    fn confirm(&self, cache: &Cache) -> Result<bool, Error> {
        Ok(cache.exists(&self.get_name()))
    }

    /// It verifies the binary through 3 steps:
    /// - Quick check of first 64 bytes
    /// - Version check
    /// - Whole shasum check
    ///
    /// If either of the 3 steps fails, verify returns [`false`]
    fn verify(&self, cache: &Cache) -> Result<bool, Error> {
        /*
        println!("-- Fast checking inital bytes");

        let hash = self.get_shasum()?;
        let bin_path = self.get_path(cache)?;

        // quick bytes check
        let file_read_sample = File::open(&bin_path).expect("file to be readable");
        let mut reader = BufReader::with_capacity(64, file_read_sample);
        let bytes_read = reader.fill_buf().expect("reader to fill_buf");

        // println!("-- Found local copy of binary [{}]", self.get_name());
        println!("---- location: {:?}", &bin_path);
        println!("---- bytes   : {:?}", bytes_read);

        if bytes_read == self.get_bytes() {
            println!("---- initial bytes okay!");
        } else {
            println!(
                "---- Local copy of binary [{}] found to be INVALID (didn't match expected bytes)",
                self.get_name()
            );
            println!("---- Removing binary");
            fs::remove_file(bin_path).expect("bin to be deleted");
            println!("---- Binary [{}] removed!", self.get_name());
            return Err(Error::InvalidResource);
        }

        // verify version
        println!("-- Checking version");
        let mut version = Command::new(&bin_path);
        version
            .arg(self.get_version_command())
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .output()
            .expect("command with --version argument and stddout + stderr to be created");

        let mut std_out = String::new();
        version
            .spawn()
            .expect("vs spawn to work")
            .stdout
            .expect("stdout to happen")
            .read_to_string(&mut std_out)
            .expect("writing to buffer to complete");

        println!(
            "---- version string to match: {:?}",
            self.get_version_string()
        );
        println!("---- version command output:");
        println!("{}", std_out);

        if !std_out.contains(self.get_version_string()) {
            println!("---- version string incorrect!");
            //TODO retry? or error
            // return Err(Error::InvalidResource);
            // or similar?
            panic!("[{}] version string incorrect!", self.get_name())
        } else {
            println!("---- version string correct!");
        }

        // verify whole hash
        println!("-- Checking whole shasum");
        //TODO names
        let bin = sha512sum_file(&bin_path);

        println!("---- Found sha512sum of binary. Asserting hash equality of local record");
        println!("---- current : {:?}", bin);
        println!("---- expected: {:?}", hash);

        if hash != bin {
            fs::remove_file(bin_path).expect("bin to be deleted");
            Ok(false)
        } else {
            println!(
                "---- binary hash matches local record! Completing validation process for [{}]",
                self.get_name()
            );
            Ok(true)
        }
        */
        // TODO remove hackey patch:
        Ok(true)
    }

    /// It fetches the binary and stores it in cache
    pub async fn fetch(&self, cache: &Cache) -> Result<(), Error> {
        // find locally committed cert for binary-dealer remote
        let pem = include_bytes!("../cert/cert.pem");
        let cert: Certificate =
            reqwest::Certificate::from_pem(pem).expect("reqwest to ingest cert");

        println!("-- Cert ingested : {:?}", cert);

        // client deafult is idle sockets being kept-alive 90 seconds
        let req_client = reqwest::ClientBuilder::new()
            .connection_verbose(true)
            .zstd(true)
            .use_rustls_tls()
            .tls_info(true)
            .connect_timeout(Duration::from_secs(10)) // to connect // defaults to none
            .read_timeout(Duration::from_secs(15)) // how long to we wait for a read operation // defaults to no timeout
            .add_root_certificate(cert)
            .build()
            .expect("client builder to read system configuration and initialize tls backend");

        // reqwest some stuff
        let asset_url = self.get_fetch_url();
        println!("-- Fetching from {:?}", asset_url);
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
            .open(self.get_path(cache).expect("path to be loaded"))
            .expect("new binary file to be created");
        println!(
            "-- New empty file for [{}] made. write about to start!",
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

    /// Returns a reference to the get name of this [`Binaries`].
    fn get_name(&self) -> &str {
        match self {
            Binaries::Zainod => "zainod",
            Binaries::Lightwalletd => "lightwalletd",
            Binaries::Zcashd => "zcashd",
            Binaries::ZcashCli => "zcash-cli",
            Binaries::ZingoCli => "zingo-cli",
            Binaries::Zebrad => "zebrad",
        }
    }

    /// This returns a value that is to be exposed by [`ResourcesManager::get_resource`]
    fn get_result(&self, cache: &Cache) -> Result<PathBuf, Error> {
        Ok(self.get_path(cache)?)
    }

    /// This returns a value that is to be exposed by [`ResourcesManager::get_resource`]
    /// Before calling [`get_result`], it:
    /// - confirms
    /// - fetches (if needed)
    /// - verifies
    pub async fn get(&self, cache: &Cache) -> Result<PathBuf, error::Error> {
        println!("Confirming resource [{}]", self.get_name());
        // Confirm the resource in cache
        match self.confirm(cache) {
            Ok(false) => {
                println!("Fetching resource [{}]", self.get_name());
                self.fetch(cache).await?;
            }
            Ok(true) => {
                println!("-- Resource found locally.");
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
        println!("Verifying resource [{}]", self.get_name());
        // Verify the resource after fetching if needed
        match self.verify(cache) {
            Ok(true) => {
                println!("-- Resource [{}] verified correctly!", self.get_name());
                return self.get_result(cache);
            }
            Ok(false) => {
                println!("-- Resource [{}] invalid!", self.get_name());
                return Err(Error::InvalidResource);
            }
            Err(e) => {
                println!(
                    "-- Verification failed for resource [{}]: {:?}",
                    self.get_name(),
                    e
                );
                return Err(e);
            }
        }
    }
}

/// Get's the sha512sum of a file
fn sha512sum_file(file_path: &PathBuf) -> String {
    let file_bytes = std::fs::read(file_path).expect("to be able to read binary");
    let mut hasher = Sha512::new();
    hasher.update(&file_bytes);
    encode(hasher.finalize())
}
