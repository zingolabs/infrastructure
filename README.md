# Zcash Local Net

## Overview

A Rust test utility crate designed to facilitate the launching and management of Zcash processes on a local network (regtest/localnet mode). This crate is ideal for integration testing in the development of light-clients/light-wallets, indexers/light-nodes and validators/full-nodes as it provides a simple and configurable interface for launching and managing other proccesses in the local network to simulate a Zcash environment.

## List of Processes

- Zcashd
- Zainod
- Lightwalletd

## Prerequisites

Ensure that any processes used in this crate are installed on your system. The binaries can be in $PATH or the path to the binaries can be specified when launching a process.

## Testing

Pre-requisities for running integration tests successfully:
- Build the Zcashd, Zebrad, Zainod and Lightwalletd binaries and add to $PATH.
- Run `cargo test generate_zebrad_large_chain_cache --features test_fixtures -- --ignored` or `cargo nextest run generate_zebrad_large_chain_cache --run-ignored ignored-only --features test_fixtures`.
- To run the `get_subtree_roots_sapling` test, sync Zebrad in testnet mode and copy the cache to `zcash_local_net/chain_cache/testnet_get_subtree_roots_sapling`. At least 2 sapling shards must be synced to pass. See `zcash_local_net::test_fixtures::get_subtree_roots_sapling` doc comments for more details.
- To run the `get_subtree_roots_orchard` test, sync Zebrad in mainnet mode and copy the cache to `zcash_local_net/chain_cache/testnet_get_subtree_roots_orchard`. At least 2 orchard shards must be synced to pass. See `zcash_local_net::test_fixtures::get_subtree_roots_orchard` doc comments for more details.

See `src/test_fixtures.rs` doc comments for running client rpc tests from external crates for indexer/validator development.

Test should be run with the `test_fixtures` feature enabled.

