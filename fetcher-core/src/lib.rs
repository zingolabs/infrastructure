mod cache;

pub struct ResourcesManager {
    cache: cache::Cache,
}

impl ResourcesManager {
    pub fn new(store_path: &str) -> Self {
        let cache = cache::Cache::new(store_path);
        ResourcesManager { cache }
    }

    pub fn get_resource(
        &mut self,
        resource_type: ResourceType,
        resource_id: &str,
    ) -> Result<Box<dyn Resource>, error::Error> {
        // Check if the resource is cached
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
        resource_type: ResourceType,
        resource_id: &str,
    ) -> Result<Box<dyn Resource>, error::Error> {
        let resource = resource_type.fetch(resource_id)?;
        // Store the resource in the cache (disk)
        resource.store(&self.cache.store_path.to_string())?;
        Ok(resource)
    }
}
