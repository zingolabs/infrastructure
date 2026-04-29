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
async fn zebrad_healthy_endpoint_responds_200_after_launch() {
    tracing_subscriber::fmt().init();
    // /healthy with min_connected_peers=0 (regtest default) returns
    // 200 as soon as the HTTP server has bound. Launch implies
    // wait_for_rpc_ready has already passed, so the listener must
    // be up.
    let zebrad = Zebrad::launch_default()
        .await
        .expect("zebrad launch_default");
    assert!(
        zebrad.healthy().await.expect("/healthy fetch"),
        "/healthy returned non-200 immediately after launch"
    );
}

#[tokio::test]
async fn zebrad_ready_endpoint_responds_200_after_one_block() {
    tracing_subscriber::fmt().init();
    // /ready requires the latest committed block to be recent
    // (within ready_max_tip_age, default 300s) and chain-tip lag
    // bounded. launch_default mines genesis as part of its
    // sequence, so post-launch the chain has a fresh block and
    // /ready should be 200.
    let zebrad = Zebrad::launch_default()
        .await
        .expect("zebrad launch_default");
    assert!(
        zebrad.ready().await.expect("/ready fetch"),
        "/ready returned non-200 after launch (genesis is the most recent block)"
    );
}

#[tokio::test]
async fn zebrad_health_endpoints_agree_with_rpc_readiness_conjunction() {
    tracing_subscriber::fmt().init();
    // Regression test for upstream Zebra changes that would let
    // /healthy or /ready report ready while the JSON-RPC surface
    // is actually broken (or vice versa). The harness's informal
    // AND-of-4 readiness contract is the cross-check: both signals
    // must agree on a healthy steady state, otherwise one side
    // has regressed.
    let zebrad = Zebrad::launch_default()
        .await
        .expect("zebrad launch_default");

    let endpoints = [
        "getinfo",
        "getnetworkinfo",
        "getblockchaininfo",
        "getblocktemplate",
    ];
    let mut rpc_failures = Vec::new();
    for endpoint in endpoints {
        if let Err(e) = zebrad
            .client()
            .json_result_from_call::<serde_json::Value>(endpoint, "[]".to_string())
            .await
        {
            rpc_failures.push(format!("{endpoint}: {e:?}"));
        }
    }
    let rpcs_ok = rpc_failures.is_empty();

    let healthy_ok = zebrad.healthy().await.expect("/healthy fetch");
    let ready_ok = zebrad.ready().await.expect("/ready fetch");
    let endpoints_ok = healthy_ok && ready_ok;

    assert_eq!(
        rpcs_ok, endpoints_ok,
        "informal AND-of-4 RPC readiness ({rpcs_ok}) diverged from \
         (/healthy AND /ready) ({endpoints_ok}); /healthy={healthy_ok}, \
         /ready={ready_ok}, RPC failures={rpc_failures:#?}"
    );
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
/// With NU6 at height 2, NU6.1 at height 5, a `Deferred` post-NU6
/// funding stream populating the lockbox each block, and a single
/// dummy disbursement, the activation block satisfies all three
/// consensus checks and the chain mines past it.
///
/// This is the proof-of-life test for the full NU6.1 plumbing:
/// `LockboxDisbursement` (P2SH address + amount) plus
/// `FundingStreams` (Deferred recipient depositing into the
/// `Deferred` value pool). Run on a regtest fixture aligned to
/// require all three checks to pass.
#[tokio::test]
async fn launch_zebrad_with_nu6_1_at_height_5_with_disbursements_and_funding_streams() {
    tracing_subscriber::fmt().init();

    let activation_heights = ActivationHeights::builder()
        .set_overwinter(Some(1))
        .set_sapling(Some(1))
        .set_blossom(Some(1))
        .set_heartwood(Some(1))
        .set_canopy(Some(1))
        .set_nu5(Some(2))
        .set_nu6(Some(2))
        // NU6.1 a few blocks after NU6 so the `Deferred` value pool
        // accumulates enough subsidy fraction to cover the
        // disbursement total.
        .set_nu6_1(Some(5))
        .set_nu7(None)
        .build();

    let mut config = ZebradConfig::default();
    config.set_test_parameters(PoolType::Transparent, activation_heights, None);
    config.lockbox_disbursements =
        zcash_local_net::validator::regtest_test_lockbox_disbursements();
    config.post_nu6_funding_streams =
        Some(zcash_local_net::validator::regtest_test_post_nu6_funding_streams());

    let zebrad = Zebrad::launch(config)
        .await
        .expect("zebrad launch with disbursements + post-NU6 funding streams");

    zebrad
        .generate_blocks(8)
        .await
        .expect("generate_blocks past NU6.1 activation");

    let final_height = zebrad.get_chain_height().await;
    assert!(
        final_height >= 9,
        "expected chain to advance past NU6.1 (5) + buffer; got height {final_height}"
    );
}

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

/// Regression tests for the cross-test-subprocess port-pick race
/// that surfaced as a flake of
/// `launch_zebrad_with_nu6_1_at_height_5_with_disbursements_and_funding_streams`
/// in this crate's CI and as `ConnectionRefused` failures of
/// `integration-tests::fetch_service zcashd::get::mining_info` and
/// `integration-tests::state_service zebra::lightwallet_indexer::get_latest_block`
/// in zaino's CI.
///
/// **The race.** `network::pick_unused_port` does
/// `TcpListener::bind("127.0.0.1:0")` → drops the listener → records
/// the port in a *process-local* `RESERVED: HashSet<u16>`. nextest
/// runs each test in its own subprocess, so two parallel test
/// subprocesses each have their own registry; the kernel can hand
/// the same ephemeral port to both, and the second `*::launch` to
/// reach the child-bind step fails with
/// `Os { code: 98, kind: AddrInUse }`. The race is structural to
/// the "pick → drop → register → much-later child bind" pattern,
/// not specific to any one process — every `*::launch` follows it,
/// validators *and* indexers alike.
///
/// **What the tests assert.** Each test holds a `TcpListener` on a
/// kernel-assigned ephemeral port (taking the role of "a parallel
/// test subprocess that already bound the port"), pins that port
/// through the process's config, and calls the real `*::launch`.
/// `launch::with_retry_on_collision` should detect the AddrInUse on
/// the first attempt, clear the process's port pins (so the next
/// pick re-rolls fresh ephemerals), and succeed on a subsequent
/// attempt with a port that does not collide with the squatter. The
/// `assert_ne!` confirms the recovered port differs from the pinned
/// one — i.e., that retry actually re-rolled rather than producing a
/// same-port success by accident.
///
/// **Why four tests.** The race is structural — a single test would
/// suffice as a regression marker. Four tests discriminate among
/// regression causes: a fix that addresses zebrad's path but not
/// zcashd's would let one validator pass and one fail; same for
/// indexers vs. validators. The validator tests run standalone; the
/// indexer tests pre-launch a real validator (whose own retry covers
/// any TOCTOU on its picks) so the indexer's launch has a live RPC
/// endpoint to point at.
///
/// **What "fail" looks like.** If the retry helper is broken for a
/// given process, that test fails through `diagnose` with a
/// multi-line `REGRESSION-MARKER:` panic identifying the process, the
/// pinned conflicted port, the captured stderr, and which
/// `LaunchError` variant the last attempt produced
/// (`ProcessFailed` vs. `LaunchAborted`). If the failure stderr
/// does not contain a known bind-failure signature, the panic is
/// flagged `UNEXPECTED FAILURE MODE` so a divergent regression
/// cannot masquerade as the port-collision repro.
mod launch_recovers_from_rpc_port_collision {
    use super::*;
    use std::net::TcpListener;
    use zcash_local_net::error::LaunchError;
    use zcash_local_net::indexer::lightwalletd::LightwalletdConfig;
    use zcash_local_net::indexer::zainod::ZainodConfig;
    use zcash_local_net::validator::zcashd::ZcashdConfig;

    /// Run `launch_fut` and, on failure, panic with a multi-line
    /// regression-marker message that names the validator, the
    /// pinned conflicted port, the `LaunchError` variant the retry
    /// helper bottomed out on, and whether the captured stderr
    /// matched any of the per-validator RPC-bind `stderr_signatures`.
    /// On the happy path the launched handle is returned unchanged.
    ///
    /// `stderr_signatures` is the union of strings the validator
    /// emits when its bind hits AddrInUse. Both `ProcessFailed`
    /// (child exited) and `LaunchAborted` (`launch::wait`'s
    /// indicator scan tripped before the child exited) carry the
    /// captured stderr — `LaunchError::stderr()` extracts it for
    /// either variant. A signature miss flags the failure as
    /// UNEXPECTED so a future divergent regression doesn't get
    /// silently classified as a port-collision repro.
    async fn diagnose<V>(
        validator: &'static str,
        conflicted_rpc_port: u16,
        stderr_signatures: &'static [&'static str],
        launch_fut: impl std::future::Future<Output = Result<V, LaunchError>>,
    ) -> V {
        let err = match launch_fut.await {
            Ok(handle) => return handle,
            Err(e) => e,
        };

        let header = format!(
            "REGRESSION-MARKER: {validator}::launch retry-on-collision did not recover\n  \
             conflicted RPC port (held by squatter): {conflicted_rpc_port}\n  \
             see module docstring (`mod launch_recovers_from_rpc_port_collision`) for the race\n  \
             retry helper:                           zcash_local_net/src/launch.rs::with_retry_on_collision (3 attempts, expects to recover on attempt ≥ 2)"
        );

        let captured = err.captured_output();
        let signature_hit = stderr_signatures
            .iter()
            .copied()
            .find(|sig| captured.contains(sig));

        let mode_line = match (&err, signature_hit) {
            (LaunchError::ProcessFailed { exit_status, .. }, Some(sig)) => format!(
                "mode: every retry attempt hit ProcessFailed (last exit={exit_status}); \
                 captured output contains expected RPC-bind signature {sig:?}"
            ),
            (LaunchError::LaunchAborted { matched_indicator, .. }, Some(sig)) => format!(
                "mode: every retry attempt hit indicator-scan abort (last matched_indicator={matched_indicator:?}); \
                 captured output contains expected RPC-bind signature {sig:?}"
            ),
            (LaunchError::ListenerNotResponsive { port, .. }, Some(sig)) => format!(
                "mode: every retry attempt left the listener at 127.0.0.1:{port} unresponsive; \
                 captured output contains expected RPC-bind signature {sig:?}"
            ),
            (LaunchError::ProcessFailed { exit_status, .. }, None) => format!(
                "mode: LaunchError::ProcessFailed (exit={exit_status}) — UNEXPECTED FAILURE MODE: \
                 captured output does not contain any of the expected RPC-bind signatures {stderr_signatures:?}"
            ),
            (LaunchError::LaunchAborted { matched_indicator, .. }, None) => format!(
                "mode: LaunchError::LaunchAborted (indicator={matched_indicator:?}) — UNEXPECTED FAILURE MODE: \
                 captured output does not contain any of the expected RPC-bind signatures {stderr_signatures:?}"
            ),
            (LaunchError::ListenerNotResponsive { port, .. }, None) => format!(
                "mode: LaunchError::ListenerNotResponsive (port={port}) — UNEXPECTED FAILURE MODE: \
                 captured output does not contain any of the expected RPC-bind signatures {stderr_signatures:?}"
            ),
            (other, _) => format!("mode: UNEXPECTED FAILURE MODE — {other:?}"),
        };

        panic!("{header}\n  {mode_line}\n  child captured output (full):\n{captured}");
    }

    /// Bind a kernel-ephemeral TCP listener and return both the
    /// listener (held for the lifetime of the test) and the port it
    /// claimed. The listener takes the role of "a parallel test
    /// subprocess that already bound the port" — pinning that port
    /// through a process's config is what forces the launch path to
    /// hit AddrInUse and exercise its retry.
    fn squat_a_port() -> (TcpListener, u16) {
        let squatter = TcpListener::bind("127.0.0.1:0").expect("squatter bind");
        let port = squatter.local_addr().expect("local_addr").port();
        (squatter, port)
    }

    /// One collision-recovery test in template form. Used by every
    /// process-specific test in this module:
    ///
    ///   1. Squat a kernel-ephemeral port.
    ///   2. Hand it to `pin_and_launch` so the caller can build the
    ///      process's config with that port pinned and invoke its
    ///      `*::launch`.
    ///   3. Funnel the launch future through `diagnose`, which
    ///      produces a `REGRESSION-MARKER` panic on failure or
    ///      returns the launched handle on success.
    ///   4. Assert (via `extract_port`) that the recovered port is
    ///      different from the squatted one — i.e., that retry
    ///      actually re-rolled rather than producing a same-port
    ///      success by accident.
    ///
    /// The squatter is held until after the assertion so the bind
    /// stays in effect for the entire collision/retry sequence, then
    /// dropped explicitly to free the port.
    async fn run_collision_test<V, F>(
        process_name: &'static str,
        stderr_signatures: &'static [&'static str],
        pin_and_launch: impl FnOnce(u16) -> F,
        extract_port: impl FnOnce(&V) -> u16,
    ) where
        F: std::future::Future<Output = Result<V, LaunchError>>,
    {
        let (squatter, conflicted) = squat_a_port();
        let v = diagnose(
            process_name,
            conflicted,
            stderr_signatures,
            pin_and_launch(conflicted),
        )
        .await;
        assert_ne!(
            extract_port(&v),
            conflicted,
            "after retry, {process_name} should be on a different listen port than the squatter"
        );
        drop(squatter);
    }

    #[tokio::test]
    async fn zcashd() {
        let _ = tracing_subscriber::fmt().try_init();
        run_collision_test(
            "Zcashd",
            &["Unable to start HTTP server", "Unable to bind any endpoint"],
            |conflicted| {
                let mut config = ZcashdConfig::default();
                config.rpc_listen_port = Some(conflicted);
                Zcashd::launch(config)
            },
            |z| z.port(),
        )
        .await;
    }

    #[tokio::test]
    async fn zebrad() {
        let _ = tracing_subscriber::fmt().try_init();
        run_collision_test(
            "Zebrad",
            &["AddrInUse", "code: 98", "Address already in use"],
            |conflicted| {
                let mut config = ZebradConfig::default();
                config.rpc_listen_port = Some(conflicted);
                Zebrad::launch(config)
            },
            |z| z.rpc_listen_port(),
        )
        .await;
    }

    #[tokio::test]
    async fn zainod() {
        let _ = tracing_subscriber::fmt().try_init();

        // Zainod connects to a validator's JSON-RPC port; launch one
        // first (default config, no pinning — its own retry covers
        // any TOCTOU on its picks) and capture its port.
        let zebrad = Zebrad::launch(ZebradConfig::default())
            .await
            .expect("zebrad launch must succeed for the Zainod collision test");
        let validator_port = zebrad.rpc_listen_port();

        run_collision_test(
            "Zainod",
            // Zaino's gRPC server (tonic/tower) surfaces AddrInUse
            // through the standard libc strings. If a future Zaino
            // build emits something else, the diagnose helper flags
            // UNEXPECTED FAILURE MODE and the list gets an entry.
            &["address already in use", "Address already in use", "AddrInUse"],
            |conflicted| {
                let mut config = ZainodConfig::default();
                config.listen_port = Some(conflicted);
                config.validator_port = validator_port;
                Zainod::launch(config)
            },
            |z| z.port(),
        )
        .await;

        drop(zebrad);
    }

    #[tokio::test]
    async fn lightwalletd() {
        let _ = tracing_subscriber::fmt().try_init();

        // Lightwalletd reads its validator's RPC port from a
        // zcash.conf file; launch a zcashd (default config) and
        // hand the conf path through.
        let zcashd = Zcashd::launch(ZcashdConfig::default())
            .await
            .expect("zcashd launch must succeed for the Lightwalletd collision test");
        let zcashd_conf = zcashd.get_zcashd_conf_path();

        run_collision_test(
            "Lightwalletd",
            // lightwalletd is Go; `net.Listen` surfaces AddrInUse as
            // "bind: address already in use" / "address already in use".
            &["address already in use", "bind:"],
            |conflicted| {
                let mut config = LightwalletdConfig::default();
                config.listen_port = Some(conflicted);
                config.zcashd_conf = zcashd_conf.clone();
                Lightwalletd::launch(config)
            },
            |lwd| lwd.port(),
        )
        .await;

        drop(zcashd);
    }
}

