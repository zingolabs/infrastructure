use clap::Parser;
use local_net::validator::REGTEST_FIXTURE_HEIGHTS_CLI_STRING;
use zebra_rpc::client::zebra_chain::parameters::testnet::ConfiguredActivationHeights;

#[derive(Parser, Debug)]
pub struct Cli {
    /// Comma-separated activation heights, e.g.
    /// "all=1,nu5=1000,nu6=off,nu6_1=off,nu7=off"
    ///
    /// Keys: before_overwinter, overwinter, sapling, blossom, heartwood, canopy, nu5, nu6, nu6_1, nu7, all
    /// Values: u32 or off|none|disable
    ///
    /// Default comes from
    /// [`local_net::validator::REGTEST_FIXTURE_HEIGHTS_CLI_STRING`] —
    /// the single source of truth for regtest fixture activation heights
    /// across this repo. See `regtest_test_activation_heights` for why
    /// these specific values matter (NU6.1 lockbox / zainod commitment
    /// computation).
    #[arg(
        long,
        value_parser = parse_activation_heights,
        default_value = REGTEST_FIXTURE_HEIGHTS_CLI_STRING
    )]
    pub activation_heights: ConfiguredActivationHeights,

    /// Optional miner address for receiving block rewards.
    #[arg(long)]
    pub miner_address: Option<String>,
}

// TODO: update regtest-launcher to nu6.2

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum UpgradeKey {
    BeforeOverwinter,
    Overwinter,
    Sapling,
    Blossom,
    Heartwood,
    Canopy,
    Nu5,
    Nu6,
    Nu6_1,
    Nu7,
}

const UPGRADE_ORDER: [UpgradeKey; 10] = [
    UpgradeKey::BeforeOverwinter,
    UpgradeKey::Overwinter,
    UpgradeKey::Sapling,
    UpgradeKey::Blossom,
    UpgradeKey::Heartwood,
    UpgradeKey::Canopy,
    UpgradeKey::Nu5,
    UpgradeKey::Nu6,
    UpgradeKey::Nu6_1,
    UpgradeKey::Nu7,
];

fn parse_key(k: &str) -> Option<UpgradeKey> {
    match k {
        "before_overwinter" | "pre_overwinter" | "beforeoverwinter" => {
            Some(UpgradeKey::BeforeOverwinter)
        }
        "overwinter" => Some(UpgradeKey::Overwinter),
        "sapling" => Some(UpgradeKey::Sapling),
        "blossom" => Some(UpgradeKey::Blossom),
        "heartwood" => Some(UpgradeKey::Heartwood),
        "canopy" => Some(UpgradeKey::Canopy),
        "nu5" => Some(UpgradeKey::Nu5),
        "nu6" => Some(UpgradeKey::Nu6),
        "nu6_1" | "nu6.1" | "nu61" => Some(UpgradeKey::Nu6_1),
        "nu7" => Some(UpgradeKey::Nu7),
        _ => None,
    }
}

fn set_field(cfg: &mut ConfiguredActivationHeights, key: UpgradeKey, val: Option<u32>) {
    match key {
        UpgradeKey::BeforeOverwinter => cfg.before_overwinter = val,
        UpgradeKey::Overwinter => cfg.overwinter = val,
        UpgradeKey::Sapling => cfg.sapling = val,
        UpgradeKey::Blossom => cfg.blossom = val,
        UpgradeKey::Heartwood => cfg.heartwood = val,
        UpgradeKey::Canopy => cfg.canopy = val,
        UpgradeKey::Nu5 => cfg.nu5 = val,
        UpgradeKey::Nu6 => cfg.nu6 = val,
        UpgradeKey::Nu6_1 => cfg.nu6_1 = val,
        UpgradeKey::Nu7 => cfg.nu7 = val,
    }
}

fn set_all(cfg: &mut ConfiguredActivationHeights, val: Option<u32>) {
    for k in UPGRADE_ORDER {
        set_field(cfg, k, val);
    }
}

fn cascade_from(cfg: &mut ConfiguredActivationHeights, from: UpgradeKey, val: Option<u32>) {
    let mut apply = false;
    for k in UPGRADE_ORDER {
        if k == from {
            apply = true;
        }
        if apply {
            set_field(cfg, k, val);
        }
    }
}

fn parse_activation_heights(s: &str) -> Result<ConfiguredActivationHeights, String> {
    let mut cfg = ConfiguredActivationHeights {
        before_overwinter: None,
        overwinter: None,
        sapling: None,
        blossom: None,
        heartwood: None,
        canopy: None,
        nu5: None,
        nu6: None,
        nu6_1: None,
        nu6_2: None,
        nu7: None,
    };

    // Matches clap's default behaviour. Is there a better way to do this?
    set_all(&mut cfg, Some(1));
    cfg.nu7 = None;

    for part in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (k, v) = part
            .split_once('=')
            .ok_or_else(|| format!("Bad token '{part}': expected key=value"))?;

        let key = k.trim().to_ascii_lowercase();
        let val = parse_val(v)?;

        if key == "all" {
            set_all(&mut cfg, val);
            continue;
        }

        let from = parse_key(&key).ok_or_else(|| {
            format!(
                "Unknown activation key '{k}'. Valid keys: \
before_overwinter, overwinter, sapling, blossom, heartwood, canopy, nu5, nu6, nu6_1, nu7, all"
            )
        })?;

        cascade_from(&mut cfg, from, val);
    }

    Ok(cfg)
}

