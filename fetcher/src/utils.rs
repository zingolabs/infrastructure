use std::{env::var, path::PathBuf, str::FromStr};

pub(crate) fn get_out_dir() -> PathBuf {
    let env_var = get_out_dir_env_var();
    PathBuf::from_str(&env_var).expect("OUT_DIR to be parsed into PathBuf")
}

pub(crate) fn get_out_dir_env_var() -> String {
    var("OUT_DIR").expect("OUT_DIR to be defined")
}
