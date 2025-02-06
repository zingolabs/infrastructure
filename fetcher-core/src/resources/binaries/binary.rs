use crate::resources::resource::Resource;

use super::super::super::error::Error;
use super::super::ResourceType;

#[derive(Clone)]
pub struct BinaryResource {
    resource_id: String,
    data: Vec<u8>, // The binary data to be stored
    checksum: String,
    version: String,
}

impl BinaryResource {
    pub fn new(resource_id: &str, checksum: &str, version: &str) -> Self {
        BinaryResource {
            resource_id: resource_id.to_string(),
            data: Vec::new(), // Initially empty, to be filled later
            checksum: checksum.to_string(),
            version: version.to_string(),
        }
    }

    // Example fetch logic
    fn fetch_from_source(resource_id: &str) -> Result<Vec<u8>, Error> {
        // Simulated fetching logic (e.g., download or retrieve from a database)
        println!("Fetching binary: {}", resource_id);
        Ok(vec![0u8; 1024]) // Simulating 1 KB of binary data
    }

    fn verify_checksum(&self) -> bool {
        // Placeholder logic for checksum verification
        // Actual implementation could calculate a checksum of self.data
        self.checksum == "expected_checksum" // Replace with real validation logic
    }
}

impl Resource for BinaryResource {
    fn fetch(resource_id: &str) -> Result<Self, Error> {
        let checksum = "expected_checksum"; // Replace with actual retrieval logic
        let version = "1.0.0"; // Replace with actual retrieval logic

        let data = Self::fetch_from_source(resource_id)?;
        let mut binary_resource = BinaryResource::new(resource_id, checksum, version);
        binary_resource.data = data; // Set fetched data

        Ok(binary_resource)
    }

    fn store(&self, store_path: &str) -> Result<(), Error> {
        // Save binary data to disk
        let key = format!("{}.bin", self.resource_id);
        let cache = super::super::super::cache::Cache::new(store_path);
        cache.store(&key, &self.data).map_err(Error::IoError)?;
        Ok(())
    }

    fn verify(&self) -> bool {
        self.verify_checksum()
    }

    fn load_from_data(data: Vec<u8>) -> Result<Self, Error> {
        // Logic to load a BinaryResource from existing data
        Ok(BinaryResource {
            resource_id: String::from("loaded_resource_id"), // Set appropriately
            data,
            checksum: "expected_checksum".to_string(), // Set the expected checksum
            version: "1.0.0".to_string(),              // Set the expected version
        })
    }
}
