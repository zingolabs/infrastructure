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

/// Network identity without activation heights.
///
/// The configuration shape for components that must know *which* network
/// they serve but must not be told activation heights: the Validator is
/// the single source of truth for heights (infras ADR 0003), so a config
/// that accepted heights on such a component would be a false affordance.
/// Use [`NetworkType`] where heights are genuinely configured (validators)
/// or asserted by the caller (wallet clients on unmanaged stacks).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkKind {
    /// Mainnet
    Mainnet,
    /// Testnet
    Testnet,
    /// Regtest
    Regtest,
}

impl From<&NetworkType> for NetworkKind {
    fn from(network: &NetworkType) -> Self {
        match network {
            NetworkType::Mainnet => NetworkKind::Mainnet,
            NetworkType::Testnet => NetworkKind::Testnet,
            NetworkType::Regtest(_) => NetworkKind::Regtest,
        }
    }
}

impl From<NetworkType> for NetworkKind {
    fn from(network: NetworkType) -> Self {
        (&network).into()
    }
}

/// The pool a validator mines block rewards to.
///
/// Validator support differs: zcashd can mine to any variant, while zebrad
/// supports only `Transparent` and `Orchard`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MinerPool {
    /// Mine to a transparent (P2PKH) address.
    Transparent,
    /// Mine to a Sapling shielded address. Not supported by zebrad.
    Sapling,
    /// Mine to an Orchard shielded address.
    Orchard,
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
}

/// Generates the per-upgrade activation-height getter on
/// [`ActivationHeights`] for each listed field. A `macro_rules!`
/// because method definitions cannot be deduplicated with a helper fn.
macro_rules! height_getters {
    ($($(#[$doc:meta])* $field:ident),+ $(,)?) => {
        impl ActivationHeights {
            $(
                $(#[$doc])*
                pub fn $field(&self) -> Option<u32> {
                    self.$field
                }
            )+
        }
    };
}

height_getters!(
    /// Returns overwinter network upgrade activation height.
    overwinter,
    /// Returns sapling network upgrade activation height.
    sapling,
    /// Returns blossom network upgrade activation height.
    blossom,
    /// Returns heartwood network upgrade activation height.
    heartwood,
    /// Returns canopy network upgrade activation height.
    canopy,
    /// Returns nu5 network upgrade activation height.
    nu5,
    /// Returns nu6 network upgrade activation height.
    nu6,
    /// Returns nu6.1 network upgrade activation height.
    nu6_1,
    /// Returns nu6.2 network upgrade activation height.
    nu6_2,
    /// Returns nu6.3 network upgrade activation height.
    nu6_3,
    /// Returns nu7 network upgrade activation height.
    nu7,
);

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

/// Generates the chaining per-upgrade setter on
/// [`ActivationHeightsBuilder`] for each listed `set_x => x` pair. A
/// `macro_rules!` because method definitions cannot be deduplicated
/// with a helper fn.
macro_rules! height_setters {
    ($($(#[$doc:meta])* $setter:ident => $field:ident),+ $(,)?) => {
        impl ActivationHeightsBuilder {
            $(
                $(#[$doc])*
                pub fn $setter(mut self, height: Option<u32>) -> Self {
                    self.$field = height;
                    self
                }
            )+
        }
    };
}

height_setters!(
    /// Set `overwinter` field.
    set_overwinter => overwinter,
    /// Set `sapling` field.
    set_sapling => sapling,
    /// Set `blossom` field.
    set_blossom => blossom,
    /// Set `heartwood` field.
    set_heartwood => heartwood,
    /// Set `canopy` field.
    set_canopy => canopy,
    /// Set `nu5` field.
    set_nu5 => nu5,
    /// Set `nu6` field.
    set_nu6 => nu6,
    /// Set `nu6_1` field.
    set_nu6_1 => nu6_1,
    /// Set `nu6_2` field.
    set_nu6_2 => nu6_2,
    /// Set `nu6_3` field.
    set_nu6_3 => nu6_3,
    /// Set `nu7` field.
    set_nu7 => nu7,
);

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
