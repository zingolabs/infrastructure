use std::path::PathBuf;

mod binary;
mod cache;
mod error;
mod utils;

#[derive(Debug, Clone)]
/// All supported binaries
pub enum Binaries {
    Zainod,
    Lightwalletd,
    Zcashd,
}

pub enum ResourceType {
    Binaries, // General binary category
}
pub enum ResourcesEnum {
    Binaries(Binaries),
}

pub struct ResourcesManager {
    cache: cache::Cache, // Disk-based cache
}

impl ResourcesManager {
    pub fn new(store_path: &str) -> Self {
        let cache = cache::Cache::new(store_path);
        ResourcesManager { cache }
    }

    pub async fn get_resource(&mut self, res: ResourcesEnum) -> Result<PathBuf, error::Error> {
        match res {
            ResourcesEnum::Binaries(bin) => bin.get(&self.cache).await,
        }
    }
}

#[tokio::test]
async fn zainod_exists() {
    test_utils::binary_exists(Binaries::Zainod).await
}

#[tokio::test]
async fn lightwalletd_exists() {
    test_utils::binary_exists(Binaries::Lightwalletd).await
}

#[tokio::test]
async fn zcashd_exists() {
    test_utils::binary_exists(Binaries::Zcashd).await
}

mod test_utils {
    use crate::{Binaries, ResourcesEnum, ResourcesManager};

    pub(crate) async fn binary_exists(bin: Binaries) {
        let location = "./fetched_resources";
        let mut manager = ResourcesManager::new(&location);

        match manager.get_resource(ResourcesEnum::Binaries(bin)).await {
            Err(e) => {
                println!("{:}", e);
                assert!(false)
            }
            Ok(bin_path) => {
                assert!(bin_path.exists());
                assert!(bin_path.starts_with(location));
            }
        };
    }
}
