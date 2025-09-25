/// Get the default all nu activated at 1, Network
pub fn active_nus_regtest_network() -> zebra_chain::parameters::Network {
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
/// Get sequentially activated (1,2,3,4,5,6,7,8 nus network
pub fn active_sequential_nus_regtest_network() -> zebra_chain::parameters::Network {
    zebra_chain::parameters::Network::new_regtest(
        zebra_chain::parameters::testnet::ConfiguredActivationHeights {
            before_overwinter: Some(1),
            overwinter: Some(2),
            sapling: Some(3),
            blossom: Some(4),
            heartwood: Some(5),
            canopy: Some(6),
            nu5: Some(7),
            nu6: Some(8),
            // see https://zips.z.cash/#nu6-1-candidate-zips for info on NU6.1
            nu6_1: None,
            nu7: None,
        },
    )
}
