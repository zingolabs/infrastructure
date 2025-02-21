use zingo_infra_fetcher_core::{Binaries, ResourcesEnum, ResourcesManager};

pub async fn binary_exists(bin: Binaries) {
    let location = "./fetched_resources";
    let mut manager = ResourcesManager::new(&location);

    match manager.get_resource(ResourcesEnum::Binaries(bin)).await {
        Err(e) => {
            println!("{:}", e);
            assert!(false)
        }
        Ok(bin_path) => {
            assert!(bin_path.exists());
            assert!(bin_path.starts_with(location));
        }
    };
}

#[tokio::test]
pub(crate) async fn zainod_exists() {
    binary_exists(Binaries::Zainod).await
}

#[tokio::test]
pub(crate) async fn lightwalletd_exists() {
    binary_exists(Binaries::Lightwalletd).await
}

#[tokio::test]
pub(crate) async fn zcashd_exists() {
    binary_exists(Binaries::Zcashd).await
}

#[tokio::test]
pub(crate) async fn zcash_cli_exists() {
    binary_exists(Binaries::ZcashCli).await
}

#[tokio::test]
pub(crate) async fn zingo_cli_exists() {
    binary_exists(Binaries::ZingoCli).await
}

#[tokio::test]
pub(crate) async fn zebrad_exists() {
    binary_exists(Binaries::Zebrad).await
}
