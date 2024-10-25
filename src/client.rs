//! Module for structs and functions associated with light-clients

use zcash_client_backend::proto::service::compact_tx_streamer_client::CompactTxStreamerClient;
use zingo_netutils::{GetClientError, GrpcConnector, UnderlyingService};

/// Builds a client for creating RPC requests to the indexer/light-node
pub fn build_client(
    uri: http::Uri,
) -> impl std::future::Future<Output = Result<CompactTxStreamerClient<UnderlyingService>, GetClientError>>
{
    GrpcConnector::new(uri).get_client()
}
