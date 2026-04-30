//! Module for configuring processes and writing configuration files

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use zingo_common_components::protocol::{ActivationHeights, NetworkType};

/// Convert `NetworkKind` to its config string representation
fn network_type_to_string(network: NetworkType) -> &'static str {
    match network {
        NetworkType::Mainnet => "Mainnet",
        NetworkType::Testnet => "Testnet",
        NetworkType::Regtest(_) => "Regtest",
    }
}

/// Used in subtree roots tests in `zaino_testutils`.  Fix later.
pub const ZCASHD_FILENAME: &str = "zcash.conf";
pub(crate) const ZEBRAD_FILENAME: &str = "zebrad.toml";
pub(crate) const ZAINOD_FILENAME: &str = "zindexer.toml";
pub(crate) const LIGHTWALLETD_FILENAME: &str = "lightwalletd.yml";

/// Writes the Zcashd config file to the specified config directory.
/// Returns the path to the config file.
pub(crate) fn write_zcashd_config(
    config_dir: &Path,
    rpc_port: u16,
    activation_heights: ActivationHeights,
    miner_address: Option<&str>,
) -> std::io::Result<PathBuf> {
    let config_file_path = config_dir.join(ZCASHD_FILENAME);
    let file = File::create(config_file_path.clone())?;
    let mut config_file = BufWriter::new(file);

    let overwinter_activation_height = activation_heights
        .overwinter()
        .expect("overwinter activation height must be specified");
    let sapling_activation_height = activation_heights
        .sapling()
        .expect("sapling activation height must be specified");
    let blossom_activation_height = activation_heights
        .blossom()
        .expect("blossom activation height must be specified");
    let heartwood_activation_height = activation_heights
        .heartwood()
        .expect("heartwood activation height must be specified");
    let canopy_activation_height = activation_heights
        .canopy()
        .expect("canopy activation height must be specified");
    let nu5_activation_height = activation_heights
        .nu5()
        .expect("nu5 activation height must be specified");
    let nu6_activation_height = activation_heights
        .nu6()
        .expect("nu6 activation height must be specified");
    let nu6_1_activation_height = activation_heights
        .nu6_1()
        .expect("nu6.1 activation height must be specified");

    let mut cfg = format!(
        "\
### Blockchain Configuration
regtest=1
nuparams=5ba81b19:{overwinter_activation_height} # Overwinter
nuparams=76b809bb:{sapling_activation_height} # Sapling
nuparams=2bb40e60:{blossom_activation_height} # Blossom
nuparams=f5b9230b:{heartwood_activation_height} # Heartwood
nuparams=e9ff75a6:{canopy_activation_height} # Canopy
nuparams=c2d6d0b4:{nu5_activation_height} # NU5 (Orchard)
nuparams=c8e71055:{nu6_activation_height} # NU6
nuparams=4dec4df0:{nu6_1_activation_height} # NU6_1 https://zips.z.cash/zip-0255#nu6.1deployment

### MetaData Storage and Retrieval
# txindex:
# https://zcash.readthedocs.io/en/latest/rtd_pages/zcash_conf_guide.html#miscellaneous-options
txindex=1
# insightexplorer:
# https://zcash.readthedocs.io/en/latest/rtd_pages/insight_explorer.html?highlight=insightexplorer#additional-getrawtransaction-fields
insightexplorer=1
experimentalfeatures=1
lightwalletd=1

### RPC Server Interface Options:
# https://zcash.readthedocs.io/en/latest/rtd_pages/zcash_conf_guide.html#json-rpc-options
rpcuser=xxxxxx
rpcpassword=xxxxxx
rpcport={rpc_port}
rpcallowip=127.0.0.1

# Buried config option to allow non-canonical RPC-PORT:
# https://zcash.readthedocs.io/en/latest/rtd_pages/zcash_conf_guide.html#zcash-conf-guide
listen=0

i-am-aware-zcashd-will-be-replaced-by-zebrad-and-zallet-in-2025=1"
    );

    if let Some(addr) = miner_address {
        cfg.push_str(&format!(
            "\n\n\
### Zcashd Help provides documentation of the following:
mineraddress={addr}
minetolocalwallet=0 # This is set to false so that we can mine to a wallet, other than the zcashd wallet."
        ));
    }

    config_file.write_all(cfg.as_bytes())?;
    config_file.flush()?;

    Ok(config_file_path)
}

