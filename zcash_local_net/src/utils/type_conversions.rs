use zebra_rpc::client::zebra_chain::parameters::testnet::ConfiguredActivationHeights;
use zingo_consensus::ActivationHeights;

pub(crate) fn zingo_to_zebra_activation_heights(
    value: ActivationHeights,
) -> ConfiguredActivationHeights {
    ConfiguredActivationHeights {
        before_overwinter: Some(1),
        overwinter: value.overwinter(),
        sapling: value.sapling(),
        blossom: value.blossom(),
        heartwood: value.heartwood(),
        canopy: value.canopy(),
        nu5: value.nu5(),
        nu6: value.nu6(),
        nu6_1: value.nu6_1(),
        nu6_2: value.nu6_2(),
        nu6_3: value.nu6_3(),
        nu7: value.nu7(),
    }
}
