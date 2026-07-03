//! Zcash network-upgrade activation schedules and network identity types shared by Zingo
//! projects.
//!
//! Types in this crate are intended to be suitable for use in the public API of other crates
//! so must only include types that have a stable public API that will only increase major
//! semver version in rare cases.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Network types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkType {
    /// Mainnet
    Mainnet,
    /// Testnet
    Testnet,
    /// Regtest
    Regtest(ActivationHeights),
}

impl std::fmt::Display for NetworkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let chain = match self {
            NetworkType::Mainnet => "mainnet",
            NetworkType::Testnet => "testnet",
            NetworkType::Regtest(_) => "regtest",
        };
        write!(f, "{chain}")
    }
}

/// Network upgrade activation heights for custom testnet and regtest network configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActivationHeights {
    overwinter: Option<u32>,
    sapling: Option<u32>,
    blossom: Option<u32>,
    heartwood: Option<u32>,
    canopy: Option<u32>,
    nu5: Option<u32>,
    nu6: Option<u32>,
    nu6_1: Option<u32>,
    nu6_2: Option<u32>,
    nu6_3: Option<u32>,
    nu7: Option<u32>,
}

/// The standard regtest activation schedule: every deployed network upgrade active from
/// block 1, with undeployed upgrades unset.
///
/// This is the schedule formerly published as `for_test::all_height_one_nus`.
impl Default for ActivationHeights {
    fn default() -> Self {
        Self::builder()
            .set_overwinter(Some(1))
            .set_sapling(Some(1))
            .set_blossom(Some(1))
            .set_heartwood(Some(1))
            .set_canopy(Some(1))
            .set_nu5(Some(1))
            .set_nu6(Some(1))
            .set_nu6_1(Some(1))
            .set_nu6_2(Some(1))
            .set_nu6_3(Some(1))
            .set_nu7(None)
            .build()
    }
}

impl ActivationHeights {
    /// Constructs new builder.
    pub fn builder() -> ActivationHeightsBuilder {
        ActivationHeightsBuilder::new()
    }

    /// Returns overwinter network upgrade activation height.
    pub fn overwinter(&self) -> Option<u32> {
        self.overwinter
    }

    /// Returns sapling network upgrade activation height.
    pub fn sapling(&self) -> Option<u32> {
        self.sapling
    }

    /// Returns blossom network upgrade activation height.
    pub fn blossom(&self) -> Option<u32> {
        self.blossom
    }

    /// Returns heartwood network upgrade activation height.
    pub fn heartwood(&self) -> Option<u32> {
        self.heartwood
    }

    /// Returns canopy network upgrade activation height.
    pub fn canopy(&self) -> Option<u32> {
        self.canopy
    }

    /// Returns nu5 network upgrade activation height.
    pub fn nu5(&self) -> Option<u32> {
        self.nu5
    }

    /// Returns nu6 network upgrade activation height.
    pub fn nu6(&self) -> Option<u32> {
        self.nu6
    }

    /// Returns nu6.1 network upgrade activation height.
    pub fn nu6_1(&self) -> Option<u32> {
        self.nu6_1
    }

    /// Returns nu6.2 network upgrade activation height.
    pub fn nu6_2(&self) -> Option<u32> {
        self.nu6_2
    }

    /// Returns nu6.3 network upgrade activation height.
    pub fn nu6_3(&self) -> Option<u32> {
        self.nu6_3
    }

    /// Returns nu7 network upgrade activation height.
    pub fn nu7(&self) -> Option<u32> {
        self.nu7
    }
}

/// A builder, so that new network upgrades do not cause breaking changes to the public API.
pub struct ActivationHeightsBuilder {
    overwinter: Option<u32>,
    sapling: Option<u32>,
    blossom: Option<u32>,
    heartwood: Option<u32>,
    canopy: Option<u32>,
    nu5: Option<u32>,
    nu6: Option<u32>,
    nu6_1: Option<u32>,
    nu6_2: Option<u32>,
    nu6_3: Option<u32>,
    nu7: Option<u32>,
}

impl Default for ActivationHeightsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ActivationHeightsBuilder {
    /// Constructs a builder with all fields set to `None`.
    pub fn new() -> Self {
        Self {
            overwinter: None,
            sapling: None,
            blossom: None,
            heartwood: None,
            canopy: None,
            nu5: None,
            nu6: None,
            nu6_1: None,
            nu6_2: None,
            nu6_3: None,
            nu7: None,
        }
    }

    /// Set `overwinter` field.
    pub fn set_overwinter(mut self, height: Option<u32>) -> Self {
        self.overwinter = height;

        self
    }

    /// Set `sapling` field.
    pub fn set_sapling(mut self, height: Option<u32>) -> Self {
        self.sapling = height;

        self
    }

    /// Set `blossom` field.
    pub fn set_blossom(mut self, height: Option<u32>) -> Self {
        self.blossom = height;

        self
    }

    /// Set `heartwood` field.
    pub fn set_heartwood(mut self, height: Option<u32>) -> Self {
        self.heartwood = height;

        self
    }

