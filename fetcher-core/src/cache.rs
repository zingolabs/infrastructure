use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::PathBuf;

pub struct Cache {
    store_path: PathBuf,
}

impl Cache {
    pub fn new(store_path: &str) -> Self {
        let path = PathBuf::from(store_path);
        fs::create_dir_all(&path).expect("Failed to create cache directory"); // Ensure the directory exists
        Cache { store_path: path }
    }

    pub fn load(&self, key: &str) -> io::Result<Vec<u8>> {
        let path = self.get_path(key);
        let mut file = File::open(&path)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        Ok(data)
    }

    pub fn store(&self, key: &str, resource_data: &[u8]) -> io::Result<()> {
        let path = self.get_path(key);
        let mut file = File::create(&path)?;
        file.write_all(resource_data)?;
        Ok(())
    }

    pub fn exists(&self, key: &str) -> bool {
        let path = self.get_path(key);
        path.exists()
    }

    pub fn get_path(&self, key: &str) -> PathBuf {
        self.store_path.join(key)
    }

    pub fn get_store_path(&self) -> &PathBuf {
        &self.store_path
    }
}
