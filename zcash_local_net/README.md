# zcash_local_net

## Overview

Utilities that launch and manage Zcash processes. This is used for integration
testing in the development of:

  - lightclients
  - indexers
  - validators


## List of Managed Processes
- Zebrad
- Zainod
- zcash-devtool (wallet client; per-operation subprocess, see [`crate::client`])

## Prerequisites

Set `TEST_BINARIES_DIR` to a directory containing the executables
the harness needs (`zebrad`, `zainod`, `zcash-devtool`); otherwise
each binary is resolved via `PATH`. `zcash-devtool` must be built
with `--features regtest_support` for regtest wallets.
Each processes `launch` fn and [`crate::LocalNet::launch`] take
config structs for defining additional parameters; see the config
structs for each process in `validator.rs` and `indexer.rs`.

### Legacy stack (feature `legacy-stack`)

The `Zcashd` validator and `Lightwalletd` indexer are gated behind
the non-default `legacy-stack` cargo feature. The feature is
**unsupported and untested** — CI never enables it — and both
processes are scheduled for complete removal (see
`docs/adr/0001-excise-legacy-stack.md`). It exists only as a
short-lived stopgap for consumers migrating to the zebrad + zainod
stack. Running the legacy processes additionally requires `zcashd`,
`zcash-cli`, and `lightwalletd` binaries, and zcashd's
default-`true` `disable_shielded_proving` fast path requires the
Zingolabs patched fork (<https://github.com/zingolabs/zcash>).

### Launching multiple processes

See [`crate::LocalNet`].


Current version: 0.7.0

License: MIT
