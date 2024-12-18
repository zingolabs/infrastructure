//! Module for configuring processes and writing configuration files

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use portpicker::Port;

use crate::network::{ActivationHeights, Network};

pub(crate) const ZCASHD_FILENAME: &str = "zcash.conf";
pub(crate) const ZEBRAD_FILENAME: &str = "zebrad.toml";
pub(crate) const ZAINOD_FILENAME: &str = "zindexer.toml";
pub(crate) const LIGHTWALLETD_FILENAME: &str = "lightwalletd.yml";

/// Writes the Zcashd config file to the specified config directory.
/// Returns the path to the config file.
pub(crate) fn zcashd(
    config_dir: &Path,
    rpc_port: Port,
    activation_heights: &ActivationHeights,
    miner_address: Option<&str>,
) -> std::io::Result<PathBuf> {
    let config_file_path = config_dir.join(ZCASHD_FILENAME);
    let mut config_file = File::create(config_file_path.clone())?;

    let overwinter_activation_height = activation_heights.overwinter;
    let sapling_activation_height = activation_heights.sapling;
    let blossom_activation_height = activation_heights.blossom;
    let heartwood_activation_height = activation_heights.heartwood;
    let canopy_activation_height = activation_heights.canopy;
    let nu5_activation_height = activation_heights.nu5;
    let nu6_activation_height = activation_heights.nu6;

    config_file.write_all(format!("\
### Blockchain Configuration
regtest=1
nuparams=5ba81b19:{overwinter_activation_height} # Overwinter
nuparams=76b809bb:{sapling_activation_height} # Sapling
nuparams=2bb40e60:{blossom_activation_height} # Blossom
nuparams=f5b9230b:{heartwood_activation_height} # Heartwood
nuparams=e9ff75a6:{canopy_activation_height} # Canopy
nuparams=c2d6d0b4:{nu5_activation_height} # NU5 (Orchard)
nuparams=c8e71055:{nu6_activation_height} # NU6

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
listen=0"
            ).as_bytes())?;

    if let Some(addr) = miner_address {
        config_file.write_all(

                format!("\n\n\
### Zcashd Help provides documentation of the following:
mineraddress={addr}
minetolocalwallet=0 # This is set to false so that we can mine to a wallet, other than the zcashd wallet."
                ).as_bytes()
        )?;
    }

    Ok(config_file_path)
}

/// Writes the Zebrad config file to the specified config directory.
/// Returns the path to the config file.
///
/// Canopy (and all earlier netwrok upgrades) must have an activation height of 1 for zebrad regtest mode
pub(crate) fn zebrad(
    config_dir: PathBuf,
    cache_dir: PathBuf,
    network_listen_port: Port,
    rpc_listen_port: Port,
    activation_heights: &ActivationHeights,
    miner_address: &str,
    network: Network,
) -> std::io::Result<PathBuf> {
    let config_file_path = config_dir.join(ZEBRAD_FILENAME);
    let mut config_file = File::create(config_file_path.clone())?;

    if activation_heights.canopy != 1.into() {
        panic!("canopy must be active for zebrad regtest mode. please set activation height to 1");
    }
    let nu5_activation_height: u32 = activation_heights.nu5.into();
    let nu6_activation_height: u32 = activation_heights.nu6.into();

    let chain_cache = cache_dir.to_str().unwrap();

    let network_string = network.to_string();

    config_file.write_all(
        format!(
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
initial_mainnet_peers = [
    \"dnsseed.z.cash:8233\",
    \"dnsseed.str4d.xyz:8233\",
    \"mainnet.seeder.zfnd.org:8233\",
    \"mainnet.is.yolo.money:8233\",
]
initial_testnet_peers = [
    \"dnsseed.testnet.z.cash:18233\",
    \"testnet.seeder.zfnd.org:18233\",
    \"testnet.is.yolo.money:18233\",
]
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
use_journald = false"
        )
        .as_bytes(),
    )?;

    if matches!(network, Network::Regtest) {
        config_file.write_all(
            format!(
                "\n\n\
[mining]
debug_like_zcashd = true
miner_address = \"{miner_address}\"

[network.testnet_parameters]
disable_pow = true

[network.testnet_parameters.activation_heights]
# Configured activation heights must be greater than or equal to 1,
# block height 0 is reserved for the Genesis network upgrade in Zebra
NU5 = {nu5_activation_height}
NU6 = {nu6_activation_height}"
            )
            .as_bytes(),
        )?;
    } else {
        config_file.write_all(
            format!(
                "\n\n\
[mining]
debug_like_zcashd = true"
            )
            .as_bytes(),
        )?;
    }

    Ok(config_file_path)
}

