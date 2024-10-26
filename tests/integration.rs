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

// NOTE: this should be migrated to zingolib when LocalNet replaces regtest manager in zingoilb::testutils
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

#[cfg(feature = "client")]
mod client_rpcs {
    use zcash_client_backend::proto;
    use zcash_local_net::{
        client,
        indexer::{Indexer as _, Lightwalletd, LightwalletdConfig, Zainod, ZainodConfig},
        network,
        validator::{Validator as _, Zcashd, ZcashdConfig},
    };
    use zingolib::testvectors::REG_O_ADDR_FROM_ABANDONART;

    #[tokio::test]
    async fn get_lightd_info() {
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
        let lightwalletd = Lightwalletd::launch(LightwalletdConfig {
            lightwalletd_bin: None,
            listen_port: None,
            validator_conf: zcashd.config_path(),
        })
        .unwrap();

        let mut zainod_client = client::build_client(network::localhost_uri(zainod.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(proto::service::Empty {});
        let zainod_response = zainod_client
            .get_lightd_info(request)
            .await
            .unwrap()
            .into_inner();

        let mut lwd_client = client::build_client(network::localhost_uri(lightwalletd.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(proto::service::Empty {});
        let lwd_response = lwd_client
            .get_lightd_info(request)
            .await
            .unwrap()
            .into_inner();

        println!("Asserting GetLightdInfo responses...");

        println!("\nZainod response:");
        println!("taddr support: {}", zainod_response.taddr_support);
        println!("chain name: {}", zainod_response.chain_name);
        println!(
            "sapling activation height: {}",
            zainod_response.sapling_activation_height
        );
        println!(
            "consensus branch id: {}",
            zainod_response.consensus_branch_id
        );
        println!("block height: {}", zainod_response.block_height);
        println!("estimated height: {}", zainod_response.estimated_height);
        println!("zcashd build: {}", zainod_response.zcashd_build);
        println!("zcashd subversion: {}", zainod_response.zcashd_subversion);

        println!("\nLightwalletd response:");
        println!("taddr support: {}", lwd_response.taddr_support);
        println!("chain name: {}", lwd_response.chain_name);
        println!(
            "sapling activation height: {}",
            lwd_response.sapling_activation_height
        );
        println!("consensus branch id: {}", lwd_response.consensus_branch_id);
        println!("block height: {}", lwd_response.block_height);
        println!("estimated height: {}", lwd_response.estimated_height);
        println!("zcashd build: {}", lwd_response.zcashd_build);
        println!("zcashd subversion: {}", lwd_response.zcashd_subversion);

        println!("");

        assert_eq!(zainod_response.taddr_support, lwd_response.taddr_support);
        assert_eq!(zainod_response.chain_name, lwd_response.chain_name);
        assert_eq!(
            zainod_response.sapling_activation_height,
            lwd_response.sapling_activation_height
        );
        assert_eq!(
            zainod_response.consensus_branch_id,
            lwd_response.consensus_branch_id
        );
        assert_eq!(zainod_response.block_height, lwd_response.block_height);
        assert_eq!(
            zainod_response.estimated_height,
            lwd_response.estimated_height
        );
        assert_eq!(zainod_response.zcashd_build, lwd_response.zcashd_build);
        assert_eq!(
            zainod_response.zcashd_subversion,
            lwd_response.zcashd_subversion
        );
    }

    #[tokio::test]
    async fn get_latest_block() {
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
        let lightwalletd = Lightwalletd::launch(LightwalletdConfig {
            lightwalletd_bin: None,
            listen_port: None,
            validator_conf: zcashd.config_path(),
        })
        .unwrap();

        let mut zainod_client = client::build_client(network::localhost_uri(zainod.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(proto::service::ChainSpec {});
        let zainod_response = zainod_client
            .get_latest_block(request)
            .await
            .unwrap()
            .into_inner();

        let mut lwd_client = client::build_client(network::localhost_uri(lightwalletd.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(proto::service::ChainSpec {});
        let lwd_response = lwd_client
            .get_latest_block(request)
            .await
            .unwrap()
            .into_inner();

        println!("Asserting GetLatestBlock responses...");

        println!("\nZainod response:");
        println!("block id: {:?}", zainod_response);

        println!("\nLightwalletd response:");
        println!("block id: {:?}", lwd_response);

        println!("");

        assert_eq!(zainod_response, lwd_response);
    }

    #[ignore = "test fails, unimplemented in lightwalletd"]
    #[tokio::test]
    async fn get_block() {
        tracing_subscriber::fmt().init();

        let zcashd = Zcashd::launch(ZcashdConfig {
            zcashd_bin: None,
            zcash_cli_bin: None,
            rpc_port: None,
            activation_heights: network::ActivationHeights::default(),
            miner_address: Some(REG_O_ADDR_FROM_ABANDONART),
        })
        .unwrap();
        let lightwalletd = Lightwalletd::launch(LightwalletdConfig {
            lightwalletd_bin: None,
            listen_port: None,
            validator_conf: zcashd.config_path(),
        })
        .unwrap();

        let block_id = proto::service::BlockId {
            height: 1,
            hash: vec![
                1, 44, 10, 178, 199, 92, 139, 114, 253, 82, 78, 237, 245, 94, 159, 128, 73, 10,
                206, 129, 207, 13, 208, 251, 125, 214, 82, 127, 83, 65, 177, 222,
            ],
        };

        let mut lwd_client = client::build_client(network::localhost_uri(lightwalletd.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(block_id.clone());
        let _lwd_response = lwd_client.get_block(request).await.unwrap().into_inner();
    }
}
