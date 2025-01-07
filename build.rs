use std::fs;
use std::path::Path;

fn main() {
    let out_dir = Path::new("./test_binaries");
    let dest_path = Path::new(&out_dir).join("bld_outputtest.txt");
    fs::write(&dest_path, "goodevening").unwrap();
    //println!("cargo::rerun-if-changed=build.rs");
}
