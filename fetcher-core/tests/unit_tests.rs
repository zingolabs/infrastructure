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
