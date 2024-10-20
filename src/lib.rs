use std::{path::PathBuf, process::Child};

use tempfile::TempDir;

pub mod launch;
pub mod network;

pub struct Zcashd {
    handle: Child,
    data_dir: TempDir,
    logs_dir: TempDir,
    config_dir: TempDir,
}