/// Writes the Zebrad config file to the specified config directory.
/// Returns the path to the config file.
///
/// Canopy (and all earlier network upgrades) must have an activation height of 1 for zebrad regtest mode
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_zebrad_config(
    output_config_dir: PathBuf,
    cache_dir: PathBuf,
    network_listen_port: u16,
    rpc_listen_port: u16,
    indexer_listen_port: u16,
    health_listen_port: u16,
    miner_address: &str,
    network: NetworkType,
    lockbox_disbursements: &[crate::validator::LockboxDisbursement],
    post_nu6_funding_streams: Option<&crate::validator::FundingStreams>,
    min_connected_peers: usize,
) -> std::io::Result<PathBuf> {
    let config_file_path = output_config_dir.join(ZEBRAD_FILENAME);
    let mut config_file = File::create(config_file_path.clone())?;
    let chain_cache = cache_dir.to_str().unwrap();
    let network_string = network_type_to_string(network);

    // Regtest is single-node by definition — the upstream-default seeder
    // lists for mainnet/testnet have nothing useful to contribute and
    // their DNS resolution dominates zebrad's TOC→TOU lag for the RPC
    // port (see `zebrad_regtest_skips_seed_peer_dns` regression test
    // and `mod launch_recovers_from_rpc_port_collision` for why that
    // matters). Force both lists to `[]` on regtest; preserve the
    // upstream defaults on cached mainnet/testnet runs where peer
    // discovery still matters.
    let regtest = matches!(network, NetworkType::Regtest(_));
    let initial_mainnet_peers_block = if regtest {
        "initial_mainnet_peers = []".to_string()
    } else {
        "initial_mainnet_peers = [\n    \
            \"dnsseed.z.cash:8233\",\n    \
            \"dnsseed.str4d.xyz:8233\",\n    \
            \"mainnet.seeder.zfnd.org:8233\",\n    \
            \"mainnet.is.yolo.money:8233\",\n\
         ]"
        .to_string()
    };
    let initial_testnet_peers_block = if regtest {
        "initial_testnet_peers = []".to_string()
    } else {
        "initial_testnet_peers = [\n    \
            \"dnsseed.testnet.z.cash:18233\",\n    \
            \"testnet.seeder.zfnd.org:18233\",\n    \
            \"testnet.is.yolo.money:18233\",\n\
         ]"
        .to_string()
    };

    let mut cfg = format!(
        "\
[consensus]
checkpoint_sync = true

[mempool]
eviction_memory_time = \"1h\"
tx_cost_limit = 80000000

[metrics]

[network]
cache_dir = false
crawl_new_peer_interval = \"1m 1s\"
{initial_mainnet_peers_block}
{initial_testnet_peers_block}
listen_addr = \"127.0.0.1:{network_listen_port}\"
max_connections_per_ip = 1
network = \"{network_string}\"
peerset_initial_target_size = 25

[rpc]
cookie_dir = \"{chain_cache}\"
debug_force_finished_sync = false
enable_cookie_auth = false
parallel_cpu_threads = 0
listen_addr = \"127.0.0.1:{rpc_listen_port}\"
indexer_listen_addr = \"127.0.0.1:{indexer_listen_port}\"

[state]
cache_dir = \"{chain_cache}\"
delete_old_database = true
# ephemeral is set false to enable chain caching
ephemeral = false

[sync]
checkpoint_verify_concurrency_limit = 1000
download_concurrency_limit = 50
full_verify_concurrency_limit = 20
parallel_cpu_threads = 0

[tracing]
buffer_limit = 128000
force_use_color = false
use_color = true
#log_file = \"/home/aloe/.zebradtemplogfile\"
filter = \"debug\"
use_journald = false

[health]
listen_addr = \"127.0.0.1:{health_listen_port}\"
min_connected_peers = {min_connected_peers}
enforce_on_test_networks = false",
    );

    if let NetworkType::Regtest(activation_heights) = network {
        assert!(
            activation_heights.canopy().is_some(),
            "canopy must be active for zebrad regtest mode. please set activation height to 1"
        );

        let nu5_activation_height = activation_heights
            .nu5()
            .expect("nu5 activation height must be specified");
        let nu6_activation_height = activation_heights
            .nu6()
            .expect("nu6 activation height must be specified");
        let nu6_1_activation_height = activation_heights
            .nu6_1()
            .expect("nu6.1 activation height must be specified");

        cfg.push_str(&format!(
            "\n\n\
[mining]
miner_address = \"{miner_address}\"

[network.testnet_parameters.activation_heights]
# Configured activation heights must be greater than or equal to 1,
# block height 0 is reserved for the Genesis network upgrade in Zebra
# pre-nu5 activation heights of greater than 1 are not currently supported for regtest mode
Canopy = 1
NU5 = {nu5_activation_height}
NU6 = {nu6_activation_height}
\"NU6.1\" = {nu6_1_activation_height}"
        ));

        // Lockbox disbursements (ZIP-271). Required at the NU6.1
        // activation block; an empty list trips zebrad's
        // `subsidy_is_valid` rejection. Schema matches Zebra's
        // `ConfiguredLockboxDisbursement` in
        // `zebra-chain/src/parameters/network/testnet.rs`.
        for d in lockbox_disbursements {
            cfg.push_str(&format!(
                "\n\n[[network.testnet_parameters.lockbox_disbursements]]\n\
                 address = \"{address}\"\n\
                 amount = {amount}",
                address = d.address,
                amount = d.amount_zats,
            ));
        }

        // Post-NU6 funding streams (ZIP-1015). Required to deposit
        // into Zebra's `Deferred` value pool ahead of any NU6.1
        // disbursement; without it, `subsidy_is_valid` rejects the
        // activation block on a Deferred value-pool constraint.
        // Schema matches Zebra's `ConfiguredFundingStreams` in
        // `zebra-chain/src/parameters/network/testnet.rs`.
        if let Some(streams) = post_nu6_funding_streams {
            cfg.push_str(&format!(
                "\n\n[network.testnet_parameters.post_nu6_funding_streams.height_range]\n\
                 start = {start}\n\
                 end = {end}",
                start = streams.start_height,
                end = streams.end_height,
            ));
            for r in &streams.recipients {
                cfg.push_str(&format!(
                    "\n\n[[network.testnet_parameters.post_nu6_funding_streams.recipients]]\n\
                     receiver = \"{receiver}\"\n\
                     numerator = {numerator}",
                    receiver = r.receiver.as_toml(),
                    numerator = r.numerator,
                ));
                if let Some(addresses) = &r.addresses {
                    if !addresses.is_empty() {
                        let quoted: Vec<String> =
                            addresses.iter().map(|a| format!("\"{a}\"")).collect();
                        cfg.push_str(&format!("\naddresses = [{}]", quoted.join(", ")));
                    }
                }
            }
        }
    }

    config_file.write_all(cfg.as_bytes())?;
    config_file.flush()?;

    Ok(config_file_path)
}

