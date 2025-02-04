use std::{env, fs::File, io::Write, path::PathBuf};

mod binaries;
mod utils;
fn main() {
    println!("FETCHER build.rs running");
    generate_config_file();
    binaries::create_test_file_with_parents();
}

fn generate_config_file() {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR to be defined");
    let out_dir_path = PathBuf::from(&out_dir);

    // Create the generated config file
    let config_file_path = out_dir_path.join("config.rs");
    let mut file = File::create(&config_file_path).expect("config.rs to be created");
    file.write_fmt(core::format_args!(
        r#"
        pub const FETCHER_OUT_DIR: &str = {:?};
        "#,
        out_dir_path.display()
    ))
    .expect("config file to be written")
}
