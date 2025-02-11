use core::panic;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use hex::encode;
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
            .join(self.get_name());

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
        println!("Im confirming... (not really)");
        Ok(cache.exists(&self.get_key()))
    }

    fn verify(&self, cache: &Cache) -> Result<bool, Error> {
        println!("I'm veryfying...");
        let hash = self._get_shasum()?;
        let bin_path = self._get_path(cache)?;

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

    pub async fn fetch(&self, _cache: &Cache) -> Result<(), Error> {
        println!("I'm fetching... (not really)");
        Ok(())
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
                println!("Resource [{}] is already confirmed.", self.get_name());
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
