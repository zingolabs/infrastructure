use zingo_infra_fetcher_core::Binaries;
mod test_utils;

#[tokio::test]
pub(crate) async fn zainod_exists() {
    test_utils::binary_exists(Binaries::Zainod).await
}

#[tokio::test]
pub(crate) async fn lightwalletd_exists() {
    test_utils::binary_exists(Binaries::Lightwalletd).await
}

#[tokio::test]
pub(crate) async fn zcashd_exists() {
    test_utils::binary_exists(Binaries::Zcashd).await
}

#[tokio::test]
pub(crate) async fn zcash_cli_exists() {
    test_utils::binary_exists(Binaries::ZcashCli).await
}

#[tokio::test]
pub(crate) async fn zingo_cli_exists() {
    test_utils::binary_exists(Binaries::ZingoCli).await
}

#[tokio::test]
pub(crate) async fn zebrad_exists() {
    test_utils::binary_exists(Binaries::Zebrad).await
}