fn parse_val(v: &str) -> Result<Option<u32>, String> {
    let v = v.trim();
    let lv = v.to_ascii_lowercase();
    if lv == "off" || lv == "none" || lv == "disable" {
        Ok(None)
    } else {
        let n: u32 = v
            .parse()
            .map_err(|_| format!("Invalid height '{v}': expected u32 or off|none|disable"))?;
        Ok(Some(n))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_val_accepts_off_synonyms() {
        for s in ["off", "OFF", " none ", "Disable", "dIsAbLe"] {
            assert_eq!(parse_val(s).unwrap(), None, "input={s}");
        }
    }

    #[test]
    fn parse_val_accepts_u32() {
        assert_eq!(parse_val("0").unwrap(), Some(0));
        assert_eq!(parse_val(" 1 ").unwrap(), Some(1));
        assert_eq!(parse_val("4294967295").unwrap(), Some(u32::MAX));
    }

    #[test]
    fn parse_val_rejects_non_u32() {
        let err = parse_val("nope").unwrap_err();
        assert!(err.contains("Invalid height"), "{err}");

        let err = parse_val("-1").unwrap_err();
        assert!(err.contains("Invalid height"), "{err}");

        let err = parse_val("4294967296").unwrap_err();
        assert!(err.contains("Invalid height"), "{err}");
    }

    #[test]
    fn parse_activation_heights_empty_uses_seeded_defaults() {
        let cfg = parse_activation_heights("").unwrap();

        assert_eq!(cfg.before_overwinter, Some(1));
        assert_eq!(cfg.overwinter, Some(1));
        assert_eq!(cfg.sapling, Some(1));
        assert_eq!(cfg.blossom, Some(1));
        assert_eq!(cfg.heartwood, Some(1));
        assert_eq!(cfg.canopy, Some(1));
        assert_eq!(cfg.nu5, Some(1));
        assert_eq!(cfg.nu6, Some(1));
        assert_eq!(cfg.nu6_1, Some(1));
        assert_eq!(cfg.nu7, None);
    }

    #[test]
    fn parse_activation_heights_ignores_extra_commas_and_whitespace() {
        let cfg = parse_activation_heights(" , , nu5=1 , , ").unwrap();

        assert_eq!(cfg.before_overwinter, Some(1));
        assert_eq!(cfg.overwinter, Some(1));
        assert_eq!(cfg.sapling, Some(1));
        assert_eq!(cfg.blossom, Some(1));
        assert_eq!(cfg.heartwood, Some(1));
        assert_eq!(cfg.canopy, Some(1));

        assert_eq!(cfg.nu5, Some(1));
        assert_eq!(cfg.nu6, Some(1));
        assert_eq!(cfg.nu6_1, Some(1));
        assert_eq!(cfg.nu7, Some(1));
    }

    #[test]
    fn parse_activation_heights_sets_individual_fields_case_insensitive() {
        let cfg = parse_activation_heights("Sapling=2, Nu6=3, nu7=off").unwrap();
        assert_eq!(cfg.sapling, Some(2));
        assert_eq!(cfg.nu6, Some(3));
        assert_eq!(cfg.nu7, None);
    }

    #[test]
    fn parse_activation_heights_supports_nu6_1_aliases() {
        let cfg = parse_activation_heights("nu6_1=10").unwrap();
        assert_eq!(cfg.nu6_1, Some(10));

        let cfg = parse_activation_heights("nu6.1=11").unwrap();
        assert_eq!(cfg.nu6_1, Some(11));

        let cfg = parse_activation_heights("nu61=12").unwrap();
        assert_eq!(cfg.nu6_1, Some(12));
    }

    #[test]
    fn parse_activation_heights_supports_before_overwinter_aliases() {
        let cfg = parse_activation_heights("before_overwinter=1").unwrap();
        assert_eq!(cfg.before_overwinter, Some(1));

        let cfg = parse_activation_heights("pre_overwinter=2").unwrap();
        assert_eq!(cfg.before_overwinter, Some(2));

        let cfg = parse_activation_heights("beforeoverwinter=3").unwrap();
        assert_eq!(cfg.before_overwinter, Some(3));
    }

    #[test]
    fn parse_activation_heights_all_sets_everything() {
        let cfg = parse_activation_heights("all=7").unwrap();
        assert_eq!(cfg.before_overwinter, Some(7));
        assert_eq!(cfg.overwinter, Some(7));
        assert_eq!(cfg.sapling, Some(7));
        assert_eq!(cfg.blossom, Some(7));
        assert_eq!(cfg.heartwood, Some(7));
        assert_eq!(cfg.canopy, Some(7));
        assert_eq!(cfg.nu5, Some(7));
        assert_eq!(cfg.nu6, Some(7));
        assert_eq!(cfg.nu6_1, Some(7));
        assert_eq!(cfg.nu7, Some(7));
    }

    #[test]
    fn parse_activation_heights_all_off_sets_everything_none() {
        let cfg = parse_activation_heights("all=off").unwrap();
        assert_eq!(cfg.before_overwinter, None);
        assert_eq!(cfg.overwinter, None);
        assert_eq!(cfg.sapling, None);
        assert_eq!(cfg.blossom, None);
        assert_eq!(cfg.heartwood, None);
        assert_eq!(cfg.canopy, None);
        assert_eq!(cfg.nu5, None);
        assert_eq!(cfg.nu6, None);
        assert_eq!(cfg.nu6_1, None);
        assert_eq!(cfg.nu7, None);
    }

    #[test]
    fn later_tokens_override_earlier_tokens() {
        // all=1 sets everything, then nu7 overrides to off
        let cfg = parse_activation_heights("all=1,nu7=off").unwrap();
        assert_eq!(cfg.nu5, Some(1));
        assert_eq!(cfg.nu6, Some(1));
        assert_eq!(cfg.nu6_1, Some(1));
        assert_eq!(cfg.nu7, None);
    }

    #[test]
    fn later_all_overrides_previous_individuals() {
        // nu5 set first, then all overrides it
        let cfg = parse_activation_heights("nu5=9,all=1").unwrap();
        assert_eq!(cfg.nu5, Some(1));
        assert_eq!(cfg.nu6, Some(1));
        assert_eq!(cfg.sapling, Some(1));
    }

    #[test]
    fn parse_activation_heights_errors_on_missing_equals() {
        let err = parse_activation_heights("nu5").unwrap_err();
        assert!(err.contains("expected key=value"), "{err}");
    }

    #[test]
    fn parse_activation_heights_errors_on_unknown_key() {
        let err = parse_activation_heights("nope=1").unwrap_err();
        assert!(err.contains("Unknown activation key"), "{err}");
        assert!(err.contains("Valid keys"), "{err}");
    }

    #[test]
    fn parse_activation_heights_errors_on_bad_value() {
        let err = parse_activation_heights("nu5=nope").unwrap_err();
        assert!(err.contains("Invalid height"), "{err}");
    }

    #[test]
    fn parse_activation_heights_accepts_repeated_keys_last_wins() {
        let cfg = parse_activation_heights("nu5=1,nu5=2").unwrap();
        assert_eq!(cfg.nu5, Some(2));
    }

    #[test]
    fn parse_activation_heights_trims_key_and_value() {
        let cfg = parse_activation_heights(" nu5 =  42 ").unwrap();
        assert_eq!(cfg.nu5, Some(42));
    }

    #[test]
    fn parse_activation_heights_cascades_and_overrides() {
        let cfg = parse_activation_heights("nu5=1,nu7=off").unwrap();
        assert_eq!(cfg.nu5, Some(1));
        assert_eq!(cfg.nu6, Some(1));
        assert_eq!(cfg.nu6_1, Some(1));
        assert_eq!(cfg.nu7, None);
    }

    /// Drift-detection: the CLI default string and the in-code fixture
    /// helper must produce identical values. If this test fails, one
    /// of them was edited without updating the other — the bug it
    /// guards against is exactly the one that surfaced when the infras
    /// pin was bumped (zainod and validator disagreed on regtest
    /// activation heights, producing
    /// `Block commitment could not be computed`).
    #[test]
    fn cli_default_matches_fixture_helper() {
        use local_net::validator::{
            regtest_test_activation_heights, REGTEST_FIXTURE_HEIGHTS_CLI_STRING,
        };
        let parsed = parse_activation_heights(REGTEST_FIXTURE_HEIGHTS_CLI_STRING)
            .expect("CLI default string must parse");

        // Mirror the field-by-field conversion done in
        // regtest-launcher::main: ConfiguredActivationHeights ->
        // zingo_common_components::ActivationHeights. `before_overwinter`
        // exists on the former but not the latter and is dropped.
        let from_cli = zingo_common_components::protocol::ActivationHeights::builder()
            .set_overwinter(parsed.overwinter)
            .set_sapling(parsed.sapling)
            .set_blossom(parsed.blossom)
            .set_heartwood(parsed.heartwood)
            .set_canopy(parsed.canopy)
            .set_nu5(parsed.nu5)
            .set_nu6(parsed.nu6)
            .set_nu6_1(parsed.nu6_1)
            .set_nu7(parsed.nu7)
            .build();

        assert_eq!(
            from_cli,
            regtest_test_activation_heights(),
            "CLI default string drifted from regtest_test_activation_heights"
        );
    }
}
