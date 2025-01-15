[![Workflow Status](https://github.com/zingolabs/zcash-local-net/workflows/main/badge.svg)](https://github.com/zingolabs/zcash-local-net/actions?query=workflow%3A%22main%22)

# zcash_local_net

## Overview

Utilities that launch and manage Zcash processes. This is used for integration
testing in the development of:

  - lightclients
  - indexers
  - validators


## List of Managed Processes
- Zebrad
- Zcashd
- Zainod
- Lightwalletd

## Prerequisites

Ensure that any binaries manged by this crate are installed on your system.
The binaries can be referenced via $PATH or the path to the binaries can be specified when launching a process.

## Testing

Pre-requisities for running integration tests successfully:
- Build the Zcashd, Zebrad, Zainod and Lightwalletd binaries and add to $PATH.
- In order to generate a cached blockchain from zebrad run:
```BASH
./utils/compare_chain_caches.sh
```
This command generates new data in the `chain_cache` directory.  The new structure should have the following added

```BASH
 ├── [       4096]  client_rpc_tests_large
 └── [       4096]  state
     └── [       4096]  v26
         └── [       4096]  regtest
             ├── [     139458]  000004.log
             ├── [         16]  CURRENT
             ├── [         36]  IDENTITY
             ├── [          0]  LOCK
             ├── [     174621]  LOG
             ├── [       1708]  MANIFEST-000005
             ├── [     114923]  OPTIONS-000007
             └── [          3]  version
```
- To run the `get_subtree_roots_sapling` test, sync Zebrad in testnet mode and copy the cache to `zcash_local_net/chain_cache/testnet_get_subtree_roots_sapling`. At least 2 sapling shards must be synced to pass. See [crate::test_fixtures::get_subtree_roots_sapling] doc comments for more details.
- To run the `get_subtree_roots_orchard` test, sync Zebrad in mainnet mode and copy the cache to `zcash_local_net/chain_cache/testnet_get_subtree_roots_orchard`. At least 2 orchard shards must be synced to pass. See [crate::test_fixtures::get_subtree_roots_orchard] doc comments for more details.

See [crate::test_fixtures] doc comments for running client rpc tests from external crates for indexer/validator development.

Test should be run with the `test_fixtures` feature enabled.


Current version: 0.1.0

License: MIT License
