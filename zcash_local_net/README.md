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

Set `TEST_BINARIES_DIR` to a directory containing the executables
the harness needs (`zebrad`, `zcashd`, `zcash-cli`, `zainod`,
`lightwalletd`); otherwise each binary is resolved via `PATH`.
Each processes `launch` fn and [`crate::LocalNet::launch`] take
config structs for defining additional parameters; see the config
structs for each process in `validator.rs` and `indexer.rs`.

### Patched zcashd required for the default-true fast path

The harness's source-of-truth for `zcashd` is the
[`regtest_fast`](https://github.com/zingolabs/zcash/tree/regtest_fast)
branch of the Zingolabs zcash fork. Two harness defaults depend on
patches that exist only on that branch:

- `ZcashdConfig::disable_shielded_proving` defaults to `true`, which
  passes `-disableshieldedproving` at launch. Stock zcashd does not
  accept this flag.
- `ZcashdConfig::regtest_coinbase_maturity` defaults to `1`, which
  passes `-regtestcoinbasematurity=1` at launch so the genesis
  coinbase is spendable after a single additional block instead of
  the 100-block stock window. `regtest_fast` clamps the flag value
  to `1..=100` (`0` is rejected); stock zcashd does not accept the
  flag at all.

`Zcashd::launch` runs a pre-launch capability probe that fails fast
with [`crate::error::LaunchError::UnsupportedZcashdCapability`] if
the resolved binary doesn't accept `-disableshieldedproving`. To
use stock zcashd anyway, set `disable_shielded_proving = false`
(slower; loads Sapling/Orchard proving keys at startup) and bump
`regtest_coinbase_maturity` to `100` (or whatever stock value
applies).

### Launching multiple processes

See [`crate::LocalNet`].


Current version: 0.5.0

License: MIT
