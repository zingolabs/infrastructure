//! Live tests for the containerized artifact path.
//!
//! These launch the real stack from whatever a manifest names —
//! container images and/or local builds — so they are `#[ignore]`d by
//! default: they need a container runtime plus real zebrad/zainod
//! artifacts, which plain `cargo test` environments don't guarantee.
//! Run them explicitly once the environment provides both:
//!
//! ```sh
//! zcash-local-net preflight --manifest ci-artifacts.json
//! ZCASH_LOCAL_NET_MANIFEST=ci-artifacts.json \
//!     cargo test -p zcash_local_net --test containers -- --ignored
//! ```
//!
//! The container *plumbing* itself (foreground `run` semantics, host
//! networking, identical-path mounts, readiness scanning on container
//! logs, front wiring, `rm --force` teardown) is exercised without any
//! zcash artifacts by the unit tests in `src/container.rs` and was
//! validated live against a scratch image; this file pins the missing
//! piece — the real images' behavior — wherever a manifest provides
//! them.

use zcash_local_net::container::manifest::ArtifactManifest;

/// The consumer flow, end to end: manifest → preflight (the pre-check)
/// → LocalNet launch → mine to Indexer convergence. Artifact sources
/// come from `ZCASH_LOCAL_NET_MANIFEST`, so one invocation covers
/// all-images, all-local-builds, or any mix (the escape hatch).
#[ignore = "requires ZCASH_LOCAL_NET_MANIFEST plus the artifacts (images or binaries) it names"]
#[tokio::test(flavor = "multi_thread")]
async fn manifest_local_net_launches_and_converges() {
    tracing_subscriber::fmt().init();

    let manifest = ArtifactManifest::from_env()
        .expect("manifest should load")
        .expect("set ZCASH_LOCAL_NET_MANIFEST to run this test");

    let report = manifest.preflight().await;
    assert!(report.passed(), "artifact preflight failed:\n{report}");

    let local_net = manifest
        .launch_local_net()
        .await
        .expect("LocalNet should launch from the manifest's artifacts");

    local_net
        .generate_blocks_converged(2)
        .await
        .expect("mined blocks should reach the Indexer's chain index");
}