/// Regression marker: zebrad on regtest must not resolve seed peers.
///
/// Today, `Zebrad::launch` with default regtest config produces
/// ~2.4 s of stdout doing DNS for `dnsseed.testnet.z.cash`,
/// `testnet.seeder.zfnd.org`, and `testnet.is.yolo.money` — none of
/// which serve any purpose in single-node regtest. That window
/// dominates zebrad's TOC→TOU lag for its RPC port, which is the
/// primary surface area of the cross-subprocess port-collision race
/// documented in `mod launch_recovers_from_rpc_port_collision`.
///
/// The hypothesis is that `write_zebrad_config` leaves the upstream
/// defaults for `initial_testnet_peers` / `initial_mainnet_peers` in
/// place, so zebrad attempts seeder DNS regardless of `network_type`.
/// Forcing both lists to `[]` on regtest should make this test pass.
///
/// What this test asserts: zebrad's captured stdout contains none of
/// the three strings that fire only when zebrad's `add_initial_peers`
/// span runs over a non-empty seeder list. The strings are emitted
/// *before* the RPC bind (the `launch::wait` success indicator), so by
/// the time `launch` returns, all DNS work is already on disk.
#[tokio::test]
async fn zebrad_regtest_skips_seed_peer_dns() {
    let _ = tracing_subscriber::fmt().try_init();

    let zebrad = Zebrad::launch(ZebradConfig::default())
        .await
        .expect("Zebrad::launch should succeed in regtest");

    // `zcash_local_net::logs::STDOUT_LOG` is `pub(crate)` — hardcode the
    // filename here. If the launch helper ever changes the convention,
    // the read below fails loud with "stdout log should exist" before
    // any false-negative pass on the assertion.
    let stdout_path = zebrad.logs_dir().path().join("stdout.log");
    let stdout = std::fs::read_to_string(&stdout_path)
        .expect("stdout log should exist after successful launch");

    let seeder_dns_signatures = [
        "resolved seed peer IP addresses",
        "DNS error resolving peer IP addresses",
        "Seed peer DNS resolution failed",
    ];
    let hits: Vec<&str> = seeder_dns_signatures
        .iter()
        .copied()
        .filter(|sig| stdout.contains(*sig))
        .collect();

    assert!(
        hits.is_empty(),
        "zebrad regtest should not perform seed-peer DNS, but stdout contains: {hits:?}\n\n\
         Hypothesis: `write_zebrad_config` is leaving `initial_testnet_peers` / \
         `initial_mainnet_peers` populated with upstream defaults. Force both to `[]` on regtest.\n\n\
         Captured stdout for diagnosis (truncated to 4 KiB):\n{}",
        stdout.chars().take(4096).collect::<String>()
    );
}
