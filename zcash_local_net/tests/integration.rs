mod testutils;

use zcash_local_net::indexer::lightwalletd::Lightwalletd;
use zcash_local_net::logs::LogsToStdoutAndStderr as _;
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

#[tokio::test]
async fn launch_zcashd() {
    tracing_subscriber::fmt().init();

    let zcashd = Zcashd::launch_default().await.unwrap();
    zcashd.print_stdout();
    zcashd.print_stderr();
}

#[tokio::test]
async fn launch_zcashd_custom_activation_heights() {
    tracing_subscriber::fmt().init();

    let zcashd = Zcashd::launch_default().await.unwrap();

    zcashd.generate_blocks(8).await.unwrap();
    zcashd.print_stdout();
    zcashd.print_stderr();
}

#[tokio::test]
async fn launch_zebrad() {
    tracing_subscriber::fmt().init();

    let zebrad = Zebrad::launch_default().await.unwrap();
    zebrad.print_stdout();
    zebrad.print_stderr();
}

#[ignore = "temporary during refactor into workspace"]
#[tokio::test]
async fn launch_zebrad_with_cache() {
    tracing_subscriber::fmt().init();

    let mut config = ZebradConfig::default();
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
async fn launch_multiple_individual_zebrads_with_cache() {
    tracing_subscriber::fmt().init();
    let mut config = ZebradConfig::default();
    config.chain_cache = Some(utils::chain_cache_dir().join("client_rpc_tests_large"));

    let zebrad_1 = Zebrad::launch(config.clone()).await.unwrap();
    zebrad_1.print_stdout();
    zebrad_1.print_stderr();

    let zebrad_2 = Zebrad::launch(config).await.unwrap();
    zebrad_2.print_stdout();
    zebrad_2.print_stderr();

    assert_eq!(zebrad_1.get_chain_height().await, 52.into());
    assert_eq!(zebrad_2.get_chain_height().await, 52.into());
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

    let local_net = LocalNet::<Zainod, Zcashd>::launch_default().await;

    local_net.validator().print_stdout();
    local_net.validator().print_stderr();
    local_net.indexer().print_stdout();
    local_net.indexer().print_stderr();
}

#[tokio::test]
async fn launch_localnet_zainod_zebrad() {
    tracing_subscriber::fmt().init();

    let local_net = LocalNet::<Zainod, Zebrad>::launch_default().await;
    local_net.validator().print_stdout();
    local_net.validator().print_stderr();
    local_net.indexer().print_stdout();
    local_net.indexer().print_stderr();
}

#[tokio::test]
async fn launch_localnet_lightwalletd_zcashd() {
    tracing_subscriber::fmt().init();

    let local_net = LocalNet::<Lightwalletd, Zcashd>::launch_default().await;
    local_net.validator().print_stdout();
    local_net.validator().print_stderr();
    local_net.indexer().print_stdout();
    local_net.indexer().print_lwd_log();
    local_net.indexer().print_stderr();
}

#[tokio::test]
async fn launch_localnet_lightwalletd_zebrad() {
    tracing_subscriber::fmt().init();

    let local_net = LocalNet::<Lightwalletd, Zebrad>::launch_default().await;
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
