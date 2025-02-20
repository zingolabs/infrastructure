pub mod binaries;

#[cfg(test)]
mod lib_tests {
    use crate::binaries;

    #[test]
    fn run_fetcher() {
        binaries::binaries_main();
        assert_eq!(true, true);
    }
}
