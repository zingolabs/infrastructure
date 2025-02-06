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

    pub fn get_resource(
        &mut self,
        resource_type: resources::ResourceType,
        resource_id: &str,
    ) -> Result<Box<dyn resources::resource::Resource>, error::Error> {
        // Check if the resource is already cached
        if self.cache.exists(resource_id) {
            let data = self.cache.load(resource_id)?;
            let resource = resource_type.load_from_data(data)?;
            return Ok(resource);
        }

        // Fetch resource and cache it if not available
        let resource = self.fetch_and_cache_resource(resource_type, resource_id)?;
        Ok(resource)
    }

    fn fetch_and_cache_resource(
        &mut self,
        resource_type: resources::ResourceType,
        resource_id: &str,
    ) -> Result<Box<dyn resources::Resource>, error::Error> {
        let resource = resource_type.fetch(resource_id)?;
        resource.store(&self.cache.store_path.to_string())?; // Store fetched resource in cache
        Ok(resource)
    }
}
