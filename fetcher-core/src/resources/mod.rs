pub mod binaries;
//pub mod resource; // The base Resource trait // For general Binary resource types

use binaries::Binaries;

pub enum ResourceType {
    Binaries, // General binary category
}

pub enum ResourcesEnum {
    Binaries(Binaries),
}
