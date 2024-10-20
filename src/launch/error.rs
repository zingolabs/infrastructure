/// Errors associated with launching processes
#[derive(thiserror::Error, Debug, Clone)]
pub enum LaunchError {
    /// Failed to launch Zcashd
    #[error(
        "Failed to launch Zcashd.\nExit status: {exit_status}\nStdout: {stdout}\nStderr: {stderr}"
    )]
    Zcashd {
        /// Error code
        exit_status: std::process::ExitStatus,
        /// Stdout log
        stdout: String,
        /// Stderr log
        stderr: String,
    },
}
