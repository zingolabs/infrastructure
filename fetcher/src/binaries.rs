use crate::utils::get_out_dir;
use std::{
    fs::{self, File},
    io::Write,
    path::PathBuf,
};

pub fn get_binaries_dir() -> PathBuf {
    let out_dir = get_out_dir();
    out_dir.join("test_binaries")
}

// The following functions are only to try out the behaviour of files in OUT_DIR and accesing them from other crates
pub fn create_test_file_with_parents() {
    let file_path = get_test_file_path();

    // Create the parent directory if it does not exist
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).expect("Failed to create parent directories");
    }

    // Create the file
    let mut file: File = File::create(file_path).expect("file to be created/read");
    file.write_all(b"Dummy content")
        .expect("file content to be written");
}

pub fn get_test_file_path() -> PathBuf {
    let file_path = get_binaries_dir().join("dummy_file");
    // Example file path
    file_path
}
