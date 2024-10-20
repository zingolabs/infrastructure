use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::network::ActivationHeights;

const ZCASHD_FILENAME: &str = "zcash.conf";
const LIGHTWALLETD_FILENAME: &str = "lightwalletd.yml";

/// TODO: Add Doc Comment Here!
pub fn zcashd(
    config_dir: &Path,
    rpcport: u16,
    activation_heights: &ActivationHeights,
    miner_address: Option<&str>,
) -> std::io::Result<()> {
    let config_file_path = config_dir.join(ZCASHD_FILENAME);
    let mut config_file = File::create(config_file_path)?;

    let overwinter_activation_height = activation_heights.overwinter;
    let sapling_activation_height = activation_heights.sapling;
    let blossom_activation_height = activation_heights.blossom;
    let heartwood_activation_height = activation_heights.heartwood;
    let canopy_activation_height = activation_heights.canopy;
    let nu5_activation_height = activation_heights.nu5;

    config_file.write_all(format!("\
### Blockchain Configuration
regtest=1
nuparams=5ba81b19:{overwinter_activation_height} # Overwinter
nuparams=76b809bb:{sapling_activation_height} # Sapling
nuparams=2bb40e60:{blossom_activation_height} # Blossom
nuparams=f5b9230b:{heartwood_activation_height} # Heartwood
nuparams=e9ff75a6:{canopy_activation_height} # Canopy
nuparams=c2d6d0b4:{nu5_activation_height} # NU5 (Orchard)

### MetaData Storage and Retrieval
# txindex:
# https://zcash.readthedocs.io/en/latest/rtd_pages/zcash_conf_guide.html#miscellaneous-options
txindex=1
# insightexplorer:
# https://zcash.readthedocs.io/en/latest/rtd_pages/insight_explorer.html?highlight=insightexplorer#additional-getrawtransaction-fields
insightexplorer=1
experimentalfeatures=1

### RPC Server Interface Options:
# https://zcash.readthedocs.io/en/latest/rtd_pages/zcash_conf_guide.html#json-rpc-options
rpcuser=xxxxxx
rpcpassword=xxxxxx
rpcport={rpcport}
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

    Ok(())
}

fn lightwalletd(
    config_dir: &Path,
    grpc_bind_addr_port: u16
) -> std::io::Result<()> {
    let config_file_path = config_dir.join(LIGHTWALLETD_FILENAME);
    let mut config_file = File::create(config_file_path)?;

    config_file.write_all(format!("\
grpc-bind-addr: 127.0.0.1:{grpc_bind_addr_port}
cache-size: 10
log-file: ../logs/lwd.log
log-level: 10
zcash-conf-path: ./zcash.conf"
    ).as_bytes())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::network::ActivationHeights;

    #[test]
    fn zcashd() {
        let config_dir = tempfile::tempdir().unwrap();
        let activation_heights = ActivationHeights {
            overwinter : 1.into(),
            sapling: 2.into(),
            blossom: 3.into(),
            heartwood: 4.into(),
            canopy: 5.into(),
            nu5: 6.into(),
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
        let activation_heights = ActivationHeights {
            overwinter : 1.into(),
            sapling: 2.into(),
            blossom: 3.into(),
            heartwood: 4.into(),
            canopy: 5.into(),
            nu5: 6.into(),
        };

        super::zcashd(config_dir.path(), 1234, &activation_heights, Some("test_addr_1234")).unwrap();

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
    fn lightwalletd() {
        let config_dir = tempfile::tempdir().unwrap();

        super::lightwalletd(config_dir.path(), 1234).unwrap();

        assert_eq!(std::fs::read_to_string(config_dir.path().join(super::LIGHTWALLETD_FILENAME)).unwrap(),
            format!(
                "\
grpc-bind-addr: 127.0.0.1:1234
cache-size: 10
log-file: ../logs/lwd.log
log-level: 10
zcash-conf-path: ./zcash.conf"
            )
        )
    }
}
