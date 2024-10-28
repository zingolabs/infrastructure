use std::path::PathBuf;

use portpicker::Port;
use zcash_local_net::{network, Indexer as _, Validator as _};
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

    let zcashd = zcash_local_net::Zcashd::default();
    zcashd.print_stdout();
    zcashd.print_stderr();
}

#[test]
fn launch_zainod() {
    tracing_subscriber::fmt().init();

    let zcashd = zcash_local_net::Zcashd::default();
    let zainod = zcash_local_net::Zainod::launch(None, None, zcashd.port()).unwrap();

    zcashd.print_stdout();
    zcashd.print_stderr();
    zainod.print_stdout();
    zainod.print_stderr();
}

#[test]
fn launch_lightwalletd() {
    tracing_subscriber::fmt().init();

    let zcashd = zcash_local_net::Zcashd::default();
    let lwd = zcash_local_net::Lightwalletd::launch(None, None, zcashd.config_path()).unwrap();

    zcashd.print_stdout();
    zcashd.print_stderr();
    lwd.print_stdout();
    lwd.print_lwd_log();
    lwd.print_stderr();
}

#[tokio::test]
async fn zainod_basic_send() {
    tracing_subscriber::fmt().init();

    let zcashd = zcash_local_net::Zcashd::launch(
        None,
        None,
        None,
        &network::ActivationHeights::default(),
        Some(REG_O_ADDR_FROM_ABANDONART),
    )
    .unwrap();
    let zainod = zcash_local_net::Zainod::launch(None, None, zcashd.port()).unwrap();

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

    let zcashd = zcash_local_net::Zcashd::launch(
        None,
        None,
        None,
        &network::ActivationHeights::default(),
        Some(REG_O_ADDR_FROM_ABANDONART),
    )
    .unwrap();
    let lwd = zcash_local_net::Lightwalletd::launch(None, None, zcashd.config_path()).unwrap();

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
