pub mod binary;

#[derive(Debug, Clone)]
/// All supported binaries
pub enum Binaries {
    Zainod,
    Lightwalletd,
    Zcashd,
    // ...
}
