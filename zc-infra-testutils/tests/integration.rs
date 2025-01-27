use std::path::PathBuf;

use zcash_protocol::{PoolType, ShieldedProtocol};

use zingolib::{
    testutils::lightclient::{from_inputs, get_base_address},
    testvectors::REG_O_ADDR_FROM_ABANDONART,
};

use zc_infra_testutils::client;

use zc_infra_nodes::{
    indexer::{Indexer as _, Lightwalletd, LightwalletdConfig, Zainod, ZainodConfig},
    network, utils,
    validator::{Validator, Zcashd, ZcashdConfig, Zebrad, ZebradConfig, ZEBRAD_DEFAULT_MINER},
    LocalNet,
};

const ZCASHD_BIN: Option<PathBuf> = None;
const ZCASH_CLI_BIN: Option<PathBuf> = None;
const ZEBRAD_BIN: Option<PathBuf> = None;
const LIGHTWALLETD_BIN: Option<PathBuf> = None;
const ZAINOD_BIN: Option<PathBuf> = None;

#[tokio::test]
async fn launch_zcashd() {
    tracing_subscriber::fmt().init();

    let zcashd = Zcashd::launch(ZcashdConfig {
        zcashd_bin: ZCASHD_BIN,
        zcash_cli_bin: ZCASH_CLI_BIN,
        rpc_listen_port: None,
        activation_heights: network::ActivationHeights::default(),
        miner_address: Some(REG_O_ADDR_FROM_ABANDONART),
        chain_cache: None,
    })
    .await
    .unwrap();
    zcashd.print_stdout();
    zcashd.print_stderr();
}

#[tokio::test]
async fn launch_zebrad() {
    tracing_subscriber::fmt().init();

    let zebrad = Zebrad::launch(ZebradConfig {
        zebrad_bin: ZEBRAD_BIN,
        network_listen_port: None,
        rpc_listen_port: None,
        activation_heights: network::ActivationHeights::default(),
        miner_address: ZEBRAD_DEFAULT_MINER,
        chain_cache: None,
        network: network::Network::Regtest,
    })
    .await
    .unwrap();
    zebrad.print_stdout();
    zebrad.print_stderr();
}

#[ignore = "temporary during refactor into workspace"]
#[tokio::test]
async fn launch_zebrad_with_cache() {
    tracing_subscriber::fmt().init();

    let zebrad = Zebrad::launch(ZebradConfig {
        zebrad_bin: ZEBRAD_BIN,
        network_listen_port: None,
        rpc_listen_port: None,
        activation_heights: network::ActivationHeights::default(),
        miner_address: ZEBRAD_DEFAULT_MINER,
        chain_cache: Some(utils::chain_cache_dir().join("client_rpc_tests_large")),
        network: network::Network::Regtest,
    })
    .await
    .unwrap();
    zebrad.print_stdout();
    zebrad.print_stderr();

    assert_eq!(zebrad.get_chain_height().await, 52.into());
}

#[tokio::test]
async fn launch_localnet_zainod_zcashd() {
    tracing_subscriber::fmt().init();

    let local_net = LocalNet::<Zainod, Zcashd>::launch(
        ZainodConfig {
            zainod_bin: ZAINOD_BIN,
            listen_port: None,
            validator_port: 0,
            network: network::Network::Regtest,
        },
        ZcashdConfig {
            zcashd_bin: ZCASHD_BIN,
            zcash_cli_bin: ZCASH_CLI_BIN,
            rpc_listen_port: None,
            activation_heights: network::ActivationHeights::default(),
            miner_address: Some(REG_O_ADDR_FROM_ABANDONART),
            chain_cache: None,
        },
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
        ZainodConfig {
            zainod_bin: ZAINOD_BIN,
            listen_port: None,
            validator_port: 0,
            network: network::Network::Regtest,
        },
        ZebradConfig {
            zebrad_bin: ZEBRAD_BIN,
            network_listen_port: None,
            rpc_listen_port: None,
            activation_heights: network::ActivationHeights::default(),
            miner_address: ZEBRAD_DEFAULT_MINER,
            chain_cache: None,
            network: network::Network::Regtest,
        },
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
        LightwalletdConfig {
            lightwalletd_bin: LIGHTWALLETD_BIN,
            listen_port: None,
            zcashd_conf: PathBuf::new(),
        },
        ZcashdConfig {
            zcashd_bin: ZCASHD_BIN,
            zcash_cli_bin: ZCASH_CLI_BIN,
            rpc_listen_port: None,
            activation_heights: network::ActivationHeights::default(),
            miner_address: Some(REG_O_ADDR_FROM_ABANDONART),
            chain_cache: None,
        },
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
        LightwalletdConfig {
            lightwalletd_bin: LIGHTWALLETD_BIN,
            listen_port: None,
            zcashd_conf: PathBuf::new(),
        },
        ZebradConfig {
            zebrad_bin: ZEBRAD_BIN,
            network_listen_port: None,
            rpc_listen_port: None,
            activation_heights: network::ActivationHeights::default(),
            miner_address: ZEBRAD_DEFAULT_MINER,
            chain_cache: None,
            network: network::Network::Regtest,
        },
    )
    .await;

    local_net.validator().print_stdout();
    local_net.validator().print_stderr();
    local_net.indexer().print_stdout();
    local_net.indexer().print_lwd_log();
    local_net.indexer().print_stderr();
}

