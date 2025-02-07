/*
use crate::{
    cache::Cache,
    error::{self, Error},
};

pub(crate) trait Resource {
    fn confirm(&self, cache: &Cache) -> Result<bool, Error>;
    fn verify(&self, cache: &Cache) -> Result<bool, Error>;
    async fn fetch(&self, cache: &Cache) -> Result<(), Error>;
    fn get_name(&self) -> String;
    fn get_result(&self, cache: &Cache) -> Result<(), Error>;

    async fn get(&self, cache: &Cache) -> Result<(), error::Error> {
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
*/
