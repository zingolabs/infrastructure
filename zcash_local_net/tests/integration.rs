mod testutils;

use zcash_local_net::LocalNetConfig;
#[cfg(feature = "legacy-stack")]
use zcash_local_net::indexer::lightwalletd::Lightwalletd;
use zcash_local_net::logs::LogsToDir as _;
use zcash_local_net::process::Process;
use zcash_local_net::protocol::ActivationHeights;
use zcash_local_net::validator::Validator as _;
use zcash_local_net::validator::ValidatorConfig as _;
#[cfg(feature = "legacy-stack")]
use zcash_local_net::validator::zcashd::Zcashd;
use zcash_local_net::{
    LocalNet,
    indexer::{
        empty::{Empty, EmptyConfig},
        zainod::Zainod,
    },
    utils,
    validator::zebrad::{Zebrad, ZebradConfig},
};
use zingo_consensus::MinerPool;

async fn launch_default_and_print_all<P: Process>() {
    let p = P::launch_default().await.expect("Process launching!");
    p.print_all();
}

/// Install the test tracing subscriber. `try_init` (not `init`) so a
/// second call in the same process is a no-op rather than a panic.
fn init_tracing() {
    let _ = tracing_subscriber::fmt().try_init();
}

/// Regtest activation heights with the usual pre-NU6 fixture values
/// (everything ≤ canopy at 1, NU5 and NU6 at 2, NU7 off) and NU6.1 +
/// NU6.2 + NU6.3 co-activated at `nu6_1_height`. The post-NU6 upgrades
/// always move together in these tests, so they take a single height.
fn regtest_heights_nu6_1_at(nu6_1_height: u32) -> ActivationHeights {
    ActivationHeights::builder()
        .set_overwinter(Some(1))
        .set_sapling(Some(1))
        .set_blossom(Some(1))
        .set_heartwood(Some(1))
        .set_canopy(Some(1))
        .set_nu5(Some(2))
        .set_nu6(Some(2))
        .set_nu6_1(Some(nu6_1_height))
        .set_nu6_2(Some(nu6_1_height))
        .set_nu6_3(Some(nu6_1_height))
        .set_nu7(None)
        .build()
}

/// The four readiness-relevant RPCs zebrad exposes (it has no dedicated
/// /healthz or /readyz — see zingolabs/infrastructure#245). The
/// "informal readiness" contract is: live & ready iff all four return Ok.
const READINESS_RPCS: [&str; 4] = [
    "getinfo",
    "getnetworkinfo",
    "getblockchaininfo",
    "getblocktemplate",
];

/// Call each [`READINESS_RPCS`] endpoint on `zebrad` and return a
/// `"<endpoint>: <error>"` line for every one that fails (empty == all Ok).
async fn readiness_rpc_failures(zebrad: &Zebrad) -> Vec<String> {
    let mut failures = Vec::new();
    for endpoint in READINESS_RPCS {
        if let Err(e) = zebrad
            .client()
            .json_result_from_call::<serde_json::Value>(endpoint, "[]".to_string())
            .await
        {
            failures.push(format!("{endpoint}: {e:?}"));
        }
    }
    failures
}

#[cfg(feature = "legacy-stack")]
#[tokio::test]
async fn launch_zcashd() {
    init_tracing();

    launch_default_and_print_all::<Zcashd>().await;
}

#[cfg(feature = "legacy-stack")]
#[tokio::test]
async fn launch_zcashd_custom_activation_heights() {
    init_tracing();

    let zcashd = Zcashd::launch_default().await.unwrap();

    zcashd.generate_blocks(8).await.unwrap();
    zcashd.print_all();
}

#[tokio::test]
async fn launch_zebrad() {
    init_tracing();

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
    let activation_heights = regtest_heights_nu6_1_at(nu6_1_height);

    let mut config = V::Config::default();
    config.set_test_parameters(MinerPool::Transparent, activation_heights, None);

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
    init_tracing();
    probe_validator_with_nu6_1_at::<Zebrad>(2, 5).await;
}

