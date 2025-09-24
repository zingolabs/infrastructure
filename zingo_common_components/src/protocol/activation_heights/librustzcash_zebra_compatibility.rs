use zcash_protocol::{consensus::BlockHeight, local_consensus::LocalNetwork};
use zebra_chain::parameters::testnet::ConfiguredActivationHeights;
pub fn from_localnetwork_to_configuredactivationheights(
    value: LocalNetwork,
) -> ConfiguredActivationHeights {
    let LocalNetwork {
        overwinter,
        sapling,
        blossom,
        heartwood,
        canopy,
        nu5,
        nu6,
        nu6_1,
    } = value;
    ConfiguredActivationHeights {
        before_overwinter: None,
        overwinter: overwinter.map(|height| height.into()),
        sapling: sapling.map(|height| height.into()),
        blossom: blossom.map(|height| height.into()),
        heartwood: heartwood.map(|height| height.into()),
        canopy: canopy.map(|height| height.into()),
        nu5: nu5.map(|height| height.into()),
        nu6: nu6.map(|height| height.into()),
        nu6_1: nu6_1.map(|height| height.into()),
        nu7: None,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UnsupportedProtocolActivationSpecification {
    #[error("BeforeOverwinter at {0}")]
    BeforeOverwinter(BlockHeight),
    #[error("NU7 at {0}")]
    NU7(BlockHeight),
}

#[derive(Debug, thiserror::Error)]
pub enum TryFromConfiguredActivationHeightsToLocalNetworkError {
    #[error("Tried to specify {0}, but it is not supported here.")]
    Unsupported(UnsupportedProtocolActivationSpecification),
}

pub fn try_from_configuredactivationheights_to_localnetwork(
    value: ConfiguredActivationHeights,
) -> Result<LocalNetwork, TryFromConfiguredActivationHeightsToLocalNetworkError> {
    let ConfiguredActivationHeights {
        before_overwinter,
        overwinter,
        sapling,
        blossom,
        heartwood,
        canopy,
        nu5,
        nu6,
        nu6_1,
        nu7,
    } = value;

    if let Some(before_overwinter_height) = before_overwinter {
        return Err(
            TryFromConfiguredActivationHeightsToLocalNetworkError::Unsupported(
                UnsupportedProtocolActivationSpecification::BeforeOverwinter(
                    BlockHeight::from_u32(before_overwinter_height),
                ),
            ),
        );
    }
    if let Some(nu7_height) = nu7 {
        return Err(
            TryFromConfiguredActivationHeightsToLocalNetworkError::Unsupported(
                UnsupportedProtocolActivationSpecification::NU7(BlockHeight::from_u32(nu7_height)),
            ),
        );
    }

    Ok(LocalNetwork {
        overwinter: overwinter.map(BlockHeight::from_u32),
        sapling: sapling.map(BlockHeight::from_u32),
        blossom: blossom.map(BlockHeight::from_u32),
        heartwood: heartwood.map(BlockHeight::from_u32),
        canopy: canopy.map(BlockHeight::from_u32),
        nu5: nu5.map(BlockHeight::from_u32),
        nu6: nu6.map(BlockHeight::from_u32),
        nu6_1: nu6_1.map(BlockHeight::from_u32),
    })
}
