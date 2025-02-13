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
    ZcashCli,
    ZingoCli,
    Zebrad,
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
