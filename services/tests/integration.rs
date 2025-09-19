mod testutils;

use zcash_protocol::consensus::BlockHeight;
use zingo_infra_services::{
    indexer::{Indexer, Lightwalletd, LightwalletdConfig, Zainod, ZainodConfig},
    utils,
    validator::{Validator, Zcashd, ZcashdConfig, Zebrad, ZebradConfig},
    LocalNet,
};

#[tokio::test]
async fn launch_zcashd() {
    tracing_subscriber::fmt().init();

    let config = ZcashdConfig::default_test();
    let zcashd = Zcashd::launch(config).await.unwrap();
    zcashd.print_stdout();
    zcashd.print_stderr();
}

#[tokio::test]
async fn launch_zcashd_custom_activation_heights() {
    tracing_subscriber::fmt().init();

    let activation_heights = zcash_protocol::local_consensus::LocalNetwork {
        overwinter: Some(BlockHeight::from(1)),
        sapling: Some(BlockHeight::from(1)),
        blossom: Some(BlockHeight::from(1)),
        heartwood: Some(BlockHeight::from(1)),
        canopy: Some(BlockHeight::from(3)),
        nu5: Some(BlockHeight::from(5)),
        nu6: Some(BlockHeight::from(7)),
        nu6_1: Some(BlockHeight::from(9)),
    };
    let mut config = ZcashdConfig::default_test();
    config.activation_heights = activation_heights;
    let zcashd = Zcashd::launch(config).await.unwrap();

    zcashd.generate_blocks(8).await.unwrap();
    zcashd.print_stdout();
    zcashd.print_stderr();
}

#[tokio::test]
async fn launch_zebrad() {
    tracing_subscriber::fmt().init();

    let config = ZebradConfig::default_test();
    let zebrad = Zebrad::launch(config).await.unwrap();
    zebrad.print_stdout();
    zebrad.print_stderr();
}

#[ignore = "temporary during refactor into workspace"]
#[tokio::test]
async fn launch_zebrad_with_cache() {
    tracing_subscriber::fmt().init();

    let mut config = ZebradConfig::default_test();
    config.chain_cache = Some(utils::chain_cache_dir().join("client_rpc_tests_large"));

    let zebrad = Zebrad::launch(config).await.unwrap();
    zebrad.print_stdout();
    zebrad.print_stderr();

    assert_eq!(zebrad.get_chain_height().await, 52.into());
}

#[tokio::test]
async fn launch_localnet_zainod_zcashd() {
    tracing_subscriber::fmt().init();

    let local_net = LocalNet::<Zainod, Zcashd>::launch(
        ZainodConfig::default_test(),
        ZcashdConfig::default_test(),
    )
    .await;

    local_net.validator().print_stdout();
    local_net.validator().print_stderr();
    local_net.indexer().print_stdout();
    local_net.indexer().print_stderr();
}

#[tokio::test]
async fn launch_localnet_zainod_zebrad() {
    tracing_subscriber::fmt().init();

    let local_net = LocalNet::<Zainod, Zebrad>::launch(
        ZainodConfig::default_test(),
        ZebradConfig::default_test(),
    )
    .await;

    local_net.validator().print_stdout();
    local_net.validator().print_stderr();
    local_net.indexer().print_stdout();
    local_net.indexer().print_stderr();
}

#[tokio::test]
async fn launch_localnet_lightwalletd_zcashd() {
    tracing_subscriber::fmt().init();

    let local_net = LocalNet::<Lightwalletd, Zcashd>::launch(
        LightwalletdConfig::default_test(),
        ZcashdConfig::default_test(),
    )
    .await;

    local_net.validator().print_stdout();
    local_net.validator().print_stderr();
    local_net.indexer().print_stdout();
    local_net.indexer().print_lwd_log();
    local_net.indexer().print_stderr();
}

#[tokio::test]
async fn launch_localnet_lightwalletd_zebrad() {
    tracing_subscriber::fmt().init();

    let local_net = LocalNet::<Lightwalletd, Zebrad>::launch(
        LightwalletdConfig::default_test(),
        ZebradConfig::default_test(),
    )
    .await;

    local_net.validator().print_stdout();
    local_net.validator().print_stderr();
    local_net.indexer().print_stdout();
    local_net.indexer().print_lwd_log();
    local_net.indexer().print_stderr();
}

#[ignore = "not a test. generates chain cache for client_rpc tests."]
#[tokio::test]
async fn generate_zebrad_large_chain_cache() {
    tracing_subscriber::fmt().init();

    crate::testutils::generate_zebrad_large_chain_cache(
        ZebradConfig::default_location(),
        LightwalletdConfig::default_location(),
    )
    .await;
}

// FIXME: This is not a test, so it shouldn't be marked as one.
// and TODO: Pre-test setups should be moved elsewhere.
#[ignore = "not a test. generates chain cache for client_rpc tests."]
#[tokio::test]
async fn generate_zcashd_chain_cache() {
    tracing_subscriber::fmt().init();

    crate::testutils::generate_zcashd_chain_cache(
        ZcashdConfig::default_location(),
        ZcashdConfig::default_cli_location(),
        LightwalletdConfig::default_location(),
    )
    .await;
}
