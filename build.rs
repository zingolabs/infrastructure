use std::env;
use std::fs;
use std::path::Path;

fn main() {
    //let out_dir = env::var_os("OUT_DIR").unwrap();
    let out_dir = Path::new(".");
    let dest_path = Path::new(&out_dir).join("build.rs.test.txt");
    fs::write(&dest_path, "goodevening").unwrap();
    //println!("cargo::rerun-if-changed=build.rs");
}