/// Writes the Zainod config file to the specified config directory.
/// Returns the path to the config file.
// TODO: make fetch/state a parameter
pub(crate) fn write_zainod_config(
    config_dir: &Path,
    validator_cache_dir: PathBuf,
    listen_port: u16,
    validator_port: u16,
    network: NetworkType,
) -> std::io::Result<PathBuf> {
    let config_file_path = config_dir.join(ZAINOD_FILENAME);
    let mut config_file = File::create(config_file_path.clone())?;

    let zaino_cache_dir = validator_cache_dir.join("zaino");
    let chain_cache = zaino_cache_dir.to_str().unwrap();

    let network_string = network_type_to_string(network);

    config_file.write_all(
        format!(
            "\
backend = \"fetch\"
network = \"{network_string}\"

[grpc_settings]
listen_address = \"127.0.0.1:{listen_port}\"

[validator_settings]
validator_jsonrpc_listen_address = \"127.0.0.1:{validator_port}\"
validator_user = \"xxxxxx\"
validator_password = \"xxxxxx\"

[storage]
database.path = \"{chain_cache}\""
        )
        .as_bytes(),
    )?;

    Ok(config_file_path)
}

/// Writes the Lightwalletd config file to the specified config directory.
/// Returns the path to the config file.
#[allow(dead_code)]
pub(crate) fn write_lightwalletd_config(
    config_dir: &Path,
    grpc_bind_addr_port: u16,
    log_file: PathBuf,
    zcashd_conf: PathBuf,
) -> std::io::Result<PathBuf> {
    let zcashd_conf = zcashd_conf.to_str().unwrap();
    let log_file = log_file.to_str().unwrap();

    let config_file_path = config_dir.join(LIGHTWALLETD_FILENAME);
    let mut config_file = File::create(config_file_path.clone())?;

    config_file.write_all(
        format!(
            "\
grpc-bind-addr: 127.0.0.1:{grpc_bind_addr_port}
cache-size: 10
log-file: {log_file}
log-level: 10
zcash-conf-path: {zcashd_conf}"
        )
        .as_bytes(),
    )?;

    Ok(config_file_path)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use zingo_common_components::protocol::{ActivationHeights, NetworkType};

    use crate::logs;

    const EXPECTED_CONFIG: &str = "\
### Blockchain Configuration
regtest=1
nuparams=5ba81b19:2 # Overwinter
nuparams=76b809bb:3 # Sapling
nuparams=2bb40e60:4 # Blossom
nuparams=f5b9230b:5 # Heartwood
nuparams=e9ff75a6:6 # Canopy
nuparams=c2d6d0b4:7 # NU5 (Orchard)
nuparams=c8e71055:8 # NU6
nuparams=4dec4df0:9 # NU6_1 https://zips.z.cash/zip-0255#nu6.1deployment

### MetaData Storage and Retrieval
# txindex:
# https://zcash.readthedocs.io/en/latest/rtd_pages/zcash_conf_guide.html#miscellaneous-options
txindex=1
# insightexplorer:
# https://zcash.readthedocs.io/en/latest/rtd_pages/insight_explorer.html?highlight=insightexplorer#additional-getrawtransaction-fields
insightexplorer=1
experimentalfeatures=1
lightwalletd=1

### RPC Server Interface Options:
# https://zcash.readthedocs.io/en/latest/rtd_pages/zcash_conf_guide.html#json-rpc-options
rpcuser=xxxxxx
rpcpassword=xxxxxx
rpcport=1234
rpcallowip=127.0.0.1

# Buried config option to allow non-canonical RPC-PORT:
# https://zcash.readthedocs.io/en/latest/rtd_pages/zcash_conf_guide.html#zcash-conf-guide
listen=0

i-am-aware-zcashd-will-be-replaced-by-zebrad-and-zallet-in-2025=1";

    fn sequential_activation_heights() -> ActivationHeights {
        ActivationHeights::builder()
            .set_overwinter(Some(2))
            .set_sapling(Some(3))
            .set_blossom(Some(4))
            .set_heartwood(Some(5))
            .set_canopy(Some(6))
            .set_nu5(Some(7))
            .set_nu6(Some(8))
            .set_nu6_1(Some(9))
            .set_nu7(None)
            .build()
    }

    #[test]
    fn zcashd() {
        let config_dir = tempfile::tempdir().unwrap();
        let test_activation_heights = sequential_activation_heights();

        super::write_zcashd_config(config_dir.path(), 1234, test_activation_heights, None).unwrap();

        assert_eq!(
            std::fs::read_to_string(config_dir.path().join(super::ZCASHD_FILENAME)).unwrap(),
            format!("{EXPECTED_CONFIG}"),
        );
    }

    #[test]
    fn zcashd_funded() {
        let config_dir = tempfile::tempdir().unwrap();
        let test_activation_heights = sequential_activation_heights();

        super::write_zcashd_config(
            config_dir.path(),
            1234,
            test_activation_heights,
            Some("test_addr_1234"),
        )
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(config_dir.path().join(super::ZCASHD_FILENAME)).unwrap(),
            format!("{}{}", EXPECTED_CONFIG , "

### Zcashd Help provides documentation of the following:
mineraddress=test_addr_1234
minetolocalwallet=0 # This is set to false so that we can mine to a wallet, other than the zcashd wallet."
            )
        );
    }

    #[test]
    fn zainod() {
        let config_dir = tempfile::tempdir().unwrap();
        let cache_dir = tempfile::tempdir().unwrap();
        let zaino_cache_dir = cache_dir.keep();
        let zaino_test_dir = zaino_cache_dir.join("zaino");
        let zaino_test_path = zaino_test_dir.to_str().unwrap();

        super::write_zainod_config(
            config_dir.path(),
            zaino_cache_dir,
            1234,
            18232,
            NetworkType::Regtest(ActivationHeights::default()),
        )
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(config_dir.path().join(super::ZAINOD_FILENAME)).unwrap(),
            format!(
                "\
backend = \"fetch\"
network = \"Regtest\"

[grpc_settings]
listen_address = \"127.0.0.1:1234\"

[validator_settings]
validator_jsonrpc_listen_address = \"127.0.0.1:18232\"
validator_user = \"xxxxxx\"
validator_password = \"xxxxxx\"

[storage]
database.path = \"{zaino_test_path}\""
            )
        );
    }

    #[test]
    fn lightwalletd() {
        let config_dir = tempfile::tempdir().unwrap();
        let logs_dir = tempfile::tempdir().unwrap();
        let log_file_path = logs_dir.path().join(logs::LIGHTWALLETD_LOG);

        super::write_lightwalletd_config(
            config_dir.path(),
            1234,
            log_file_path.clone(),
            PathBuf::from("conf_path"),
        )
        .unwrap();
        let log_file_path = log_file_path.to_str().unwrap();

        assert_eq!(
            std::fs::read_to_string(config_dir.path().join(super::LIGHTWALLETD_FILENAME)).unwrap(),
            format!(
                "\
grpc-bind-addr: 127.0.0.1:1234
cache-size: 10
log-file: {log_file_path}
log-level: 10
zcash-conf-path: conf_path"
            )
        );
    }
}
