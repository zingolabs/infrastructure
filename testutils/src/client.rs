//! Module for structs and functions associated with light-clients

use std::path::PathBuf;

use portpicker::Port;
use testvectors::seeds;
use zcash_client_backend::proto::service::compact_tx_streamer_client::CompactTxStreamerClient;
use zingo_infra_services::network;
use zingo_netutils::{GetClientError, GrpcConnector, UnderlyingService};
use zingolib::{
    config::RegtestNetwork, lightclient::LightClient, testutils::scenarios::setup::ClientBuilder,
};

/// Builds a client for creating RPC requests to the indexer/light-node
pub fn build_client(
    uri: http::Uri,
) -> impl std::future::Future<Output = Result<CompactTxStreamerClient<UnderlyingService>, GetClientError>>
{
    GrpcConnector::new(uri).get_client()
}

// NOTE: this should be migrated to zingolib when LocalNet replaces regtest manager in zingoilb::testutils
/// Builds faucet (miner) and recipient lightclients for local network integration testing
pub async fn build_lightclients(
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
