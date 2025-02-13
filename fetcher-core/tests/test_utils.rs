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
