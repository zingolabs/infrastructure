//! Oracle tests proving `zcash_local_net::zebra_rpc` equivalent to the
//! zebra-rpc dependency for every way this repo uses it.
//!
//! The live differential test launches a real zebrad, and at each height
//! parses the same template with both implementations, asserts byte-for-byte
//! equality of the assembled proposal and its hash, then submits *our* bytes
//! and asserts the chain advances. Heights 1 (Canopy commitment branch)
//! through 6 (NU5+ branch, crossing the NU6.1/NU6.2 lockbox activation at 5)
//! are all exercised.
//!
//! Set `CAPTURE_PROPOSAL_FIXTURES=1` to refresh the committed golden
//! fixtures in `tests/fixtures/zebra_rpc/` from the oracle's outputs; the
//! offline regression test in `zebra_rpc_golden.rs` replays those without
//! zebra-rpc or a zebrad binary. (The variable deliberately avoids the
//! `ZEBRA_` prefix: zebrad parses `ZEBRA_*` environment variables as
//! configuration and refuses to launch on unknown fields.)

use zcash_local_net::process::Process;
use zcash_local_net::validator::{Validator, regtest_test_activation_heights, zebrad::Zebrad};
use zcash_local_net::zebra_rpc as local_impl;

use zebra_rpc::client::zebra_chain::parameters::Network;
use zebra_rpc::client::zebra_chain::parameters::testnet::ConfiguredActivationHeights;
use zebra_rpc::client::zebra_chain::serialization::ZcashSerialize as _;
use zebra_rpc::client::{BlockTemplateResponse, BlockTemplateTimeSource};
use zebra_rpc::proposal_block_from_template;

fn oracle_network() -> Network {
    let zingo = regtest_test_activation_heights();
    Network::new_regtest(
        ConfiguredActivationHeights {
            before_overwinter: Some(1),
            overwinter: zingo.overwinter(),
            sapling: zingo.sapling(),
            blossom: zingo.blossom(),
            heartwood: zingo.heartwood(),
            canopy: zingo.canopy(),
            nu5: zingo.nu5(),
            nu6: zingo.nu6(),
            nu6_1: zingo.nu6_1(),
            nu6_2: zingo.nu6_2(),
            nu6_3: zingo.nu6_3(),
            nu7: zingo.nu7(),
        }
        .into(),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn proposals_match_the_oracle_and_zebrad_accepts_ours() {
    let _ = tracing_subscriber::fmt().try_init();

    let zebrad = Zebrad::launch_default().await.expect("zebrad launches");
    let client = zebrad.client();
    let network = oracle_network();
    let heights = regtest_test_activation_heights();

    let capture = std::env::var("CAPTURE_PROPOSAL_FIXTURES").is_ok();
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/zebra_rpc");

    // `Zebrad::launch` has already mined block 1 (the height-1, Canopy-branch
    // proposal happens inside launch); templates here start at height 2 and
    // run past the NU6.1/NU6.2 lockbox activation at height 5.
    let start_height = zebrad.get_chain_height().await;
    for target_height in (start_height + 1)..=(start_height + 5) {
        let envelope: serde_json::Value = serde_json::from_str(
            &client
                .text_from_call("getblocktemplate", "[]")
                .await
                .expect("getblocktemplate succeeds"),
        )
        .expect("valid envelope");
        let result = envelope
            .get("result")
            .cloned()
            .expect("envelope has result");

        // Both sides parse the same result payload.
        let ours: local_impl::BlockTemplate =
            serde_json::from_value(result.clone()).expect("our parser accepts the template");
        let oracle_template: BlockTemplateResponse =
            serde_json::from_value(result.clone()).expect("oracle parser accepts the template");

        assert_eq!(ours.height, target_height, "template height");

        // Byte-for-byte proposal equivalence.
        let our_bytes =
            local_impl::proposal_block_bytes(&ours, &heights).expect("our assembly succeeds");
        let oracle_block = proposal_block_from_template(
            &oracle_template,
            BlockTemplateTimeSource::default(),
            &network,
        )
        .expect("oracle assembly succeeds");
        let oracle_bytes = oracle_block
            .zcash_serialize_to_vec()
            .expect("oracle serialization succeeds");

        assert_eq!(
            hex::encode(&our_bytes),
            hex::encode(&oracle_bytes),
            "serialized proposal differs from the oracle at height {target_height}"
        );
        assert_eq!(
            local_impl::block_hash_hex(&our_bytes),
            oracle_block.hash().to_string(),
            "block hash differs from the oracle at height {target_height}"
        );

        if capture && (target_height == 2 || target_height == 5) {
            std::fs::create_dir_all(&fixture_dir).expect("fixture dir");
            std::fs::write(
                fixture_dir.join(format!("template_h{target_height}.json")),
                serde_json::to_string_pretty(&result).expect("serialize fixture"),
            )
            .expect("write template fixture");
            std::fs::write(
                fixture_dir.join(format!("block_h{target_height}.hex")),
                hex::encode(&oracle_bytes),
            )
            .expect("write block fixture");
            std::fs::write(
                fixture_dir.join(format!("hash_h{target_height}.txt")),
                oracle_block.hash().to_string(),
            )
            .expect("write hash fixture");
        }

        // The chain must accept OUR bytes, not just match the oracle's.
        let block_hex = hex::encode(&our_bytes);
        let mut advanced = false;
        for _ in 0..30 {
            client
                .text_from_call("submitblock", format!(r#"["{block_hex}"]"#))
                .await
                .expect("submitblock succeeds");
            if zebrad.get_chain_height().await >= target_height {
                advanced = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        assert!(
            advanced,
            "zebrad did not accept our proposal at height {target_height}"
        );
    }
}

/// Differential coverage for the Canopy commitment branch (height 1, where
/// NU5 is not yet active under the fixture heights). The live test can't
/// observe a height-1 template because `Zebrad::launch` consumes it, so this
/// replays the captured height-2 fixture with the height rewritten to 1;
/// the oracle is a pure function and evaluates it without a node.
#[test]
fn canopy_branch_matches_the_oracle() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/zebra_rpc/template_h2.json");
    let mut template: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(fixture).expect("fixture exists"))
            .expect("fixture parses");
    template["height"] = serde_json::json!(1);

    let ours: local_impl::BlockTemplate =
        serde_json::from_value(template.clone()).expect("our parser");
    let oracle_template: BlockTemplateResponse =
        serde_json::from_value(template).expect("oracle parser");

    let our_bytes = local_impl::proposal_block_bytes(&ours, &regtest_test_activation_heights())
        .expect("our assembly");
    let oracle_bytes = proposal_block_from_template(
        &oracle_template,
        BlockTemplateTimeSource::default(),
        &oracle_network(),
    )
    .expect("oracle assembly")
    .zcash_serialize_to_vec()
    .expect("oracle serialization");

    assert_eq!(
        hex::encode(&our_bytes),
        hex::encode(&oracle_bytes),
        "Canopy-branch proposal differs from the oracle"
    );
}