/// Writes the Zainod config file to the specified config directory.
/// Returns the path to the config file.
pub(crate) fn zainod(
    config_dir: &Path,
    listen_port: Port,
    validator_port: Port,
) -> std::io::Result<PathBuf> {
    let config_file_path = config_dir.join(ZAINOD_FILENAME);
    let mut config_file = File::create(config_file_path.clone())?;

    config_file.write_all(
        format!(
            "\
# Configuration for Zaino

# Sets the TcpIngestor's status (true or false)
tcp_active = true

# Optional TcpIngestors listen port (use None or specify a port number)
listen_port = {listen_port}

# Sets the NymIngestor's and NymDispatchers status (true or false)
nym_active = false

# Optional Nym conf path used for micnet client conf
nym_conf_path = \"/tmp/indexer/nym\"

# LightWalletD listen port [DEPRECATED]
lightwalletd_port = 9067

# Full node / validator listen port
zebrad_port = {validator_port}

# Optional full node Username
node_user = \"xxxxxx\"

# Optional full node Password
node_password = \"xxxxxx\"

# Maximum requests allowed in the request queue
max_queue_size = 1024

# Maximum workers allowed in the worker pool
max_worker_pool_size = 64

# Minimum number of workers held in the worker pool when idle
idle_worker_pool_size = 4"
        )
        .as_bytes(),
    )?;

    Ok(config_file_path)
}

/// Writes the Lightwalletd config file to the specified config directory.
/// Returns the path to the config file.
#[allow(dead_code)]
pub(crate) fn lightwalletd(
    config_dir: &Path,
    grpc_bind_addr_port: Port,
    log_file: PathBuf,
    validator_conf: PathBuf,
) -> std::io::Result<PathBuf> {
    let validator_conf = validator_conf.to_str().unwrap();
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
zcash-conf-path: {validator_conf}"
        )
        .as_bytes(),
    )?;

    Ok(config_file_path)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{logs, network};

    #[test]
    fn zcashd() {
        let config_dir = tempfile::tempdir().unwrap();
        let activation_heights = network::ActivationHeights {
            overwinter: 1.into(),
            sapling: 2.into(),
            blossom: 3.into(),
            heartwood: 4.into(),
            canopy: 5.into(),
            nu5: 6.into(),
            nu6: 7.into(),
        };

        super::zcashd(config_dir.path(), 1234, &activation_heights, None).unwrap();

        assert_eq!(std::fs::read_to_string(config_dir.path().join(super::ZCASHD_FILENAME)).unwrap(),
                        format!("\
### Blockchain Configuration
regtest=1
nuparams=5ba81b19:1 # Overwinter
nuparams=76b809bb:2 # Sapling
nuparams=2bb40e60:3 # Blossom
nuparams=f5b9230b:4 # Heartwood
nuparams=e9ff75a6:5 # Canopy
nuparams=c2d6d0b4:6 # NU5 (Orchard)

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
listen=0"
                        )

        );
    }

    #[test]
    fn zcashd_funded() {
        let config_dir = tempfile::tempdir().unwrap();
        let activation_heights = network::ActivationHeights {
            overwinter: 1.into(),
            sapling: 2.into(),
            blossom: 3.into(),
            heartwood: 4.into(),
            canopy: 5.into(),
            nu5: 6.into(),
            nu6: 7.into(),
        };

        super::zcashd(
            config_dir.path(),
            1234,
            &activation_heights,
            Some("test_addr_1234"),
        )
        .unwrap();

        assert_eq!(std::fs::read_to_string(config_dir.path().join(super::ZCASHD_FILENAME)).unwrap(),
                        format!("\
### Blockchain Configuration
regtest=1
nuparams=5ba81b19:1 # Overwinter
nuparams=76b809bb:2 # Sapling
nuparams=2bb40e60:3 # Blossom
nuparams=f5b9230b:4 # Heartwood
nuparams=e9ff75a6:5 # Canopy
nuparams=c2d6d0b4:6 # NU5 (Orchard)

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

### Zcashd Help provides documentation of the following:
mineraddress=test_addr_1234
minetolocalwallet=0 # This is set to false so that we can mine to a wallet, other than the zcashd wallet."
                        )

        );
    }

    #[test]
    fn zainod() {
        let config_dir = tempfile::tempdir().unwrap();

        super::zainod(config_dir.path(), 1234, 18232).unwrap();

        assert_eq!(
            std::fs::read_to_string(config_dir.path().join(super::ZAINOD_FILENAME)).unwrap(),
            format!(
                "\
# Configuration for Zaino

# Sets the TcpIngestor's status (true or false)
tcp_active = true

# Optional TcpIngestors listen port (use None or specify a port number)
listen_port = 1234

# Sets the NymIngestor's and NymDispatchers status (true or false)
nym_active = false

# Optional Nym conf path used for micnet client conf
nym_conf_path = \"/tmp/indexer/nym\"

# LightWalletD listen port [DEPRECATED]
lightwalletd_port = 9067

# Full node / validator listen port
zebrad_port = 18232

# Optional full node Username
node_user = \"xxxxxx\"

# Optional full node Password
node_password = \"xxxxxx\"

# Maximum requests allowed in the request queue
max_queue_size = 1024

# Maximum workers allowed in the worker pool
max_worker_pool_size = 64

# Minimum number of workers held in the worker pool when idle
idle_worker_pool_size = 4"
            )
        )
    }

    #[test]
    fn lightwalletd() {
        let config_dir = tempfile::tempdir().unwrap();
        let logs_dir = tempfile::tempdir().unwrap();
        let log_file_path = logs_dir.path().join(logs::LIGHTWALLETD_LOG);

        super::lightwalletd(
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
        )
    }
}
