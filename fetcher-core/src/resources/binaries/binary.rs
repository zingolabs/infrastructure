use crate::{
    cache::Cache,
    error::{self, Error},
};

use super::Binaries;

impl Binaries {
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

    fn get_path(&self, cache: &Cache) -> Result<std::path::PathBuf, crate::error::Error> {
        let key = self.get_key();
        if cache.exists(&key) {
            Ok(cache.get_path(&key))
        } else {
            Err(Error::ResourceNotFound)
        }
    }
    fn confirm(&self, cache: &Cache) -> Result<bool, Error> {
        println!("Im confirming... (not really)");
        Ok(true)
    }

    fn verify(&self, cache: &Cache) -> Result<bool, Error> {
        println!("I'm veryfying... (not really)");
        Ok(true)
    }

    pub async fn fetch(&self, cache: &Cache) -> Result<(), Error> {
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

    fn get_result(&self, cache: &Cache) -> Result<(), Error> {
        // self.get_path(cache)
        Ok(())
    }
    pub async fn get(&self, cache: &Cache) -> Result<(), error::Error> {
        println!("confirming resource [{}]", self.get_name());
        // check if the resource is in cache
        match self.confirm(&cache) {
            Ok(res) => {
                if !res {
                    println!("fetching resource [{}]", self.get_name());
                    // if it's not, fetch it
                    self.fetch(&cache).await;
                } else {
                    // not much to do here... maybe print some logs
                }
                // now verify that the fetched stuff is valid
                match self.verify(cache) {
                    Ok(is_valid) => {
                        if is_valid {
                            // the resource is valid
                            println!("resource [{}] verified correctly!", self.get_name());
                            // return the result
                            self.get_result(cache)
                        } else {
                            // the resource is invalid
                            println!("resource [{}] invalid!", self.get_name());
                            // throw error
                            Err(Error::InvalidResource)
                        }
                    }
                    Err(e) => todo!(),
                }
            }
            Err(_e) => todo!(),
        }
    }
}

/*
impl Resource for Binaries {
}
*/
