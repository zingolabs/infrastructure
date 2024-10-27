use std::path::PathBuf;

use portpicker::Port;
use zcash_local_net::{
    indexer::{Indexer as _, Lightwalletd, LightwalletdConfig, Zainod, ZainodConfig},
    network,
    validator::{Validator, Zcashd, ZcashdConfig},
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
            chain_cache: None,
        },
    );

    let lightclient_dir = tempfile::tempdir().unwrap();
    let (faucet, recipient) = build_lightclients(
        lightclient_dir.path().to_path_buf(),
        local_net.indexer().port(),
    )
    .await;

    faucet.do_sync(false).await.unwrap();
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
    faucet.do_sync(false).await.unwrap();
    recipient.do_sync(false).await.unwrap();

    let recipient_balance = recipient.do_balance().await;
    assert_eq!(recipient_balance.verified_orchard_balance, Some(100_000));

    local_net.validator().print_stdout();
    local_net.validator().print_stderr();
    local_net.indexer().print_stdout();
    local_net.indexer().print_stderr();
    println!("faucet balance:");
    println!("{:?}\n", faucet.do_balance().await);
    println!("recipient balance:");
    println!("{:?}\n", recipient_balance);
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
            chain_cache: None,
        },
    );

    let lightclient_dir = tempfile::tempdir().unwrap();
    let (faucet, recipient) = build_lightclients(
        lightclient_dir.path().to_path_buf(),
        local_net.indexer().port(),
    )
    .await;

    faucet.do_sync(false).await.unwrap();
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
    faucet.do_sync(false).await.unwrap();
    recipient.do_sync(false).await.unwrap();

    let recipient_balance = recipient.do_balance().await;
    assert_eq!(recipient_balance.verified_orchard_balance, Some(100_000));

    local_net.validator().print_stdout();
    local_net.validator().print_stderr();
    local_net.indexer().print_stdout();
    local_net.indexer().print_lwd_log();
    local_net.indexer().print_stderr();
    println!("faucet balance:");
    println!("{:?}\n", faucet.do_balance().await);
    println!("recipient balance:");
    println!("{:?}\n", recipient_balance);
}

#[cfg(feature = "client")]
mod client_rpcs {
    use std::path::PathBuf;

    use zcash_client_backend::proto;
    use zcash_local_net::{
        client,
        indexer::{Indexer as _, Lightwalletd, LightwalletdConfig, Zainod, ZainodConfig},
        network, utils,
        validator::{Validator, Zcashd, ZcashdConfig},
        LocalNet,
    };
    use zcash_primitives::transaction::Transaction;
    use zcash_protocol::{
        consensus::{BlockHeight, BranchId},
        PoolType, ShieldedProtocol,
    };

    use zingolib::{
        config::{ChainType, RegtestNetwork},
        testutils::lightclient::{from_inputs, get_base_address},
        testvectors::REG_O_ADDR_FROM_ABANDONART,
    };

    use crate::build_lightclients;

    #[ignore = "not a test. generates chain cache for client_rpc tests."]
    #[tokio::test]
    async fn generate_chain_cache() {
        tracing_subscriber::fmt().init();

        let mut local_net = LocalNet::<Lightwalletd, Zcashd>::launch(
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
                chain_cache: None,
            },
        );

        local_net.validator().generate_blocks(2).unwrap();

        let lightclient_dir = tempfile::tempdir().unwrap();
        let (faucet, recipient) = build_lightclients(
            lightclient_dir.path().to_path_buf(),
            local_net.indexer().port(),
        )
        .await;

        // TODO: use second recipient taddr
        // recipient.do_new_address("ozt").await.unwrap();
        // let recipient_addresses = recipient.do_addresses().await;
        // recipient taddr child index 0:
        // tmFLszfkjgim4zoUMAXpuohnFBAKy99rr2i
        //
        // recipient taddr child index 1:
        // tmAtLC3JkTDrXyn5okUbb6qcMGE4Xq4UdhD
        //
        // faucet taddr child index 0:
        // tmBsTi2xWTjUdEXnuTceL7fecEQKeWaPDJd