    /// Set `canopy` field.
    pub fn set_canopy(mut self, height: Option<u32>) -> Self {
        self.canopy = height;

        self
    }

    /// Set `nu5` field.
    pub fn set_nu5(mut self, height: Option<u32>) -> Self {
        self.nu5 = height;

        self
    }

    /// Set `nu6` field.
    pub fn set_nu6(mut self, height: Option<u32>) -> Self {
        self.nu6 = height;

        self
    }

    /// Set `nu6_1` field.
    pub fn set_nu6_1(mut self, height: Option<u32>) -> Self {
        self.nu6_1 = height;

        self
    }

    /// Set `nu6_2` field.
    pub fn set_nu6_2(mut self, height: Option<u32>) -> Self {
        self.nu6_2 = height;

        self
    }

    /// Set `nu6_3` field.
    pub fn set_nu6_3(mut self, height: Option<u32>) -> Self {
        self.nu6_3 = height;

        self
    }

    /// Set `nu7` field.
    pub fn set_nu7(mut self, height: Option<u32>) -> Self {
        self.nu7 = height;

        self
    }

    /// Builds `ActivationHeights` with assertions to ensure all earlier network upgrades are active with an activation
    /// height equal to or lower than the later network upgrades.
    pub fn build(self) -> ActivationHeights {
        if let Some(b) = self.sapling {
            assert!(self.overwinter.is_some_and(|a| a <= b));
        }
        if let Some(b) = self.blossom {
            assert!(self.sapling.is_some_and(|a| a <= b));
        }
        if let Some(b) = self.heartwood {
            assert!(self.blossom.is_some_and(|a| a <= b));
        }
        if let Some(b) = self.canopy {
            assert!(self.heartwood.is_some_and(|a| a <= b));
        }
        if let Some(b) = self.nu5 {
            assert!(self.canopy.is_some_and(|a| a <= b));
        }
        if let Some(b) = self.nu6 {
            assert!(self.nu5.is_some_and(|a| a <= b));
        }
        if let Some(b) = self.nu6_1 {
            assert!(self.nu6.is_some_and(|a| a <= b));
        }
        if let Some(b) = self.nu6_2 {
            assert!(self.nu6_1.is_some_and(|a| a <= b));
        }
        if let Some(b) = self.nu6_3 {
            assert!(self.nu6_2.is_some_and(|a| a <= b));
        }
        if let Some(b) = self.nu7 {
            assert!(
                self.nu6_3
                    .or(self.nu6_2)
                    .or(self.nu6_1)
                    .is_some_and(|a| a <= b)
            );
        }

        ActivationHeights {
            overwinter: self.overwinter,
            sapling: self.sapling,
            blossom: self.blossom,
            heartwood: self.heartwood,
            canopy: self.canopy,
            nu5: self.nu5,
            nu6: self.nu6,
            nu6_1: self.nu6_1,
            nu6_2: self.nu6_2,
            nu6_3: self.nu6_3,
            nu7: self.nu7,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ActivationHeights;

    #[test]
    fn activation_heights_preserve_nu6_2() {
        let heights = ActivationHeights::builder()
            .set_overwinter(Some(1))
            .set_sapling(Some(2))
            .set_blossom(Some(3))
            .set_heartwood(Some(4))
            .set_canopy(Some(5))
            .set_nu5(Some(6))
            .set_nu6(Some(7))
            .set_nu6_1(Some(8))
            .set_nu6_2(Some(9))
            .set_nu7(None)
            .build();

        assert_eq!(heights.nu6_2(), Some(9));
    }

    #[test]
    #[should_panic]
    fn activation_heights_reject_nu6_2_before_nu6_1() {
        let _ = ActivationHeights::builder()
            .set_nu6(Some(7))
            .set_nu6_1(Some(8))
            .set_nu6_2(Some(7))
            .build();
    }

    #[test]
    fn activation_heights_preserve_nu6_3() {
        let heights = ActivationHeights::builder()
            .set_overwinter(Some(1))
            .set_sapling(Some(2))
            .set_blossom(Some(3))
            .set_heartwood(Some(4))
            .set_canopy(Some(5))
            .set_nu5(Some(6))
            .set_nu6(Some(7))
            .set_nu6_1(Some(8))
            .set_nu6_2(Some(9))
            .set_nu6_3(Some(10))
            .set_nu7(None)
            .build();

        assert_eq!(heights.nu6_3(), Some(10));
    }

    #[test]
    #[should_panic]
    fn activation_heights_reject_nu6_3_before_nu6_2() {
        let _ = ActivationHeights::builder()
            .set_nu6(Some(7))
            .set_nu6_1(Some(8))
            .set_nu6_2(Some(9))
            .set_nu6_3(Some(8))
            .build();
    }

    #[test]
    fn default_is_the_all_height_one_schedule() {
        let heights = ActivationHeights::default();

        assert_eq!(heights.overwinter(), Some(1));
        assert_eq!(heights.nu6_3(), Some(1));
        assert_eq!(heights.nu7(), None);
    }
}
