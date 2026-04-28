mod testutils;

use zcash_local_net::indexer::lightwalletd::Lightwalletd;
use zcash_local_net::process::Process;
use zcash_local_net::validator::Validator as _;
use zcash_local_net::validator::ValidatorConfig as _;
use zcash_local_net::protocol::ActivationHeights;
use zcash_local_net::LocalNetConfig;
use zcash_local_net::{
    indexer::{
        empty::{Empty, EmptyConfig},
        zainod::Zainod,
    },
    utils,
    validator::{
        zcashd::Zcashd,
        zebrad::{Zebrad, ZebradConfig},
    },
    LocalNet,
};
use zcash_protocol::PoolType;

async fn launch_default_and_print_all<P: Process>() {
    let p = P::launch_default().await.expect("Process launching!");
    p.print_all();
}

#[tokio::test]
async fn launch_zcashd() {
    tracing_subscriber::fmt().init();

    launch_default_and_print_all::<Zcashd>().await;
}

#[tokio::test]
async fn launch_zcashd_custom_activation_heights() {
    tracing_subscriber::fmt().init();

    let zcashd = Zcashd::launch_default().await.unwrap();

    zcashd.generate_blocks(8).await.unwrap();
    zcashd.print_all();
}

#[tokio::test]
async fn launch_zebrad() {
    tracing_subscriber::fmt().init();

    launch_default_and_print_all::<Zebrad>().await;
}

/// Probe whether NU6.1 can be activated at a given regtest height.
///
/// Background: zebrad's `subsidy_is_valid` (in
/// `zebra-consensus/src/block/check.rs`) rejects the NU6.1 activation
/// block when `Network::lockbox_disbursements(height).is_empty()`.
/// On regtest, `RegtestParameters.lockbox_disbursements` defaults to
/// `Vec::new()`, so *every* NU6.1 activation height fails on zebrad
/// with `submitblock` returning `"rejected"`. The check has no window
/// or accumulation rule — it's purely "is the disbursements list set."
/// zcashd has no equivalent check today, so it accepts any height.
///
/// These tests document the validator divergence and stand as a
/// regression record for when `ZebradConfig` learns to inject
/// non-empty `lockbox_disbursements` into Zebra's parameters; that
/// future test is the one that proves NU6.1 can actually be exercised
/// in regtest.
async fn probe_validator_with_nu6_1_at<V: zcash_local_net::validator::Validator>(
    nu6_1_height: u32,
    mine_count: u32,
) {
    let activation_heights = ActivationHeights::builder()
        .set_overwinter(Some(1))
        .set_sapling(Some(1))
        .set_blossom(Some(1))
        .set_heartwood(Some(1))
        .set_canopy(Some(1))
        .set_nu5(Some(2))
        .set_nu6(Some(2))
        .set_nu6_1(Some(nu6_1_height))
        .set_nu7(None)
        .build();

    let mut config = V::Config::default();
    config.set_test_parameters(PoolType::Transparent, activation_heights, None);

    let validator = V::launch(config).await.unwrap_or_else(|e| {
        panic!(
            "{}: launch with NU6.1 at height {nu6_1_height} failed: {e:?}",
            std::any::type_name::<V>()
        )
    });
    validator
        .generate_blocks(mine_count)
        .await
        .unwrap_or_else(|e| {
            panic!(
                "{}: generate_blocks past NU6.1 at height {nu6_1_height} failed: {e:?}",
                std::any::type_name::<V>()
            )
        });

    let final_height = validator.get_chain_height().await;
    assert!(
        final_height >= 1 + mine_count,
        "{}: expected chain to advance to >= {}, got {final_height}",
        std::any::type_name::<V>(),
        1 + mine_count
    );
    validator.print_all();
}

// Zebrad: rejects every NU6.1 activation height while regtest's
// lockbox_disbursements is empty. Three heights document that the
// rejection is height-independent (right at activation, just past NU6,
// well past NU6).
//
// All three are `#[ignore]`d in normal runs because they assert the
// known-failing path — leaving them enabled would surface as red CI
// without representing a regression. Run on demand via
// `cargo nextest run --run-ignored only ...` to confirm the behavior
// hasn't changed (e.g. after a zebrad version bump or a CHANGELOG
// claim that the lockbox check has moved). Once #244 closes and the
// disbursement-armed test in `launch_zebrad_with_nu6_1_at_height_2_and_dummy_disbursements`
// becomes the canonical positive test, these probes can be removed.