#[ignore = "documents empty-default failure of zebrad NU6.1 activation; see #244"]
#[tokio::test]
async fn launch_zebrad_with_nu6_1_at_height_3() {
    init_tracing();
    probe_validator_with_nu6_1_at::<Zebrad>(3, 5).await;
}

#[ignore = "documents empty-default failure of zebrad NU6.1 activation; see #244"]
#[tokio::test]
async fn launch_zebrad_with_nu6_1_at_height_50() {
    init_tracing();
    probe_validator_with_nu6_1_at::<Zebrad>(50, 52).await;
}

// Zcashd: accepts NU6.1 at the lowest meaningful height — documents
// the validator divergence (zcashd has no equivalent NU6.1 lockbox
// check today). One test is enough; higher heights all pass for the
// same reason.

#[cfg(feature = "legacy-stack")]
#[tokio::test]
async fn launch_zcashd_with_nu6_1_at_height_2() {
    init_tracing();
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
    init_tracing();
    // Most permissive endpoint — answers as soon as the JSON-RPC
    // dispatcher is registered. If this fails, the process is dead
    // or the listener never bound.
    probe_zebrad_rpc_endpoint("getinfo").await;
}

#[tokio::test]
async fn zebrad_responds_to_getnetworkinfo() {
    init_tracing();
    // Network module loaded; peer subsystem reachable.
    probe_zebrad_rpc_endpoint("getnetworkinfo").await;
}

#[tokio::test]
async fn zebrad_responds_to_getblockchaininfo() {
    init_tracing();
    // State module loaded; chain tip readable from the database.
    probe_zebrad_rpc_endpoint("getblockchaininfo").await;
}

#[tokio::test]
async fn zebrad_responds_to_getblocktemplate() {
    init_tracing();
    // Mining service active and consensus is in a state where the
    // next block can be mined. Most restrictive of the four.
    probe_zebrad_rpc_endpoint("getblocktemplate").await;
}

