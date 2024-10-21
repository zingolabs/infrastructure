use zcash_primitives::consensus::BlockHeight;

/// Activation heights for local network upgrades
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActivationHeights {
    /// Overwinter network upgrade activation height
    pub overwinter: BlockHeight,
    /// Sapling network upgrade activation height
    pub sapling: BlockHeight,
    /// Blossom network upgrade activation height
    pub blossom: BlockHeight,
    /// Heartwood network upgrade activation height
    pub heartwood: BlockHeight,
    /// Canopy network upgrade activation height
    pub canopy: BlockHeight,
    /// Nu5 (a.k.a. Orchard) network upgrade activation height
    pub nu5: BlockHeight,
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
