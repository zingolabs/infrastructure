pub mod binaries;
pub mod resource; // The base Resource trait // For general Binary resource types

pub enum ResourceType {
    Binaries, // General binary category
}

impl ResourceType {
    pub fn fetch(
        &self,
        resource_id: &str,
    ) -> Result<Box<dyn resource::Resource>, crate::error::Error> {
        match self {
            ResourceType::Binaries => {
                binaries::binary::BinaryResource::fetch(resource_id).map(|res| Box::new(res))
            }
        }
    }

    pub fn load_from_data(
        &self,
        data: Vec<u8>,
    ) -> Result<Box<dyn resource::Resource>, crate::error::Error> {
        match self {
            ResourceType::Binaries => {
                binaries::binary::BinaryResource::load_from_data(data).map(|res| Box::new(res))
            }
        }
    }
}
