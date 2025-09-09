//! Module for structs and functions associated with light-clients

use std::path::PathBuf;

use http_body_util::combinators::UnsyncBoxBody;
use portpicker::Port;
use testvectors::seeds;
use tower::util::BoxCloneService;
use zingo_infra_services::network;
use zingolib::{
    config::RegtestNetwork, lightclient::LightClient, testutils::scenarios::setup::ClientBuilder,
};

/// The underlying service type used for gRPC connections
pub type UnderlyingService = BoxCloneService<
    http::Request<UnsyncBoxBody<prost::bytes::Bytes, tonic::Status>>,
    http::Response<hyper::body::Incoming>,
    hyper_util::client::legacy::Error,
>;

// NOTE: this should be migrated to zingolib when LocalNet replaces regtest manager in zingoilb::testutils
/// Builds faucet (miner) and recipient lightclients for local network integration testing
pub fn build_lightclients(
    lightclient_dir: PathBuf,
    indexer_port: Port,
) -> (LightClient, LightClient) {
    let mut client_builder =
        ClientBuilder::new(network::localhost_uri(indexer_port), lightclient_dir);
    let faucet = client_builder.build_faucet(true, RegtestNetwork::all_upgrades_active());
    let recipient = client_builder.build_client(
        seeds::HOSPITAL_MUSEUM_SEED.to_string(),
        1,
        true,
        RegtestNetwork::all_upgrades_active(),
    );

    (faucet, recipient)
}
