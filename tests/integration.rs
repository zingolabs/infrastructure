use std::path::PathBuf;

use portpicker::Port;
use zcash_local_net::{
    indexer::{Indexer as _, Lightwalletd, LightwalletdConfig, Zainod, ZainodConfig},
    network,
    validator::{Validator as _, Zcashd, ZcashdConfig},
};
use zcash_protocol::{PoolType, ShieldedProtocol};
use zingolib::{
    config::RegtestNetwork,
    lightclient::LightClient,
    testutils::{
        lightclient::{from_inputs, get_base_address},
        scenarios::setup::ClientBuilder,
    },
    testvectors::{seeds, REG_O_ADDR_FROM_ABANDONART},
};

async fn build_lightclients(
    lightclient_dir: PathBuf,
    indexer_port: Port,
) -> (LightClient, LightClient) {
    let mut client_builder =
        ClientBuilder::new(network::localhost_uri(indexer_port), lightclient_dir);
    let faucet = client_builder
        .build_faucet(true, RegtestNetwork::all_upgrades_active())
        .await;
    let recipient = client_builder
        .build_client(
            seeds::HOSPITAL_MUSEUM_SEED.to_string(),
            1,
            true,
            RegtestNetwork::all_upgrades_active(),
        )
        .await;

    (faucet, recipient)
}

#[test]
fn launch_zcashd() {
    tracing_subscriber::fmt().init();

    let zcashd = Zcashd::default();
    zcashd.print_stdout();
    zcashd.print_stderr();
}

#[test]
fn launch_zainod() {
    tracing_subscriber::fmt().init();

    let zcashd = Zcashd::default();
    let zainod = Zainod::launch(ZainodConfig {
        zainod_bin: None,
        listen_port: None,
        validator_port: zcashd.port(),
    })
    .unwrap();

    zcashd.print_stdout();
    zcashd.print_stderr();
    zainod.print_stdout();
    zainod.print_stderr();
}

#[test]
fn launch_lightwalletd() {
    tracing_subscriber::fmt().init();

    let zcashd = Zcashd::default();
    let lwd = Lightwalletd::launch(LightwalletdConfig {
        lightwalletd_bin: None,
        listen_port: None,
        validator_conf: zcashd.config_path(),
    })
    .unwrap();

    zcashd.print_stdout();
    zcashd.print_stderr();
    lwd.print_stdout();
    lwd.print_lwd_log();
    lwd.print_stderr();
}

#[tokio::test]
async fn zainod_basic_send() {
    tracing_subscriber::fmt().init();

    let zcashd = Zcashd::launch(ZcashdConfig {
        zcashd_bin: None,
        zcash_cli_bin: None,
        rpc_port: None,
        activation_heights: network::ActivationHeights::default(),
        miner_address: Some(REG_O_ADDR_FROM_ABANDONART),
    })
    .unwrap();
    let zainod = Zainod::launch(ZainodConfig {
        zainod_bin: None,
        listen_port: None,
        validator_port: zcashd.port(),
    })
    .unwrap();

    let lightclient_dir = tempfile::tempdir().unwrap();
    let (faucet, recipient) =
        build_lightclients(lightclient_dir.path().to_path_buf(), zainod.port()).await;

    faucet.do_sync(true).await.unwrap();
    from_inputs::quick_send(
        &faucet,
        vec![(
            &get_base_address(&recipient, PoolType::Shielded(ShieldedProtocol::Orchard)).await,
            100_000,
            None,
        )],
    )
    .await
    .unwrap();
    zcashd.generate_blocks(1).unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    faucet.do_sync(true).await.unwrap();
    recipient.do_sync(true).await.unwrap();

    zcashd.print_stdout();
    zcashd.print_stderr();
    zainod.print_stdout();
    zainod.print_stderr();
    println!("faucet balance:");
    println!("{:?}\n", faucet.do_balance().await);
    println!("recipient balance:");
    println!("{:?}\n", recipient.do_balance().await);
}

#[tokio::test]
async fn lightwalletd_basic_send() {
    tracing_subscriber::fmt().init();

    let zcashd = Zcashd::launch(ZcashdConfig {
        zcashd_bin: None,
        zcash_cli_bin: None,
        rpc_port: None,
        activation_heights: network::ActivationHeights::default(),
        miner_address: Some(REG_O_ADDR_FROM_ABANDONART),
    })
    .unwrap();
    let lwd = Lightwalletd::launch(LightwalletdConfig {
        lightwalletd_bin: None,
        listen_port: None,
        validator_conf: zcashd.config_path(),
    })
    .unwrap();

    let lightclient_dir = tempfile::tempdir().unwrap();
    let (faucet, recipient) =
        build_lightclients(lightclient_dir.path().to_path_buf(), lwd.port()).await;

    faucet.do_sync(true).await.unwrap();
    from_inputs::quick_send(
        &faucet,
        vec![(
            &get_base_address(&recipient, PoolType::Shielded(ShieldedProtocol::Orchard)).await,
            100_000,
            None,
        )],
    )
    .await
    .unwrap();
    zcashd.generate_blocks(1).unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    faucet.do_sync(true).await.unwrap();
    recipient.do_sync(true).await.unwrap();

    zcashd.print_stdout();
    zcashd.print_stderr();
    lwd.print_stdout();
    lwd.print_lwd_log();
    lwd.print_stderr();
    println!("faucet balance:");
    println!("{:?}\n", faucet.do_balance().await);
    println!("recipient balance:");
    println!("{:?}\n", recipient.do_balance().await);
}
