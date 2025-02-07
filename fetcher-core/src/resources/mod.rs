pub mod binaries;

use binaries::Binaries;

pub enum ResourceType {
    Binaries, // General binary category
}

pub enum ResourcesEnum {
    Binaries(Binaries),
}
