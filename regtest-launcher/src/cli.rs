use clap::Parser;
use zebra_rpc::client::zebra_chain::parameters::testnet::ConfiguredActivationHeights;

#[derive(Parser, Debug)]
pub struct Cli {
    /// Comma-separated activation heights, e.g.
    /// "all=1,nu5=1000,nu6=off,nu6_1=off,nu7=off"
    ///
    /// Keys: before_overwinter, overwinter, sapling, blossom, heartwood, canopy, nu5, nu6, nu6_1, nu7, all
    /// Values: u32 or off|none|disable
    #[arg(
        long,
        value_parser = parse_activation_heights,
        default_value = "all=1,nu7=off"
    )]
    pub activation_heights: ConfiguredActivationHeights,
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
        nu7: None,
    };

    for part in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (k, v) = part
            .split_once('=')
            .ok_or_else(|| format!("Bad token '{part}': expected key=value"))?;

        let key = k.trim().to_ascii_lowercase();
        let val = parse_val(v)?;

        match key.as_str() {
            "all" => set_all(&mut cfg, val),

            "before_overwinter" | "pre_overwinter" | "beforeoverwinter" => {
                cfg.before_overwinter = val
            }
            "overwinter" => cfg.overwinter = val,
            "sapling" => cfg.sapling = val,
            "blossom" => cfg.blossom = val,
            "heartwood" => cfg.heartwood = val,
            "canopy" => cfg.canopy = val,

            "nu5" => cfg.nu5 = val,
            "nu6" => cfg.nu6 = val,

            "nu6_1" | "nu6.1" | "nu61" => cfg.nu6_1 = val,

            "nu7" => cfg.nu7 = val,

            _ => {
                return Err(format!(
                    "Unknown activation key '{k}'. Valid keys: \
                     before_overwinter, overwinter, sapling, blossom, heartwood, canopy, nu5, nu6, nu6_1, nu7, all"
                ));
            }
        }
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

fn set_all(cfg: &mut ConfiguredActivationHeights, val: Option<u32>) {
    cfg.before_overwinter = val;
    cfg.overwinter = val;
    cfg.sapling = val;
    cfg.blossom = val;
    cfg.heartwood = val;
    cfg.canopy = val;
    cfg.nu5 = val;
    cfg.nu6 = val;
    cfg.nu6_1 = val;
    cfg.nu7 = val;
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
    fn parse_activation_heights_empty_is_all_none() {
        let cfg = parse_activation_heights("").unwrap();
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
    fn parse_activation_heights_ignores_extra_commas_and_whitespace() {
        let cfg = parse_activation_heights(" , , nu5=1 , , ").unwrap();
        assert_eq!(cfg.nu5, Some(1));
        // others remain None
        assert_eq!(cfg.nu6, None);
        assert_eq!(cfg.nu7, None);
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
}
