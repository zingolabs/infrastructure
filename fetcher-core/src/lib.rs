use resources::{binaries::Binaries, resource::Resource, ResourcesEnum};

pub mod cache;
pub mod error;
pub mod resources; // This will import all resource types

pub struct ResourcesManager {
    cache: cache::Cache, // Disk-based cache
}

impl ResourcesManager {
    pub fn new(store_path: &str) -> Self {
        let cache = cache::Cache::new(store_path);
        ResourcesManager { cache }
    }

    pub async fn get_resource(&mut self, res: ResourcesEnum) -> Result<(), error::Error> {
        match res {
            ResourcesEnum::Binaries(bin) => bin.get(&self.cache).await,
        }
    }
}

#[tokio::test]
async fn hello_world() {
    let mut manager = ResourcesManager::new("./fetched_resources");

    let zainod = manager
        .get_resource(ResourcesEnum::Binaries(Binaries::Zainod))
        .await;

    dbg!(zainod);
}
