pub trait BinaryConfig {
    fn resource_id(&self) -> &str;
    fn checksum(&self) -> &str;
    fn version(&self) -> &str;
    fn fetch_path(&self) -> &str;
}

pub struct BinaryAConfig;
impl BinaryConfig for BinaryAConfig {
    fn resource_id(&self) -> &str {
        "BinaryA"
    }
    fn checksum(&self) -> &str {
        "abc123"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }
    fn fetch_path(&self) -> &str {
        "url/to/binary_a"
    }
}

// Continue for BinaryBConfig, BinaryCConfig, etc.