#[ignore = "documents empty-default failure of zebrad NU6.1 activation; see #244"]
#[tokio::test]
async fn launch_zebrad_with_nu6_1_at_height_2() {
    tracing_subscriber::fmt().init();
    probe_validator_with_nu6_1_at::<Zebrad>(2, 5).await;
}

#[ignore = "documents empty-default failure of zebrad NU6.1 activation; see #244"]
#[tokio::test]
async fn launch_zebrad_with_nu6_1_at_height_3() {
    tracing_subscriber::fmt().init();
    probe_validator_with_nu6_1_at::<Zebrad>(3, 5).await;
}

#[ignore = "documents empty-default failure of zebrad NU6.1 activation; see #244"]
#[tokio::test]
async fn launch_zebrad_with_nu6_1_at_height_50() {
    tracing_subscriber::fmt().init();
    probe_validator_with_nu6_1_at::<Zebrad>(50, 52).await;
}

// Zcashd: accepts NU6.1 at the lowest meaningful height — documents
// the validator divergence (zcashd has no equivalent NU6.1 lockbox
// check today). One test is enough; higher heights all pass for the
// same reason.

#[tokio::test]
async fn launch_zcashd_with_nu6_1_at_height_2() {
    tracing_subscriber::fmt().init();
    probe_validator_with_nu6_1_at::<Zcashd>(2, 5).await;
}

// ─── Zebrad RPC liveness probes ─────────────────────────────────────────
//
// zebrad has no dedicated /healthz or /readyz endpoint (see
// zingolabs/infrastructure#245). Until upstream provides one, the
// harness's best signal is the four readiness-relevant RPCs the
// daemon already exposes. These probes assert each one responds after
// `Zebrad::launch_default` returns, plus a conjunction test that
// implements the "informal readiness" definition from #245
// (live & ready iff all four return Ok).
//
// Per-endpoint probes give nextest-level reporting of which subsystem
// is unhealthy when something regresses; the conjunction test is the
// single canary that fails fast if any of them does.

async fn probe_zebrad_rpc_endpoint(endpoint: &'static str) {
    let zebrad = Zebrad::launch_default()
        .await
        .expect("zebrad launch_default");
    zebrad
        .client()
        .json_result_from_call::<serde_json::Value>(endpoint, "[]".to_string())
        .await
        .unwrap_or_else(|e| panic!("zebrad {endpoint} probe failed: {e:?}"));
}

#[tokio::test]
async fn zebrad_responds_to_getinfo() {
    tracing_subscriber::fmt().init();
    // Most permissive endpoint — answers as soon as the JSON-RPC
    // dispatcher is registered. If this fails, the process is dead
    // or the listener never bound.
    probe_zebrad_rpc_endpoint("getinfo").await;
}

#[tokio::test]
async fn zebrad_responds_to_getnetworkinfo() {
    tracing_subscriber::fmt().init();
    // Network module loaded; peer subsystem reachable.
    probe_zebrad_rpc_endpoint("getnetworkinfo").await;
}

#[tokio::test]
async fn zebrad_responds_to_getblockchaininfo() {
    tracing_subscriber::fmt().init();
    // State module loaded; chain tip readable from the database.
    probe_zebrad_rpc_endpoint("getblockchaininfo").await;
}

#[tokio::test]
async fn zebrad_responds_to_getblocktemplate() {
    tracing_subscriber::fmt().init();
    // Mining service active and consensus is in a state where the
    // next block can be mined. Most restrictive of the four.
    probe_zebrad_rpc_endpoint("getblocktemplate").await;
}