        faucet.do_sync(false).await.unwrap();
        from_inputs::quick_send(
            &faucet,
            vec![(
                &get_base_address(&recipient, PoolType::Shielded(ShieldedProtocol::Orchard)).await,
                100_000,
                Some("orchard test memo"),
            )],
        )
        .await
        .unwrap();
        from_inputs::quick_send(
            &faucet,
            vec![(
                &get_base_address(&recipient, PoolType::Shielded(ShieldedProtocol::Sapling)).await,
                100_000,
                Some("sapling test memo"),
            )],
        )
        .await
        .unwrap();
        from_inputs::quick_send(
            &faucet,
            vec![(
                &get_base_address(&recipient, PoolType::Transparent).await,
                100_000,
                None,
            )],
        )
        .await
        .unwrap();
        local_net.validator().generate_blocks(1).unwrap();

        recipient.do_sync(false).await.unwrap();
        recipient.quick_shield().await.unwrap();
        local_net.validator().generate_blocks(1).unwrap();

        faucet.do_sync(false).await.unwrap();
        from_inputs::quick_send(
            &faucet,
            vec![(
                &get_base_address(&recipient, PoolType::Transparent).await,
                200_000,
                None,
            )],
        )
        .await
        .unwrap();
        local_net.validator().generate_blocks(1).unwrap();

        recipient.do_sync(false).await.unwrap();
        from_inputs::quick_send(
            &recipient,
            vec![(
                &get_base_address(&faucet, PoolType::Transparent).await,
                10_000,
                None,
            )],
        )
        .await
        .unwrap();
        local_net.validator().generate_blocks(1).unwrap();

        recipient.do_sync(false).await.unwrap();
        from_inputs::quick_send(
            &recipient,
            vec![(
                &get_base_address(&recipient, PoolType::Shielded(ShieldedProtocol::Orchard)).await,
                10_000,
                Some("orchard test memo"),
            )],
        )
        .await
        .unwrap();
        local_net.validator().generate_blocks(2).unwrap();

        faucet.do_sync(false).await.unwrap();
        from_inputs::quick_send(
            &faucet,
            vec![(
                &get_base_address(&recipient, PoolType::Shielded(ShieldedProtocol::Sapling)).await,
                100_000,
                None,
            )],
        )
        .await
        .unwrap();
        local_net.validator().generate_blocks(1).unwrap();

