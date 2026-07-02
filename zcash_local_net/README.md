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
- zcash-devtool (wallet client; per-operation subprocess, see [`crate::client`])

## Prerequisites

Set `TEST_BINARIES_DIR` to a directory containing the executables
the harness needs (`zebrad`, `zcashd`, `zcash-cli`, `zainod`,
`lightwalletd`, `zcash-devtool`); otherwise each binary is resolved
via `PATH`. `zcash-devtool` must be built with
`--features regtest_support` for regtest wallets.
Each processes `launch` fn and [`crate::LocalNet::launch`] take
config structs for defining additional parameters; see the config
structs for each process in `validator.rs` and `indexer.rs`.

### Patched zcashd required for the default-true fast path

`ZcashdConfig::disable_shielded_proving` defaults to `true`, which
passes `-disableshieldedproving` at launch. Stock zcashd does not
accept this flag; the harness requires the Zingolabs patched fork
(<https://github.com/zingolabs/zcash>). `Zcashd::launch` runs a
pre-launch capability probe that fails fast with
[`crate::error::LaunchError::UnsupportedZcashdCapability`] if the
resolved binary doesn't accept the flag. To use stock zcashd
anyway, set `disable_shielded_proving = false` (slower; loads
Sapling/Orchard proving keys at startup).

### Launching multiple processes

See [`crate::LocalNet`].


Current version: 0.6.0

License: MIT