#[tokio::test]
async fn zebrad_passes_informal_readiness_conjunction() {
    tracing_subscriber::fmt().init();
    // The "informal readiness" contract from
    // zingolabs/infrastructure#245: live & ready iff all four
    // readiness-relevant RPCs return Ok. Reports which endpoints
    // failed when the conjunction does, instead of the single-RPC
    // ambiguity of polling getblocktemplate alone.
    let zebrad = Zebrad::launch_default()
        .await
        .expect("zebrad launch_default");

    let endpoints = [
        "getinfo",
        "getnetworkinfo",
        "getblockchaininfo",
        "getblocktemplate",
    ];
    let mut failures = Vec::new();
    for endpoint in endpoints {
        if let Err(e) = zebrad
            .client()
            .json_result_from_call::<serde_json::Value>(endpoint, "[]".to_string())
            .await
        {
            failures.push(format!("{endpoint}: {e:?}"));
        }
    }
    assert!(
        failures.is_empty(),
        "informal readiness conjunction failed:\n{}",
        failures.join("\n")
    );
}

/// Inverse of the failing zebrad probes above: with a non-empty
/// `lockbox_disbursements` list configured into Zebra's regtest
/// parameters, the NU6.1 activation block should pass
/// `subsidy_is_valid` and the chain should mine past it.
///
/// **Currently `#[ignore]`d** — empirically zebrad runs *three*
/// consensus checks at the activation block, and a single dummy
/// disbursement only clears two of them:
///
///   1. `lockbox_disbursements.is_empty()` — passes (we configure
///      one entry).
///   2. `addr.is_script_hash()` — passes (`dummy()` uses a P2SH
///      address).
///   3. `Deferred` value-pool constraint — **fails**: the
///      activation block tries to withdraw 1 zat from the deferred
///      pool, but regtest's default `funding_streams: []` never
///      deposited anything into it, so the post-block deferred
///      balance lands at -1 and the consensus check rejects.
///
/// Re-enabling requires harness support for configuring NU6 funding
/// streams with a `Deferred` recipient (so the lockbox accumulates
/// before NU6.1 activates). Tracked in zingolabs/infrastructure#244
/// (and possibly a follow-up sub-issue once that lands).
#[ignore = "blocked: deferred-pool empty without NU6 funding streams; see #244"]
#[tokio::test]
async fn launch_zebrad_with_nu6_1_at_height_2_and_dummy_disbursements() {
    tracing_subscriber::fmt().init();

    let activation_heights = ActivationHeights::builder()
        .set_overwinter(Some(1))
        .set_sapling(Some(1))
        .set_blossom(Some(1))
        .set_heartwood(Some(1))
        .set_canopy(Some(1))
        .set_nu5(Some(2))
        .set_nu6(Some(2))
        .set_nu6_1(Some(2))
        .set_nu7(None)
        .build();

    let mut config = ZebradConfig::default();
    config.set_test_parameters(PoolType::Transparent, activation_heights, None);
    config.lockbox_disbursements = zcash_local_net::validator::regtest_test_lockbox_disbursements();

    let zebrad = Zebrad::launch(config)
        .await
        .expect("zebrad launch with dummy disbursements");

    // Print zebrad's stdout+stderr to the test runner's stderr no
    // matter how the rest of the test exits — captures the actual
    // error message when a downstream call panics on a transport
    // error. Bypasses `Process::print_all`, which routes through
    // `tracing::trace!` and is silently dropped at the default INFO
    // level (#244 diagnostic).
    struct PrintOnDrop<'a>(&'a Zebrad);
    impl Drop for PrintOnDrop<'_> {
        fn drop(&mut self) {
            for (label, name) in [("stdout", "stdout.log"), ("stderr", "stderr.log")] {
                let path = self.0.logs_dir().path().join(name);
                match std::fs::read_to_string(&path) {
                    Ok(s) if s.is_empty() => {
                        eprintln!("=== zebrad {label}: <empty> ===");
                    }
                    Ok(s) => eprintln!("=== zebrad {label} ===\n{s}=== end {label} ==="),
                    Err(e) => eprintln!("=== zebrad {label}: read failed ({e}) ==="),
                }
            }
        }
    }
    let _print_on_drop = PrintOnDrop(&zebrad);

    zebrad
        .generate_blocks(5)
        .await
        .expect("generate_blocks past NU6.1 with disbursements configured");

    let final_height = zebrad.get_chain_height().await;
    assert!(
        final_height >= 6,
        "expected chain to advance past NU6.1; got height {final_height}"
    );
}

