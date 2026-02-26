# cargo-check-external-types requires this for now
PINNED_NIGHTLY := "nightly-2025-10-18"

check-external-types-zcash-local-net:
    cargo +{{PINNED_NIGHTLY}} check-external-types --manifest-path zcash_local_net/Cargo.toml

check-external-types-zingo-test-vectors:
    cargo +{{PINNED_NIGHTLY}} check-external-types --manifest-path zingo_test_vectors/Cargo.toml

check-external-types:
    just check-external-types-zcash-local-net
    just check-external-types-zingo-test-vectors