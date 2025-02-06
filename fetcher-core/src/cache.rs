use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::PathBuf;

pub struct Cache {
    store_path: PathBuf,
}

impl Cache {
    // Initialize the cache with a store path where resources will be saved.
    pub fn new(store_path: &str) -> Self {
        let path = PathBuf::from(store_path);
        fs::create_dir_all(&path).expect("Failed to create cache directory"); // Ensure the directory exists
        Cache { store_path: path }
    }

    // Load a resource from the cache
    pub fn load(&self, key: &str) -> io::Result<Vec<u8>> {
        let path = self.store_path.join(key);
        let mut file = File::open(&path)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        Ok(data)
    }

    // Store a resource in the cache
    pub fn store(&self, key: &str, resource_data: &[u8]) -> io::Result<()> {
        let path = self.store_path.join(key);
        let mut file = File::create(&path)?;
        file.write_all(resource_data)?;
        Ok(())
    }

    // Check if a resource exists in the cache
    pub fn exists(&self, key: &str) -> bool {
        let path = self.store_path.join(key);
        path.exists()
    }
}