#[ignore = "temporary during refactor into workspace"]
#[tokio::test]
async fn launch_zebrad_with_cache() {
    tracing_subscriber::fmt().init();

    let config = ZebradConfig {
        chain_cache: Some(utils::chain_cache_dir().join("client_rpc_tests_large")),
        ..Default::default()
    };

    let zebrad = Zebrad::launch(config).await.unwrap();
    zebrad.print_all();

    assert_eq!(zebrad.get_chain_height().await, 52u32);
}

#[ignore = "requires chain cache to be generated"]
/// Asserts that launching 2 `zebrad` instances with the same cache fails.
/// The second instance cannot open the database, due to it already being in use by the first instance.
#[tokio::test]
async fn launch_multiple_individual_zebrads_with_cache() {
    tracing_subscriber::fmt().init();
    let config = ZebradConfig {
        chain_cache: Some(utils::chain_cache_dir().join("client_rpc_tests_large")),
        ..Default::default()
    };

    let zebrad_1 = Zebrad::launch(config.clone()).await.unwrap();
    zebrad_1.print_all();

    let zebrad_2 = Zebrad::launch(config).await.unwrap();
    zebrad_2.print_all();

    assert_eq!(zebrad_1.get_chain_height().await, 52u32);
    assert_eq!(zebrad_2.get_chain_height().await, 52u32);
}

#[ignore = "requires chain cache to be generated"]
/// Tests that 2 `zebrad` instances, each with a copy of the chain cache, can be launched.
#[tokio::test]
async fn localnet_launch_multiple_zebrads_with_cache() {
    tracing_subscriber::fmt().init();

    let config = ZebradConfig {
        chain_cache: Some(utils::chain_cache_dir().join("client_rpc_tests_large")),
        ..Default::default()
    };

    let local_net_1 = LocalNet::<Zebrad, Empty>::launch(LocalNetConfig {
        indexer_config: EmptyConfig {},
        validator_config: config.clone(),
    })
    .await
    .unwrap();

    let local_net_2 = LocalNet::<Zebrad, Empty>::launch(LocalNetConfig {
        indexer_config: EmptyConfig {},
        validator_config: config.clone(),
    })
    .await
    .unwrap();

    let zebrad_1 = local_net_1.validator();
    let zebrad_2 = local_net_2.validator();

    assert_eq!(zebrad_1.get_chain_height().await, 52u32);
    assert_eq!(zebrad_2.get_chain_height().await, 52u32);

    zebrad_1.print_all();
    zebrad_2.print_all();
}

#[tokio::test]
async fn launch_localnet_zainod_zcashd() {
    tracing_subscriber::fmt().init();

    launch_default_and_print_all::<LocalNet<Zcashd, Zainod>>().await;
}

#[tokio::test]
async fn launch_localnet_zainod_zebrad() {
    tracing_subscriber::fmt().init();

    launch_default_and_print_all::<LocalNet<Zebrad, Zainod>>().await;
}

#[tokio::test]
async fn launch_localnet_lightwalletd_zcashd() {
    tracing_subscriber::fmt().init();

    launch_default_and_print_all::<LocalNet<Zcashd, Lightwalletd>>().await;
}

#[tokio::test]
async fn launch_localnet_lightwalletd_zebrad() {
    tracing_subscriber::fmt().init();

    launch_default_and_print_all::<LocalNet<Zebrad, Lightwalletd>>().await;
}

#[ignore = "not a test. generates chain cache for client_rpc tests."]
#[tokio::test]
async fn generate_zebrad_large_chain_cache() {
    tracing_subscriber::fmt().init();

    crate::testutils::generate_zebrad_large_chain_cache().await;
}

// FIXME: This is not a test, so it shouldn't be marked as one.
// and TODO: Pre-test setups should be moved elsewhere.
#[ignore = "not a test. generates chain cache for client_rpc tests."]
#[tokio::test]
async fn generate_zcashd_chain_cache() {
    tracing_subscriber::fmt().init();

    crate::testutils::generate_zcashd_chain_cache().await;
}
