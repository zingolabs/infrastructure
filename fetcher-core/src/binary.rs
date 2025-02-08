use crate::{
    cache::Cache,
    error::{self, Error},
};

use super::Binaries;

impl Binaries {
    // TODO: make this truly unique
    fn _get_key(&self) -> String {
        format!("binaries_{}", self.get_name())
    }

    fn _get_version_string(&self) -> String {
        match self {
            Binaries::Zainod => "6.0.0",
            Binaries::Lightwalletd => "6.0.0",
            Binaries::Zcashd => "6.0.0",
        }
        .to_string()
    }

    fn _get_checksum(&self) -> String {
        match self {
            Binaries::Zainod => "some_checkum_string",
            Binaries::Lightwalletd => "some_checkum_string",
            Binaries::Zcashd => "some_checkum_string",
        }
        .to_string()
    }

    fn _get_fetch_url(&self) -> String {
        format!("some_base_url/{}", self.get_name())
    }

    fn _get_path(&self, cache: &Cache) -> Result<std::path::PathBuf, crate::error::Error> {
        let key = self._get_key();
        if cache.exists(&key) {
            Ok(cache.get_path(&key))
        } else {
            Err(Error::ResourceNotFound)
        }
    }
    fn confirm(&self, _cache: &Cache) -> Result<bool, Error> {
        println!("Im confirming... (not really)");
        Ok(true)
    }

    fn verify(&self, _cache: &Cache) -> Result<bool, Error> {
        println!("I'm veryfying... (not really)");
        Ok(true)
    }

    pub async fn fetch(&self, _cache: &Cache) -> Result<(), Error> {
        println!("I'm fetching... (not really)");
        Ok(())
    }

    fn get_name(&self) -> String {
        match self {
            Binaries::Zainod => "zainod",
            Binaries::Lightwalletd => "lightwalletd",
            Binaries::Zcashd => "zcashd",
        }
        .to_string()
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
