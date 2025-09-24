mod testutils;

use zcash_protocol::consensus::BlockHeight;
use zingo_infra_services::{
    indexer::{
        Empty, EmptyConfig, Indexer, Lightwalletd, LightwalletdConfig, Zainod, ZainodConfig,
    },
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

#[ignore = "requires chain cache to be generated"]
/// Asserts that launching 2 `zebrad` instances with the same cache fails.
/// The second instance cannot open the database, due to it already being in use by the first instance.
#[tokio::test]
async fn launch_multiple_zebrads_with_cache_fails() {
    tracing_subscriber::fmt().init();
    let mut config_1 = ZebradConfig::default_test();
    config_1.chain_cache = Some(utils::chain_cache_dir().join("client_rpc_tests_large"));

    let zebrad_1 = Zebrad::launch(config_1).await.unwrap();
    zebrad_1.print_stdout();
    zebrad_1.print_stderr();

    let mut config_2 = ZebradConfig::default_test();
    config_2.chain_cache = Some(utils::chain_cache_dir().join("client_rpc_tests_large"));

    let zebrad_2 = Zebrad::launch(config_2).await;

    assert_eq!(zebrad_2.is_err(), true);
}

#[ignore = "requires chain cache to be generated"]
/// Tests that 2 `zebrad` instances, each with a copy of the chain cache, can be launched.
#[tokio::test]
async fn localnet_launch_multiple_zebrads_with_cache() {
    tracing_subscriber::fmt().init();

    let local_net_1 = LocalNet::<Empty, Zebrad>::launch_with_chain_cache(
        EmptyConfig {},
        ZebradConfig::default_test(),
        utils::chain_cache_dir().join("client_rpc_tests_large"),
    )
    .await;

    let local_net_2 = LocalNet::<Empty, Zebrad>::launch_with_chain_cache(
        EmptyConfig {},
        ZebradConfig::default_test(),
        utils::chain_cache_dir().join("client_rpc_tests_large"),
    )
    .await;

    let zebrad_1 = local_net_1.validator();
    let zebrad_2 = local_net_2.validator();

    assert_eq!(zebrad_1.get_chain_height().await, 52.into());
    assert_eq!(zebrad_2.get_chain_height().await, 52.into());

    zebrad_1.print_stdout();
    zebrad_1.print_stderr();
    zebrad_2.print_stdout();
    zebrad_2.print_stderr();
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
