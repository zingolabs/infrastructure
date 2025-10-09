mod testutils;

use zcash_local_net::indexer::lightwalletd::Lightwalletd;
use zcash_local_net::process::ItsAProcess;
use zcash_local_net::validator::Validator as _;
use zcash_local_net::{
    indexer::{
        empty::{Empty, EmptyConfig},
        zainod::Zainod,
    },
    process::ItsAProcess as _,
    utils,
    validator::{
        zcashd::Zcashd,
        zebrad::{Zebrad, ZebradConfig},
    },
    LocalNet,
};

async fn launch_default_and_print_all<P: ItsAProcess>() {
    tracing_subscriber::fmt().init();

    let p = P::launch_default().await.expect("Process launching!");
    p.print_all();
}

#[tokio::test]
async fn launch_zcashd() {
    launch_default_and_print_all::<Zcashd>().await;
}

#[tokio::test]
async fn launch_zcashd_custom_activation_heights() {
    tracing_subscriber::fmt().init();

    let zcashd = Zcashd::launch_default().await.unwrap();

    zcashd.generate_blocks(8).await.unwrap();
    zcashd.print_all();
}

#[tokio::test]
async fn launch_zebrad() {
    launch_default_and_print_all::<Zebrad>().await;
}

#[ignore = "temporary during refactor into workspace"]
#[tokio::test]
async fn launch_zebrad_with_cache() {
    tracing_subscriber::fmt().init();

    let mut config = ZebradConfig::default();
    config.chain_cache = Some(utils::chain_cache_dir().join("client_rpc_tests_large"));

    let zebrad = Zebrad::launch(config).await.unwrap();
    zebrad.print_all();

    assert_eq!(zebrad.get_chain_height().await, 52u32);
}

#[ignore = "requires chain cache to be generated"]
/// Asserts that launching 2 `zebrad` instances with the same cache fails.
/// The second instance cannot open the database, due to it already being in use by the first instance.
#[tokio::test]
async fn launch_multiple_individual_zebrads_with_cache() {
    tracing_subscriber::fmt().init();
    let mut config = ZebradConfig::default();
    config.chain_cache = Some(utils::chain_cache_dir().join("client_rpc_tests_large"));

    let zebrad_1 = Zebrad::launch(config.clone()).await.unwrap();
    zebrad_1.print_all();

    let zebrad_2 = Zebrad::launch(config).await.unwrap();
    zebrad_2.print_all();

    assert_eq!(zebrad_1.get_chain_height().await, 52u32);
    assert_eq!(zebrad_2.get_chain_height().await, 52u32);
}

#[ignore = "requires chain cache to be generated"]
/// Tests that 2 `zebrad` instances, each with a copy of the chain cache, can be launched.
#[tokio::test]
async fn localnet_launch_multiple_zebrads_with_cache() {
    tracing_subscriber::fmt().init();

    let chain_cache_source = utils::chain_cache_dir().join("client_rpc_tests_large");

    let mut zebrad_config = ZebradConfig::default();
    zebrad_config.chain_cache = Some(chain_cache_source);

    let local_net_1 =
        LocalNet::<Empty, Zebrad>::launch(EmptyConfig {}, zebrad_config.clone()).await;

    let local_net_2 = LocalNet::<Empty, Zebrad>::launch(EmptyConfig {}, zebrad_config).await;

    let zebrad_1 = local_net_1.validator();
    let zebrad_2 = local_net_2.validator();

    assert_eq!(zebrad_1.get_chain_height().await, 52u32);
    assert_eq!(zebrad_2.get_chain_height().await, 52u32);

    zebrad_1.print_all();
    zebrad_2.print_all();
}

#[tokio::test]
async fn launch_localnet_zainod_zcashd() {
    launch_default_and_print_all::<LocalNet<Zainod, Zcashd>>().await;
}

#[tokio::test]
async fn launch_localnet_zainod_zebrad() {
    launch_default_and_print_all::<LocalNet<Zainod, Zebrad>>().await;
}

#[tokio::test]
async fn launch_localnet_lightwalletd_zcashd() {
    launch_default_and_print_all::<LocalNet<Lightwalletd, Zcashd>>().await;
}

#[tokio::test]
async fn launch_localnet_lightwalletd_zebrad() {
    launch_default_and_print_all::<LocalNet<Lightwalletd, Zebrad>>().await;
}

#[ignore = "not a test. generates chain cache for client_rpc tests."]
#[tokio::test]
async fn generate_zebrad_large_chain_cache() {
    tracing_subscriber::fmt().init();

    crate::testutils::generate_zebrad_large_chain_cache().await;
}

// FIXME: This is not a test, so it shouldn't be marked as one.
// and TODO: Pre-test setups should be moved elsewhere.
#[ignore = "not a test. generates chain cache for client_rpc tests."]
#[tokio::test]
async fn generate_zcashd_chain_cache() {
    tracing_subscriber::fmt().init();

    crate::testutils::generate_zcashd_chain_cache().await;
}
