// #[ignore = "Out of scope. Should set up a chain cache and compare with a basic gprc request"]
// #[tokio::test]
// async fn zainod_zcashd_basic_send() {
//     tracing_subscriber::fmt().init();

//     let local_net = LocalNet::<Zainod, Zcashd>::launch(
//         ZainodConfig::default_test(),
//         ZcashdConfig::default_test(),
//     )
//     .await;

//     let lightclient_dir = tempfile::tempdir().unwrap();
//     let (mut faucet, mut recipient) = client::build_lightclients(
//         lightclient_dir.path().to_path_buf(),
//         local_net.indexer().port(),
//     );
//     tokio::time::sleep(std::time::Duration::from_millis(500)).await;

//     faucet.sync_and_await().await.unwrap();
//     from_inputs::quick_send(
//         &mut faucet,
//         vec![(
//             &get_base_address(&recipient, PoolType::Shielded(ShieldedProtocol::Orchard)).await,
//             100_000,
//             None,
//         )],
//     )
//     .await
//     .unwrap();
//     local_net.validator().generate_blocks(1).await.unwrap();
//     tokio::time::sleep(std::time::Duration::from_millis(500)).await;

//     faucet.sync_and_await().await.unwrap();
//     recipient.sync_and_await().await.unwrap();

//     let recipient_balance = recipient.do_balance().await;
//     assert_eq!(recipient_balance.verified_orchard_balance, Some(100_000));

//     local_net.validator().print_stdout();
//     local_net.validator().print_stderr();
//     local_net.indexer().print_stdout();
//     local_net.indexer().print_stderr();
//     println!("faucet balance:");
//     println!("{:?}\n", faucet.do_balance().await);
//     println!("recipient balance:");
//     println!("{:?}\n", recipient_balance);
// }

// #[ignore = "Out of scope. Should set up a chain cache and compare with a basic gprc request"]
// #[tokio::test]
// async fn zainod_zebrad_basic_send() {
//     tracing_subscriber::fmt().init();

//     let local_net = LocalNet::<Zainod, Zebrad>::launch(
//         ZainodConfig::default_test(),
//         ZebradConfig::default_test(),
//     )
//     .await;

//     let lightclient_dir = tempfile::tempdir().unwrap();
//     let (mut faucet, mut recipient) = client::build_lightclients(
//         lightclient_dir.path().to_path_buf(),
//         local_net.indexer().port(),
//     );

//     local_net.validator().generate_blocks(100).await.unwrap();
//     tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

//     faucet.sync_and_await().await.unwrap();
//     faucet.quick_shield().await.unwrap();
//     local_net.validator().generate_blocks(1).await.unwrap();
//     tokio::time::sleep(std::time::Duration::from_millis(500)).await;

//     faucet.sync_and_await().await.unwrap();

//     from_inputs::quick_send(
//         &mut faucet,
//         vec![(
//             &get_base_address(&recipient, PoolType::Shielded(ShieldedProtocol::Orchard)).await,
//             100_000,
//             None,
//         )],
//     )
//     .await
//     .unwrap();
//     local_net.validator().generate_blocks(1).await.unwrap();
//     tokio::time::sleep(std::time::Duration::from_millis(500)).await;

//     faucet.sync_and_await().await.unwrap();
//     recipient.sync_and_await().await.unwrap();

//     let recipient_balance = recipient.do_balance().await;
//     assert_eq!(recipient_balance.verified_orchard_balance, Some(100_000));

//     local_net.validator().print_stdout();
//     local_net.validator().print_stderr();
//     local_net.indexer().print_stdout();
//     local_net.indexer().print_stderr();
//     println!("faucet balance:");
//     println!("{:?}\n", faucet.do_balance().await);
//     println!("recipient balance:");
//     println!("{:?}\n", recipient_balance);
// }

// #[ignore = "lightwalletd v0.4.18+ incorrectly expects 1344-byte Equihash solutions for regtest blocks"]
// #[tokio::test]
// async fn lightwalletd_zcashd_basic_send() {
//     tracing_subscriber::fmt().init();

//     let local_net = LocalNet::<Lightwalletd, Zcashd>::launch(
//         LightwalletdConfig::default_test(),
//         ZcashdConfig::default_test(),
//     )
//     .await;

