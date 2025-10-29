//! Common strategies for logging.

use std::{fs::File, io::Read, path::PathBuf, process::Child};

use tempfile::TempDir;

pub(crate) const STDOUT_LOG: &str = "stdout.log";
pub(crate) const STDERR_LOG: &str = "stderr.log";
pub(crate) const LIGHTWALLETD_LOG: &str = "lwd.log";

/// Print the log file in `log_path`
pub(crate) fn print_log(log_path: PathBuf) {
    let mut log_file = File::open(log_path).unwrap();
    let mut log = String::new();
    log_file.read_to_string(&mut log).unwrap();
    tracing::info!("{log}");
}

/// Write the stdout and stderr log of the `handle` to the `logs_dir`
pub(crate) fn write_logs(handle: &mut Child, logs_dir: &TempDir) {
    let stdout_log_path = logs_dir.path().join(STDOUT_LOG);
    let mut stdout_log = File::create(&stdout_log_path).unwrap();
    let mut stdout = handle.stdout.take().unwrap();
    std::thread::spawn(move || std::io::copy(&mut stdout, &mut stdout_log).unwrap());

    let stderr_log_path = logs_dir.path().join(STDERR_LOG);
    let mut stderr_log = File::create(&stderr_log_path).unwrap();
    let mut stderr = handle.stderr.take().unwrap();
    std::thread::spawn(move || std::io::copy(&mut stderr, &mut stderr_log).unwrap());
}

/// Uses log files in a log dir to log.
pub trait LogsToDir {
    /// Get temporary logs directory.
    fn logs_dir(&self) -> &TempDir;
}

/// Uses a common pattern for logging.
pub trait LogsToStdoutAndStderr {
    /// Prints the stdout log.
    fn print_stdout(&self);
    /// Prints the stdout log.
    fn print_stderr(&self);
}

impl<T: LogsToDir> LogsToStdoutAndStderr for T {
    /// Prints the stdout log.
    fn print_stdout(&self) {
        let stdout_log_path = self.logs_dir().path().join(STDOUT_LOG);
        print_log(stdout_log_path);
    }

    /// Prints the stdout log.
    fn print_stderr(&self) {
        let stdout_log_path = self.logs_dir().path().join(STDERR_LOG);
        print_log(stdout_log_path);
    }
}
