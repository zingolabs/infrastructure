use std::{fs::File, io::Read as _, path::PathBuf, process::Child};

use tempfile::TempDir;

use crate::{error::LaunchError, logs, Process};

/// Wait until the process logs indicate the launch has succeeded or failed.
pub(crate) fn wait(
    process: Process,
    handle: &mut Child,
    logs_dir: &TempDir,
    additional_log_path: Option<PathBuf>,
    success_indicator: &str,
    error_indicator: &str,
) -> Result<(), LaunchError> {
    let stdout_log_path = logs_dir.path().join(logs::STDOUT_LOG);
    let mut stdout_log = File::open(stdout_log_path).expect("should be able to open log");
    let mut stdout = String::new();

    let stderr_log_path = logs_dir.path().join(logs::STDERR_LOG);
    let mut stderr_log = File::open(stderr_log_path).expect("should be able to open log");
    let mut stderr = String::new();

    let (mut additional_log_file, mut additional_log) = if let Some(log_path) = additional_log_path
    {
        let log_file = File::open(log_path).expect("should be able to open log");
        let log = String::new();

        (Some(log_file), Some(log))
    } else {
        (None, None)
    };

    // wait for stdout log entry that indicates daemon is ready
    let interval = std::time::Duration::from_millis(100);
    loop {
        match handle.try_wait() {
            Ok(Some(exit_status)) => {
                stdout_log.read_to_string(&mut stdout).unwrap();
                stderr_log.read_to_string(&mut stderr).unwrap();

                return Err(LaunchError::ProcessFailed {
                    process_name: process.to_string(),
                    exit_status,
                    stdout,
                    stderr,
                });
            }
            Ok(None) => (),
            Err(e) => {
                panic!("Unexpected Error: {e}")
            }
        };

        stdout_log.read_to_string(&mut stdout).unwrap();
        stderr_log.read_to_string(&mut stderr).unwrap();
        if stdout.contains(error_indicator) || stderr.contains(error_indicator) {
            panic!("{} launch failed without reporting an error code!\nexiting with panic. you may have to shut the daemon down manually.", process);
        } else if stdout.contains(success_indicator) {
            // launch successful
            break;
        }

        if additional_log_file.is_some() {
            let mut log_file = additional_log_file
                .take()
                .expect("additional log exists in this scope");
            let mut log = additional_log
                .take()
                .expect("additional log exists in this scope");

            log_file.read_to_string(&mut log).unwrap();
            if log.contains(success_indicator) {
                // launch successful
                break;
            } else {
                additional_log_file = Some(log_file);
                additional_log = Some(log);
            }
        }

        std::thread::sleep(interval);
    }

    Ok(())
}
