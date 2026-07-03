//! Offline golden regression tests for `zcash_local_net::zebra_rpc`.
//!
//! The fixtures in `tests/fixtures/zebra_rpc/` are real zebrad
//! `getblocktemplate` results paired with the block bytes and hashes the
//! zebra-rpc oracle produced for them. They were captured by the oracle
//! differential suite (`tests/zebra_rpc_oracle.rs`, deleted with the
//! zebra-rpc dev-dependency; recover both from git history to recapture)
//! which also proved live equivalence: byte-identical proposals at heights
//! 2 through 6 against a real zebrad, with our bytes accepted by the chain.
//! These tests replay the fixtures with no zebrad binary and no zebra-rpc
//! dependency, pinning that equivalence permanently. Height 2 is a plain
//! NU5+ template; height 5 is the NU6.1/NU6.2 activation block whose
//! coinbase carries the lockbox disbursement outputs; the Canopy case
//! replays the height-2 template at height 1, where the header commitment
//! switches to the chain history root.

use zcash_local_net::validator::regtest_test_activation_heights;
use zcash_local_net::zebra_rpc::{BlockTemplate, block_hash_hex, proposal_block_bytes};

fn assert_fixture_reproduced(height: u32) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/zebra_rpc");
    let read = |name: String| std::fs::read_to_string(dir.join(name)).expect("fixture exists");

    let template: BlockTemplate =
        serde_json::from_str(&read(format!("template_h{height}.json"))).expect("template parses");
    let expected_bytes = read(format!("block_h{height}.hex"));
    let expected_hash = read(format!("hash_h{height}.txt"));

    let our_bytes = proposal_block_bytes(&template, &regtest_test_activation_heights())
        .expect("assembly succeeds");

    assert_eq!(
        hex::encode(&our_bytes),
        expected_bytes.trim(),
        "block bytes drifted from the oracle capture at height {height}"
    );
    assert_eq!(
        block_hash_hex(&our_bytes),
        expected_hash.trim(),
        "block hash drifted from the oracle capture at height {height}"
    );
}

#[test]
fn golden_nu5_plus_template_reproduces_oracle_output() {
    assert_fixture_reproduced(2);
}

#[test]
fn golden_lockbox_activation_template_reproduces_oracle_output() {
    assert_fixture_reproduced(5);
}

#[test]
fn golden_canopy_branch_reproduces_oracle_output() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/zebra_rpc");
    let read = |name: &str| std::fs::read_to_string(dir.join(name)).expect("fixture exists");

    let mut template: serde_json::Value =
        serde_json::from_str(&read("template_h2.json")).expect("template parses");
    template["height"] = serde_json::json!(1);
    let template: BlockTemplate = serde_json::from_value(template).expect("template converts");

    let our_bytes = proposal_block_bytes(&template, &regtest_test_activation_heights())
        .expect("assembly succeeds");

    assert_eq!(
        hex::encode(&our_bytes),
        read("block_h1_canopy.hex").trim(),
        "Canopy-branch block bytes drifted from the oracle capture"
    );
    assert_eq!(
        block_hash_hex(&our_bytes),
        read("hash_h1_canopy.txt").trim(),
        "Canopy-branch block hash drifted from the oracle capture"
    );
}
