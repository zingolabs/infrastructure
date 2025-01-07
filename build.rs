fn main() {
    // Tell Cargo that if the given file changes, to rerun this build script.
    println!("cargo::rerun-if-changed=test_binaries/.gitignore");
    println!("binary changed");
}