#[tokio::test]
async fn zainod_zcashd_basic_send() {
    tracing_subscriber::fmt().init();

    let local_net = LocalNet::<Zainod, Zcashd>::launch(
        ZainodConfig {
            zainod_bin: ZAINOD_BIN,
            listen_port: None,
            validator_port: 0,
            network: network::Network::Regtest,
        },
        ZcashdConfig {
            zcashd_bin: ZCASHD_BIN,
            zcash_cli_bin: ZCASH_CLI_BIN,
            rpc_listen_port: None,
            activation_heights: network::ActivationHeights::default(),
            miner_address: Some(REG_O_ADDR_FROM_ABANDONART),
            chain_cache: None,
        },
    )
    .await;

    let lightclient_dir = tempfile::tempdir().unwrap();
    let (faucet, recipient) = client::build_lightclients(
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
    local_net.validator().generate_blocks(1).await.unwrap();
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
#[ignore = "flake"]
#[tokio::test]
async fn zainod_zebrad_basic_send() {
    tracing_subscriber::fmt().init();

    let local_net = LocalNet::<Zainod, Zebrad>::launch(
        ZainodConfig {
            zainod_bin: ZAINOD_BIN,
            listen_port: None,
            validator_port: 0,
            network: network::Network::Regtest,
        },
        ZebradConfig {
            zebrad_bin: ZEBRAD_BIN,
            network_listen_port: None,
            rpc_listen_port: None,
            activation_heights: network::ActivationHeights::default(),
            miner_address: ZEBRAD_DEFAULT_MINER,
            chain_cache: None,
            network: network::Network::Regtest,
        },
    )
    .await;

    let lightclient_dir = tempfile::tempdir().unwrap();
    let (faucet, recipient) = client::build_lightclients(
        lightclient_dir.path().to_path_buf(),
        local_net.indexer().port(),
    )
    .await;

    local_net.validator().generate_blocks(100).await.unwrap();
    faucet.do_sync(false).await.unwrap();
    faucet.quick_shield().await.unwrap();
    local_net.validator().generate_blocks(1).await.unwrap();
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
    local_net.validator().generate_blocks(1).await.unwrap();
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
async fn lightwalletd_zcashd_basic_send() {
    tracing_subscriber::fmt().init();

    let local_net = LocalNet::<Lightwalletd, Zcashd>::launch(
        LightwalletdConfig {
            lightwalletd_bin: LIGHTWALLETD_BIN,
            listen_port: None,
            zcashd_conf: PathBuf::new(),
        },
        ZcashdConfig {
            zcashd_bin: ZCASHD_BIN,
            zcash_cli_bin: ZCASH_CLI_BIN,
            rpc_listen_port: None,
            activation_heights: network::ActivationHeights::default(),
            miner_address: Some(REG_O_ADDR_FROM_ABANDONART),
            chain_cache: None,
        },
    )
    .await;

    let lightclient_dir = tempfile::tempdir().unwrap();
    let (faucet, recipient) = client::build_lightclients(
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
    local_net.validator().generate_blocks(1).await.unwrap();
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

#[tokio::test]
async fn lightwalletd_zebrad_basic_send() {
    tracing_subscriber::fmt().init();

    let local_net = LocalNet::<Lightwalletd, Zebrad>::launch(
        LightwalletdConfig {
            lightwalletd_bin: LIGHTWALLETD_BIN,
            listen_port: None,
            zcashd_conf: PathBuf::new(),
        },
        ZebradConfig {
            zebrad_bin: ZEBRAD_BIN,
            network_listen_port: None,
            rpc_listen_port: None,
            activation_heights: network::ActivationHeights::default(),
            miner_address: ZEBRAD_DEFAULT_MINER,
            chain_cache: None,
            network: network::Network::Regtest,
        },
    )
    .await;

    let lightclient_dir = tempfile::tempdir().unwrap();
    let (faucet, recipient) = client::build_lightclients(
        lightclient_dir.path().to_path_buf(),
        local_net.indexer().port(),
    )
    .await;

    local_net.validator().generate_blocks(100).await.unwrap();
    faucet.do_sync(false).await.unwrap();
    faucet.quick_shield().await.unwrap();
    local_net.validator().generate_blocks(1).await.unwrap();
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
    local_net.validator().generate_blocks(1).await.unwrap();
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

mod client_rpcs {
    //! - In order to generate a cached blockchain from zebrad run:
    //! ```BASH
    //! ./utils/regenerate_chain_caches_report_diff.sh
    //! ```
    //! This command generates new data in the `chain_cache` directory.  The new structure should have the following added
    //!
    //! ```BASH
    //!  ├── [       4096]  client_rpc_tests_large
    //!  └── [       4096]  state
    //!      └── [       4096]  v26
    //!          └── [       4096]  regtest
    //!              ├── [     139458]  000004.log
    //!              ├── [         16]  CURRENT
    //!              ├── [         36]  IDENTITY
    //!              ├── [          0]  LOCK
    //!              ├── [     174621]  LOG
    //!              ├── [       1708]  MANIFEST-000005
    //!              ├── [     114923]  OPTIONS-000007
    //!              └── [          3]  version
    //! ```
    use zc_infra_nodes::network::Network;

    use crate::{LIGHTWALLETD_BIN, ZAINOD_BIN, ZCASHD_BIN, ZCASH_CLI_BIN, ZEBRAD_BIN};

    #[ignore = "not a test. generates chain cache for client_rpc tests."]
    #[tokio::test]
    async fn generate_zebrad_large_chain_cache() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::generate_zebrad_large_chain_cache(
            ZEBRAD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[ignore = "not a test. generates chain cache for client_rpc tests."]
    #[tokio::test]
    async fn generate_zcashd_chain_cache() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::generate_zcashd_chain_cache(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_lightd_info() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_lightd_info(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_latest_block() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_latest_block(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_block() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_block(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[ignore = "probable zaino lwd mismatch"]
    #[tokio::test]
    async fn get_block_out_of_bounds() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_block_out_of_bounds(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_block_nullifiers() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_block_nullifiers(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_block_range_nullifiers() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_block_range_nullifiers(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_block_range_nullifiers_reverse() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_block_range_nullifiers_reverse(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_block_range_lower() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_block_range_lower(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_block_range_upper() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_block_range_upper(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_block_range_reverse() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_block_range_reverse(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[ignore = "temporary during refactor into workspace"]
    #[tokio::test]
    async fn get_block_range_out_of_bounds() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_block_range_out_of_bounds(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_transaction() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_transaction(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[ignore = "incomplete"]
    #[tokio::test]
    async fn send_transaction() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::send_transaction(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_taddress_txids_all() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_taddress_txids_all(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_taddress_txids_lower() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_taddress_txids_lower(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_taddress_txids_upper() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_taddress_txids_upper(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_taddress_balance() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_taddress_balance(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_taddress_balance_stream() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_taddress_balance_stream(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_mempool_tx() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_mempool_tx(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[ignore = "temporary during refactor into workspace"]
    #[tokio::test]
    async fn get_mempool_stream_zingolib_mempool_monitor() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_mempool_stream_zingolib_mempool_monitor(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[ignore = "temporary during refactor into workspace"]
    #[tokio::test]
    async fn get_mempool_stream() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_mempool_stream(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_tree_state_by_height() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_tree_state_by_height(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_tree_state_by_hash() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_tree_state_by_hash(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_tree_state_out_of_bounds() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_tree_state_out_of_bounds(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_latest_tree_state() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_latest_tree_state(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_address_utxos_all() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_address_utxos_all(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_address_utxos_lower() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_address_utxos_lower(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_address_utxos_upper() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_address_utxos_upper(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_address_utxos_out_of_bounds() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_address_utxos_out_of_bounds(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_address_utxos_stream_all() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_address_utxos_stream_all(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_address_utxos_stream_lower() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_address_utxos_stream_lower(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_address_utxos_stream_upper() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_address_utxos_stream_upper(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_address_utxos_stream_out_of_bounds() {
        tracing_subscriber::fmt().init();

        zc_infra_testutils::test_fixtures::get_address_utxos_stream_out_of_bounds(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }
    mod get_subtree_roots {
        //! - To run the `get_subtree_roots_sapling` test, sync Zebrad in testnet mode and copy the cache to `zcash_local_net/chain_cache/testnet_get_subtree_roots_sapling`. At least 2 sapling shards must be synced to pass. See [crate::test_fixtures::get_subtree_roots_sapling] doc comments for more details.
        //! - To run the `get_subtree_roots_orchard` test, sync Zebrad in mainnet mode and copy the cache to `zcash_local_net/chain_cache/testnet_get_subtree_roots_orchard`. At least 2 orchard shards must be synced to pass. See [crate::test_fixtures::get_subtree_roots_orchard] doc comments for more details.
        use super::*;
        /// This test requires Zebrad testnet to be already synced to at least 2 sapling shards with the cache at
        /// `zcash_local_net/chain_cache/get_subtree_roots_sapling`
        #[ignore = "this test requires manual setup"]
        #[tokio::test]
        async fn sapling() {
            tracing_subscriber::fmt().init();

            zc_infra_testutils::test_fixtures::get_subtree_roots_sapling(
                ZEBRAD_BIN,
                ZAINOD_BIN,
                LIGHTWALLETD_BIN,
                Network::Testnet,
            )
            .await;
        }

        /// This test requires Zebrad mainnet to be already synced to at least 2 sapling shards with the cache at
        /// `zcash_local_net/chain_cache/get_subtree_roots_orchard`
        #[ignore = "this test requires manual setup"]
        #[tokio::test]
        async fn orchard() {
            tracing_subscriber::fmt().init();

            zc_infra_testutils::test_fixtures::get_subtree_roots_orchard(
                ZEBRAD_BIN,
                ZAINOD_BIN,
                LIGHTWALLETD_BIN,
                Network::Mainnet,
            )
            .await;
        }
    }
}