//     let lightclient_dir = tempfile::tempdir().unwrap();
//     let (mut faucet, mut recipient) = client::build_lightclients(
//         lightclient_dir.path().to_path_buf(),
//         local_net.indexer().port(),
//     );

//     faucet.sync_and_await().await.unwrap();
//     from_inputs::quick_send(
//         &mut faucet,
//         vec![(
//             &get_base_address(&recipient, PoolType::Shielded(ShieldedProtocol::Orchard)).await,
//             100_000,
//             None,
//         )],
//     )
//     .await
//     .unwrap();
//     local_net.validator().generate_blocks(1).await.unwrap();
//     faucet.sync_and_await().await.unwrap();
//     recipient.sync_and_await().await.unwrap();

//     let recipient_balance = recipient.do_balance().await;
//     assert_eq!(recipient_balance.verified_orchard_balance, Some(100_000));

//     local_net.validator().print_stdout();
//     local_net.validator().print_stderr();
//     local_net.indexer().print_stdout();
//     local_net.indexer().print_lwd_log();
//     local_net.indexer().print_stderr();
//     println!("faucet balance:");
//     println!("{:?}\n", faucet.do_balance().await);
//     println!("recipient balance:");
//     println!("{:?}\n", recipient_balance);
// }

// #[ignore = "Out of scope. Should set up a chain cache and compare with a basic gprc request"]
// #[tokio::test]
// async fn lightwalletd_zebrad_basic_send() {
//     tracing_subscriber::fmt().init();

//     let local_net = LocalNet::<Lightwalletd, Zebrad>::launch(
//         LightwalletdConfig::default_test(),
//         ZebradConfig::default_test(),
//     )
//     .await;

//     let lightclient_dir = tempfile::tempdir().unwrap();
//     let (mut faucet, mut recipient) = client::build_lightclients(
//         lightclient_dir.path().to_path_buf(),
//         local_net.indexer().port(),
//     );

//     local_net.validator().generate_blocks(100).await.unwrap();
//     faucet.sync_and_await().await.unwrap();
//     faucet.quick_shield().await.unwrap();
//     local_net.validator().generate_blocks(1).await.unwrap();
//     faucet.sync_and_await().await.unwrap();

//     from_inputs::quick_send(
//         &mut faucet,
//         vec![(
//             &get_base_address(&recipient, PoolType::Shielded(ShieldedProtocol::Orchard)).await,
//             100_000,
//             None,
//         )],
//     )
//     .await
//     .unwrap();
//     local_net.validator().generate_blocks(1).await.unwrap();
//     faucet.sync_and_await().await.unwrap();
//     recipient.sync_and_await().await.unwrap();

//     let recipient_balance = recipient.do_balance().await;
//     assert_eq!(recipient_balance.verified_orchard_balance, Some(100_000));

