use std::path::PathBuf;

mod binary;
mod cache;
mod error;

#[derive(Debug, Clone)]

/// All supported binaries that are to be served as resources
pub enum Binaries {
    Zainod,
    Lightwalletd,
    Zcashd,
    ZcashCli,
    ZingoCli,
    Zebrad,
}

/// This enum includes all top level resource types
pub enum ResourcesEnum {
    /// Binary resources
    Binaries(Binaries),
}

/// A manager client that can serve resources
pub struct ResourcesManager {
    // Disk-based cache
    cache: cache::Cache,
}

impl ResourcesManager {
    /// Creates a new [`ResourcesManager`].
    ///
    /// * store_path: the path to a directory where the resources are to be cache-ed in
    pub fn new(store_path: &str) -> Self {
        let cache = cache::Cache::new(store_path);
        ResourcesManager { cache }
    }

    /// Get's a specific value for a specific resource.
    /// The value itself it defined in the resource enum implementation's method ['get_result']
    ///
    /// * res: resource to get
    ///
    /// # Examples
    /// ```
    /// let zainod = manager.get_resource(ResourcesEnum::Binaries(Binaries::Zainod)).await?
    /// ```
    pub async fn get_resource(&mut self, res: ResourcesEnum) -> Result<PathBuf, error::Error> {
        match res {
            ResourcesEnum::Binaries(bin) => bin.get(&self.cache).await,
        }
    }
}
