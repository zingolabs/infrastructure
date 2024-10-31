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
    use std::{path::PathBuf, sync::Arc};

    use tokio::sync::mpsc::unbounded_channel;
    use zcash_client_backend::proto::{self, service::RawTransaction};
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
        lightclient::LightClient,
        testutils::lightclient::{from_inputs, get_base_address},
        testvectors::REG_O_ADDR_FROM_ABANDONART,
        wallet::data::summaries::TransactionSummaryInterface,
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

    #[tokio::test]
    async fn get_block_out_of_bounds() {
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
            height: 20,
            hash: vec![],
        };

        let mut zainod_client = client::build_client(network::localhost_uri(zainod.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(block_id.clone());
        let zainod_err_status = zainod_client.get_block(request).await.unwrap_err();

        let mut lwd_client = client::build_client(network::localhost_uri(lightwalletd.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(block_id.clone());
        let lwd_err_status = lwd_client.get_block(request).await.unwrap_err();

        println!("Asserting GetBlock responses...");

        println!("\nZainod response:");
        println!("error status: {:?}", zainod_err_status);

        println!("\nLightwalletd response:");
        println!("error status: {:?}", lwd_err_status);

        println!("");

        assert_eq!(zainod_err_status.code(), lwd_err_status.code());
        assert_eq!(zainod_err_status.message(), lwd_err_status.message());
    }

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

        println!("Asserting GetBlockRangeNullifiers responses...");

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
                height: 20,
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

        assert_eq!(zainod_response.value_zat, 210_000i64);
        assert_eq!(zainod_response, lwd_response);
    }

    #[tokio::test]
    async fn get_taddress_balance_stream() {
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

        let address_list = vec![
            proto::service::Address {
                address: "tmFLszfkjgim4zoUMAXpuohnFBAKy99rr2i".to_string(),
            },
            proto::service::Address {
                address: "tmBsTi2xWTjUdEXnuTceL7fecEQKeWaPDJd".to_string(),
            },
        ];

        let mut zainod_client = client::build_client(network::localhost_uri(zainod.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(tokio_stream::iter(address_list.clone()));
        let zainod_response = zainod_client
            .get_taddress_balance_stream(request)
            .await
            .unwrap()
            .into_inner();

        let mut lwd_client = client::build_client(network::localhost_uri(lightwalletd.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(tokio_stream::iter(address_list.clone()));
        let lwd_response = lwd_client
            .get_taddress_balance_stream(request)
            .await
            .unwrap()
            .into_inner();

        println!("Asserting GetTaddressBalanceStream responses...");

        println!("\nZainod response:");
        println!("block id: {:?}", zainod_response);

        println!("\nLightwalletd response:");
        println!("block id: {:?}", lwd_response);

        println!("");

        assert_eq!(zainod_response.value_zat, 210_000i64);
        assert_eq!(zainod_response, lwd_response);
    }

    #[tokio::test]
    async fn get_mempool_tx() {
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

        let lightclient_dir = tempfile::tempdir().unwrap();
        let (faucet, recipient) =
            build_lightclients(lightclient_dir.path().to_path_buf(), lightwalletd.port()).await;

        faucet.do_sync(false).await.unwrap();
        let txids_1 = from_inputs::quick_send(
            &faucet,
            vec![(
                &get_base_address(&recipient, PoolType::Shielded(ShieldedProtocol::Orchard)).await,
                200_000,
                Some("orchard test memo"),
            )],
        )
        .await
        .unwrap();
        let txids_2 = from_inputs::quick_send(
            &faucet,
            vec![(
                &get_base_address(&recipient, PoolType::Shielded(ShieldedProtocol::Sapling)).await,
                100_000,
                Some("sapling test memo"),
            )],
        )
        .await
        .unwrap();

        recipient.do_sync(false).await.unwrap();
        let txids_3 = from_inputs::quick_send(
            &recipient,
            vec![(
                &get_base_address(&faucet, PoolType::Shielded(ShieldedProtocol::Orchard)).await,
                50_000,
                Some("orchard test memo"),
            )],
        )
        .await
        .unwrap();
        let txids_4 = from_inputs::quick_send(
            &recipient,
            vec![(
                &get_base_address(&faucet, PoolType::Shielded(ShieldedProtocol::Sapling)).await,
                25_000,
                Some("sapling test memo"),
            )],
        )
        .await
        .unwrap();

        let full_txid_2 = txids_2.first().as_ref().to_vec();
        // the excluded list only accepts truncated txids when they are truncated at the start, not end.
        let mut full_txid_4 = txids_4.first().as_ref().to_vec();
        let truncated_txid_4 = full_txid_4.drain(16..).collect();

        let exclude_list = proto::service::Exclude {
            txid: vec![full_txid_2, truncated_txid_4],
        };

        let mut zainod_client = client::build_client(network::localhost_uri(zainod.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(exclude_list.clone());
        let mut zainod_response = zainod_client
            .get_mempool_tx(request)
            .await
            .unwrap()
            .into_inner();
        let mut zainod_txs = Vec::new();
        while let Some(compact_tx) = zainod_response.message().await.unwrap() {
            zainod_txs.push(compact_tx);
        }
        zainod_txs.sort_by(|a, b| a.hash.cmp(&b.hash));

        let mut lwd_client = client::build_client(network::localhost_uri(lightwalletd.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(exclude_list.clone());
        let mut lwd_response = lwd_client
            .get_mempool_tx(request)
            .await
            .unwrap()
            .into_inner();
        let mut lwd_txs = Vec::new();
        while let Some(compact_tx) = lwd_response.message().await.unwrap() {
            lwd_txs.push(compact_tx);
        }
        lwd_txs.sort_by(|a, b| a.hash.cmp(&b.hash));

        println!("Asserting GetMempoolTx responses...");

        println!("\nZainod response:");
        println!("transactions: {:?}", zainod_txs);

        println!("\nLightwalletd response:");
        println!("transactions: {:?}", lwd_txs);

        println!("");

        // the response txid is the reverse of the txid returned from quick send
        let mut txid_1_rev = txids_1.first().as_ref().to_vec();
        txid_1_rev.reverse();
        let mut txid_3_rev = txids_3.first().as_ref().to_vec();
        txid_3_rev.reverse();
        let mut txids = vec![txid_1_rev, txid_3_rev];
        txids.sort_by(|a, b| a.cmp(&b));

        assert_eq!(lwd_txs.len(), 2);
        assert_eq!(lwd_txs[0].hash, txids[0]);
        assert_eq!(lwd_txs[1].hash, txids[1]);
        assert_eq!(zainod_txs, lwd_txs);

        let recipient = Arc::new(recipient);
        LightClient::start_mempool_monitor(recipient.clone());
        recipient.clear_state().await;
        recipient.do_sync(false).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        let lwd_tx_summaries = recipient.transaction_summaries().await;

        drop(recipient);
        drop(faucet);

        let lightclient_dir = tempfile::tempdir().unwrap();
        let (_faucet, recipient) =
            build_lightclients(lightclient_dir.path().to_path_buf(), zainod.port()).await;

        let recipient = Arc::new(recipient);
        LightClient::start_mempool_monitor(recipient.clone());
        recipient.clear_state().await;
        recipient.do_sync(false).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        let zainod_tx_summaries = recipient.transaction_summaries().await;

        println!("Asserting wallet transaction summaries...");

        println!("\nZainod:");
        println!("{}", zainod_tx_summaries);

        println!("\nLightwalletd:");
        println!("{}", lwd_tx_summaries);

        println!("");

        assert_eq!(zainod_tx_summaries, lwd_tx_summaries);
    }

    #[tokio::test]
    async fn get_mempool_stream() {
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

        // start mempool tasks
        let (zainod_sender, mut zainod_receiver) = unbounded_channel::<RawTransaction>();
        let zainod_port = zainod.port().clone();
        let _zainod_handle = tokio::spawn(async move {
            let mut zainod_client = client::build_client(network::localhost_uri(zainod_port))
                .await
                .unwrap();
            loop {
                let request = tonic::Request::new(proto::service::Empty {});
                let mut zainod_response = zainod_client
                    .get_mempool_stream(request)
                    .await
                    .unwrap()
                    .into_inner();
                while let Some(raw_tx) = zainod_response.message().await.unwrap() {
                    zainod_sender.send(raw_tx).unwrap();
                }
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            }
        });

        let (lwd_sender, mut lwd_receiver) = unbounded_channel::<RawTransaction>();
        let lwd_port = lightwalletd.port().clone();
        let _lwd_handle = tokio::spawn(async move {
            let mut lwd_client = client::build_client(network::localhost_uri(lwd_port))
                .await
                .unwrap();
            loop {
                let request = tonic::Request::new(proto::service::Empty {});
                let mut lwd_response = lwd_client
                    .get_mempool_stream(request)
                    .await
                    .unwrap()
                    .into_inner();
                while let Some(raw_tx) = lwd_response.message().await.unwrap() {
                    lwd_sender.send(raw_tx).unwrap();
                }
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            }
        });

        // send txs to mempool
        let lightclient_dir = tempfile::tempdir().unwrap();
        let (faucet, recipient) =
            build_lightclients(lightclient_dir.path().to_path_buf(), lightwalletd.port()).await;

        faucet.do_sync(false).await.unwrap();
        let txids_1 = from_inputs::quick_send(
            &faucet,
            vec![(
                &get_base_address(&recipient, PoolType::Shielded(ShieldedProtocol::Orchard)).await,
                200_000,
                Some("orchard test memo"),
            )],
        )
        .await
        .unwrap();
        let txids_2 = from_inputs::quick_send(
            &faucet,
            vec![(
                &get_base_address(&recipient, PoolType::Shielded(ShieldedProtocol::Sapling)).await,
                100_000,
                Some("sapling test memo"),
            )],
        )
        .await
        .unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;

        // receive txs from mempool
        let chain_type = ChainType::Regtest(RegtestNetwork::all_upgrades_active());

        let mut zainod_raw_txs = Vec::new();
        while let Some(raw_tx) = zainod_receiver.recv().await {
            zainod_raw_txs.push(raw_tx);
            if zainod_raw_txs.len() == 2 {
                break;
            }
        }
        let mut zainod_txs = zainod_raw_txs
            .iter()
            .map(|raw_tx| {
                Transaction::read(
                    &raw_tx.data[..],
                    BranchId::for_height(&chain_type, BlockHeight::from_u32(raw_tx.height as u32)),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        zainod_txs.sort_by(|a, b| a.txid().cmp(&b.txid()));

        let mut lwd_raw_txs = Vec::new();
        while let Some(raw_tx) = lwd_receiver.recv().await {
            lwd_raw_txs.push(raw_tx);
            if lwd_raw_txs.len() == 2 {
                break;
            }
        }
        let mut lwd_txs = lwd_raw_txs
            .iter()
            .map(|raw_tx| {
                Transaction::read(
                    &raw_tx.data[..],
                    BranchId::for_height(&chain_type, BlockHeight::from_u32(raw_tx.height as u32)),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        lwd_txs.sort_by(|a, b| a.txid().cmp(&b.txid()));

        println!("Asserting GetMempoolStream responses...");

        println!("\nZainod response:");
        println!("transactions: {:?}", zainod_txs);

        println!("\nLightwalletd response:");
        println!("transactions: {:?}", lwd_txs);

        println!("");

        let mut txids = vec![txids_1.first().clone(), txids_2.first().clone()];
        txids.sort_by(|a, b| a.cmp(&b));

        assert_eq!(lwd_txs.len(), 2);
        assert_eq!(lwd_txs[0].txid(), txids[0]);
        assert_eq!(lwd_txs[1].txid(), txids[1]);
        assert_eq!(zainod_txs, lwd_txs);

        // send more txs to mempool
        recipient.do_sync(false).await.unwrap();
        let txids_3 = from_inputs::quick_send(
            &recipient,
            vec![(
                &get_base_address(&faucet, PoolType::Shielded(ShieldedProtocol::Orchard)).await,
                50_000,
                Some("orchard test memo"),
            )],
        )
        .await
        .unwrap();
        // TODO: add generate block here to test next block behaviour
        let txids_4 = from_inputs::quick_send(
            &recipient,
            vec![(
                &get_base_address(&faucet, PoolType::Shielded(ShieldedProtocol::Sapling)).await,
                25_000,
                Some("sapling test memo"),
            )],
        )
        .await
        .unwrap();

        // receive txs from mempool
        while let Some(raw_tx) = zainod_receiver.recv().await {
            zainod_raw_txs.push(raw_tx);
            if lwd_raw_txs.len() == 4 {
                break;
            }
        }
        let mut zainod_txs = zainod_raw_txs
            .iter()
            .map(|raw_tx| {
                Transaction::read(
                    &raw_tx.data[..],
                    BranchId::for_height(&chain_type, BlockHeight::from_u32(raw_tx.height as u32)),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        zainod_txs.sort_by(|a, b| a.txid().cmp(&b.txid()));

        while let Some(raw_tx) = lwd_receiver.recv().await {
            lwd_raw_txs.push(raw_tx);
            if lwd_raw_txs.len() == 4 {
                break;
            }
        }
        let mut lwd_txs = lwd_raw_txs
            .iter()
            .map(|raw_tx| {
                Transaction::read(
                    &raw_tx.data[..],
                    BranchId::for_height(&chain_type, BlockHeight::from_u32(raw_tx.height as u32)),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        lwd_txs.sort_by(|a, b| a.txid().cmp(&b.txid()));

        println!("Asserting GetMempoolStream responses (pt2)...");

        println!("\nZainod response:");
        println!("transactions: {:?}", zainod_txs);

        println!("\nLightwalletd response:");
        println!("transactions: {:?}", lwd_txs);

        println!("");

        txids.push(txids_3.first().clone());
        txids.push(txids_4.first().clone());
        txids.sort_by(|a, b| a.cmp(&b));

        assert_eq!(lwd_txs.len(), 4);
        assert_eq!(lwd_txs[0].txid(), txids[0]);
        assert_eq!(lwd_txs[1].txid(), txids[1]);
        assert_eq!(lwd_txs[2].txid(), txids[2]);
        assert_eq!(lwd_txs[3].txid(), txids[3]);
        assert_eq!(zainod_txs, lwd_txs);

        let recipient = Arc::new(recipient);
        LightClient::start_mempool_monitor(recipient.clone());
        recipient.clear_state().await;
        recipient.do_sync(false).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        let mut lwd_tx_summaries = recipient.transaction_summaries().await.0;
        lwd_tx_summaries.sort_by(|a, b| a.txid().cmp(&b.txid()));

        drop(recipient);
        drop(faucet);

        let lightclient_dir = tempfile::tempdir().unwrap();
        let (_faucet, recipient) =
            build_lightclients(lightclient_dir.path().to_path_buf(), zainod.port()).await;

        let recipient = Arc::new(recipient);
        LightClient::start_mempool_monitor(recipient.clone());
        recipient.clear_state().await;
        recipient.do_sync(false).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        let mut zainod_tx_summaries = recipient.transaction_summaries().await.0;
        zainod_tx_summaries.sort_by(|a, b| a.txid().cmp(&b.txid()));

        assert_eq!(zainod_tx_summaries, lwd_tx_summaries);
    }

    #[tokio::test]
    async fn get_tree_state_by_height() {
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
            .get_tree_state(request)
            .await
            .unwrap()
            .into_inner();

        let mut lwd_client = client::build_client(network::localhost_uri(lightwalletd.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(block_id.clone());
        let lwd_response = lwd_client
            .get_tree_state(request)
            .await
            .unwrap()
            .into_inner();

        println!("Asserting GetTreeState responses...");

        println!("\nZainod response:");
        println!("tree state: {:?}", zainod_response);

        println!("\nLightwalletd response:");
        println!("tree state: {:?}", lwd_response);

        println!("");

        assert_eq!(zainod_response, lwd_response);
    }

    #[tokio::test]
    async fn get_tree_state_by_hash() {
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

        let mut lwd_client = client::build_client(network::localhost_uri(lightwalletd.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(block_id.clone());
        let block = lwd_client.get_block(request).await.unwrap().into_inner();
        let mut block_hash = block.hash().clone().0.to_vec();
        block_hash.reverse();

        let block_id = proto::service::BlockId {
            height: 0,
            hash: block_hash,
        };

        let mut zainod_client = client::build_client(network::localhost_uri(zainod.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(block_id.clone());
        let zainod_response = zainod_client
            .get_tree_state(request)
            .await
            .unwrap()
            .into_inner();

        let request = tonic::Request::new(block_id.clone());
        let lwd_response = lwd_client
            .get_tree_state(request)
            .await
            .unwrap()
            .into_inner();

        println!("Asserting GetTreeState responses...");

        println!("\nZainod response:");
        println!("tree state: {:?}", zainod_response);

        println!("\nLightwalletd response:");
        println!("tree state: {:?}", lwd_response);

        println!("");

        assert_eq!(zainod_response, lwd_response);
    }

    #[tokio::test]
    async fn get_tree_state_out_of_bounds() {
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
            height: 20,
            hash: vec![],
        };

        let mut zainod_client = client::build_client(network::localhost_uri(zainod.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(block_id.clone());
        let zainod_err_status = zainod_client.get_tree_state(request).await.unwrap_err();

        let mut lwd_client = client::build_client(network::localhost_uri(lightwalletd.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(block_id.clone());
        let lwd_err_status = lwd_client.get_tree_state(request).await.unwrap_err();

        println!("Asserting GetTreeState responses...");

        println!("\nZainod response:");
        println!("error status: {:?}", zainod_err_status);

        println!("\nLightwalletd response:");
        println!("error status: {:?}", lwd_err_status);

        println!("");

        assert_eq!(zainod_err_status.code(), lwd_err_status.code());
        assert_eq!(zainod_err_status.message(), lwd_err_status.message());
    }

    #[tokio::test]
    async fn get_latest_tree_state() {
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
        let request = tonic::Request::new(proto::service::Empty {});
        let zainod_response = zainod_client
            .get_latest_tree_state(request)
            .await
            .unwrap()
            .into_inner();

        let mut lwd_client = client::build_client(network::localhost_uri(lightwalletd.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(proto::service::Empty {});
        let lwd_response = lwd_client
            .get_latest_tree_state(request)
            .await
            .unwrap()
            .into_inner();

        println!("Asserting GetLatestTreeState responses...");

        println!("\nZainod response:");
        println!("tree state: {:?}", zainod_response);

        println!("\nLightwalletd response:");
        println!("tree state: {:?}", lwd_response);

        println!("");

        assert_eq!(zainod_response, lwd_response);
    }

    // this is not a satisfactory test for this rpc and will return empty vecs.
    // this rpc should also be tested in testnet/mainnet or a local chain with at least 2 shards should be cached.
    #[tokio::test]
    async fn get_subtree_roots_sapling() {
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

        let subtree_roots_arg = proto::service::GetSubtreeRootsArg {
            start_index: 0,
            shielded_protocol: 0,
            max_entries: 0,
        };

        let mut zainod_client = client::build_client(network::localhost_uri(zainod.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(subtree_roots_arg.clone());
        let mut zainod_response = zainod_client
            .get_subtree_roots(request)
            .await
            .unwrap()
            .into_inner();
        let mut zainod_subtree_roots = Vec::new();
        while let Some(subtree_root) = zainod_response.message().await.unwrap() {
            zainod_subtree_roots.push(subtree_root);
        }

        let mut lwd_client = client::build_client(network::localhost_uri(lightwalletd.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(subtree_roots_arg.clone());
        let mut lwd_response = lwd_client
            .get_subtree_roots(request)
            .await
            .unwrap()
            .into_inner();
        let mut lwd_subtree_roots = Vec::new();
        while let Some(subtree_root) = lwd_response.message().await.unwrap() {
            lwd_subtree_roots.push(subtree_root);
        }

        println!("Asserting GetSubtreeRoots responses...");

        println!("\nZainod response:");
        println!("subtree roots: {:?}", zainod_subtree_roots);

        println!("\nLightwalletd response:");
        println!("subtree roots: {:?}", lwd_subtree_roots);

        println!("");

        assert_eq!(zainod_subtree_roots, lwd_subtree_roots);
    }

    // this is not a satisfactory test for this rpc and will return empty vecs.
    // this rpc should also be tested in testnet/mainnet or a local chain with at least 2 shards should be cached.
    #[tokio::test]
    async fn get_subtree_roots_orchard() {
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

        let subtree_roots_arg = proto::service::GetSubtreeRootsArg {
            start_index: 0,
            shielded_protocol: 1,
            max_entries: 0,
        };

        let mut zainod_client = client::build_client(network::localhost_uri(zainod.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(subtree_roots_arg.clone());
        let mut zainod_response = zainod_client
            .get_subtree_roots(request)
            .await
            .unwrap()
            .into_inner();
        let mut zainod_subtree_roots = Vec::new();
        while let Some(subtree_root) = zainod_response.message().await.unwrap() {
            zainod_subtree_roots.push(subtree_root);
        }

        let mut lwd_client = client::build_client(network::localhost_uri(lightwalletd.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(subtree_roots_arg.clone());
        let mut lwd_response = lwd_client
            .get_subtree_roots(request)
            .await
            .unwrap()
            .into_inner();
        let mut lwd_subtree_roots = Vec::new();
        while let Some(subtree_root) = lwd_response.message().await.unwrap() {
            lwd_subtree_roots.push(subtree_root);
        }

        println!("Asserting GetSubtreeRoots responses...");

        println!("\nZainod response:");
        println!("subtree roots: {:?}", zainod_subtree_roots);

        println!("\nLightwalletd response:");
        println!("subtree roots: {:?}", lwd_subtree_roots);

        println!("");

        assert_eq!(zainod_subtree_roots, lwd_subtree_roots);
    }

    #[tokio::test]
    async fn get_address_utxos_all() {
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

        let address_utxos_arg = proto::service::GetAddressUtxosArg {
            addresses: vec![
                "tmFLszfkjgim4zoUMAXpuohnFBAKy99rr2i".to_string(),
                "tmBsTi2xWTjUdEXnuTceL7fecEQKeWaPDJd".to_string(),
            ],
            start_height: 0,
            max_entries: 0,
        };

        let mut zainod_client = client::build_client(network::localhost_uri(zainod.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(address_utxos_arg.clone());
        let zainod_response = zainod_client
            .get_address_utxos(request)
            .await
            .unwrap()
            .into_inner();

        let mut lwd_client = client::build_client(network::localhost_uri(lightwalletd.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(address_utxos_arg.clone());
        let lwd_response = lwd_client
            .get_address_utxos(request)
            .await
            .unwrap()
            .into_inner();

        println!("Asserting GetAddressUtxos responses...");

        println!("\nZainod response:");
        println!("address utxos replies: {:?}", zainod_response);

        println!("\nLightwalletd response:");
        println!("address utxos replies: {:?}", lwd_response);

        println!("");

        assert_eq!(lwd_response.address_utxos.len(), 2);
        assert_eq!(zainod_response, lwd_response);
    }

    #[tokio::test]
    async fn get_address_utxos_lower() {
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

        let address_utxos_arg = proto::service::GetAddressUtxosArg {
            addresses: vec![
                "tmFLszfkjgim4zoUMAXpuohnFBAKy99rr2i".to_string(),
                "tmBsTi2xWTjUdEXnuTceL7fecEQKeWaPDJd".to_string(),
            ],
            start_height: 0,
            max_entries: 1,
        };

        let mut zainod_client = client::build_client(network::localhost_uri(zainod.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(address_utxos_arg.clone());
        let zainod_response = zainod_client
            .get_address_utxos(request)
            .await
            .unwrap()
            .into_inner();

        let mut lwd_client = client::build_client(network::localhost_uri(lightwalletd.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(address_utxos_arg.clone());
        let lwd_response = lwd_client
            .get_address_utxos(request)
            .await
            .unwrap()
            .into_inner();

        println!("Asserting GetAddressUtxos responses...");

        println!("\nZainod response:");
        println!("address utxos replies: {:?}", zainod_response);

        println!("\nLightwalletd response:");
        println!("address utxos replies: {:?}", lwd_response);

        println!("");

        assert_eq!(lwd_response.address_utxos.len(), 1);
        assert_eq!(lwd_response.address_utxos.first().unwrap().height, 6);
        assert_eq!(zainod_response, lwd_response);
    }

    #[tokio::test]
    async fn get_address_utxos_upper() {
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

        let address_utxos_arg = proto::service::GetAddressUtxosArg {
            addresses: vec![
                "tmFLszfkjgim4zoUMAXpuohnFBAKy99rr2i".to_string(),
                "tmBsTi2xWTjUdEXnuTceL7fecEQKeWaPDJd".to_string(),
            ],
            start_height: 7,
            max_entries: 1,
        };

        let mut zainod_client = client::build_client(network::localhost_uri(zainod.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(address_utxos_arg.clone());
        let zainod_response = zainod_client
            .get_address_utxos(request)
            .await
            .unwrap()
            .into_inner();

        let mut lwd_client = client::build_client(network::localhost_uri(lightwalletd.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(address_utxos_arg.clone());
        let lwd_response = lwd_client
            .get_address_utxos(request)
            .await
            .unwrap()
            .into_inner();

        println!("Asserting GetAddressUtxos responses...");

        println!("\nZainod response:");
        println!("address utxos replies: {:?}", zainod_response);

        println!("\nLightwalletd response:");
        println!("address utxos replies: {:?}", lwd_response);

        println!("");

        assert_eq!(lwd_response.address_utxos.len(), 1);
        assert_eq!(lwd_response.address_utxos.first().unwrap().height, 7);
        assert_eq!(zainod_response, lwd_response);
    }

    #[tokio::test]
    async fn get_address_utxos_out_of_bounds() {
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

        let address_utxos_arg = proto::service::GetAddressUtxosArg {
            addresses: vec![
                "tmFLszfkjgim4zoUMAXpuohnFBAKy99rr2i".to_string(),
                "tmBsTi2xWTjUdEXnuTceL7fecEQKeWaPDJd".to_string(),
            ],
            start_height: 20,
            max_entries: 0,
        };

        let mut zainod_client = client::build_client(network::localhost_uri(zainod.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(address_utxos_arg.clone());
        let zainod_response = zainod_client
            .get_address_utxos(request)
            .await
            .unwrap()
            .into_inner();

        let mut lwd_client = client::build_client(network::localhost_uri(lightwalletd.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(address_utxos_arg.clone());
        let lwd_response = lwd_client
            .get_address_utxos(request)
            .await
            .unwrap()
            .into_inner();

        println!("Asserting GetAddressUtxos responses...");

        println!("\nZainod response:");
        println!("address utxos replies: {:?}", zainod_response);

        println!("\nLightwalletd response:");
        println!("address utxos replies: {:?}", lwd_response);

        println!("");

        assert_eq!(lwd_response.address_utxos.len(), 0);
        assert_eq!(zainod_response, lwd_response);
    }

    #[tokio::test]
    async fn get_address_utxos_stream_all() {
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

        let address_utxos_arg = proto::service::GetAddressUtxosArg {
            addresses: vec![
                "tmFLszfkjgim4zoUMAXpuohnFBAKy99rr2i".to_string(),
                "tmBsTi2xWTjUdEXnuTceL7fecEQKeWaPDJd".to_string(),
            ],
            start_height: 0,
            max_entries: 0,
        };

        let mut zainod_client = client::build_client(network::localhost_uri(zainod.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(address_utxos_arg.clone());
        let mut zainod_response = zainod_client
            .get_address_utxos_stream(request)
            .await
            .unwrap()
            .into_inner();
        let mut zainod_address_utxo_replies = Vec::new();
        while let Some(address_utxo_reply) = zainod_response.message().await.unwrap() {
            zainod_address_utxo_replies.push(address_utxo_reply);
        }

        let mut lwd_client = client::build_client(network::localhost_uri(lightwalletd.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(address_utxos_arg.clone());
        let mut lwd_response = lwd_client
            .get_address_utxos_stream(request)
            .await
            .unwrap()
            .into_inner();
        let mut lwd_address_utxo_replies = Vec::new();
        while let Some(address_utxo_reply) = lwd_response.message().await.unwrap() {
            lwd_address_utxo_replies.push(address_utxo_reply);
        }

        println!("Asserting GetAddressUtxosStream responses...");

        println!("\nZainod response:");
        println!("address utxos replies: {:?}", zainod_address_utxo_replies);

        println!("\nLightwalletd response:");
        println!("address utxos replies: {:?}", lwd_address_utxo_replies);

        println!("");

        assert_eq!(lwd_address_utxo_replies.len(), 2);
        assert_eq!(zainod_address_utxo_replies, lwd_address_utxo_replies);
    }

    #[tokio::test]
    async fn get_address_utxos_stream_lower() {
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

        let address_utxos_arg = proto::service::GetAddressUtxosArg {
            addresses: vec![
                "tmFLszfkjgim4zoUMAXpuohnFBAKy99rr2i".to_string(),
                "tmBsTi2xWTjUdEXnuTceL7fecEQKeWaPDJd".to_string(),
            ],
            start_height: 0,
            max_entries: 1,
        };

        let mut zainod_client = client::build_client(network::localhost_uri(zainod.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(address_utxos_arg.clone());
        let mut zainod_response = zainod_client
            .get_address_utxos_stream(request)
            .await
            .unwrap()
            .into_inner();
        let mut zainod_address_utxo_replies = Vec::new();
        while let Some(address_utxo_reply) = zainod_response.message().await.unwrap() {
            zainod_address_utxo_replies.push(address_utxo_reply);
        }

        let mut lwd_client = client::build_client(network::localhost_uri(lightwalletd.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(address_utxos_arg.clone());
        let mut lwd_response = lwd_client
            .get_address_utxos_stream(request)
            .await
            .unwrap()
            .into_inner();
        let mut lwd_address_utxo_replies = Vec::new();
        while let Some(address_utxo_reply) = lwd_response.message().await.unwrap() {
            lwd_address_utxo_replies.push(address_utxo_reply);
        }

        println!("Asserting GetAddressUtxosStream responses...");

        println!("\nZainod response:");
        println!("address utxos replies: {:?}", zainod_address_utxo_replies);

        println!("\nLightwalletd response:");
        println!("address utxos replies: {:?}", lwd_address_utxo_replies);

        println!("");

        assert_eq!(lwd_address_utxo_replies.len(), 1);
        assert_eq!(lwd_address_utxo_replies.first().unwrap().height, 6);
        assert_eq!(zainod_address_utxo_replies, lwd_address_utxo_replies);
    }

    #[tokio::test]
    async fn get_address_utxos_stream_upper() {
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

        let address_utxos_arg = proto::service::GetAddressUtxosArg {
            addresses: vec![
                "tmFLszfkjgim4zoUMAXpuohnFBAKy99rr2i".to_string(),
                "tmBsTi2xWTjUdEXnuTceL7fecEQKeWaPDJd".to_string(),
            ],
            start_height: 7,
            max_entries: 1,
        };

        let mut zainod_client = client::build_client(network::localhost_uri(zainod.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(address_utxos_arg.clone());
        let mut zainod_response = zainod_client
            .get_address_utxos_stream(request)
            .await
            .unwrap()
            .into_inner();
        let mut zainod_address_utxo_replies = Vec::new();
        while let Some(address_utxo_reply) = zainod_response.message().await.unwrap() {
            zainod_address_utxo_replies.push(address_utxo_reply);
        }

        let mut lwd_client = client::build_client(network::localhost_uri(lightwalletd.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(address_utxos_arg.clone());
        let mut lwd_response = lwd_client
            .get_address_utxos_stream(request)
            .await
            .unwrap()
            .into_inner();
        let mut lwd_address_utxo_replies = Vec::new();
        while let Some(address_utxo_reply) = lwd_response.message().await.unwrap() {
            lwd_address_utxo_replies.push(address_utxo_reply);
        }

        println!("Asserting GetAddressUtxosStream responses...");

        println!("\nZainod response:");
        println!("address utxos replies: {:?}", zainod_address_utxo_replies);

        println!("\nLightwalletd response:");
        println!("address utxos replies: {:?}", lwd_address_utxo_replies);

        println!("");

        assert_eq!(lwd_address_utxo_replies.len(), 1);
        assert_eq!(lwd_address_utxo_replies.first().unwrap().height, 7);
        assert_eq!(zainod_address_utxo_replies, lwd_address_utxo_replies);
    }

    #[tokio::test]
    async fn get_address_utxos_stream_out_of_bounds() {
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

        let address_utxos_arg = proto::service::GetAddressUtxosArg {
            addresses: vec![
                "tmFLszfkjgim4zoUMAXpuohnFBAKy99rr2i".to_string(),
                "tmBsTi2xWTjUdEXnuTceL7fecEQKeWaPDJd".to_string(),
            ],
            start_height: 20,
            max_entries: 0,
        };

        let mut zainod_client = client::build_client(network::localhost_uri(zainod.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(address_utxos_arg.clone());
        let mut zainod_response = zainod_client
            .get_address_utxos_stream(request)
            .await
            .unwrap()
            .into_inner();
        let mut zainod_address_utxo_replies = Vec::new();
        while let Some(address_utxo_reply) = zainod_response.message().await.unwrap() {
            zainod_address_utxo_replies.push(address_utxo_reply);
        }

        let mut lwd_client = client::build_client(network::localhost_uri(lightwalletd.port()))
            .await
            .unwrap();
        let request = tonic::Request::new(address_utxos_arg.clone());
        let mut lwd_response = lwd_client
            .get_address_utxos_stream(request)
            .await
            .unwrap()
            .into_inner();
        let mut lwd_address_utxo_replies = Vec::new();
        while let Some(address_utxo_reply) = lwd_response.message().await.unwrap() {
            lwd_address_utxo_replies.push(address_utxo_reply);
        }

        println!("Asserting GetAddressUtxosStream responses...");

        println!("\nZainod response:");
        println!("address utxos replies: {:?}", zainod_address_utxo_replies);

        println!("\nLightwalletd response:");
        println!("address utxos replies: {:?}", lwd_address_utxo_replies);

        println!("");

        println!("");

        assert_eq!(lwd_address_utxo_replies.len(), 0);
        assert_eq!(zainod_address_utxo_replies, lwd_address_utxo_replies);
    }
}
