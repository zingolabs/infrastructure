//! Utilities module

use std::{env, path::PathBuf};

/// Functions for picking executables from env variables in the global runtime.
pub mod executable_finder;

/// Returns path to cargo manifest directory (project root)
pub(crate) fn cargo_manifest_dir() -> PathBuf {
    PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo manifest to resolve to pathbuf"))
}

/// Returns a path to the chain cache directory
#[must_use]
pub fn chain_cache_dir() -> PathBuf {
    cargo_manifest_dir().join("chain_cache")
}

// // NOTE: this should be migrated to zingolib when LocalNet replaces regtest manager in zingoilb::testutils
// /// Builds faucet (miner) and recipient lightclients for local network integration testing
// pub fn build_lightclients(
//     lightclient_dir: PathBuf,
//     indexer_port: Port,
// ) -> (LightClient, LightClient) {
//     let mut client_builder =
//         ClientBuilder::new(network::localhost_uri(indexer_port), lightclient_dir);
//     let faucet = client_builder.build_faucet(true, RegtestNetwork::all_upgrades_active());
//     let recipient = client_builder.build_client(
//         seeds::HOSPITAL_MUSEUM_SEED.to_string(),
//         1,
//         true,
//         RegtestNetwork::all_upgrades_active(),
//     );

//     (faucet, recipient)
// }
