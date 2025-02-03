mod binaries;
mod utils;
fn main() {
    println!("FETCHER build.rs running");
    binaries::main();
    set_out_dir_env_var();
}

fn set_out_dir_env_var() {
    let out_dir = utils::get_out_dir();
    println!("cargo:rustc-env=FETCHER_OUT_DIR={:?}", out_dir)
}
