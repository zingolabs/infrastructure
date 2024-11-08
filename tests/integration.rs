#![cfg(feature = "client")]

use std::path::PathBuf;

use zcash_protocol::{PoolType, ShieldedProtocol};

use zingolib::{
    testutils::lightclient::{from_inputs, get_base_address},
    testvectors::REG_O_ADDR_FROM_ABANDONART,
};

use zcash_local_net::{
    client,
    indexer::{Indexer as _, Lightwalletd, LightwalletdConfig, Zainod, ZainodConfig},
    network,
    validator::{Validator, Zcashd, ZcashdConfig, Zebrad, ZebradConfig, ABANDON_ABANDON_UA},
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
        rpc_port: None,
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
        miner_address: ABANDON_ABANDON_UA,
        chain_cache: None,
    })
    .await
    .unwrap();
    zebrad.print_stdout();
    zebrad.print_stderr();
}

#[tokio::test]
async fn launch_localnet_zainod_zcashd() {
    tracing_subscriber::fmt().init();

    let local_net = LocalNet::<Zainod, Zcashd>::launch(
        ZainodConfig {
            zainod_bin: ZAINOD_BIN,
            listen_port: None,
            validator_port: 0,
        },
        ZcashdConfig {
            zcashd_bin: ZCASHD_BIN,
            zcash_cli_bin: ZCASH_CLI_BIN,
            rpc_port: None,
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
        },
        ZebradConfig {
            zebrad_bin: ZEBRAD_BIN,
            network_listen_port: None,
            rpc_listen_port: None,
            activation_heights: network::ActivationHeights::default(),
            miner_address: ABANDON_ABANDON_UA,
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
async fn launch_localnet_lightwalletd_zcashd() {
    tracing_subscriber::fmt().init();

    let local_net = LocalNet::<Lightwalletd, Zcashd>::launch(
        LightwalletdConfig {
            lightwalletd_bin: LIGHTWALLETD_BIN,
            listen_port: None,
            validator_conf: PathBuf::new(),
        },
        ZcashdConfig {
            zcashd_bin: ZCASHD_BIN,
            zcash_cli_bin: ZCASH_CLI_BIN,
            rpc_port: None,
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
            validator_conf: PathBuf::new(),
        },
        ZebradConfig {
            zebrad_bin: ZEBRAD_BIN,
            network_listen_port: None,
            rpc_listen_port: Some(18232),
            activation_heights: network::ActivationHeights::default(),
            miner_address: ABANDON_ABANDON_UA,
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
async fn zainod_basic_send() {
    tracing_subscriber::fmt().init();

    let local_net = LocalNet::<Zainod, Zcashd>::launch(
        ZainodConfig {
            zainod_bin: ZAINOD_BIN,
            listen_port: None,
            validator_port: 0,
        },
        ZcashdConfig {
            zcashd_bin: ZCASHD_BIN,
            zcash_cli_bin: ZCASH_CLI_BIN,
            rpc_port: None,
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

#[tokio::test]
async fn lightwalletd_basic_send() {
    tracing_subscriber::fmt().init();

    let local_net = LocalNet::<Lightwalletd, Zcashd>::launch(
        LightwalletdConfig {
            lightwalletd_bin: LIGHTWALLETD_BIN,
            listen_port: None,
            validator_conf: PathBuf::new(),
        },
        ZcashdConfig {
            zcashd_bin: ZCASHD_BIN,
            zcash_cli_bin: ZCASH_CLI_BIN,
            rpc_port: None,
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

#[cfg(feature = "test_fixtures")]
mod client_rpcs {
    use crate::{LIGHTWALLETD_BIN, ZAINOD_BIN, ZCASHD_BIN, ZCASH_CLI_BIN};

    #[ignore = "not a test. generates chain cache for client_rpc tests."]
    #[tokio::test]
    async fn generate_zcashd_chain_cache() {
        tracing_subscriber::fmt().init();

        zcash_local_net::test_fixtures::generate_zcashd_chain_cache(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_lightd_info() {
        tracing_subscriber::fmt().init();

        zcash_local_net::test_fixtures::get_lightd_info(
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

        zcash_local_net::test_fixtures::get_latest_block(
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

        zcash_local_net::test_fixtures::get_block(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_block_out_of_bounds() {
        tracing_subscriber::fmt().init();

        zcash_local_net::test_fixtures::get_block_out_of_bounds(
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

        zcash_local_net::test_fixtures::get_block_nullifiers(
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

        zcash_local_net::test_fixtures::get_block_range_nullifiers(
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

        zcash_local_net::test_fixtures::get_block_range_nullifiers_reverse(
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

        zcash_local_net::test_fixtures::get_block_range_lower(
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

        zcash_local_net::test_fixtures::get_block_range_upper(
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

        zcash_local_net::test_fixtures::get_block_range_reverse(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_block_range_out_of_bounds() {
        tracing_subscriber::fmt().init();

        zcash_local_net::test_fixtures::get_block_range_out_of_bounds(
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

        zcash_local_net::test_fixtures::get_transaction(
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

        zcash_local_net::test_fixtures::send_transaction(
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

        zcash_local_net::test_fixtures::get_taddress_txids_all(
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

        zcash_local_net::test_fixtures::get_taddress_txids_lower(
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

        zcash_local_net::test_fixtures::get_taddress_txids_upper(
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

        zcash_local_net::test_fixtures::get_taddress_balance(
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

        zcash_local_net::test_fixtures::get_taddress_balance_stream(
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

        zcash_local_net::test_fixtures::get_mempool_tx(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    #[tokio::test]
    async fn get_mempool_stream() {
        tracing_subscriber::fmt().init();

        zcash_local_net::test_fixtures::get_mempool_stream(
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

        zcash_local_net::test_fixtures::get_tree_state_by_height(
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

        zcash_local_net::test_fixtures::get_tree_state_by_hash(
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

        zcash_local_net::test_fixtures::get_tree_state_out_of_bounds(
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

        zcash_local_net::test_fixtures::get_latest_tree_state(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    // this is not a satisfactory test for this rpc and will return empty vecs.
    // this rpc should also be tested in testnet/mainnet or a local chain with at least 2 shards should be cached.
    #[tokio::test]
    async fn get_subtree_roots_sapling() {
        tracing_subscriber::fmt().init();

        zcash_local_net::test_fixtures::get_subtree_roots_sapling(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }

    // this is not a satisfactory test for this rpc and will return empty vecs.
    // this rpc should also be tested in testnet/mainnet or a local chain with at least 2 shards should be cached.
    #[tokio::test]
    async fn get_subtree_roots_orchard() {
        tracing_subscriber::fmt().init();

        zcash_local_net::test_fixtures::get_subtree_roots_orchard(
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

        zcash_local_net::test_fixtures::get_address_utxos_all(
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

        zcash_local_net::test_fixtures::get_address_utxos_lower(
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

        zcash_local_net::test_fixtures::get_address_utxos_upper(
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

        zcash_local_net::test_fixtures::get_address_utxos_out_of_bounds(
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

        zcash_local_net::test_fixtures::get_address_utxos_stream_all(
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

        zcash_local_net::test_fixtures::get_address_utxos_stream_lower(
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

        zcash_local_net::test_fixtures::get_address_utxos_stream_upper(
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

        zcash_local_net::test_fixtures::get_address_utxos_stream_out_of_bounds(
            ZCASHD_BIN,
            ZCASH_CLI_BIN,
            ZAINOD_BIN,
            LIGHTWALLETD_BIN,
        )
        .await;
    }
}
