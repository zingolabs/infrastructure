use zcash_local_net::indexer::lightwalletd::{Lightwalletd, LightwalletdConfig};
use zcash_local_net::indexer::zainod::ZainodConfig;
use zcash_local_net::process::Process;
use zcash_local_net::validator::zcashd::ZcashdConfig;
use zcash_local_net::validator::Validator as _;
use zcash_local_net::LocalNetConfig;
use zcash_local_net::{
    indexer::{
        empty::{Empty, EmptyConfig},
        zainod::Zainod,
    },
    utils,
    validator::{
        zcashd::Zcashd,
        zebrad::{Zebrad, ZebradConfig},
    },
    LocalNet,
};
use zebra_chain::parameters::testnet::ConfiguredActivationHeights;
use zebra_chain::parameters::NetworkKind;
use zingo_test_vectors::{REG_O_ADDR_FROM_ABANDONART, ZEBRAD_DEFAULT_MINER};

// TODO: remove after we have fully updated to depend on zebra 3.0.0.
// temporarily sets nu5+ to a height of 2 due to a zebra bug that has been fixed in 3.0.0
const TEMP_ZCASHD_CONFIG: ZcashdConfig = ZcashdConfig {
    rpc_listen_port: None,
    configured_activation_heights: ConfiguredActivationHeights {
        before_overwinter: Some(1),
        overwinter: Some(1),
        sapling: Some(1),
        blossom: Some(1),
        heartwood: Some(1),
        canopy: Some(1),
        nu5: Some(2),
        nu6: Some(2),
        nu6_1: Some(2),
        nu7: None,
    },
    miner_address: Some(REG_O_ADDR_FROM_ABANDONART),
    chain_cache: None,
};
const TEMP_ZEBRAD_CONFIG: ZebradConfig = ZebradConfig {
    network_listen_port: None,
    rpc_listen_port: None,
    indexer_listen_port: None,
    configured_activation_heights: ConfiguredActivationHeights {
        before_overwinter: Some(1),
        overwinter: Some(1),
        sapling: Some(1),
        blossom: Some(1),
        heartwood: Some(1),
        canopy: Some(1),
        nu5: Some(2),
        nu6: Some(2),
        nu6_1: Some(2),
        nu7: None,
    },
    miner_address: ZEBRAD_DEFAULT_MINER,
    chain_cache: None,
    network: NetworkKind::Regtest,
};

async fn launch_default_and_print_all<P: Process>() {
    let p = P::launch_default().await.expect("Process launching!");
    p.print_all();
}

#[tokio::test]
async fn launch_zcashd() {
    tracing_subscriber::fmt().init();

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
    tracing_subscriber::fmt().init();

    launch_default_and_print_all::<Zebrad>().await;
}

#[ignore = "temporary during refactor into workspace"]
#[tokio::test]
async fn launch_zebrad_with_cache() {
    tracing_subscriber::fmt().init();

    let config = ZebradConfig {
        chain_cache: Some(utils::chain_cache_dir().join("client_rpc_tests_large")),
        ..Default::default()
    };

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
    let config = ZebradConfig {
        chain_cache: Some(utils::chain_cache_dir().join("client_rpc_tests_large")),
        ..Default::default()
    };

    let zebrad_1 = Zebrad::launch(config.clone()).await.unwrap();
    zebrad_1.print_all();

    let zebrad_2 = Zebrad::launch(config).await.unwrap();
    zebrad_2.print_all();

    assert_eq!(zebrad_1.get_chain_height().await, 52u32);
    assert_eq!(zebrad_2.get_chain_height().await, 52u32);
}

#[ignore = "requires chain cache to be generated. see `client_rpc_test_fixtures` crate"]
/// Tests that 2 `zebrad` instances, each with a copy of the chain cache, can be launched.
#[tokio::test]
async fn localnet_launch_multiple_zebrads_with_cache() {
    tracing_subscriber::fmt().init();

    let config = ZebradConfig {
        chain_cache: Some(utils::chain_cache_dir().join("client_rpc_tests_large")),
        ..Default::default()
    };

    let local_net_1 = LocalNet::<Zebrad, Empty>::launch(LocalNetConfig {
        indexer_config: EmptyConfig {},
        validator_config: config.clone(),
    })
    .await
    .unwrap();

    let local_net_2 = LocalNet::<Zebrad, Empty>::launch(LocalNetConfig {
        indexer_config: EmptyConfig {},
        validator_config: config.clone(),
    })
    .await
    .unwrap();

    let zebrad_1 = local_net_1.validator();
    let zebrad_2 = local_net_2.validator();

    assert_eq!(zebrad_1.get_chain_height().await, 52u32);
    assert_eq!(zebrad_2.get_chain_height().await, 52u32);

    zebrad_1.print_all();
    zebrad_2.print_all();
}

// NOTE: the following tests use a temporary validator config to set nu5+ to a height of 2 due to a zebra bug that has been fixed in 3.0.0
// when we are fully updated to zebra 3.0.0 we can set back to use the launch defaults

#[tokio::test]
async fn launch_localnet_zainod_zcashd() {
    tracing_subscriber::fmt().init();

    let p = LocalNet::<Zcashd, Zainod>::launch(LocalNetConfig {
        validator_config: TEMP_ZCASHD_CONFIG,
        indexer_config: ZainodConfig::default(),
    })
    .await
    .expect("Process launching!");
    p.print_all();
    // launch_default_and_print_all::<LocalNet<Zcashd, Zainod>>().await;
}

#[tokio::test]
async fn launch_localnet_zainod_zebrad() {
    tracing_subscriber::fmt().init();

    let p = LocalNet::<Zebrad, Zainod>::launch(LocalNetConfig {
        validator_config: TEMP_ZEBRAD_CONFIG,
        indexer_config: ZainodConfig::default(),
    })
    .await
    .expect("Process launching!");
    p.print_all();
    // launch_default_and_print_all::<LocalNet<Zebrad, Zainod>>().await;
}

#[tokio::test]
async fn launch_localnet_lightwalletd_zcashd() {
    tracing_subscriber::fmt().init();

    let p = LocalNet::<Zcashd, Lightwalletd>::launch(LocalNetConfig {
        validator_config: TEMP_ZCASHD_CONFIG,
        indexer_config: LightwalletdConfig::default(),
    })
    .await
    .expect("Process launching!");
    p.print_all();
    // launch_default_and_print_all::<LocalNet<Zcashd, Lightwalletd>>().await;
}

#[tokio::test]
async fn launch_localnet_lightwalletd_zebrad() {
    tracing_subscriber::fmt().init();

    let p = LocalNet::<Zebrad, Lightwalletd>::launch(LocalNetConfig {
        validator_config: TEMP_ZEBRAD_CONFIG,
        indexer_config: LightwalletdConfig::default(),
    })
    .await
    .expect("Process launching!");
    p.print_all();
    // launch_default_and_print_all::<LocalNet<Zebrad, Lightwalletd>>().await;
}