//     local_net.validator().print_stdout();
//     local_net.validator().print_stderr();
//     local_net.indexer().print_stdout();
//     local_net.indexer().print_stderr();
//     println!("faucet balance:");
//     println!("{:?}\n", faucet.do_balance().await);
//     println!("recipient balance:");
//     println!("{:?}\n", recipient_balance);
// }

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
    use zingo_infra_services::{
        indexer::LightwalletdConfig,
        validator::{ZcashdConfig, ZebradConfig},
    };

    #[ignore = "not a test. generates chain cache for client_rpc tests."]
    #[tokio::test]
    async fn generate_zebrad_large_chain_cache() {
        tracing_subscriber::fmt().init();

        zingo_infra_testutils::test_fixtures::generate_zebrad_large_chain_cache(
            ZebradConfig::default_location(),
            LightwalletdConfig::default_location(),
        )
        .await;
    }

    // FIXME: This is not a test, so it shouldn't be marked as one.
    // and TODO: Pre-test setups should be moved elsewhere.
    #[ignore = "not a test. generates chain cache for client_rpc tests."]
    #[tokio::test]
    async fn generate_zcashd_chain_cache() {
        tracing_subscriber::fmt().init();

        zingo_infra_testutils::test_fixtures::generate_zcashd_chain_cache(
            ZcashdConfig::default_location(),
            ZcashdConfig::default_cli_location(),
            LightwalletdConfig::default_location(),
        )
        .await;
    }

    macro_rules! _rpc_fixture_test {
        ($test_name:ident) => {
            #[tokio::test]
            async fn $test_name() {
                tracing_subscriber::fmt().init();

                zingo_infra_testutils::test_fixtures::$test_name(
                    ZcashdConfig::default_location(),
                    ZcashdConfig::default_cli_location(),
                    ZainodConfig::default_location(),
                    LightwalletdConfig::default_location(),
                )
                .await;
            }
        };
    }

    // FIXME: These tests DO make sense to keep around.
    // They need to be refactored so that they don't require a specific lightclient usage (a grpc client should be enough).
    // mod get_subtree_roots {
    //     //! - To run the `get_subtree_roots_sapling` test, sync Zebrad in testnet mode and copy the cache to `zcash_local_net/chain_cache/testnet_get_subtree_roots_sapling`. At least 2 sapling shards must be synced to pass. See [crate::test_fixtures::get_subtree_roots_sapling] doc comments for more details.
    //     //! - To run the `get_subtree_roots_orchard` test, sync Zebrad in mainnet mode and copy the cache to `zcash_local_net/chain_cache/testnet_get_subtree_roots_orchard`. At least 2 orchard shards must be synced to pass. See [crate::test_fixtures::get_subtree_roots_orchard] doc comments for more details.
    //     use super::*;
    //     /// This test requires Zebrad testnet to be already synced to at least 2 sapling shards with the cache at
    //     /// `zcash_local_net/chain_cache/get_subtree_roots_sapling`
    //     #[ignore = "this test requires manual setup"]
    //     #[tokio::test]
    //     async fn sapling() {
    //         tracing_subscriber::fmt().init();

    //         zingo_infra_testutils::test_fixtures::get_subtree_roots_sapling(
    //             ZebradConfig::default_location(),
    //             ZainodConfig::default_location(),
    //             LightwalletdConfig::default_location(),
    //             Network::Testnet,
    //         )
    //         .await;
    //     }

    //     /// This test requires Zebrad mainnet to be already synced to at least 2 sapling shards with the cache at
    //     /// `zcash_local_net/chain_cache/get_subtree_roots_orchard`
    //     #[ignore = "this test requires manual setup"]
    //     #[tokio::test]
    //     async fn orchard() {
    //         tracing_subscriber::fmt().init();

    //         zingo_infra_testutils::test_fixtures::get_subtree_roots_orchard(
    //             ZebradConfig::default_location(),
    //             ZainodConfig::default_location(),
    //             LightwalletdConfig::default_location(),
    //             Network::Mainnet,
    //         )
    //         .await;
    //     }
    // }
    // previously ignored
    // rpc_fixture_test!(get_block_out_of_bounds);
    // rpc_fixture_test!(get_block_range_out_of_bounds);
    // rpc_fixture_test!(send_transaction);
    // rpc_fixture_test!(get_mempool_stream_zingolib_mempool_monitor);
    // rpc_fixture_test!(get_mempool_stream);

    // slow
    // rpc_fixture_test!(get_mempool_tx);
    // rpc_fixture_test!(get_transaction);

    // FIXME: These tests are out of scope
    // rpc_fixture_test!(get_lightd_info);
    // rpc_fixture_test!(get_latest_block);
    // rpc_fixture_test!(get_taddress_txids_all);
    // rpc_fixture_test!(get_taddress_txids_lower);
    // rpc_fixture_test!(get_taddress_txids_upper);
    // rpc_fixture_test!(get_taddress_balance);
    // rpc_fixture_test!(get_taddress_balance_stream);
    // rpc_fixture_test!(get_tree_state_by_height);
    // rpc_fixture_test!(get_tree_state_out_of_bounds);
    // rpc_fixture_test!(get_latest_tree_state);
    // rpc_fixture_test!(get_address_utxos_all);
    // rpc_fixture_test!(get_address_utxos_lower);
    // rpc_fixture_test!(get_address_utxos_upper);
    // rpc_fixture_test!(get_address_utxos_out_of_bounds);
    // rpc_fixture_test!(get_address_utxos_stream_all);
    // rpc_fixture_test!(get_address_utxos_stream_lower);
    // rpc_fixture_test!(get_address_utxos_stream_upper);
    // rpc_fixture_test!(get_address_utxos_stream_out_of_bounds);
}
