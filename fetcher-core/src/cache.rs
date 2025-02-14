use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::PathBuf;

/// This struct handles all the actual filesystem interaction (dir where the resources are to be stored)
pub struct Cache {
    store_path: PathBuf,
}

impl Cache {
    /// Creates a new [`Cache`].
    ///
    /// * store_path: The directory where the files are to be cached into
    ///
    pub fn new(store_path: &str) -> Self {
        let path = PathBuf::from(store_path);
        fs::create_dir_all(&path).expect("Failed to create cache directory"); // Ensure the directory exists
        Cache { store_path: path }
    }

    /// Loads some file from the cached directory
    ///
    /// * key: the unique key idenifier of the resource
    ///
    /// # Examples
    /// ```
    /// let zcashd_data = cache.load("binaries_zcashd");
    /// ```
    pub fn load(&self, key: &str) -> io::Result<Vec<u8>> {
        let path = self.get_path(key);
        let mut file = File::open(&path)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        Ok(data)
    }

    /// Stores data for the specified resource
    ///
    /// * key: the unique key identifier of the resource
    ///
    /// # Examples
    /// ```
    /// cache.store("binaries_zcashd",[12,0,13]);
    /// ```
    pub fn store(&self, key: &str, resource_data: &[u8]) -> io::Result<()> {
        let path = self.store_path.join(key);
        let mut file = File::create(&path)?;
        file.write_all(resource_data)?;
        Ok(())
    }

    /// Checks whether the resource exists in the cache
    ///
    /// * key: the unique key identifier of the resource
    pub fn exists(&self, key: &str) -> bool {
        let path = self.store_path.join(key);
        path.exists()
    }

    /// Returns the path to the resource for a specific resource
    ///
    /// * key: the unique key identifier of the resource    
    pub fn get_path(&self, key: &str) -> PathBuf {
        self.store_path.join(key)
    }

    pub fn get_store_path(&self) -> &PathBuf {
        &self.store_path
    }
}
