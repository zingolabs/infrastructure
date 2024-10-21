use std::{fs::File, io::Read, path::PathBuf};

use error::LaunchError;
use portpicker::Port;

use crate::{network::ActivationHeights, Zcashd, ZCASHD_STDOUT_LOG};

pub(crate) mod config;
mod error;

/// Checks `fixed_port` is not in use.
/// If `fixed_port` is `None`, returns a random free port between 15_000 and 25_000.
fn pick_unused_port(fixed_port: Option<Port>) -> Port {
    if let Some(port) = fixed_port {
        if !portpicker::is_free(port) {
            panic!("Fixed port is not free!");
        };
        port
    } else {
        portpicker::pick_unused_port().expect("No ports free!")
    }
}

/// Launches Zcashd and returns a [`crate::Zcashd`] with the handle and associated directories.
///
/// Use `zcashd_bin` and `zcash_cli_bin` to specify the paths to the binaries.
/// If these binaries are in $PATH, `None` can be specified to run "zcashd" / "zcash-cli".
///
/// Use `fixed_port` to specify a port for zcashd. Otherwise, a port is picked at random.
///
/// Use `activation_heights` to specify custom network upgrade activation heights
///
/// Use `miner_address` to specify the target address for the block rewards when blocks are generated.  
pub fn zcashd(
    zcashd_bin: Option<PathBuf>,
    zcash_cli_bin: Option<PathBuf>,
    fixed_port: Option<Port>,
    activation_heights: &ActivationHeights,
    miner_address: Option<&str>,
) -> Result<Zcashd, LaunchError> {
    let data_dir = tempfile::tempdir().unwrap();

    let config_dir = tempfile::tempdir().unwrap();
    let config_file_path = config::zcashd(
        config_dir.path(),
        pick_unused_port(fixed_port),
        activation_heights,
        miner_address,
    )
    .unwrap();

    let mut command = match zcashd_bin {
        Some(path) => std::process::Command::new(path),
        None => std::process::Command::new("zcashd"),
    };
    command
        .args([
            "--printtoconsole",
            format!(
                "--conf={}",
                config_file_path.to_str().expect("should be valid UTF-8")
            )
            .as_str(),
            format!(
                "--datadir={}",
                data_dir.path().to_str().expect("should be valid UTF-8")
            )
            .as_str(),
            "-debug=1",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut handle = command.spawn().unwrap();

    let logs_dir = tempfile::tempdir().unwrap();
    let stdout_log_path = logs_dir.path().join(ZCASHD_STDOUT_LOG);
    let mut stdout_log = File::create(&stdout_log_path).unwrap();
    let mut stdout = handle.stdout.take().unwrap();
    // TODO: consider writing logs in a runtime to increase performance
    std::thread::spawn(move || {
        std::io::copy(&mut stdout, &mut stdout_log)
            .expect("should be able to read/write stdout log");
    });

    let mut stdout_log = File::open(stdout_log_path).expect("should be able to open log");
    let mut stdout = String::new();

    let check_interval = std::time::Duration::from_millis(100);

    // wait for string that indicates daemon is ready
    loop {
        match handle.try_wait() {
            Ok(Some(exit_status)) => {
                stdout_log.read_to_string(&mut stdout).unwrap();

                let mut stderr = String::new();
                handle
                    .stderr
                    .take()
                    .unwrap()
                    .read_to_string(&mut stderr)
                    .unwrap();

                return Err(LaunchError::Zcashd {
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
        if stdout.contains("Error:") {
            panic!("Zcashd launch failed without reporting an error code!\nexiting with panic. you may have to shut the daemon down manually.");
        } else if stdout.contains("init message: Done loading") {
            // launch successful
            break;
        }

        std::thread::sleep(check_interval);
    }

    Ok(Zcashd {
        handle,
        _data_dir: data_dir,
        logs_dir,
        config_dir,
        zcash_cli_bin,
    })
}
