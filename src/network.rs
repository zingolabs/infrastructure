use zcash_primitives::consensus::BlockHeight;

/// Activation heights for local network upgrades
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActivationHeights {
    pub overwinter: BlockHeight,
    pub sapling: BlockHeight,
    pub blossom: BlockHeight,
    pub heartwood: BlockHeight,
    pub canopy: BlockHeight,
    pub nu5: BlockHeight, // a.k.a. orchard
}

impl Default for ActivationHeights {
    fn default() -> Self {
        Self {
            overwinter: 1.into(),
            sapling: 1.into(),
            blossom: 1.into(),
            heartwood: 1.into(),
            canopy: 1.into(),
            nu5: 1.into(),
        }
    }
}