        local_net
            .validator_mut()
            .cache_chain(utils::chain_cache_dir().join("client_rpc_tests"));
    }

    #[tokio::test]
    async fn get_lightd_info() {
        tracing_subscriber::fmt().init();

        let zcashd = Zcashd::launch(ZcashdConfig {
            zcashd_bin: None,
            zcash_cli_bin: None,
            rpc_port: None,
            activation_heights: network::ActivationHeights::default(),
            miner_address: Some(REG_O_ADDR_FROM_ABANDONART),
            chain_cache: None,
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
            chain_cache: Some(utils::chain_cache_dir().join("client_rpc_tests")),
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

    #[tokio::test]
    async fn get_block() {
        tracing_subscriber::fmt().init();

        let zcashd = Zcashd::launch(ZcashdConfig {
            zcashd_bin: None,
            zcash_cli_bin: None,
            rpc_port: None,
            activation_heights: network::ActivationHeights::default(),
            miner_address: Some(REG_O_ADDR_FROM_ABANDONART),
            chain_cache: Some(utils::chain_cache_dir().join("client_rpc_tests")),
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

        let block_id = proto::service::BlockId {
            height: 5,
            hash: vec![],
        };

        let mut zainod_client = client::build_client(network::localhost_uri(zainod.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(block_id.clone());
        let zainod_response = zainod_client.get_block(request).await.unwrap().into_inner();

        let mut lwd_client = client::build_client(network::localhost_uri(lightwalletd.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(block_id.clone());
        let lwd_response = lwd_client.get_block(request).await.unwrap().into_inner();

        println!("Asserting GetBlock responses...");

        println!("\nZainod response:");
        println!("compact block: {:?}", zainod_response);

        println!("\nLightwalletd response:");
        println!("compact block: {:?}", lwd_response);

        println!("");

        assert_eq!(zainod_response, lwd_response);
    }

    // test verifies not implemented in lightwalletd
    #[tokio::test]
    async fn get_block_nullifiers() {
        tracing_subscriber::fmt().init();

        let zcashd = Zcashd::launch(ZcashdConfig {
            zcashd_bin: None,
            zcash_cli_bin: None,
            rpc_port: None,
            activation_heights: network::ActivationHeights::default(),
            miner_address: Some(REG_O_ADDR_FROM_ABANDONART),
            chain_cache: Some(utils::chain_cache_dir().join("client_rpc_tests")),
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

        let block_id = proto::service::BlockId {
            height: 5,
            hash: vec![],
        };

        let mut zainod_client = client::build_client(network::localhost_uri(zainod.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(block_id.clone());
        let zainod_response = zainod_client
            .get_block_nullifiers(request)
            .await
            .unwrap()
            .into_inner();

        let mut lwd_client = client::build_client(network::localhost_uri(lightwalletd.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(block_id.clone());
        let lwd_response = lwd_client
            .get_block_nullifiers(request)
            .await
            .unwrap()
            .into_inner();

        println!("Asserting GetBlockNullifiers responses...");

        println!("\nZainod response:");
        println!("compact block: {:?}", zainod_response);

        println!("\nLightwalletd response:");
        println!("compact block: {:?}", lwd_response);

        println!("");

        assert_eq!(zainod_response, lwd_response);
    }

    #[tokio::test]
    async fn get_block_range_nullifiers() {
        tracing_subscriber::fmt().init();

        let zcashd = Zcashd::launch(ZcashdConfig {
            zcashd_bin: None,
            zcash_cli_bin: None,
            rpc_port: None,
            activation_heights: network::ActivationHeights::default(),
            miner_address: Some(REG_O_ADDR_FROM_ABANDONART),
            chain_cache: Some(utils::chain_cache_dir().join("client_rpc_tests")),
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

        let block_range = proto::service::BlockRange {
            start: Some(proto::service::BlockId {
                height: 1,
                hash: vec![],
            }),
            end: Some(proto::service::BlockId {
                height: 6,
                hash: vec![],
            }),
        };

        let mut zainod_client = client::build_client(network::localhost_uri(zainod.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(block_range.clone());
        let mut zainod_response = zainod_client
            .get_block_range_nullifiers(request)
            .await
            .unwrap()
            .into_inner();
        let mut zainod_blocks = Vec::new();
        while let Some(compact_block) = zainod_response.message().await.unwrap() {
            zainod_blocks.push(compact_block);
        }

        let mut lwd_client = client::build_client(network::localhost_uri(lightwalletd.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(block_range.clone());
        let mut lwd_response = lwd_client
            .get_block_range_nullifiers(request)
            .await
            .unwrap()
            .into_inner();
        let mut lwd_blocks = Vec::new();
        while let Some(compact_block) = lwd_response.message().await.unwrap() {
            lwd_blocks.push(compact_block);
        }

        println!("Asserting GetBlockRange responses...");

        println!("\nZainod response:");
        println!("compact blocks: {:?}", zainod_blocks);

        println!("\nLightwalletd response:");
        println!("compact blocks: {:?}", lwd_blocks);

        println!("");

        assert_eq!(zainod_blocks, lwd_blocks);
    }

    #[tokio::test]
    async fn get_block_range_lower() {
        tracing_subscriber::fmt().init();

        let zcashd = Zcashd::launch(ZcashdConfig {
            zcashd_bin: None,
            zcash_cli_bin: None,
            rpc_port: None,
            activation_heights: network::ActivationHeights::default(),
            miner_address: Some(REG_O_ADDR_FROM_ABANDONART),
            chain_cache: Some(utils::chain_cache_dir().join("client_rpc_tests")),
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

        let block_range = proto::service::BlockRange {
            start: Some(proto::service::BlockId {
                height: 1,
                hash: vec![],
            }),
            end: Some(proto::service::BlockId {
                height: 6,
                hash: vec![],
            }),
        };

        let mut zainod_client = client::build_client(network::localhost_uri(zainod.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(block_range.clone());
        let mut zainod_response = zainod_client
            .get_block_range(request)
            .await
            .unwrap()
            .into_inner();
        let mut zainod_blocks = Vec::new();
        while let Some(compact_block) = zainod_response.message().await.unwrap() {
            zainod_blocks.push(compact_block);
        }

        let mut lwd_client = client::build_client(network::localhost_uri(lightwalletd.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(block_range.clone());
        let mut lwd_response = lwd_client
            .get_block_range(request)
            .await
            .unwrap()
            .into_inner();
        let mut lwd_blocks = Vec::new();
        while let Some(compact_block) = lwd_response.message().await.unwrap() {
            lwd_blocks.push(compact_block);
        }

        println!("Asserting GetBlockRange responses...");

        println!("\nZainod response:");
        println!("compact blocks: {:?}", zainod_blocks);

        println!("\nLightwalletd response:");
        println!("compact blocks: {:?}", lwd_blocks);

        println!("");

        assert_eq!(zainod_blocks, lwd_blocks);
    }

    #[tokio::test]
    async fn get_block_range_upper() {
        tracing_subscriber::fmt().init();

        let zcashd = Zcashd::launch(ZcashdConfig {
            zcashd_bin: None,
            zcash_cli_bin: None,
            rpc_port: None,
            activation_heights: network::ActivationHeights::default(),
            miner_address: Some(REG_O_ADDR_FROM_ABANDONART),
            chain_cache: Some(utils::chain_cache_dir().join("client_rpc_tests")),
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

        let block_range = proto::service::BlockRange {
            start: Some(proto::service::BlockId {
                height: 4,
                hash: vec![],
            }),
            end: Some(proto::service::BlockId {
                height: 10,
                hash: vec![],
            }),
        };

        let mut zainod_client = client::build_client(network::localhost_uri(zainod.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(block_range.clone());
        let mut zainod_response = zainod_client
            .get_block_range(request)
            .await
            .unwrap()
            .into_inner();
        let mut zainod_blocks = Vec::new();
        while let Some(compact_block) = zainod_response.message().await.unwrap() {
            zainod_blocks.push(compact_block);
        }

        let mut lwd_client = client::build_client(network::localhost_uri(lightwalletd.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(block_range.clone());
        let mut lwd_response = lwd_client
            .get_block_range(request)
            .await
            .unwrap()
            .into_inner();
        let mut lwd_blocks = Vec::new();
        while let Some(compact_block) = lwd_response.message().await.unwrap() {
            lwd_blocks.push(compact_block);
        }

        println!("Asserting GetBlockRange responses...");

        println!("\nZainod response:");
        println!("compact blocks: {:?}", zainod_blocks);

        println!("\nLightwalletd response:");
        println!("compact blocks: {:?}", lwd_blocks);

        println!("");

        assert_eq!(zainod_blocks, lwd_blocks);
    }

    #[tokio::test]
    async fn get_block_range_out_of_bounds() {
        tracing_subscriber::fmt().init();

        let zcashd = Zcashd::launch(ZcashdConfig {
            zcashd_bin: None,
            zcash_cli_bin: None,
            rpc_port: None,
            activation_heights: network::ActivationHeights::default(),
            miner_address: Some(REG_O_ADDR_FROM_ABANDONART),
            chain_cache: Some(utils::chain_cache_dir().join("client_rpc_tests")),
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

        let block_range = proto::service::BlockRange {
            start: Some(proto::service::BlockId {
                height: 4,
                hash: vec![],
            }),
            end: Some(proto::service::BlockId {
                height: 15,
                hash: vec![],
            }),
        };

        let mut zainod_client = client::build_client(network::localhost_uri(zainod.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(block_range.clone());
        let mut zainod_response = zainod_client
            .get_block_range(request)
            .await
            .unwrap()
            .into_inner();
        let mut zainod_blocks = Vec::new();
        let mut zainod_err_status = None;
        while let Some(compact_block) = match zainod_response.message().await {
            Ok(message) => message,
            Err(e) => {
                zainod_err_status = Some(e);
                None
            }
        } {
            zainod_blocks.push(compact_block);
        }

        let mut lwd_client = client::build_client(network::localhost_uri(lightwalletd.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(block_range.clone());
        let mut lwd_response = lwd_client
            .get_block_range(request)
            .await
            .unwrap()
            .into_inner();
        let mut lwd_blocks = Vec::new();
        let mut lwd_err_status = None;
        while let Some(compact_block) = match lwd_response.message().await {
            Ok(message) => message,
            Err(e) => {
                lwd_err_status = Some(e);
                None
            }
        } {
            lwd_blocks.push(compact_block);
        }

        let lwd_err_status = lwd_err_status.unwrap();
        let zainod_err_status = zainod_err_status.unwrap();

        println!("Asserting GetBlockRange responses...");

        println!("\nZainod response:");
        println!("compact blocks: {:?}", zainod_blocks);
        println!("error status: {:?}", zainod_err_status);

        println!("\nLightwalletd response:");
        println!("compact blocks: {:?}", lwd_blocks);
        println!("error status: {:?}", lwd_err_status);

        println!("");

        assert_eq!(zainod_blocks, lwd_blocks);
        assert_eq!(zainod_err_status.code(), lwd_err_status.code());
        assert_eq!(zainod_err_status.message(), lwd_err_status.message());
    }

    #[tokio::test]
    async fn get_transaction() {
        tracing_subscriber::fmt().init();

        let zcashd = Zcashd::launch(ZcashdConfig {
            zcashd_bin: None,
            zcash_cli_bin: None,
            rpc_port: None,
            activation_heights: network::ActivationHeights::default(),
            miner_address: Some(REG_O_ADDR_FROM_ABANDONART),
            chain_cache: Some(utils::chain_cache_dir().join("client_rpc_tests")),
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

        // TODO: get txid from chain cache
        let lightclient_dir = tempfile::tempdir().unwrap();
        let (faucet, recipient) =
            build_lightclients(lightclient_dir.path().to_path_buf(), lightwalletd.port()).await;
        faucet.do_sync(false).await.unwrap();
        let txids = from_inputs::quick_send(
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

        let tx_filter = proto::service::TxFilter {
            block: None,
            index: 0,
            hash: txids.first().as_ref().to_vec(),
        };

        let mut zainod_client = client::build_client(network::localhost_uri(zainod.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(tx_filter.clone());
        let zainod_response = zainod_client
            .get_transaction(request)
            .await
            .unwrap()
            .into_inner();

        let mut lwd_client = client::build_client(network::localhost_uri(lightwalletd.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(tx_filter.clone());
        let lwd_response = lwd_client
            .get_transaction(request)
            .await
            .unwrap()
            .into_inner();

        println!("Asserting GetTransaction responses...");

        println!("\nZainod response:");
        println!("raw transaction: {:?}", zainod_response);

        println!("\nLightwalletd response:");
        println!("raw transaction: {:?}", lwd_response);

        println!("");

        assert_eq!(zainod_response, lwd_response);
    }

    #[ignore = "incomplete"]
    #[tokio::test]
    async fn send_transaction() {
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
                chain_cache: Some(utils::chain_cache_dir().join("client_rpc_tests")),
            },
        );

        let lightclient_dir = tempfile::tempdir().unwrap();
        let (faucet, recipient) = build_lightclients(
            lightclient_dir.path().to_path_buf(),
            local_net.indexer().port(),
        )
        .await;
        faucet.do_sync(false).await.unwrap();
        let txids = from_inputs::quick_send(
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

        let tx_filter = proto::service::TxFilter {
            block: None,
            index: 0,
            hash: txids.first().as_ref().to_vec(),
        };

        let mut zainod_client =
            client::build_client(network::localhost_uri(local_net.indexer().port()))
                .await
                .unwrap();
        let request = tonic::Request::new(tx_filter.clone());
        let zainod_response = zainod_client
            .get_transaction(request)
            .await
            .unwrap()
            .into_inner();

        drop(local_net);

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
                chain_cache: Some(utils::chain_cache_dir().join("client_rpc_tests")),
            },
        );

        let lightclient_dir = tempfile::tempdir().unwrap();
        let (faucet, recipient) = build_lightclients(
            lightclient_dir.path().to_path_buf(),
            local_net.indexer().port(),
        )
        .await;
        faucet.do_sync(false).await.unwrap();
        let txids = from_inputs::quick_send(
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

        let tx_filter = proto::service::TxFilter {
            block: None,
            index: 0,
            hash: txids.first().as_ref().to_vec(),
        };

        let mut zainod_client =
            client::build_client(network::localhost_uri(local_net.indexer().port()))
                .await
                .unwrap();
        let request = tonic::Request::new(tx_filter.clone());
        let lwd_response = zainod_client
            .get_transaction(request)
            .await
            .unwrap()
            .into_inner();

        let chain_type = ChainType::Regtest(RegtestNetwork::all_upgrades_active());
        let zainod_tx = Transaction::read(
            &zainod_response.data[..],
            BranchId::for_height(
                &chain_type,
                BlockHeight::from_u32(zainod_response.height as u32),
            ),
        )
        .unwrap();
        let lwd_tx = Transaction::read(
            &lwd_response.data[..],
            BranchId::for_height(
                &chain_type,
                BlockHeight::from_u32(lwd_response.height as u32),
            ),
        )
        .unwrap();

        println!("Asserting transactions sent using SendTransacton...");

        println!("\nZainod:");
        println!("transaction: {:?}", zainod_tx);

        println!("\nLightwalletd:");
        println!("transaction: {:?}", lwd_tx);

        println!("");

        assert_eq!(zainod_tx, lwd_tx);
    }

    #[tokio::test]
    async fn get_taddress_balance() {
        tracing_subscriber::fmt().init();

        let zcashd = Zcashd::launch(ZcashdConfig {
            zcashd_bin: None,
            zcash_cli_bin: None,
            rpc_port: None,
            activation_heights: network::ActivationHeights::default(),
            miner_address: Some(REG_O_ADDR_FROM_ABANDONART),
            chain_cache: Some(utils::chain_cache_dir().join("client_rpc_tests")),
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

        let address_list = proto::service::AddressList {
            addresses: vec![
                "tmFLszfkjgim4zoUMAXpuohnFBAKy99rr2i".to_string(),
                "tmBsTi2xWTjUdEXnuTceL7fecEQKeWaPDJd".to_string(),
            ],
        };

        let mut zainod_client = client::build_client(network::localhost_uri(zainod.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(address_list.clone());
        let zainod_response = zainod_client
            .get_taddress_balance(request)
            .await
            .unwrap()
            .into_inner();

        let mut lwd_client = client::build_client(network::localhost_uri(lightwalletd.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(address_list.clone());
        let lwd_response = lwd_client
            .get_taddress_balance(request)
            .await
            .unwrap()
            .into_inner();

        println!("Asserting GetTaddressBalance responses...");

        println!("\nZainod response:");
        println!("block id: {:?}", zainod_response);

        println!("\nLightwalletd response:");
        println!("block id: {:?}", lwd_response);

        println!("");

        assert_eq!(zainod_response, lwd_response);
    }
}
