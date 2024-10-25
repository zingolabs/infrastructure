use std::path::PathBuf;

use portpicker::Port;
use zcash_local_net::{
    indexer::{Indexer as _, Lightwalletd, LightwalletdConfig, Zainod, ZainodConfig},
    network,
    validator::{Validator as _, Zcashd, ZcashdConfig},
    LocalNet,
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

    let local_net = LocalNet::<Zainod, Zcashd>::launch(
        ZainodConfig {
            zainod_bin: None,
            listen_port: None,
            validator_port: 0,
        },
        ZcashdConfig::default(),
    );

    local_net.validator().print_stdout();
    local_net.validator().print_stderr();
    local_net.indexer().print_stdout();
    local_net.indexer().print_stderr();
}

#[test]
fn launch_lightwalletd() {
    tracing_subscriber::fmt().init();

    let local_net = LocalNet::<Lightwalletd, Zcashd>::launch(
        LightwalletdConfig {
            lightwalletd_bin: None,
            listen_port: None,
            validator_conf: PathBuf::new(),
        },
        ZcashdConfig::default(),
    );

    local_net.validator().print_stdout();
    local_net.validator().print_stderr();
    local_net.indexer().print_stdout();
    local_net.indexer().print_lwd_log();
    local_net.indexer().print_stderr();
}

#[tokio::test]
async fn zainod_basic_send() {
    tracing_subscriber::fmt().init();

    let local_net = LocalNet::<Zainod, Zcashd>::launch(
        ZainodConfig {
            zainod_bin: None,
            listen_port: None,
            validator_port: 0,
        },
        ZcashdConfig {
            zcashd_bin: None,
            zcash_cli_bin: None,
            rpc_port: None,
            activation_heights: network::ActivationHeights::default(),
            miner_address: Some(REG_O_ADDR_FROM_ABANDONART),
        },
    );

    let lightclient_dir = tempfile::tempdir().unwrap();
    let (faucet, recipient) = build_lightclients(
        lightclient_dir.path().to_path_buf(),
        local_net.indexer().port(),
    )
    .await;

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
    local_net.validator().generate_blocks(1).unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    faucet.do_sync(true).await.unwrap();
    recipient.do_sync(true).await.unwrap();

    local_net.validator().print_stdout();
    local_net.validator().print_stderr();
    local_net.indexer().print_stdout();
    local_net.indexer().print_stderr();
    println!("faucet balance:");
    println!("{:?}\n", faucet.do_balance().await);
    println!("recipient balance:");
    println!("{:?}\n", recipient.do_balance().await);
}

#[tokio::test]
async fn lightwalletd_basic_send() {
    tracing_subscriber::fmt().init();

    let local_net = LocalNet::<Lightwalletd, Zcashd>::launch(
        LightwalletdConfig {
            lightwalletd_bin: None,
            listen_port: None,
            validator_conf: PathBuf::new(),
        },
        ZcashdConfig {
            zcashd_bin: None,
            zcash_cli_bin: None,
            rpc_port: None,
            activation_heights: network::ActivationHeights::default(),
            miner_address: Some(REG_O_ADDR_FROM_ABANDONART),
        },
    );

    let lightclient_dir = tempfile::tempdir().unwrap();
    let (faucet, recipient) = build_lightclients(
        lightclient_dir.path().to_path_buf(),
        local_net.indexer().port(),
    )
    .await;

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
    local_net.validator().generate_blocks(1).unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    faucet.do_sync(true).await.unwrap();
    recipient.do_sync(true).await.unwrap();

    local_net.validator().print_stdout();
    local_net.validator().print_stderr();
    local_net.indexer().print_stdout();
    local_net.indexer().print_lwd_log();
    local_net.indexer().print_stderr();
    println!("faucet balance:");
    println!("{:?}\n", faucet.do_balance().await);
    println!("recipient balance:");
    println!("{:?}\n", recipient.do_balance().await);
}
