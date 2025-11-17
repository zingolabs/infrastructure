//! Client RPC tests

use zebra_chain::parameters::NetworkKind;

#[ignore = "not a test. generates chain cache for client_rpc tests."]
#[tokio::test]
async fn generate_zebrad_large_chain_cache() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::generate_zebrad_large_chain_cache().await;
}

#[ignore = "not a test. generates chain cache for client_rpc tests."]
#[tokio::test]
async fn generate_zcashd_chain_cache() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::generate_zcashd_chain_cache().await;
}

#[tokio::test]
async fn get_lightd_info() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_lightd_info().await;
}

#[tokio::test]
async fn get_latest_block() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_latest_block().await;
}

#[tokio::test]
async fn get_block() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_block().await;
}

#[tokio::test]
async fn get_block_out_of_bounds() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_block_out_of_bounds().await;
}

#[tokio::test]
async fn get_block_nullifiers() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_block_nullifiers().await;
}

#[tokio::test]
async fn get_block_range_nullifiers() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_block_range_nullifiers().await;
}

#[tokio::test]
async fn get_block_range_nullifiers_reverse() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_block_range_nullifiers_reverse().await;
}

#[tokio::test]
async fn get_block_range_lower() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_block_range_lower().await;
}

#[tokio::test]
async fn get_block_range_upper() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_block_range_upper().await;
}

#[tokio::test]
async fn get_block_range_reverse() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_block_range_reverse().await;
}

#[tokio::test]
async fn get_block_range_out_of_bounds() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_block_range_out_of_bounds().await;
}

#[tokio::test]
async fn get_transaction() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_transaction().await;
}

#[ignore = "incomplete"]
#[tokio::test]
async fn send_transaction() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::send_transaction().await;
}

#[tokio::test]
async fn get_taddress_txids_all() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_taddress_txids_all().await;
}

#[tokio::test]
async fn get_taddress_txids_lower() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_taddress_txids_lower().await;
}

#[tokio::test]
async fn get_taddress_txids_upper() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_taddress_txids_upper().await;
}

#[tokio::test]
async fn get_taddress_balance() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_taddress_balance().await;
}

#[tokio::test]
async fn get_taddress_balance_stream() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_taddress_balance_stream().await;
}

#[tokio::test]
async fn get_mempool_tx() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_mempool_tx().await;
}

#[tokio::test]
async fn get_mempool_stream_zingolib_mempool_monitor() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_mempool_stream_zingolib_mempool_monitor().await;
}

#[tokio::test]
async fn get_mempool_stream() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_mempool_stream().await;
}

#[tokio::test]
async fn get_tree_state_by_height() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_tree_state_by_height().await;
}

#[tokio::test]
async fn get_tree_state_by_hash() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_tree_state_by_hash().await;
}

#[tokio::test]
async fn get_tree_state_out_of_bounds() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_tree_state_out_of_bounds().await;
}

#[tokio::test]
async fn get_latest_tree_state() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_latest_tree_state().await;
}

/// This test requires Zebrad testnet to be already synced to at least 2 sapling shards with the cache at
/// `zcash_local_net/chain_cache/get_subtree_roots_sapling`
#[tokio::test]
async fn get_subtree_roots_sapling() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_subtree_roots_sapling(NetworkKind::Testnet).await;
}

/// This test requires Zebrad mainnet to be already synced to at least 2 orchard shards with the cache at
/// `zcash_local_net/chain_cache/get_subtree_roots_orchard`
#[tokio::test]
async fn get_subtree_roots_orchard() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_subtree_roots_orchard(NetworkKind::Mainnet).await;
}

#[tokio::test]
async fn get_address_utxos_all() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_address_utxos_all().await;
}

#[tokio::test]
async fn get_address_utxos_lower() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_address_utxos_lower().await;
}

#[tokio::test]
async fn get_address_utxos_upper() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_address_utxos_upper().await;
}

#[tokio::test]
async fn get_address_utxos_out_of_bounds() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_address_utxos_out_of_bounds().await;
}

#[tokio::test]
async fn get_address_utxos_stream_all() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_address_utxos_stream_all().await;
}

#[tokio::test]
async fn get_address_utxos_stream_lower() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_address_utxos_stream_lower().await;
}

#[tokio::test]
async fn get_address_utxos_stream_upper() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_address_utxos_stream_upper().await;
}

#[tokio::test]
async fn get_address_utxos_stream_out_of_bounds() {
    tracing_subscriber::fmt().init();

    client_rpc_test_fixtures::get_address_utxos_stream_out_of_bounds().await;
}