#[tokio::test]
async fn zebrad_healthy_endpoint_responds_200_after_launch() {
    init_tracing();
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
    init_tracing();
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
    init_tracing();
    // Regression test for upstream Zebra changes that would let
    // /healthy or /ready report ready while the JSON-RPC surface
    // is actually broken (or vice versa). The harness's informal
    // AND-of-4 readiness contract is the cross-check: both signals
    // must agree on a healthy steady state, otherwise one side
    // has regressed.
    let zebrad = Zebrad::launch_default()
        .await
        .expect("zebrad launch_default");

    let rpc_failures = readiness_rpc_failures(&zebrad).await;
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
    init_tracing();
    // The "informal readiness" contract from
    // zingolabs/infrastructure#245: live & ready iff all four
    // readiness-relevant RPCs return Ok. Reports which endpoints
    // failed when the conjunction does, instead of the single-RPC
    // ambiguity of polling getblocktemplate alone.
    let zebrad = Zebrad::launch_default()
        .await
        .expect("zebrad launch_default");

    let failures = readiness_rpc_failures(&zebrad).await;
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
    init_tracing();

    // NU6.1 a few blocks after NU6 so the `Deferred` value pool
    // accumulates enough subsidy fraction to cover the disbursement total.
    let activation_heights = regtest_heights_nu6_1_at(5);

    let mut config = ZebradConfig::default();
    config.set_test_parameters(MinerPool::Transparent, activation_heights, None);
    config.lockbox_disbursements = zcash_local_net::validator::regtest_test_lockbox_disbursements();
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
    init_tracing();

    let activation_heights = regtest_heights_nu6_1_at(2);

    let mut config = ZebradConfig::default();
    config.set_test_parameters(MinerPool::Transparent, activation_heights, None);
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
    init_tracing();

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
    init_tracing();
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
    init_tracing();

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

#[cfg(feature = "legacy-stack")]
#[tokio::test]
async fn launch_localnet_zainod_zcashd() {
    init_tracing();

    launch_default_and_print_all::<LocalNet<Zcashd, Zainod>>().await;
}

#[tokio::test]
async fn launch_localnet_zainod_zebrad() {
    init_tracing();

    launch_default_and_print_all::<LocalNet<Zebrad, Zainod>>().await;
}

/// Pins the Indexer-convergence contract against the real binaries:
/// the barrier must return only once zainod's chain index has logged
/// the validator's tip, and zainod's `Syncing block` log line — the
/// harness's only view of the `fetch` backend's own progress — must
/// still parse. If zainod's log format drifts, this test fails with
/// `SyncMarkerDrift` naming the offending line (or times out with the
/// log tail), rather than letting downstream suites flake.
#[tokio::test]
async fn zainod_converges_to_validator_tip_after_generate_blocks() {
    init_tracing();
    let net = LocalNet::<Zebrad, Zainod>::launch_default().await.unwrap();
    net.generate_blocks_converged(3).await.unwrap();

    let target = net.validator().get_chain_height().await;
    let logged = net.indexer().logged_sync_height().unwrap();
    assert!(
        logged.is_some_and(|height| height >= target),
        "barrier returned but the indexer's logged height is {logged:?}, validator tip {target}"
    );
}

#[cfg(feature = "legacy-stack")]
#[tokio::test]
async fn launch_localnet_lightwalletd_zcashd() {
    init_tracing();

    launch_default_and_print_all::<LocalNet<Zcashd, Lightwalletd>>().await;
}

#[cfg(feature = "legacy-stack")]
#[tokio::test]
async fn launch_localnet_lightwalletd_zebrad() {
    init_tracing();

    launch_default_and_print_all::<LocalNet<Zebrad, Lightwalletd>>().await;
}

#[ignore = "not a test. generates chain cache for client_rpc tests."]
#[tokio::test]
async fn generate_zebrad_large_chain_cache() {
    init_tracing();

    crate::testutils::generate_zebrad_large_chain_cache().await;
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
    #[cfg(feature = "legacy-stack")]
    use zcash_local_net::indexer::lightwalletd::LightwalletdConfig;
    use zcash_local_net::indexer::zainod::ZainodConfig;
    #[cfg(feature = "legacy-stack")]
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
            (
                LaunchError::LaunchAborted {
                    matched_indicator, ..
                },
                Some(sig),
            ) => format!(
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
            (
                LaunchError::LaunchAborted {
                    matched_indicator, ..
                },
                None,
            ) => format!(
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

    #[cfg(feature = "legacy-stack")]
    #[tokio::test]
    async fn zcashd() {
        init_tracing();
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
        init_tracing();
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
        init_tracing();

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
            &[
                "address already in use",
                "Address already in use",
                "AddrInUse",
            ],
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

    #[cfg(feature = "legacy-stack")]
    #[tokio::test]
    async fn lightwalletd() {
        init_tracing();

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
    init_tracing();

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

/// zcash-devtool client management: pins the devtool CLI contract
/// (flags, stdout shapes, regtest activation-height alignment) against
/// the real binary, per the "behaviour drift from a contract this code
/// mirrors" rule — the parsers in `wallet::zcash_devtool` are only
/// trusted because these tests exercise them live.
///
/// Requires `zcash-devtool` (built with `--features regtest_support`)
/// in `TEST_BINARIES_DIR` or on `PATH`.
mod devtool_client {
    use zcash_local_net::indexer::zainod::ZainodConfig;
    use zcash_local_net::validator::Validator as _;
    use zcash_local_net::wallet::zcash_devtool::{
        ZcashDevtool, ZcashDevtoolConfig, supported_regtest_activation_heights,
    };
    use zcash_local_net::wallet::{AddressReceiver, Wallet, WalletNetwork};
    use zingo_test_vectors::{
        REG_O_ADDR_FROM_ABANDONART, REG_T_ADDR_FROM_ABANDONART, REG_Z_ADDR_FROM_ABANDONART,
    };

    use super::*;

    /// Per-block miner reward in zats once the default regtest fixture's
    /// post-NU6 funding stream (1% to `Deferred`, active from height 2)
    /// starts deducting from the 6.25 ZEC subsidy.
    const POST_NU6_MINER_REWARD: u64 = 618_750_000;
    /// Block 1 predates the funding stream: full subsidy, mined to the
    /// sapling receiver of the unified miner address (NU5 activates at
    /// height 2, so block 1's coinbase cannot be orchard).
    const BLOCK_1_SAPLING_REWARD: u64 = 625_000_000;

    const SEND_VALUE: u64 = 250_000;

    /// An orchard-mining zebrad + zainod stack, the environment the
    /// devtool faucet wallet is designed for: every coinbase lands in a
    /// pool the abandon-art wallet can spend without coinbase maturity.
    ///
    /// Launched with [`supported_regtest_activation_heights`] (all
    /// upgrades active by height 2), not the default fixture heights:
    /// they are the heights compiled into the devtool binary, and they
    /// are also required for orchard mining itself — zebra 5.1.0's
    /// shielded-coinbase templates fail their own orchard-proof
    /// verification while a configured upgrade is still in the future.
    async fn launch_orchard_net() -> LocalNet<Zebrad, Zainod> {
        launch_net_with_heights(supported_regtest_activation_heights()).await
    }

    /// An orchard-mining zebrad + zainod stack on the given activation
    /// heights. The indexer config carries no heights at all
    /// (`NetworkKind::Regtest`): per ADR 0003 the Indexer must learn
    /// the schedule from the Validator, and only the kind string ever
    /// reached the zainod TOML anyway.
    async fn launch_net_with_heights(
        heights: zcash_local_net::protocol::ActivationHeights,
    ) -> LocalNet<Zebrad, Zainod> {
        let mut validator_config = ZebradConfig::default();
        validator_config.set_test_parameters(MinerPool::Orchard, heights, None);
        LocalNet::<Zebrad, Zainod>::launch_from_two_configs(
            validator_config,
            ZainodConfig::default(),
        )
        .await
        .unwrap()
    }

    /// Launch a devtool wallet through the harness's generic actuation
    /// path: `LocalNet::launch_wallet` mints the wallet's network from
    /// the running validator (ADR 0003) and wires the indexer
    /// connection, for any `Wallet` implementation — the devtool one
    /// here. `make_config` is one of the [`ZcashDevtoolConfig`]
    /// constructors, e.g. `ZcashDevtoolConfig::faucet`.
    async fn launch_client(
        net: &LocalNet<Zebrad, Zainod>,
        make_config: impl FnOnce(WalletNetwork) -> ZcashDevtoolConfig,
    ) -> ZcashDevtool {
        net.launch_wallet::<ZcashDevtool>(make_config)
            .await
            .unwrap()
    }

    /// Sync the wallet until its view of the chain tip reaches
    /// `target_height`. The validator reports the target height as soon
    /// as it mines, but the indexer serves the wallet and may still be
    /// catching up — so poll sync rather than assume one pass suffices.
    async fn sync_to_height(
        client: &ZcashDevtool,
        target_height: u32,
    ) -> zcash_local_net::wallet::WalletBalance {
        const ATTEMPTS: u32 = 120;
        for _ in 0..ATTEMPTS {
            client.sync().await.unwrap();
            let balance = client.balance().await.unwrap();
            if balance.chain_tip_height >= target_height {
                return balance;
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
        panic!("wallet did not reach height {target_height} after {ATTEMPTS} sync attempts");
    }

    /// The faucet linchpin: devtool's account-0 derivation of the
    /// abandon-art seed must yield the same addresses the validators
    /// mine to, otherwise the "faucet" never sees a reward. Pins every
    /// receiver of [`Wallet::address`] against the `zingo_test_vectors`
    /// constants — the unified address (== the orchard miner address)
    /// and the bare transparent/sapling receivers — proving the
    /// abandon-art wallet owns the addresses the harness pays.
    #[tokio::test]
    async fn faucet_addresses_match_miner_addresses() {
        init_tracing();
        let net = launch_orchard_net().await;
        let faucet = launch_client(&net, ZcashDevtoolConfig::faucet).await;

        // default_address() is the convenience for address(Unified).
        assert_eq!(
            faucet.default_address().await.unwrap(),
            REG_O_ADDR_FROM_ABANDONART,
        );
        assert_eq!(
            faucet.address(AddressReceiver::Unified).await.unwrap(),
            REG_O_ADDR_FROM_ABANDONART,
        );
        assert_eq!(
            faucet.address(AddressReceiver::Transparent).await.unwrap(),
            REG_T_ADDR_FROM_ABANDONART,
        );
        assert_eq!(
            faucet.address(AddressReceiver::Sapling).await.unwrap(),
            REG_Z_ADDR_FROM_ABANDONART,
        );
        // The orchard receiver has no bare encoding; devtool emits a
        // UA carrying only the orchard receiver, so it differs from the
        // full UA but must still decode as a unified regtest address.
        let orchard = faucet.address(AddressReceiver::Orchard).await.unwrap();
        assert!(
            orchard.starts_with("uregtest1"),
            "orchard receiver should be a regtest UA, got {orchard:?}"
        );
    }

    /// Smoke check that the wallet can reach and talk to its indexer,
    /// the `get_info` analogue of zingolib's `do_info` "connect to
    /// node" test. The original discards the result; this port adds a
    /// light contract check — the parsed shape is populated and the
    /// server-tip semantics hold — without over-constraining (chain
    /// names and the exact tip are the server's to define).
    #[tokio::test]
    async fn connect_to_node_get_info() {
        init_tracing();
        let net = launch_orchard_net().await;
        net.validator().generate_blocks(2).await.unwrap();
        let faucet = launch_client(&net, ZcashDevtoolConfig::faucet).await;

        let info = faucet.get_info().await.unwrap();
        assert!(
            !info.server_uri.is_empty(),
            "server_uri should be populated, got {info:?}"
        );
        assert!(
            !info.chain_name.is_empty(),
            "chain_name should be populated, got {info:?}"
        );

        // get-info reports the server (node/indexer) tip, not a
        // wallet-synced height — so it needs no wallet sync, but the
        // indexer may briefly lag the validator's freshly-mined blocks.
        // Poll until it catches up to confirm the server-tip semantics
        // rather than asserting on a single possibly-stale read.
        const ATTEMPTS: u32 = 60;
        let mut tip = info.chain_tip_height;
        for _ in 0..ATTEMPTS {
            if tip >= 2 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            tip = faucet.get_info().await.unwrap().chain_tip_height;
        }
        assert!(
            tip >= 2,
            "chain_tip_height should reach the mined server tip (>= 2), last saw {tip}"
        );
    }

    /// zaino's `ORCHARD_THEN_IRONWOOD_ACTIVATION_HEIGHTS` fixture:
    /// NU6.3 mid-chain at height 6, everything else active by 2. The
    /// acceptance shape of `zaino-ironwood-activation-infra-spec.md`;
    /// these heights configure the validator only, and the wallet
    /// derives them back from it. Their TOML bytes are pinned by the
    /// `validator_heights_emit_acceptance_toml` unit test.
    fn orchard_then_ironwood_heights() -> zcash_local_net::protocol::ActivationHeights {
        zcash_local_net::protocol::ActivationHeights::builder()
            .set_overwinter(Some(1))
            .set_sapling(Some(1))
            .set_blossom(Some(1))
            .set_heartwood(Some(1))
            .set_canopy(Some(1))
            .set_nu5(Some(2))
            .set_nu6(Some(2))
            .set_nu6_1(Some(2))
            .set_nu6_2(Some(2))
            .set_nu6_3(Some(6))
            .set_nu7(None)
            .build()
    }

    /// The ZIP 318 migration shape on a mid-chain boundary: NU6.3
    /// activates at height 6, so the wallet scans Orchard-era coinbase
    /// at heights 2–5 and must build a spend at tip >= 6 that carries
    /// the NU6.3 consensus branch ID — the validator accepting it and
    /// the receipt landing in the recipient's ironwood pool proves
    /// both era-correct scanning and era-correct construction come
    /// from the validator-derived heights file, not compiled-in
    /// defaults.
    ///
    /// Ignored: the wallet syncs through zainod, and zainod adopts
    /// its regtest heights from compiled-in defaults instead of
    /// querying the validator, so a mid-chain NU6.3 kills its sync
    /// loop with `InvalidData("Block commitment could not be
    /// computed")`. Also unverified until then: whether zebrad's
    /// shielded-coinbase templates mine orchard blocks 2–5 while
    /// NU6.3 is configured-but-future (zebra 5.1.0 failed its own
    /// orchard-proof check in that shape; current floor is >= 6.0.0).
    #[tokio::test]
    #[ignore = "needs a zainod that learns heights from the validator (zingolabs/zaino#1076)"]
    async fn orchard_note_spends_to_ironwood_across_midchain_boundary() {
        init_tracing();
        let net = launch_net_with_heights(orchard_then_ironwood_heights()).await;
        let faucet = launch_client(&net, ZcashDevtoolConfig::faucet).await;
        let recipient = launch_client(&net, ZcashDevtoolConfig::recipient).await;
        let recipient_address = recipient.default_address().await.unwrap();

        // Height 2 mints the first Orchard coinbase; a third block
        // makes it one confirmation deep (spendable) with the tip
        // still below the NU6.3 boundary at 6.
        net.validator().generate_blocks(3).await.unwrap();
        let pre = sync_to_height(&faucet, net.validator().get_chain_height().await).await;
        assert!(
            pre.orchard_spendable > 0,
            "pre-boundary coinbase must scan as Orchard-era notes, got {pre:?}"
        );
        assert_eq!(
            pre.ironwood_spendable, 0,
            "no Ironwood notes may exist below the boundary"
        );

        // Cross the boundary and spend: tip >= 6 puts transaction
        // construction in the Ironwood era.
        net.validator().generate_blocks(3).await.unwrap();
        let tip = net.validator().get_chain_height().await;
        assert!(tip >= 6, "expected the tip past the boundary, saw {tip}");
        sync_to_height(&faucet, tip).await;
        faucet.send(&recipient_address, SEND_VALUE).await.unwrap();
        net.validator().generate_blocks(1).await.unwrap();

        let received = sync_to_height(&recipient, net.validator().get_chain_height().await).await;
        assert_eq!(received.total, SEND_VALUE);
        assert_eq!(
            received.ironwood_spendable, SEND_VALUE,
            "a post-boundary receipt must land in the ironwood pool"
        );
    }

    /// The full faucet→recipient loop: fund by orchard mining, send
    /// twice (once near the tip the chain starts at, once later),
    /// receive, and survive a rescan from scratch. The sends are the
    /// live consensus-branch-ID alignment check: they fail with a
    /// validator rejection if the devtool binary's compiled-in regtest
    /// heights drift from [`supported_regtest_activation_heights`].
    #[tokio::test]
    async fn faucet_sends_recipient_receives_and_rescans() {
        init_tracing();
        let net = launch_orchard_net().await;
        let faucet = launch_client(&net, ZcashDevtoolConfig::faucet).await;
        let recipient = launch_client(&net, ZcashDevtoolConfig::recipient).await;
        let recipient_address = recipient.default_address().await.unwrap();

        // Mining to orchard is the expensive part (~4.5-9.5s/block of Halo2
        // coinbase proving), so mine the minimum each step needs. 2 blocks
        // puts the first orchard coinbase (height 2) one confirmation deep,
        // making it spendable; Zebrad::launch already pre-mined block 1
        // (sapling, pre-NU5). Sync to the validator's real tip and derive the
        // expected balance from it, so the reduced counts stay correct
        // regardless of the launch-primed block.
        net.validator().generate_blocks(2).await.unwrap();
        let tip = net.validator().get_chain_height().await;
        let balance = sync_to_height(&faucet, tip).await;
        assert_eq!(
            balance.total,
            BLOCK_1_SAPLING_REWARD + u64::from(tip - 1) * POST_NU6_MINER_REWARD,
        );
        assert_eq!(balance.sapling_spendable, BLOCK_1_SAPLING_REWARD);
        assert_eq!(balance.transparent_spendable, 0);

        // First send, confirmed by one block.
        faucet.send(&recipient_address, SEND_VALUE).await.unwrap();
        net.validator().generate_blocks(1).await.unwrap();
        let received = sync_to_height(&recipient, net.validator().get_chain_height().await).await;
        assert_eq!(received.total, SEND_VALUE);

        // One more block matures the faucet's change note, then send again.
        net.validator().generate_blocks(1).await.unwrap();
        sync_to_height(&faucet, net.validator().get_chain_height().await).await;
        faucet.send(&recipient_address, SEND_VALUE).await.unwrap();
        net.validator().generate_blocks(1).await.unwrap();
        let tip = net.validator().get_chain_height().await;
        let received = sync_to_height(&recipient, tip).await;
        assert_eq!(received.total, 2 * SEND_VALUE);

        // Rescan from scratch and verify the balance survives.
        recipient.rescan().await.unwrap();
        let rescanned = sync_to_height(&recipient, tip).await;
        assert_eq!(rescanned.total, 2 * SEND_VALUE);
    }

    /// Shield non-coinbase transparent funds: the faucet sends to its
    /// own transparent address, then shields the result into orchard.
    #[tokio::test]
    async fn faucet_shields_transparent_funds() {
        init_tracing();
        let net = launch_orchard_net().await;
        let faucet = launch_client(&net, ZcashDevtoolConfig::faucet).await;

        // Mine the minimum orchard coinbase needed: 2 blocks makes the first
        // orchard coinbase (height 2) one confirmation deep, hence spendable.
        net.validator().generate_blocks(2).await.unwrap();
        sync_to_height(&faucet, net.validator().get_chain_height().await).await;

        faucet
            .send(REG_T_ADDR_FROM_ABANDONART, SEND_VALUE)
            .await
            .unwrap();
        // Two blocks so the new transparent output is one confirmation deep
        // (spendable) when snapshotted.
        net.validator().generate_blocks(2).await.unwrap();
        let funded = sync_to_height(&faucet, net.validator().get_chain_height().await).await;
        assert_eq!(funded.transparent_spendable, SEND_VALUE);

        faucet.shield().await.unwrap();
        // Two blocks so the shielded orchard output is confirmed/spendable.
        net.validator().generate_blocks(2).await.unwrap();
        let shielded = sync_to_height(&faucet, net.validator().get_chain_height().await).await;
        assert_eq!(shielded.transparent_spendable, 0);

        // The faucet is also the miner, so the ZIP-317 fee it pays to
        // shield returns to it in that block's coinbase — fees net to
        // zero across `total`, which grows by exactly one subsidy per
        // block mined since the funded snapshot. Derive the block count
        // from the snapshots' own tip heights rather than assuming a
        // fixed number: the faucet-is-miner coupling plus burst mining
        // makes the exact capture height race run-to-run. NU6.3 is
        // active for this whole window (all-at-2 heights), so every
        // coinbase subsidy and the shielded value itself land in the
        // ironwood pool — consensus forbids value entering orchard from
        // NU6.3 onward, and zebra routes the orchard-receiver miner
        // address to the ironwood output builder.
        let blocks = u64::from(shielded.chain_tip_height - funded.chain_tip_height);
        assert!(
            blocks >= 1,
            "expected the shield window to mine blocks, saw {blocks}"
        );
        assert_eq!(
            shielded.total,
            funded.total + blocks * POST_NU6_MINER_REWARD
        );
        assert_eq!(
            shielded.ironwood_spendable,
            funded.ironwood_spendable + blocks * POST_NU6_MINER_REWARD + SEND_VALUE,
        );
        assert_eq!(
            shielded.orchard_spendable, 0,
            "no value may enter the orchard pool from NU6.3 onward",
        );
    }
}
