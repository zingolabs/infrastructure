use clap::Parser;
use zebra_rpc::client::zebra_chain::parameters::testnet::ConfiguredActivationHeights;

#[derive(Parser, Debug)]
pub struct Cli {
    /// Comma-separated activation heights, e.g.
    /// "all=1,nu5=1000,nu6=off,nu6_1=off,nu7=off"
    ///
    /// Keys: before_overwinter, overwinter, sapling, blossom, heartwood, canopy, nu5, nu6, nu6_1, nu7, all
    /// Values: <u32> or off|none|disable
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
