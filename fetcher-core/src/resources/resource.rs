use super::super::error::Error;

pub trait Resource: Clone {
    fn fetch(resource_id: &str) -> Result<Self, Error>
    where
        Self: Sized;
    fn store(&self, store_path: &str) -> Result<(), Error>;
    fn verify(&self) -> bool;
    fn load_from_data(data: Vec<u8>) -> Result<Self, Error>
    where
        Self: Sized;
}
