use zcash_protocol::{consensus::BlockHeight, local_consensus::LocalNetwork};
use zebra_chain::parameters::testnet::ConfiguredActivationHeights;
pub fn default_regtest_heights() -> zebra_chain::parameters::Network {
    zebra_chain::parameters::Network::new_regtest(
        zebra_chain::parameters::testnet::ConfiguredActivationHeights {
            before_overwinter: Some(1),
            overwinter: Some(1),
            sapling: Some(1),
            blossom: Some(1),
            heartwood: Some(1),
            canopy: Some(1),
            nu5: Some(1),
            nu6: Some(1),
            // see https://zips.z.cash/#nu6-1-candidate-zips for info on NU6.1
            nu6_1: None,
            nu7: None,
        },
    )
}

#[cfg(feature = "for_test")]
pub mod for_test;
