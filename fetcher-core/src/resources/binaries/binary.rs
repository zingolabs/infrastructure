use crate::{cache::Cache, error::Error, resources::resource::Resource};

use super::Binaries;

impl Binaries {
    fn get_name(&self) -> &str {
        match self {
            Binaries::Zainod => "zainod",
            Binaries::Lightwalletd => "lightwalletd",
            Binaries::Zcashd => "zcashd",
        }
    }

    // TODO: make this truly unique
    fn get_key(&self) -> String {
        format!("binaries_{}", self.get_name())
    }

    fn get_version_string(&self) -> String {
        match self {
            Binaries::Zainod => "6.0.0",
            Binaries::Lightwalletd => "6.0.0",
            Binaries::Zcashd => "6.0.0",
        }
        .to_string()
    }

    fn get_checksum(&self) -> String {
        match self {
            Binaries::Zainod => "some_checkum_string",
            Binaries::Lightwalletd => "some_checkum_string",
            Binaries::Zcashd => "some_checkum_string",
        }
        .to_string()
    }

    fn get_fetch_url(&self) -> String {
        format!("some_base_url/{}", self.get_name())
    }

    fn get_path(&self, cache: Cache) -> Result<std::path::PathBuf, crate::error::Error> {
        let key = self.get_key();
        if cache.exists(&key) {
            Ok(cache.get_path(&key))
        } else {
            Err(Error::ResourceNotFound)
        }
    }
}

impl Resource for Binaries {
    async fn get(&self, cache: &Cache) -> Result<(), crate::error::Error> {
        todo!()
    }

    fn confirm(&self, cache: &Cache) -> Result<bool, Error> {
        todo!()
    }

    fn verify(&self, cache: &Cache) -> Result<bool, Error> {
        todo!()
    }

    async fn fetch(&self, cache: &Cache) -> Result<(), Error> {
        todo!()
    }

    fn get_name(&self) -> String {
        todo!()
    }

    fn get_result(&self) -> () {
        todo!()
    }
}
