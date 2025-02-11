#[derive(Debug)]
pub enum Error {
    IoError(std::io::Error),
    ResourceNotFound,
    InvalidResource,
    InvalidShasumFile,
    // Add other relevant error types as needed
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for Error {}
