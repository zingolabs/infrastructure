use std::env;

pub fn get_out_dir() -> String {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR to be defined");
    out_dir
}
