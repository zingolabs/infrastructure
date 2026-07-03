//! Offline golden regression tests for `zcash_local_net::zebra_rpc`.
//!
//! The fixtures in `tests/fixtures/zebra_rpc/` are real zebrad
//! `getblocktemplate` results paired with the block bytes and hashes the
//! zebra-rpc oracle produced for them (captured by `zebra_rpc_oracle.rs`
//! with `CAPTURE_PROPOSAL_FIXTURES=1`). These tests replay them with no
//! zebrad binary and no zebra-rpc dependency, pinning oracle equivalence
//! after the dependency is gone. Height 2 is a plain NU5+ template; height 5
//! is the NU6.1/NU6.2 activation block whose coinbase carries the lockbox
//! disbursement outputs.

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
